use std::path::Path;

use super::config::{canonical_ssot_dir, get_active_runtime_name, get_active_version};
use super::paths::{
    calculate_dir_size, get_platform_exe_rel_path, get_platform_key, get_runtime_dir,
    get_runtime_exe_path,
};
use super::types::{
    DetectedHostBrowser, InstalledRuntimeInfo, RuntimeStatus, PINNED_CHROMIUM_REVISION,
    PINNED_CHROMIUM_VERSION, PINNED_STEALTH_CHROMIUM_VERSION,
};

/// Returns the dedicated standalone Chromium runtime descriptor (Zero Host Scanning)
pub fn detect_host_browsers() -> Vec<DetectedHostBrowser> {
    let exe_path = get_runtime_exe_path();
    let is_stealth = exe_path.to_string_lossy().contains("stealth");
    let name = if is_stealth {
        format!("Chromium C++ Antidetect Engine (v{})", PINNED_STEALTH_CHROMIUM_VERSION)
    } else {
        format!("Chromium Open Source (v{})", PINNED_CHROMIUM_VERSION)
    };
    vec![DetectedHostBrowser {
        browser_type: "chromium".to_string(),
        name,
        executable_path: exe_path.to_string_lossy().to_string(),
    }]
}

/// Inspects current installation status and disk consumption of dedicated runtime
pub fn get_runtime_status() -> RuntimeStatus {
    let exe_path = get_runtime_exe_path();
    let runtime_dir = get_runtime_dir();
    let installed = exe_path.exists();
    let size_mb = if installed {
        calculate_dir_size(&runtime_dir).map(|bytes| bytes as f64 / 1_048_576.0)
    } else {
        None
    };

    let is_stealth = exe_path.to_string_lossy().contains("stealth");
    let pinned_version = if is_stealth {
        format!("v{} (C++ Antidetect LTS)", PINNED_STEALTH_CHROMIUM_VERSION)
    } else {
        format!("v{} (rev {})", PINNED_CHROMIUM_VERSION, PINNED_CHROMIUM_REVISION)
    };

    RuntimeStatus {
        installed,
        platform: get_platform_key().to_string(),
        executable_path: exe_path.to_string_lossy().to_string(),
        directory: runtime_dir.to_string_lossy().to_string(),
        pinned_version,
        size_mb,
    }
}

/// Inspects directory contents to discover the exact Chromium version (from *.manifest or directory name)
pub fn detect_version_from_dir(dir: &Path) -> Option<String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension()
                && ext == "manifest"
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && stem.contains('.')
            {
                return Some(stem.to_string());
            }
        }
    }

    if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
        let stripped = name.strip_prefix('v').unwrap_or(name);
        if stripped.contains('.') {
            return Some(stripped.to_string());
        }
        if stripped == "stealth" {
            return Some(PINNED_STEALTH_CHROMIUM_VERSION.to_string());
        }
    }
    None
}

/// Scans and lists all locally installed Antidetect Chromium versions in ~/.specter/browser/runtimes/
pub fn list_installed_runtimes() -> Vec<InstalledRuntimeInfo> {
    let runtimes_root = canonical_ssot_dir().join("browser").join("runtimes");
    let active_ver = get_active_version();
    let active_id = get_active_runtime_name();
    let manifest = crate::manifest::BrowserManifest::get_embedded();
    let mut list = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&runtimes_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let id = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();

                // STRICT FILTER: Discard vanilla chromium completely
                if id == "chromium-win64" || id.starts_with("chromium-") {
                    continue;
                }

                let exe = path.join(get_platform_exe_rel_path());
                if exe.exists() {
                    let ver = detect_version_from_dir(&path)
                        .unwrap_or_else(|| PINNED_STEALTH_CHROMIUM_VERSION.to_string());
                    let major_version = ver.split('.').next().unwrap_or(&ver).to_string();
                    let is_active = ver == active_ver
                        || id == active_id
                        || (active_ver == PINNED_STEALTH_CHROMIUM_VERSION && id == "stealth");
                    let size_mb = calculate_dir_size(&path).map(|b| b as f64 / 1_048_576.0);

                    let (channel, status_badge) = if let Some(rel) = manifest.find_release(&ver) {
                        let badge = match rel.status.as_str() {
                            "recommended" => "★ GOLDEN LTS".to_string(),
                            "buggy" => "⚠ BUGGY".to_string(),
                            "deprecated" => "○ ARCHIVE".to_string(),
                            _ => format!("? {}", rel.status.to_uppercase()),
                        };
                        (rel.channel.clone(), badge)
                    } else if id == "stealth" || ver == PINNED_STEALTH_CHROMIUM_VERSION {
                        ("lts".to_string(), "★ GOLDEN LTS".to_string())
                    } else {
                        ("custom".to_string(), "CUSTOM".to_string())
                    };

                    let name = format!("Chromium C++ Antidetect v{}", ver);

                    list.push(InstalledRuntimeInfo {
                        id,
                        name,
                        version: ver,
                        major_version,
                        channel,
                        status_badge,
                        path: exe.to_string_lossy().to_string(),
                        is_active,
                        size_mb,
                    });
                }
            }
        }
    }

    list.sort_by(|a, b| b.version.cmp(&a.version));
    list
}
