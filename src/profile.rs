use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub timezone: Option<String>,
    pub extensions: Option<Vec<String>>,
    pub custom_headers: Option<std::collections::HashMap<String, String>>,

    // --- Deterministic C++ Antidetect Fingerprint Specifications ---
    #[serde(default)]
    pub fingerprint_seed: Option<u32>,
    #[serde(default)]
    pub os_platform: Option<String>,
    #[serde(default)]
    pub os_version: Option<String>,
    #[serde(default)]
    pub browser_brand: Option<String>,
    #[serde(default)]
    pub hardware_concurrency: Option<u32>,
    #[serde(default)]
    pub device_memory_gb: Option<u32>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub accept_lang: Option<String>,
    #[serde(default)]
    pub webrtc_mode: Option<String>,

    // --- Supabase Cloud Sync & Remote Lease Attributes ---
    #[serde(default)]
    pub cloud_id: Option<String>,
    #[serde(default)]
    pub cloud_synced_at: Option<String>,
    #[serde(default)]
    pub storage_path: Option<String>,
    #[serde(default)]
    pub storage_hash: Option<String>,
    #[serde(default)]
    pub storage_size_bytes: Option<u64>,
    #[serde(default)]
    pub cookies_count: Option<u32>,
    #[serde(default)]
    pub created_at: Option<String>,
}

pub fn slugify(name: &str) -> String {
    let mut s = String::new();
    let mut last_was_dash = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            s.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            s.push('-');
            last_was_dash = true;
        }
    }
    s.trim_matches('-').to_string()
}

pub fn calculate_dir_size(dir: &Path) -> u64 {
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = entry.metadata() {
                    size += meta.len();
                }
            } else if path.is_dir() {
                size += calculate_dir_size(&path);
            }
        }
    }
    size
}

impl BrowserProfile {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            user_agent: None,
            proxy: None,
            timezone: None,
            extensions: Some(vec!["automa".to_string()]),
            custom_headers: None,
            fingerprint_seed: None,
            os_platform: Some("windows".to_string()),
            os_version: Some("10.0.0".to_string()),
            browser_brand: Some("Chrome".to_string()),
            hardware_concurrency: Some(8),
            device_memory_gb: Some(16),
            lang: Some("vi-VN".to_string()),
            accept_lang: Some("vi-VN,vi,en-US,en".to_string()),
            webrtc_mode: Some("proxy_shielded".to_string()),
            cloud_id: None,
            cloud_synced_at: None,
            storage_path: None,
            storage_hash: None,
            storage_size_bytes: None,
            cookies_count: None,
            created_at: None,
        }
    }

    /// Resolves canonical default root directory for browser profiles: ~/.specter/browser/profiles
    pub fn default_profiles_dir() -> PathBuf {
        crate::resolver::canonical_ssot_dir().join("browser").join("profiles")
    }

    /// Generates a realistic deterministic profile with pseudo-random hardware fingerprint
    #[allow(clippy::too_many_arguments)]
    pub fn generate(
        name: impl Into<String>,
        seed_opt: Option<u32>,
        os_opt: Option<String>,
        cores_opt: Option<u32>,
        ram_opt: Option<u32>,
        proxy_opt: Option<String>,
        timezone_opt: Option<String>,
        locale_opt: Option<String>,
    ) -> Self {
        let name_str = name.into();
        let seed = seed_opt.unwrap_or_else(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let low = (now & 0xFFFFFFFF) as u32;
            let high = ((now >> 32) & 0xFFFFFFFF) as u32;
            low ^ high
        });

        let slug = slugify(&name_str);
        let id = if slug.is_empty() {
            format!("prf_{:08x}", seed)
        } else {
            format!("{}-{:06x}", slug, seed & 0xFFFFFF)
        };

        let os_platform = os_opt.unwrap_or_else(|| "windows".to_string());
        let cores = cores_opt.unwrap_or(match seed % 3 {
            0 => 8,
            1 => 12,
            _ => 16,
        });
        let ram = ram_opt.unwrap_or(match seed % 3 {
            0 => 16,
            1 => 32,
            _ => 16,
        });
        let lang = locale_opt.unwrap_or_else(|| "vi-VN".to_string());
        let primary_lang = lang.split('-').next().unwrap_or("vi");
        let accept_lang = format!("{},{},en-US,en", lang, primary_lang);
        let timezone = timezone_opt.unwrap_or_else(|| "Asia/Ho_Chi_Minh".to_string());

        let now_str = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();

        Self {
            id,
            name: name_str,
            user_agent: None,
            proxy: proxy_opt,
            timezone: Some(timezone),
            extensions: Some(vec!["automa".to_string()]),
            custom_headers: None,
            fingerprint_seed: Some(seed),
            os_platform: Some(os_platform),
            os_version: Some("10.0.0".to_string()),
            browser_brand: Some("Chrome".to_string()),
            hardware_concurrency: Some(cores),
            device_memory_gb: Some(ram),
            lang: Some(lang),
            accept_lang: Some(accept_lang),
            webrtc_mode: Some("proxy_shielded".to_string()),
            cloud_id: None,
            cloud_synced_at: None,
            storage_path: None,
            storage_hash: None,
            storage_size_bytes: None,
            cookies_count: None,
            created_at: Some(now_str),
        }
    }

    pub fn get_sandbox_dir(&self, base_dir: &Path) -> PathBuf {
        base_dir.join("profiles").join(&self.id)
    }

    /// Persists profile metadata to ~/.specter/browser/profiles/<id>/profile.json
    pub fn save(&self, base_dir: &Path) -> anyhow::Result<PathBuf> {
        let dir = self.get_sandbox_dir(base_dir);
        std::fs::create_dir_all(&dir)?;
        let meta_path = dir.join("profile.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&meta_path, json)?;
        Ok(meta_path)
    }

    /// Loads a profile by ID, ID-prefix, or Name from ~/.specter/browser/profiles/
    pub fn load(id_or_name: &str, base_dir: &Path) -> anyhow::Result<Self> {
        let target = id_or_name.trim();
        let profiles_dir = base_dir.join("profiles");

        // 1. Direct path check: profiles/<target>/profile.json
        let direct_path = profiles_dir.join(target).join("profile.json");
        if direct_path.exists() {
            let content = std::fs::read_to_string(&direct_path)?;
            let profile = serde_json::from_str::<BrowserProfile>(&content)?;
            return Ok(profile);
        }

        // 2. Scan all profiles and match by exact ID, prefix, or Name
        let all = Self::list_all(base_dir)?;
        if let Some(matched) = all.iter().find(|p| {
            p.id.eq_ignore_ascii_case(target)
                || p.name.eq_ignore_ascii_case(target)
                || p.id.starts_with(target)
        }) {
            return Ok(matched.clone());
        }

        anyhow::bail!("Browser profile '{}' not found in {}", target, profiles_dir.display())
    }

    /// Scans and returns all stored browser profiles
    pub fn list_all(base_dir: &Path) -> anyhow::Result<Vec<Self>> {
        let profiles_dir = base_dir.join("profiles");
        if !profiles_dir.exists() {
            return Ok(vec![]);
        }

        let mut profiles = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let meta_file = path.join("profile.json");
                    let profile_opt = std::fs::read_to_string(&meta_file)
                        .ok()
                        .and_then(|c| serde_json::from_str::<BrowserProfile>(&c).ok());
                    if let Some(profile) = profile_opt {
                        profiles.push(profile);
                    }
                }
            }
        }
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    /// Deletes a browser profile and cleans up its sandbox directory
    pub fn delete(id_or_name: &str, base_dir: &Path) -> anyhow::Result<bool> {
        let profile = Self::load(id_or_name, base_dir)?;
        let sandbox_dir = profile.get_sandbox_dir(base_dir);
        if sandbox_dir.exists() {
            std::fs::remove_dir_all(&sandbox_dir)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Calculates total disk footprint in bytes for this profile's sandbox
    pub fn calculate_disk_size(&self, base_dir: &Path) -> u64 {
        calculate_dir_size(&self.get_sandbox_dir(base_dir))
    }

    /// Packs this profile's sandbox into a compressed .tar.zst archive, stripping cache bloat
    pub fn pack(&self, base_dir: &Path, output_archive: Option<&Path>) -> anyhow::Result<crate::packer::PackReport> {
        let sandbox_dir = self.get_sandbox_dir(base_dir);
        let default_out = sandbox_dir.with_extension("tar.zst");
        let target_out = output_archive.unwrap_or(&default_out);
        crate::packer::ProfilePacker::pack(&sandbox_dir, target_out, Some(3))
    }

    /// Restores a profile from a .tar.zst archive into the SSOT sandbox directory
    pub fn unpack_archive(archive_path: &Path, base_dir: &Path, expected_hash: Option<&str>) -> anyhow::Result<Self> {
        let temp_dir = std::env::temp_dir().join(format!("specter_unpack_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
        let report = crate::packer::ProfilePacker::unpack(archive_path, &temp_dir, expected_hash)?;

        let meta_file = temp_dir.join("profile.json");
        if !meta_file.exists() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(anyhow::anyhow!("Archive does not contain a valid profile.json metadata file"));
        }

        let content = std::fs::read_to_string(&meta_file)?;
        let mut profile = serde_json::from_str::<BrowserProfile>(&content)?;
        profile.storage_hash = Some(report.sha256_hash);
        profile.storage_size_bytes = Some(archive_path.metadata().map(|m| m.len()).unwrap_or(0));

        let final_dir = profile.get_sandbox_dir(base_dir);
        if final_dir.exists() {
            std::fs::remove_dir_all(&final_dir)?;
        }
        std::fs::rename(&temp_dir, &final_dir)?;
        let _ = profile.save(base_dir);
        Ok(profile)
    }

    /// Resolves effective extension IDs, guaranteeing core 'automa' engine is present for automation
    pub fn get_effective_extensions(&self) -> Vec<String> {
        let mut list = self.extensions.clone().unwrap_or_else(|| vec!["automa".to_string()]);
        if !list.iter().any(|e| e.eq_ignore_ascii_case("automa")) {
            list.insert(0, "automa".to_string());
        }
        list
    }

    /// Generates canonical command line arguments matching the C++ Antidetect Chromium specification
    pub fn build_cli_args(&self, base_dir: &Path) -> Vec<String> {
        let sandbox_dir = self.get_sandbox_dir(base_dir);
        let mut args = vec![
            format!("--user-data-dir={}", sandbox_dir.display()),
            "--no-first-run".to_string(),
            "--no-default-browser-check".to_string(),
        ];

        // 1. Core Deterministic PRNG Seed
        if let Some(seed) = self.fingerprint_seed {
            args.push(format!("--fingerprint={}", seed));
        }

        // 2. Platform & OS Spoffing
        if let Some(ref platform) = self.os_platform {
            args.push(format!("--fingerprint-platform={}", platform));
        }
        if let Some(ref os_ver) = self.os_version {
            args.push(format!("--fingerprint-platform-version={}", os_ver));
        }

        // 3. Browser Brand & Version
        if let Some(ref brand) = self.browser_brand {
            args.push(format!("--fingerprint-brand={}", brand));
        }

        // 4. Hardware Concurrency (CPU Cores)
        if let Some(cores) = self.hardware_concurrency {
            args.push(format!("--fingerprint-hardware-concurrency={}", cores));
        }

        // 5. Timezone & Locale
        if let Some(ref tz) = self.timezone {
            args.push(format!("--timezone={}", tz));
        }
        if let Some(ref lang) = self.lang {
            args.push(format!("--lang={}", lang));
        }
        if let Some(ref accept_lang) = self.accept_lang {
            args.push(format!("--accept-lang={}", accept_lang));
        }

        // 6. User-Agent
        if let Some(ref ua) = self.user_agent {
            args.push(format!("--user-agent={}", ua));
        }

        // 7. Proxy Server & WebRTC STUN Protection
        if let Some(ref proxy) = self.proxy {
            args.push(format!("--proxy-server={}", proxy));
            // Crucial Guardrail: Prevent real IP leak via WebRTC STUN when using Proxy
            if self.webrtc_mode.as_deref() == Some("proxy_shielded") || self.webrtc_mode.is_none() {
                args.push("--disable-non-proxied-udp".to_string());
            }
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_generate_and_persistence_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("specter_prof_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let _ = std::fs::create_dir_all(&temp_dir);

        // 1. Generate deterministic profile
        let profile = BrowserProfile::generate(
            "Test-Facebook-Ad",
            Some(133742),
            Some("windows".to_string()),
            Some(8),
            Some(16),
            Some("socks5://127.0.0.1:1080".to_string()),
            Some("Asia/Ho_Chi_Minh".to_string()),
            Some("vi-VN".to_string()),
        );

        assert_eq!(profile.name, "Test-Facebook-Ad");
        assert_eq!(profile.fingerprint_seed, Some(133742));
        assert_eq!(profile.hardware_concurrency, Some(8));
        assert_eq!(profile.device_memory_gb, Some(16));
        assert_eq!(profile.proxy, Some("socks5://127.0.0.1:1080".to_string()));

        // 2. Save profile
        let meta_path = profile.save(&temp_dir).expect("Failed to save profile");
        assert!(meta_path.exists());

        // 3. Load profile by ID and by Name
        let loaded_by_id = BrowserProfile::load(&profile.id, &temp_dir).expect("Load by ID failed");
        assert_eq!(loaded_by_id.name, "Test-Facebook-Ad");

        let loaded_by_name = BrowserProfile::load("test-facebook-ad", &temp_dir).expect("Load by name failed");
        assert_eq!(loaded_by_name.id, profile.id);

        // 4. List profiles
        let all = BrowserProfile::list_all(&temp_dir).expect("List failed");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, profile.id);

        // 5. Build CLI args
        let args = profile.build_cli_args(&temp_dir);
        assert!(args.iter().any(|a| a.starts_with("--fingerprint=133742")));
        assert!(args.iter().any(|a| a.starts_with("--proxy-server=socks5://127.0.0.1:1080")));
        assert!(args.iter().any(|a| a == "--disable-non-proxied-udp"));

        // 6. Delete profile
        let deleted = BrowserProfile::delete(&profile.id, &temp_dir).expect("Delete failed");
        assert!(deleted);

        let all_after = BrowserProfile::list_all(&temp_dir).expect("List failed");
        assert_eq!(all_after.len(), 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
