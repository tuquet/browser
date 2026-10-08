//! Enterprise Constants for Specter Dedicated Browser & Sandboxing Subsystem

/// Default Chrome DevTools Protocol (CDP) port for automation debugging
pub const DEFAULT_CDP_PORT: u16 = 9222;

/// Canonical SSOT directory name under user home
pub const DEFAULT_SSOT_DIR_NAME: &str = ".specter";

/// Pillar subdirectory name
pub const PILLAR_DIR_BROWSER: &str = "browser";

/// Configuration filename
pub const CONFIG_FILE_BROWSER_JSON: &str = "browser.json";

/// Subdirectories under ~/.specter/browser/
pub const DIR_RUNTIMES: &str = "runtimes";
pub const DIR_PROFILES: &str = "profiles";
pub const DIR_EXTENSIONS: &str = "extensions";
pub const DIR_STEALTH: &str = "stealth";

/// Environment variable overrides
pub const ENV_FORCE_NO_SANDBOX: &str = "SPECTER_FORCE_NO_SANDBOX";
pub const ENV_EXTENSION_PATH: &str = "SPECTER_EXTENSION_PATH";
pub const ENV_SPECTER_HOME: &str = "SPECTER_HOME";
pub const ENV_SPECTER_HOST: &str = "SPECTER_HOST";
pub const ENV_SPECTER_PORT: &str = "SPECTER_PORT";

/// Pinned stable Long-Term-Support (LTS) release of official Open-Source Chromium
pub const PINNED_CHROMIUM_REVISION: &str = "1148";
pub const PINNED_CHROMIUM_VERSION: &str = "131.0.6778.33";

/// Pinned stable Long-Term-Support (LTS) release of C++ Antidetect Chromium
pub const PINNED_STEALTH_CHROMIUM_VERSION: &str = "148.0.7778.215";

/// Default profile name
pub const DEFAULT_PROFILE_NAME: &str = "default";
