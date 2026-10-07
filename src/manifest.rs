use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub const RAW_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/tuquet/browser/main/manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserManifest {
    pub schema_version: String,
    pub name: String,
    pub description: String,
    pub updated_at: String,
    pub default_channel: String,
    pub channels: HashMap<String, String>,
    pub releases: Vec<ReleaseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseEntry {
    pub version: String,
    pub tag: String,
    pub channel: String,
    pub status: String,
    pub notes: String,
    pub published_at: Option<String>,
    pub platforms: HashMap<String, PlatformAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAsset {
    pub asset_name: String,
    pub url: String,
    pub size: u64,
    #[serde(default)]
    pub sha256: Option<String>,
}

impl BrowserManifest {
    /// Loads the embedded manifest packaged directly inside the crate binary (100% offline fallback)
    pub fn get_embedded() -> Self {
        let content = include_str!("../manifest.json");
        serde_json::from_str(content).expect("Embedded manifest.json must be valid JSON")
    }

    /// Finds a release by version (full e.g. '148.0.7778.215' or major e.g. '148'), tag, or channel name ('lts', 'latest')
    pub fn find_release(&self, query: &str) -> Option<&ReleaseEntry> {
        let q = query.trim();
        let stripped = q.strip_prefix('v').unwrap_or(q);

        if stripped.eq_ignore_ascii_case("lts") || stripped.eq_ignore_ascii_case("stealth") {
            return self.get_lts_release();
        }

        let target_ver = self
            .channels
            .get(stripped)
            .map(|s| s.as_str())
            .unwrap_or(stripped);

        // 1. Exact match on version, tag, or channel
        if let Some(r) = self.releases.iter().find(|r| {
            r.version == target_ver
                || r.tag == q
                || r.tag == target_ver
                || r.channel.eq_ignore_ascii_case(q)
        }) {
            return Some(r);
        }

        // 2. Major version prefix match (e.g. '148' matches '148.0.7778.215')
        self.releases.iter().find(|r| {
            r.version.starts_with(&format!("{}.", target_ver))
                || r.tag.starts_with(&format!("v{}.", target_ver))
        })
    }

    /// Returns the Golden LTS release
    pub fn get_lts_release(&self) -> Option<&ReleaseEntry> {
        let lts_ver = self.channels.get("lts").map(|s| s.as_str()).unwrap_or("148.0.7778.215");
        self.releases.iter().find(|r| r.version == lts_ver)
            .or_else(|| self.releases.iter().find(|r| r.status == "recommended"))
    }
}

/// Fetches the live manifest from GitHub CDN with fallback to embedded copy
pub async fn fetch_manifest(force_remote: bool) -> Result<BrowserManifest> {
    if force_remote {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
            .build()?;

        match client.get(RAW_MANIFEST_URL).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(manifest) = resp.json::<BrowserManifest>().await {
                    return Ok(manifest);
                }
            }
            _ => {}
        }
    }

    // Default to embedded manifest
    Ok(BrowserManifest::get_embedded())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_manifest_parses_successfully() {
        let manifest = BrowserManifest::get_embedded();
        assert_eq!(manifest.schema_version, "1.0");
        assert!(!manifest.releases.is_empty());

        let lts = manifest.get_lts_release();
        assert!(lts.is_some());
        assert_eq!(lts.unwrap().version, "148.0.7778.215");
        assert_eq!(lts.unwrap().status, "recommended");
    }

    #[test]
    fn test_find_release_by_channel_and_version() {
        let manifest = BrowserManifest::get_embedded();
        let lts = manifest.find_release("lts");
        assert!(lts.is_some());
        assert_eq!(lts.unwrap().version, "148.0.7778.215");

        let rel148 = manifest.find_release("148.0.7778.215");
        assert!(rel148.is_some());

        // Major version matching (e.g. "148", "v148", "144", "stealth")
        let major148 = manifest.find_release("148");
        assert!(major148.is_some());
        assert_eq!(major148.unwrap().version, "148.0.7778.215");

        let v148 = manifest.find_release("v148");
        assert!(v148.is_some());
        assert_eq!(v148.unwrap().version, "148.0.7778.215");

        let major144 = manifest.find_release("144");
        assert!(major144.is_some());
        assert_eq!(major144.unwrap().version, "144.0.7559.132");

        let stealth = manifest.find_release("stealth");
        assert!(stealth.is_some());
        assert_eq!(stealth.unwrap().version, "148.0.7778.215");

        let nonexistent = manifest.find_release("999.0.0.0");
        assert!(nonexistent.is_none());
    }
}
