use anyhow::{anyhow, Result};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};

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
    process: Option<Child>,
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

    /// Khởi chạy trình duyệt dưới dạng tiến trình độc lập
    pub async fn launch(&mut self) -> Result<String> {
        let is_headless = self.options.custom_args.iter().any(|a| a.starts_with("--headless"));
        let mut args = vec![
            format!("--remote-debugging-port={}", self.options.debugging_port),
            format!("--user-data-dir={}", self.options.user_data_dir),
            "--no-first-run".to_string(),
            "--password-store=basic".to_string(),
            "--no-sandbox".to_string(),
            "--disable-setuid-sandbox".to_string(),
            "--log-level=3".to_string(),
            "--test-type".to_string(),
            // Edge Case 7.1: Network guardrails (prevent hanging on extension auto-updates behind SOCKS5 proxy)
            "--disable-component-update".to_string(),
            "--disable-domain-reliability".to_string(),
            "--disable-blink-features=AutomationControlled".to_string(),
        ];

        if is_headless {
            args.push("--disable-gpu".to_string());
            args.push("--disable-software-rasterizer".to_string());
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
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn browser process: {}", e))?;

        tracing::info!("[BrowserLauncher] Browser running at PID: {:?}", child.id());
        
        self.process = Some(child);

        let ws_url = self.wait_for_ws_url().await?;
        self.ws_url = Some(ws_url.clone());
        Ok(ws_url)
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

    /// Đóng trình duyệt (Kill tiến trình chính)
    pub async fn close(&mut self) -> Result<()> {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill().await;
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
