use anyhow::{anyhow, Result};
use command_group::{AsyncCommandGroup, AsyncGroupChild};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct BrowserLauncherOptions {
    pub executable_path: String,
    pub user_data_dir: String,
    pub debugging_port: u16,
    pub extension_paths: Vec<String>,
    pub custom_args: Vec<String>,
}

pub struct BrowserLauncher {
    options: BrowserLauncherOptions,
    process: Option<AsyncGroupChild>,
    ws_url: Option<String>,
}

impl BrowserLauncher {
    pub fn new(options: BrowserLauncherOptions) -> Self {
        Self {
            options,
            process: None,
            ws_url: None,
        }
    }

    /// Sinh danh sách tham số dòng lệnh khởi chạy Chromium
    pub fn build_args(&self) -> Vec<String> {
        let is_headless = self.options.custom_args.iter().any(|a| a.starts_with("--headless"));
        let mut args = Vec::new();

        if self.options.debugging_port > 0 {
            args.push(format!("--remote-debugging-port={}", self.options.debugging_port));
        }
        args.push(format!("--user-data-dir={}", self.options.user_data_dir));
        args.push("--no-first-run".to_string());
        args.push("--password-store=basic".to_string());
        #[cfg(unix)]
        {
            // In Linux containers or when executing as root (UID 0), Chromium requires --no-sandbox
            let is_root = unsafe { libc::getuid() == 0 };
            if is_root || std::env::var("SPECTER_FORCE_NO_SANDBOX").map(|v| v == "1" || v == "true").unwrap_or(false) {
                args.push("--no-sandbox".to_string());
                args.push("--disable-setuid-sandbox".to_string());
            }
        }
        args.push("--log-level=3".to_string());
        args.push("--test-type".to_string());
        // Edge Case 7.1: Network guardrails (prevent hanging on extension auto-updates behind SOCKS5 proxy)
        args.push("--disable-component-update".to_string());
        args.push("--disable-domain-reliability".to_string());
        args.push("--disable-blink-features=AutomationControlled".to_string());

        if is_headless {
            args.push("--disable-gpu".to_string());
            args.push("--disable-software-rasterizer".to_string());
            let has_custom_ua = self.options.custom_args.iter().any(|a| a.starts_with("--user-agent"));
            if !has_custom_ua {
                let active_ver = crate::resolver::get_active_version();
                let major_ver = active_ver.split('.').next().unwrap_or("148");
                #[cfg(target_os = "windows")]
                args.push(format!("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36", major_ver));
                #[cfg(target_os = "macos")]
                args.push(format!("--user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36", major_ver));
                #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
                args.push(format!("--user-agent=Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{}.0.0.0 Safari/537.36", major_ver));
            }
        } else {
            args.push("--window-size=1280,720".to_string());
        }

        // Edge Case 2.1: Filter extension paths that exist on disk and strip verbatim prefixes
        let valid_ext_paths: Vec<String> = self
            .options
            .extension_paths
            .iter()
            .map(|p| {
                let mut s = p.clone();
                if s.starts_with(r"\\?\") {
                    s = s[4..].to_string();
                }
                s
            })
            .filter(|p| std::path::Path::new(p).exists())
            .collect();

        if !valid_ext_paths.is_empty() {
            let exts = valid_ext_paths.join(",");
            args.push(format!("--load-extension={}", exts));
            args.push(format!("--disable-extensions-except={}", exts));
        }

        // Custom args handling + Edge Case 3.1: Enforce --headless=new when extensions are loaded
        for arg in &self.options.custom_args {
            if !valid_ext_paths.is_empty() && (arg == "--headless" || arg == "--headless=true") {
                if !args.contains(&"--headless=new".to_string()) {
                    args.push("--headless=new".to_string());
                }
            } else if !args.contains(arg) {
                args.push(arg.clone());
            }
        }
        if !args.iter().any(|a| a.starts_with("http://") || a.starts_with("https://")) {
            args.push("about:blank".to_string());
        }

        args
    }

    /// Khởi chạy trình duyệt dưới dạng tiến trình độc lập
    pub async fn launch(&mut self) -> Result<String> {
        let args = self.build_args();
        tracing::info!("[BrowserLauncher] Launching: {} {}", self.options.executable_path, args.join(" "));

        #[allow(unused_mut)]
        let mut cmd = Command::new(&self.options.executable_path);
        cmd.args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        #[cfg(target_os = "windows")]
        {
            if args.iter().any(|a| a.starts_with("--headless")) {
                // CREATE_NO_WINDOW (0x08000000)
                cmd.creation_flags(0x08000000);
            }
        }

        let child = cmd
            .group_spawn()
            .map_err(|e| anyhow!("Failed to spawn browser process: {}", e))?;

        tracing::info!("[BrowserLauncher] Browser running at PID: {:?}", child.id());
        
        self.process = Some(child);

        if self.options.debugging_port > 0 {
            let ws_url = self.wait_for_ws_url().await?;
            self.ws_url = Some(ws_url.clone());
            Ok(ws_url)
        } else {
            self.ws_url = None;
            Ok("zero-port (extension mode)".to_string())
        }
    }

    /// Lấy WebSocket Debugger URL bằng cách polling
    async fn wait_for_ws_url(&mut self) -> Result<String> {
        let url = format!("http://127.0.0.1:{}/json/version", self.options.debugging_port);
        let client = reqwest::Client::new();

        for _ in 0..60 {
            if let Some(child) = self.process.as_mut()
                && let Ok(Some(_status)) = child.try_wait() {
                    return Err(anyhow!("Chrome process exited unexpectedly before opening debugger port. Browser might be locked."));
                }
            if let Ok(resp) = client.get(&url).send().await
                && let Ok(json) = resp.json::<serde_json::Value>().await
                    && let Some(ws) = json.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) {
                        return Ok(ws.to_string());
                    }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Err(anyhow!("Timeout: Không thể kết nối tới Chrome CDP Endpoint."))
    }

    /// Đóng trình duyệt (Kill tiến trình chính và toàn bộ tiến trình con thông qua command-group)
    pub async fn close(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill().await;
            let _ = tokio::time::timeout(Duration::from_millis(2000), child.wait()).await;
        }
        Ok(())
    }

    pub fn get_pid(&self) -> Option<u32> {
        self.process.as_ref().and_then(|c| c.id())
    }

    pub fn get_ws_url(&self) -> Option<&str> {
        self.ws_url.as_deref()
    }
}

impl Drop for BrowserLauncher {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.start_kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_launcher_drop_when_not_started() {
        let options = BrowserLauncherOptions {
            executable_path: "dummy".to_string(),
            user_data_dir: "dummy_dir".to_string(),
            debugging_port: 9222,
            extension_paths: vec![],
            custom_args: vec![],
        };
        let launcher = BrowserLauncher::new(options);
        drop(launcher);
    }

    #[tokio::test]
    async fn test_browser_launcher_close_when_not_started() {
        let options = BrowserLauncherOptions {
            executable_path: "dummy".to_string(),
            user_data_dir: "dummy_dir".to_string(),
            debugging_port: 9222,
            extension_paths: vec![],
            custom_args: vec![],
        };
        let mut launcher = BrowserLauncher::new(options);
        assert!(launcher.close().await.is_ok());
    }

    #[test]
    fn test_launcher_build_args_driver_mode() {
        let options = BrowserLauncherOptions {
            executable_path: "chrome.exe".to_string(),
            user_data_dir: "data_dir".to_string(),
            debugging_port: 9222,
            extension_paths: vec![],
            custom_args: vec![],
        };
        let launcher = BrowserLauncher::new(options);
        let args = launcher.build_args();
        assert!(args.contains(&"--remote-debugging-port=9222".to_string()));
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_string()));
    }

    #[test]
    fn test_launcher_build_args_extension_mode() {
        let options = BrowserLauncherOptions {
            executable_path: "chrome.exe".to_string(),
            user_data_dir: "data_dir".to_string(),
            debugging_port: 0, // Zero-Port Ultra-Stealth
            extension_paths: vec![],
            custom_args: vec![],
        };
        let launcher = BrowserLauncher::new(options);
        let args = launcher.build_args();
        assert!(!args.iter().any(|a| a.starts_with("--remote-debugging-port")));
        assert!(args.contains(&"--disable-blink-features=AutomationControlled".to_string()));
    }

    #[test]
    fn test_launcher_headless_user_agent_synchronized() {
        let options = BrowserLauncherOptions {
            executable_path: "chrome.exe".to_string(),
            user_data_dir: "data_dir".to_string(),
            debugging_port: 9222,
            extension_paths: vec![],
            custom_args: vec!["--headless=new".to_string()],
        };
        let launcher = BrowserLauncher::new(options);
        let args = launcher.build_args();
        let ua_arg = args.iter().find(|a| a.starts_with("--user-agent=")).expect("Headless mode should inject default UA");
        let active_ver = crate::resolver::get_active_version();
        let major = active_ver.split('.').next().unwrap_or("148");
        assert!(ua_arg.contains(&format!("Chrome/{}.0.0.0", major)));
    }
}


