use serde::{Deserialize, Serialize};

pub use crate::constants::{
    DEFAULT_SSOT_DIR_NAME, PINNED_CHROMIUM_REVISION, PINNED_CHROMIUM_VERSION,
    PINNED_STEALTH_CHROMIUM_VERSION,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
/// Dedicated browser runtime descriptor for Specter (Zero Host Scanning)
pub struct DetectedHostBrowser {
    /// Browser engine type (always chromium in phase 1)
    pub browser_type: String,
    /// Human-friendly display label
    pub name: String,
    /// Absolute filesystem path to browser executable
    pub executable_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub installed: bool,
    pub platform: String,
    pub executable_path: String,
    pub directory: String,
    pub pinned_version: String,
    pub size_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRuntimeInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub major_version: String,
    pub channel: String,
    pub status_badge: String,
    pub path: String,
    pub is_active: bool,
    pub size_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserConfigFile {
    pub active_version: Option<String>,
    pub active_runtime: Option<String>,
}
