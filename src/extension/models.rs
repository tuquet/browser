use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Detailed descriptor for a browser extension
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct Extension {
    /// Unique identifier slug (e.g. "automa", "ublock-origin")
    pub id: String,
    /// Human-readable display label
    pub name: String,
    /// Extension version string
    pub version: String,
    /// Brief description if specified in manifest.json
    pub description: Option<String>,
    /// Absolute filesystem path to the unpacked extension directory
    #[cfg_attr(feature = "utoipa", schema(value_type = String))]
    pub path: PathBuf,
    /// Whether this extension should be loaded into automated browser sessions
    pub enabled: bool,
    /// Whether this is a bundled system extension
    pub is_builtin: bool,
    /// Manifest specification version (2 or 3)
    pub manifest_version: u32,
}

#[derive(Debug, Deserialize)]
pub struct RawChromeManifest {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub manifest_version: Option<u32>,
}

pub fn strip_windows_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

impl Extension {
    /// Creates and validates an extension descriptor from an unpacked directory containing manifest.json
    pub fn from_unpacked_dir(dir: &Path, id_override: Option<String>) -> Result<Self> {
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.exists() {
            return Err(anyhow!(
                "Directory {:?} does not contain a valid manifest.json",
                dir
            ));
        }

        let content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| anyhow!("Failed to read manifest.json at {:?}: {}", manifest_path, e))?;

        let raw: RawChromeManifest = serde_json::from_str(&content)
            .map_err(|e| anyhow!("Failed to parse manifest.json: {}", e))?;

        let name = raw.name.unwrap_or_else(|| {
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Extension".to_string())
        });
        let version = raw.version.unwrap_or_else(|| "1.0.0".to_string());
        let manifest_version = raw.manifest_version.unwrap_or(3);

        let id = id_override.unwrap_or_else(|| {
            name.to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
                .collect::<String>()
                .trim_matches('-')
                .to_string()
        });

        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let clean_path = strip_windows_verbatim(canonical);

        Ok(Self {
            id,
            name,
            version,
            description: raw.description,
            path: clean_path,
            enabled: true,
            is_builtin: false,
            manifest_version,
        })
    }
}
