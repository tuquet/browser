use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::models::Extension;

/// Persistent registry of browser extensions managed by Specter
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct ExtensionRegistry {
    pub extensions: HashMap<String, Extension>,
}

impl ExtensionRegistry {
    /// Resolves canonical extensions directory: ~/.specter/browser/extensions/
    pub fn resolve_extensions_dir() -> PathBuf {
        crate::resolver::canonical_ssot_dir().join("browser").join("extensions")
    }

    /// Registry JSON storage path: ~/.specter/browser/extensions/registry.json
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
                description: Some("Built-in Specter worker driver for Chrome DevTools Protocol automation".to_string()),
                path: default_automa_path,
                enabled: true,
                is_builtin: true,
                manifest_version: 3,
            };
            registry.extensions.insert("automa".to_string(), automa_ext);
            if exists {
                let _ = registry.save();
            }
        } else if let Some(ext) = registry.extensions.get_mut("automa")
            && !ext.path.exists() && default_automa_path.exists() {
                ext.path = default_automa_path;
                let _ = registry.save();
            }

        registry
    }

    /// Persists registry to ~/.specter/browser/extensions/registry.json
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
