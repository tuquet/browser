use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::models::Extension;
use super::registry::ExtensionRegistry;

/// Remote extension package descriptor available in scoop catalog
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
pub struct RawScoopManifest {
    pub version: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub url: Option<String>,
    pub hash: Option<String>,
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

/// Scans and lists all downloadable extensions from the scoop catalog
pub async fn fetch_available_extensions() -> Result<Vec<RemoteExtensionManifest>> {
    let mut results = Vec::new();

    // 1. Try scanning local Scoop bucket for maximum speed and offline capability
    if let Some(bucket_dir) = find_local_bucket_dir()
        && let Ok(entries) = std::fs::read_dir(&bucket_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                    && file_name.starts_with("ext-") && file_name.ends_with(".json") {
                        let id = file_name
                            .trim_start_matches("ext-")
                            .trim_end_matches(".json")
                            .to_string();

                        if let Ok(content) = std::fs::read_to_string(&path)
                            && let Ok(raw) = serde_json::from_str::<RawScoopManifest>(&content) {
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
                                    source: "scoop-catalog (local)".to_string(),
                                });
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
            if let Ok(res) = client.get(&manifest_url).send().await
                && res.status().is_success()
                    && let Ok(raw) = res.json::<RawScoopManifest>().await {
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
                            source: "scoop-catalog (github)".to_string(),
                        });
                    }
        }
    }

    results.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(results)
}

/// Downloads, unpacks, and installs an extension from the scoop catalog
pub async fn install_remote_extension(id: &str, force: bool) -> Result<Extension> {
    let available = fetch_available_extensions().await?;
    let manifest = available
        .into_iter()
        .find(|m| m.id.eq_ignore_ascii_case(id))
        .ok_or_else(|| anyhow!("Extension '{}' not found in scoop catalog. Run 'specter browser ext catalog' to see available packages.", id))?;

    let ext_dir = ExtensionRegistry::resolve_extensions_dir().join(&manifest.id);

    if ext_dir.exists() && !force
        && let Ok(ext) = Extension::from_unpacked_dir(&ext_dir, Some(manifest.id.clone())) {
            let mut reg = ExtensionRegistry::load();
            let _ = reg.register(ext.clone());
            return Ok(ext);
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
    println!(" Specter Extension Provisioner (Scoop Catalog)");
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
