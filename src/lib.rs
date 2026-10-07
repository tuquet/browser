pub mod coordinator;
pub mod extension;
pub mod launcher;
pub mod manager;
pub mod manifest;
pub mod packer;
pub mod profile;
pub mod proxy;
pub mod resolver;

pub use packer::{DEFAULT_PROFILE_IGNORE, PackReport, ProfilePacker, UnpackReport};
pub use proxy::{ProxyProbe, ProxyProbeResult};

pub use coordinator::{
    connected_browsers, get_browser_launcher_lock, resolve_cli_runner_extension_path,
};
pub use extension::{
    fetch_available_extensions, install_remote_extension, resolve_profile_extension_paths,
    strip_windows_verbatim, Extension, ExtensionRegistry, RemoteExtensionManifest,
};
pub use launcher::{BrowserLauncher, BrowserLauncherOptions};
pub use manager::{
    browser_launchers, browser_registry, browser_sessions, graceful_stop_browser, kill_browser_processes,
    sanitize_browser_profile, BrowserManager, BrowserManagerOptions, BrowserSession,
};
pub use manifest::{fetch_manifest, BrowserManifest, PlatformAsset, ReleaseEntry};
pub use profile::BrowserProfile;
pub use resolver::{
    clean_runtime, detect_host_browsers, detect_version_from_dir, download_chromium_runtime, download_stealth_runtime,
    get_active_runtime_name, get_active_version, get_browser_config_path, get_download_url, get_platform_exe_rel_path,
    get_platform_key, get_runtime_dir, get_runtime_exe_path, get_runtime_status,
    list_installed_runtimes, resolve_data_dir, resolve_executable_path, set_active_runtime, set_active_version,
    BrowserConfigFile, DetectedHostBrowser, InstalledRuntimeInfo, RuntimeStatus,
    PINNED_CHROMIUM_REVISION, PINNED_CHROMIUM_VERSION, PINNED_STEALTH_CHROMIUM_VERSION,
};

