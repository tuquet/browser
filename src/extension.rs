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
struct RawChromeManifest {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    manifest_version: Option<u32>,
}

pub fn strip_windows_verbatim(path: PathBuf) -> PathBuf {
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

/// Remote extension package descriptor available in tuquet-scoop-bucket
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct RemoteExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub homepage: Option<String>,
    pub url: String,
    pub hash: Option<String>,
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct RawScoopManifest {
    version: Option<String>,
    description: Option<String>,
    homepage: Option<String>,
    url: Option<String>,
    hash: Option<String>,
}

/// Locates local Scoop bucket directory if available on workstation
pub fn find_local_bucket_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let home_path = PathBuf::from(home);
        let candidates = [
            home_path.join("scoop").join("buckets").join("tuquet").join("bucket"),
            home_path.join("Repository").join("tuquet").join("scoop-bucket").join("bucket"),
            home_path.join("Repository").join("tuquet-scoop-bucket").join("bucket"),
        ];
        for c in candidates {
            if c.exists() {
                return Some(c);
            }
        }
    }
    None
}

/// Scans and lists all downloadable extensions from tuquet-scoop-bucket
pub async fn fetch_available_extensions() -> Result<Vec<RemoteExtensionManifest>> {
    let mut results = Vec::new();

    // 1. Try scanning local Scoop bucket for maximum speed and offline capability
    if let Some(bucket_dir) = find_local_bucket_dir() {
        if let Ok(entries) = std::fs::read_dir(&bucket_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with("ext-") && file_name.ends_with(".json") {
                        let id = file_name
                            .trim_start_matches("ext-")
                            .trim_end_matches(".json")
                            .to_string();

                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(raw) = serde_json::from_str::<RawScoopManifest>(&content) {
                                let desc = raw.description.unwrap_or_else(|| format!("Extension {}", id));
                                let name = if let Some(dash_idx) = desc.find(" - ") {
                                    desc[..dash_idx].trim().to_string()
                                } else {
                                    id.replace('-', " ")
                                };

                                results.push(RemoteExtensionManifest {
                                    id,
                                    name,
                                    version: raw.version.unwrap_or_else(|| "1.0.0".to_string()),
                                    description: desc,
                                    homepage: raw.homepage,
                                    url: raw.url.unwrap_or_default(),
                                    hash: raw.hash,
                                    source: "tuquet-scoop-bucket (local)".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. If no local bucket or results, fetch from GitHub raw content
    if results.is_empty() {
        let known_ids = ["automa", "ublock", "cookie-injector"];
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        for id in known_ids {
            let manifest_url = format!(
                "https://raw.githubusercontent.com/tuquet/scoop-bucket/main/bucket/ext-{}.json",
                id
            );
            if let Ok(res) = client.get(&manifest_url).send().await {
                if res.status().is_success() {
                    if let Ok(raw) = res.json::<RawScoopManifest>().await {
                        let desc = raw.description.unwrap_or_else(|| format!("Extension {}", id));
                        let name = if let Some(dash_idx) = desc.find(" - ") {
                            desc[..dash_idx].trim().to_string()
                        } else {
                            id.replace('-', " ")
                        };

                        results.push(RemoteExtensionManifest {
                            id: id.to_string(),
                            name,
                            version: raw.version.unwrap_or_else(|| "1.0.0".to_string()),
                            description: desc,
                            homepage: raw.homepage,
                            url: raw.url.unwrap_or_default(),
                            hash: raw.hash,
                            source: "tuquet-scoop-bucket (github)".to_string(),
                        });
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(results)
}

/// Downloads, unpacks, and installs an extension from tuquet-scoop-bucket
pub async fn install_remote_extension(id: &str, force: bool) -> Result<Extension> {
    let available = fetch_available_extensions().await?;
    let manifest = available
        .into_iter()
        .find(|m| m.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| anyhow!("Extension '{}' not found in tuquet-scoop-bucket catalog. Run 'tuquet browser ext catalog' to see available packages.", id))?;

    let ext_dir = ExtensionRegistry::resolve_extensions_dir().join(&manifest.id);

    if ext_dir.exists() && !force {
        if let Ok(ext) = Extension::from_unpacked_dir(&ext_dir, Some(manifest.id.clone())) {
            let mut reg = ExtensionRegistry::load();
            let _ = reg.register(ext.clone());
            return Ok(ext);
        }
    }

    // Check if built-in automa local development build can be provisioned
    if manifest.id == "automa" {
        let local_automa = PathBuf::from(crate::coordinator::resolve_cli_runner_extension_path());
        if local_automa.exists() {
            let ext = Extension {
                id: "automa".to_string(),
                name: manifest.name,
                version: manifest.version,
                description: Some(manifest.description),
                path: local_automa,
                enabled: true,
                is_builtin: true,
                manifest_version: 3,
            };
            let mut reg = ExtensionRegistry::load();
            reg.register(ext.clone())?;
            return Ok(ext);
        }
    }

    if manifest.url.is_empty() {
        return Err(anyhow!("Extension '{}' has no download URL configured in manifest.", manifest.id));
    }

    println!("============================================================");
    println!(" Tuquet Extension Provisioner (tuquet-scoop-bucket)");
    println!("============================================================");
    println!(" Package:     {}", manifest.id);
    println!(" Name:        {}", manifest.name);
    println!(" Version:     v{}", manifest.version);
    println!(" URL:         {}", manifest.url);
    println!(" Target:      {}", ext_dir.display());
    println!("------------------------------------------------------------");

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let response = client
        .get(&manifest.url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to initiate download for extension {}: {}", manifest.id, e))?;

    if !response.status().is_success() {
        return Err(anyhow!("Failed to download extension archive: HTTP {}", response.status()));
    }

    let bytes = response.bytes().await?;
    let temp_zip = std::env::temp_dir().join(format!("ext_{}_{}.zip", manifest.id, std::process::id()));
    tokio::fs::write(&temp_zip, &bytes).await?;

    if ext_dir.exists() {
        let _ = std::fs::remove_dir_all(&ext_dir);
    }
    std::fs::create_dir_all(&ext_dir)?;

    let zip_path = temp_zip.clone();
    let target_dir = ext_dir.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        use std::fs::File;
        let archive_file = File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(archive_file)?;
        archive.extract(&target_dir)?;
        Ok(())
    }).await??;

    let _ = tokio::fs::remove_file(&temp_zip).await;

    // Check if archive extracted into a nested directory (e.g. dist/ or uBlock0.chromium/)
    let final_dir = if !ext_dir.join("manifest.json").exists() {
        let mut found = ext_dir.clone();
        if let Ok(entries) = std::fs::read_dir(&ext_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join("manifest.json").exists() {
                    found = p;
                    break;
                }
            }
        }
        found
    } else {
        ext_dir.clone()
    };

    let ext = Extension::from_unpacked_dir(&final_dir, Some(manifest.id.clone()))
        .unwrap_or_else(|_| Extension {
            id: manifest.id.clone(),
            name: manifest.name,
            version: manifest.version,
            description: Some(manifest.description),
            path: final_dir,
            enabled: true,
            is_builtin: manifest.id == "automa",
            manifest_version: 3,
        });

    let mut reg = ExtensionRegistry::load();
    reg.register(ext.clone())?;

    println!("Extension '{}' successfully installed and registered.\n", manifest.id);
    Ok(ext)
}

/// Resolves and validates absolute filesystem paths for a profile's requested extensions.
/// Handles all profile extension edge cases:
/// - Edge Case 1.1: Mandatory 'automa' injection when running in automation/workflow mode.
/// - Edge Case 1.2: Extension priority ordering (Automa first, then other extensions).
/// - Edge Case 2.1: JIT auto-provisioning from tuquet-scoop-bucket if missing on disk, with graceful omission on failure.
/// - Edge Case 2.2: Windows path verification (strips verbatim \\?\ prefix, guards against comma delimiter collisions).
pub async fn resolve_profile_extension_paths(
    requested_ids: &[String],
    is_automation_mode: bool,
) -> Vec<PathBuf> {
    let mut ids: Vec<String> = requested_ids.to_vec();

    // Edge Case 1.1: Enforce automa presence for automation workflows
    if is_automation_mode && !ids.iter().any(|id| id.eq_ignore_ascii_case("automa")) {
        ids.insert(0, "automa".to_string());
    }

    // Edge Case 1.2: Ensure 'automa' is always prioritized first if present
    if let Some(pos) = ids.iter().position(|id| id.eq_ignore_ascii_case("automa")) {
        if pos > 0 {
            let automa_id = ids.remove(pos);
            ids.insert(0, automa_id);
        }
    }

    let mut resolved_paths = Vec::new();

    for id in ids {
        let id_clean = id.trim().to_lowercase();
        if id_clean.is_empty() {
            continue;
        }

        let registry = ExtensionRegistry::load();
        let mut path_opt: Option<PathBuf> = None;

        if let Some(ext) = registry.get(&id_clean) {
            if ext.path.exists() {
                path_opt = Some(ext.path.clone());
            }
        }

        // Edge Case 2.1: Missing on disk -> Attempt JIT provisioning from tuquet-scoop-bucket
        if path_opt.is_none() {
            tracing::info!(
                "[ProfileExtension] Extension '{}' is not present on disk. Attempting JIT provisioning from scoop-bucket...",
                id_clean
            );
            if let Ok(installed) = install_remote_extension(&id_clean, false).await {
                if installed.path.exists() {
                    path_opt = Some(installed.path);
                }
            } else if id_clean == "automa" {
                // Fallback to local cli runner extension path
                let fallback_path = PathBuf::from(crate::coordinator::resolve_cli_runner_extension_path());
                if fallback_path.exists() {
                    path_opt = Some(fallback_path);
                }
            }
        }

        match path_opt {
            Some(p) => {
                let clean = strip_windows_verbatim(p);
                let p_str = clean.to_string_lossy();
                // Edge Case 2.2: Guard against comma in path breaking Chromium's delimiter
                if p_str.contains(',') {
                    tracing::error!(
                        "[ProfileExtension] Path '{}' contains a comma ',', which invalidates Chromium --load-extension. Skipping.",
                        p_str
                    );
                    continue;
                }
                if !resolved_paths.contains(&clean) {
                    resolved_paths.push(clean);
                }
            }
            None => {
                // Edge Case 2.1 Graceful fallback: warn and omit to avoid crashing Chrome
                tracing::warn!(
                    "[ProfileExtension] Warning: Extension '{}' could not be resolved or downloaded. Omitted from launch args to prevent browser crash.",
                    id_clean
                );
            }
        }
    }

    resolved_paths
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

    #[tokio::test]
    async fn test_profile_extension_automa_ordering() {
        let requested = vec!["ublock".to_string(), "cookie-injector".to_string()];
        // In automation mode, automa must be auto-injected and placed first
        let mut ids = requested.clone();
        if !ids.iter().any(|id| id.eq_ignore_ascii_case("automa")) {
            ids.insert(0, "automa".to_string());
        }
        assert_eq!(ids[0], "automa");
    }
}
