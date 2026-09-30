use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub path: PathBuf,
    /// Whether this extension should be loaded into automated browser sessions
    pub enabled: bool,
    /// Whether this is a bundled system extension
    pub is_builtin: bool,
    /// Manifest specification version (2 or 3)
    pub manifest_version: u32,
}

#[derive(Debug, Deserialize)]
struct RawChromeManifest {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    manifest_version: Option<u32>,
}

fn strip_windows_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    if s.starts_with(r"\\?\") {
        PathBuf::from(&s[4..])
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

/// Persistent registry of browser extensions managed by Tuquet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRegistry {
    pub extensions: HashMap<String, Extension>,
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self {
            extensions: HashMap::new(),
        }
    }
}

impl ExtensionRegistry {
    /// Resolves canonical extensions directory: ~/.tuquet/extensions/
    pub fn resolve_extensions_dir() -> PathBuf {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".tuquet").join("extensions")
    }

    /// Registry JSON storage path: ~/.tuquet/extensions/registry.json
    pub fn registry_file_path() -> PathBuf {
        Self::resolve_extensions_dir().join("registry.json")
    }

    /// Loads the extension registry from disk, automatically bootstrapping the default Automa extension
    pub fn load() -> Self {
        let path = Self::registry_file_path();
        let mut registry = if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                serde_json::from_str::<ExtensionRegistry>(&content).unwrap_or_default()
            } else {
                Self::default()
            }
        } else {
            Self::default()
        };

        // Bootstrap default Automa extension if missing or needs update
        let default_automa_path = PathBuf::from(crate::coordinator::resolve_cli_runner_extension_path());
        if !registry.extensions.contains_key("automa") {
            let exists = default_automa_path.exists();
            let automa_ext = Extension {
                id: "automa".to_string(),
                name: "Automa MV3 Runner Extension".to_string(),
                version: "1.0.0".to_string(),
                description: Some("Built-in Tuquet worker driver for Chrome DevTools Protocol automation".to_string()),
                path: default_automa_path,
                enabled: true,
                is_builtin: true,
                manifest_version: 3,
            };
            registry.extensions.insert("automa".to_string(), automa_ext);
            if exists {
                let _ = registry.save();
            }
        } else if let Some(ext) = registry.extensions.get_mut("automa") {
            if !ext.path.exists() && default_automa_path.exists() {
                ext.path = default_automa_path;
                let _ = registry.save();
            }
        }

        registry
    }

    /// Persists registry to ~/.tuquet/extensions/registry.json
    pub fn save(&self) -> Result<()> {
        let dir = Self::resolve_extensions_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        let file_path = Self::registry_file_path();
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&file_path, json)?;
        Ok(())
    }

    /// Lists all registered extensions sorted alphabetically by ID
    pub fn list(&self) -> Vec<Extension> {
        let mut list: Vec<Extension> = self.extensions.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    /// Retrieves an extension descriptor by ID
    pub fn get(&self, id: &str) -> Option<&Extension> {
        self.extensions.get(id)
    }

    /// Registers a new extension and persists the change
    pub fn register(&mut self, ext: Extension) -> Result<()> {
        self.extensions.insert(ext.id.clone(), ext);
        self.save()
    }

    /// Unregisters an extension by ID
    pub fn unregister(&mut self, id: &str) -> Result<bool> {
        if id == "automa" {
            return Err(anyhow!("Cannot unregister built-in 'automa' extension. You may disable it instead."));
        }
        let removed = self.extensions.remove(id).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// Toggles the enabled state of an extension
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<bool> {
        if let Some(ext) = self.extensions.get_mut(id) {
            ext.enabled = enabled;
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Returns absolute paths to all currently enabled and existing extensions
    pub fn get_enabled_paths(&self) -> Vec<PathBuf> {
        self.extensions
            .values()
            .filter(|e| e.enabled && e.path.exists())
            .map(|e| e.path.clone())
            .collect()
    }

    /// Formats a comma-separated string for Chromium's `--load-extension` flag
    pub fn format_load_extension_arg(paths: &[PathBuf]) -> String {
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_load_extension_arg() {
        let paths = vec![PathBuf::from("/path/one"), PathBuf::from("/path/two")];
        let arg = ExtensionRegistry::format_load_extension_arg(&paths);
        assert!(arg.contains("/path/one"));
        assert!(arg.contains("/path/two"));
        assert!(arg.contains(','));
    }

    #[test]
    fn test_extension_crud() {
        let mut reg = ExtensionRegistry::default();
        let ext = Extension {
            id: "test-ext".to_string(),
            name: "Test Extension".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            path: PathBuf::from("dummy/path"),
            enabled: true,
            is_builtin: false,
            manifest_version: 3,
        };

        reg.extensions.insert(ext.id.clone(), ext);
        assert_eq!(reg.list().len(), 1);
        assert!(reg.get("test-ext").is_some());

        // Toggle enabled
        let _ = reg.set_enabled("test-ext", false);
        assert!(!reg.get("test-ext").unwrap().enabled);

        // Built-in protection
        assert!(reg.unregister("automa").is_err());
    }
}
