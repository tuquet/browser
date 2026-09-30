pub mod coordinator;
pub mod error;
pub mod extension;
pub mod launcher;
pub mod manager;
pub mod profile;
pub mod resolver;

pub use coordinator::{
    connected_browsers, get_browser_launcher_lock, resolve_cli_runner_extension_path,
};
pub use error::BrowserError;
pub use extension::{Extension, ExtensionRegistry};
pub use launcher::{BrowserLauncher, BrowserLauncherOptions};
pub use manager::{
    browser_registry, browser_sessions, graceful_stop_browser, kill_browser_processes,
    sanitize_browser_profile, BrowserManager, BrowserManagerOptions, BrowserSession,
};
pub use profile::BrowserProfile;
pub use resolver::{
    clean_runtime, detect_host_browsers, download_chromium_runtime, get_download_url,
    get_platform_exe_rel_path, get_platform_key, get_runtime_dir, get_runtime_exe_path,
    get_runtime_status, resolve_data_dir, resolve_executable_path, DetectedHostBrowser,
    RuntimeStatus, PINNED_CHROMIUM_REVISION, PINNED_CHROMIUM_VERSION,
};
