use anyhow::Result;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use sysinfo::System;
use tokio::sync::RwLock;

use crate::launcher::{BrowserLauncher, BrowserLauncherOptions};

#[derive(Debug, Clone)]
pub struct BrowserSession {
    pub pid: u32,
    pub debugging_port: u16,
    pub ws_url: String,
    pub user_data_dir: String,
}

pub fn browser_registry() -> &'static Arc<RwLock<std::collections::HashMap<String, u32>>> {
    static REGISTRY: OnceLock<Arc<RwLock<std::collections::HashMap<String, u32>>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

pub fn browser_sessions() -> &'static Arc<RwLock<std::collections::HashMap<String, BrowserSession>>> {
    static SESSIONS: OnceLock<Arc<RwLock<std::collections::HashMap<String, BrowserSession>>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

pub fn browser_launchers() -> &'static Arc<RwLock<std::collections::HashMap<String, BrowserLauncher>>> {
    static LAUNCHERS: OnceLock<Arc<RwLock<std::collections::HashMap<String, BrowserLauncher>>>> = OnceLock::new();
    LAUNCHERS.get_or_init(|| Arc::new(RwLock::new(HashMap::new())))
}

#[derive(Debug, Clone)]
pub struct BrowserManagerOptions {
    pub default_browser: String,
    pub browser_id: String,
    pub headless: bool,
    pub extension_paths: Vec<String>,
    pub custom_args: Vec<String>,
    pub user_data_dir: Option<String>,
}

pub struct BrowserManager {
    options: BrowserManagerOptions,
    launcher: Option<BrowserLauncher>,
    created_ext_dirs: Vec<String>,
    resolved_user_data_dir: Option<String>,
    pid: Option<u32>,
}

impl BrowserManager {
    pub fn new(options: BrowserManagerOptions) -> Self {
        Self {
            options,
            launcher: None,
            created_ext_dirs: Vec::new(),
            resolved_user_data_dir: None,
            pid: None,
        }
    }

    pub async fn launch(&mut self) -> Result<String> {
        let executable_path = crate::resolver::resolve_executable_path(&self.options.default_browser).await?;
        
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let debugging_port = listener.local_addr()?.port();
        drop(listener); // Free the port

        let user_data_dir = self.options.user_data_dir.clone().unwrap_or_else(|| {
            get_temp_dir()
                .join(format!(
                    "automa_browser_{}_{}",
                    self.options.browser_id,
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()
                ))
                .to_string_lossy()
                .to_string()
        });
        self.resolved_user_data_dir = Some(user_data_dir.clone());

        let mut custom_args = self.options.custom_args.clone();
        custom_args.push("--enable-logging".to_string());
        custom_args.push("--v=1".to_string());

        if self.options.headless {
            if self.options.default_browser == "firefox" {
                if !custom_args.contains(&"--headless".to_string()) {
                    custom_args.push("--headless".to_string());
                }
            } else {
                if !custom_args.contains(&"--headless=new".to_string()) {
                    custom_args.push("--headless=new".to_string());
                }
            }
        }

        // --- DYNAMIC EXTENSION PROVISIONING ---
        let mut final_ext_paths = vec![];
        for ext_path_str in &self.options.extension_paths {
            let mut cleaned = ext_path_str.clone();
            if cleaned.starts_with(r"\\?\") {
                cleaned = cleaned[4..].to_string();
            }
            let original_ext_path = std::path::Path::new(&cleaned);
            if original_ext_path.exists() && original_ext_path.is_dir() {
                let is_automa = cleaned.to_lowercase().contains("automa")
                    || (original_ext_path.join("manifest.json").exists() && {
                        std::fs::read_to_string(original_ext_path.join("manifest.json"))
                            .map(|c| c.to_lowercase().contains("automa"))
                            .unwrap_or(false)
                    });

                if is_automa {
                    // Copy the extension to a browser-specific unique temp directory
                    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
                    let browser_ext_dir = get_temp_dir().join(format!("automa_ext_{}_{}", self.options.browser_id, timestamp));
                    self.created_ext_dirs.push(browser_ext_dir.to_string_lossy().to_string());
                    
                    // Copy directory recursively using pure Rust
                    copy_dir_all(original_ext_path.to_path_buf(), browser_ext_dir.clone()).await?;

                    // Inject daemon.json
                    let daemon_config_path = browser_ext_dir.join("daemon.json");
                    let host = std::env::var("SPECTER_HOST")
                        .or_else(|_| std::env::var("AUTOMA_HOST"))
                        .unwrap_or_else(|_| "127.0.0.1".to_string());
                    let port = std::env::var("SPECTER_PORT")
                        .or_else(|_| std::env::var("AUTOMA_PORT"))
                        .ok()
                        .and_then(|p| p.parse::<u16>().ok())
                        .unwrap_or(8765);
                    let base_url = format!("http://{}:{}", host, port);
                    let config_content = format!(
                        "{{\"browserId\": \"{}\", \"port\": {}, \"host\": \"{}\", \"baseUrl\": \"{}\"}}",
                        self.options.browser_id, port, host, base_url
                    );
                    tokio::fs::write(daemon_config_path, config_content).await?;

                    // Ensure manifest.json exists and has valid version for Chromium
                    let manifest_path = browser_ext_dir.join("manifest.json");
                    if manifest_path.exists()
                        && let Ok(content) = tokio::fs::read_to_string(&manifest_path).await
                            && let Ok(mut manifest) = serde_json::from_str::<serde_json::Value>(&content) {
                                let has_valid_version = manifest.get("version")
                                    .and_then(|v| v.as_str())
                                    .map(|s| !s.is_empty())
                                    .unwrap_or(false);
                                if !has_valid_version {
                                    manifest["version"] = serde_json::Value::String("1.28.27".to_string());
                                    if let Ok(new_content) = serde_json::to_string_pretty(&manifest) {
                                        let _ = tokio::fs::write(&manifest_path, new_content).await;
                                    }
                                }
                            }
                    
                    final_ext_paths.push(browser_ext_dir.to_string_lossy().to_string());
                } else {
                    // Non-automa extension (e.g. ublock, cookie-injector): load directly without copying
                    final_ext_paths.push(cleaned);
                }
            } else {
                tracing::warn!(
                    "[BrowserManager] Extension path '{}' does not exist on disk. Omitted from launch flags.",
                    cleaned
                );
            }
        }

        let mut launcher = BrowserLauncher::new(BrowserLauncherOptions {
            executable_path,
            user_data_dir,
            debugging_port,
            extension_paths: final_ext_paths,
            custom_args,
        });

        let ws_url = launcher.launch().await?;
        let pid_opt = launcher.get_pid();
        self.pid = pid_opt;
        
        if let Some(pid) = pid_opt {
            let mut registry = browser_registry().write().await;
            registry.insert(self.options.browser_id.clone(), pid);

            let mut sessions = browser_sessions().write().await;
            sessions.insert(self.options.browser_id.clone(), BrowserSession {
                pid,
                debugging_port,
                ws_url: ws_url.clone(),
                user_data_dir: self.options.user_data_dir.clone().unwrap_or_default(),
            });
        }

        // Store active launcher in registry so that it can be closed cleanly via command-group
        {
            let mut launchers = browser_launchers().write().await;
            if let Some(mut old) = launchers.remove(&self.options.browser_id) {
                let _ = old.close().await;
            }
            launchers.insert(self.options.browser_id.clone(), launcher);
        }
        
        Ok(ws_url)
    }

    pub fn get_pid(&self) -> Option<u32> {
        self.pid.or_else(|| self.launcher.as_ref().and_then(|l| l.get_pid()))
    }

    pub async fn cleanup(&mut self) -> Result<()> {
        let launcher = {
            let mut launchers = browser_launchers().write().await;
            launchers.remove(&self.options.browser_id)
        }.or_else(|| self.launcher.take());

        if let Some(mut launcher) = launcher {
            launcher.close().await?;
        }

        {
            let mut registry = browser_registry().write().await;
            registry.remove(&self.options.browser_id);

            let mut sessions = browser_sessions().write().await;
            sessions.remove(&self.options.browser_id);
        }

        self.pid = None;

        // Clean up copied extension directories created for this session
        for ext_dir in &self.created_ext_dirs {
            let p = std::path::Path::new(ext_dir);
            if p.exists() {
                let _ = tokio::fs::remove_dir_all(p).await;
            }
        }
        self.created_ext_dirs.clear();

        // If user_data_dir was ephemeral (created in Temp with automa_browser_), clean it up
        if let Some(ref dir) = self.resolved_user_data_dir
            && dir.contains("automa_browser_") {
                let p = std::path::Path::new(dir);
                if p.exists() {
                    let _ = tokio::fs::remove_dir_all(p).await;
                }
            }
        self.resolved_user_data_dir = None;

        Ok(())
    }

    pub async fn destroy_all() {
        // 1. Close all active command-group launchers
        {
            let mut launchers = browser_launchers().write().await;
            for (_, mut launcher) in launchers.drain() {
                let _ = launcher.close().await;
            }
        }

        let registry = browser_registry().read().await.clone();
        let mut pids_to_kill = std::collections::HashSet::new();
        for (_, pid) in registry {
            pids_to_kill.insert(pid);
        }

        // 2. Sysinfo sweeps orphaned leftover processes at startup or after crash
        let mut sys = System::new_all();
        sys.refresh_all();
        for (pid, process) in sys.processes() {
            let p_name = process.name().to_string_lossy().to_lowercase();
            if p_name.contains("chrome") || p_name.contains("chromium") || p_name.contains("edge") || p_name.contains("brave") {
                let cmd_line = process.cmd().iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" ");
                let is_child_of_target = process.parent().map(|ppid| pids_to_kill.contains(&ppid.as_u32())).unwrap_or(false);
                if cmd_line.contains("automa") || cmd_line.contains("--remote-debugging-port") || pids_to_kill.contains(&pid.as_u32()) || is_child_of_target {
                    let _ = process.kill();
                }
            }
        }

        browser_registry().write().await.clear();
        browser_sessions().write().await.clear();

        #[cfg(target_os = "windows")]
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        // Clean up all ephemeral temp folders (automa_ext_* and automa_browser_*)
        let temp_dir = get_temp_dir();
        if let Ok(mut entries) = tokio::fs::read_dir(&temp_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("automa_ext_") || name.starts_with("automa_browser_") || name.starts_with("automa_run_") {
                    let p = entry.path();
                    remove_dir_all_with_retry(&p).await;
                }
            }
        }
    }
}

async fn remove_dir_all_with_retry(path: &std::path::Path) {
    for _ in 0..5 {
        if tokio::fs::remove_dir_all(path).await.is_ok() {
            return;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    }
}

fn copy_dir_all(src: std::path::PathBuf, dst: std::path::PathBuf) -> Pin<Box<dyn Future<Output = std::io::Result<()>> + Send>> {
    Box::pin(async move {
        tokio::fs::create_dir_all(&dst).await?;
        let mut entries = tokio::fs::read_dir(src).await?;
        while let Some(entry) = entries.next_entry().await? {
            let ty = entry.file_type().await?;
            if ty.is_dir() {
                copy_dir_all(entry.path(), dst.join(entry.file_name())).await?;
            } else {
                tokio::fs::copy(entry.path(), dst.join(entry.file_name())).await?;
            }
        }
        Ok(())
    })
}

fn get_temp_dir() -> std::path::PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let temp = std::path::PathBuf::from(local_app_data).join("Temp");
        if temp.exists() {
            return temp;
        }
    }
    std::env::temp_dir()
}

pub async fn kill_browser_processes(browser_id: &str, _direct_pid: Option<u32>) {
    let launcher = {
        let mut launchers = browser_launchers().write().await;
        launchers.remove(browser_id)
    };

    if let Some(mut launcher) = launcher {
        let _ = launcher.close().await;
    }

    {
        let mut registry = browser_registry().write().await;
        registry.remove(browser_id);

        let mut sessions = browser_sessions().write().await;
        sessions.remove(browser_id);
    }
}

pub async fn graceful_stop_browser(browser_id: &str, direct_pid: Option<u32>) {
    kill_browser_processes(browser_id, direct_pid).await;
}

pub async fn sanitize_browser_profile(user_data_dir: &std::path::Path, is_force: bool) {
    if !user_data_dir.exists() {
        return;
    }

    // Always clean lock files to ensure profile is not locked on next startup
    let lock_files = ["SingletonLock", "SingletonCookie", "SingletonSocket", "lockfile", "parent.lock"];
    for lock in &lock_files {
        let lock_path = user_data_dir.join(lock);
        if lock_path.exists() {
            let _ = tokio::fs::remove_file(&lock_path).await;
        }
    }

    if !is_force {
        let volatile_cache_dirs = [
            "Cache",
            "Code Cache",
            "GPUCache",
            "DawnCache",
            "ShaderCache",
            "GrShaderCache",
            "Crashpad",
            "Default/Cache",
            "Default/Code Cache",
            "Default/GPUCache",
            "Default/DawnCache",
            "Default/ShaderCache",
            "Default/GrShaderCache",
        ];

        for cache_rel in &volatile_cache_dirs {
            let cache_path = user_data_dir.join(cache_rel);
            if cache_path.exists() {
                let _ = tokio::fs::remove_dir_all(&cache_path).await;
            }
        }

        // Clean any crash dumps (*.dmp) in Crashpad or root
        if let Ok(mut entries) = tokio::fs::read_dir(user_data_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(ext) = entry.path().extension()
                    && ext == "dmp" {
                        let _ = tokio::fs::remove_file(entry.path()).await;
                    }
            }
        }
    }
}
