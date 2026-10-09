use anyhow::Result;
use std::path::{Path, PathBuf};

use super::config::{canonical_ssot_dir, get_active_runtime_name, get_active_version};

/// Target platform identifier
pub fn get_platform_key() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "win64"
    }
    #[cfg(target_os = "macos")]
    {
        #[cfg(target_arch = "aarch64")]
        {
            "mac-arm64"
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            "mac-x64"
        }
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        "linux64"
    }
}

/// Relative path to executable within the unpacked dedicated runtime directory
pub fn get_platform_exe_rel_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from("chrome.exe")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("Chromium.app/Contents/MacOS/Chromium")
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        PathBuf::from("chrome")
    }
}

/// Resolves the browser data directory root (~/.specter/browser).
pub fn resolve_data_dir() -> PathBuf {
    canonical_ssot_dir().join("browser")
}

/// Dedicated antidetect runtime directory path: ~/.specter/browser/runtimes/<version>
pub fn get_runtime_dir() -> PathBuf {
    let runtimes_root = canonical_ssot_dir().join("browser").join("runtimes");
    let active_ver = get_active_version();
    let active_name = get_active_runtime_name();

    // Priority 1: Specifically configured active runtime directory name
    let active_dir = runtimes_root.join(&active_name);
    if active_dir.join(get_platform_exe_rel_path()).exists() {
        return active_dir;
    }

    // Priority 2: Direct match by v<active_ver> or <active_ver>
    let v_dir = runtimes_root.join(format!("v{}", active_ver));
    if v_dir.join(get_platform_exe_rel_path()).exists() {
        return v_dir;
    }
    let ver_dir = runtimes_root.join(&active_ver);
    if ver_dir.join(get_platform_exe_rel_path()).exists() {
        return ver_dir;
    }

    // Priority 3: Scan directories for one matching active_ver via manifest or stem
    if let Ok(entries) = std::fs::read_dir(&runtimes_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name.starts_with("chromium-") {
                    continue; // Skip vanilla chromium
                }
                if super::detector::detect_version_from_dir(&path).as_deref() == Some(&active_ver)
                    && path.join(get_platform_exe_rel_path()).exists()
                {
                    return path;
                }
            }
        }
    }

    // Priority 4: Official C++ Antidetect Stealth Runtime (default LTS)
    let stealth_dir = runtimes_root.join("stealth");
    if stealth_dir.join(get_platform_exe_rel_path()).exists() {
        return stealth_dir;
    }

    // Priority 5: Any installed antidetect runtime
    if let Ok(entries) = std::fs::read_dir(&runtimes_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if !name.starts_with("chromium-") && path.join(get_platform_exe_rel_path()).exists() {
                    return path;
                }
            }
        }
    }

    stealth_dir
}

/// Absolute filesystem path to the isolated Open-Source Chromium binary
pub fn get_runtime_exe_path() -> PathBuf {
    get_runtime_dir().join(get_platform_exe_rel_path())
}

/// Recursively calculates directory size in bytes
pub fn calculate_dir_size(dir: &Path) -> Option<u64> {
    if !dir.exists() {
        return None;
    }
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
            } else if path.is_dir()
                && let Some(sub_total) = calculate_dir_size(&path) {
                    total += sub_total;
                }
        }
    }
    Some(total)
}

/// Cleans and removes installed browser runtimes to free up storage
pub fn clean_runtime() -> Result<()> {
    let data_dir = resolve_data_dir();
    let runtimes_dir = data_dir.join("runtimes");
    let runtime_dir = get_runtime_dir();

    if runtime_dir.exists() {
        std::fs::remove_dir_all(&runtime_dir)?;
        println!("Successfully removed dedicated runtime directory: {:?}", runtime_dir);
    } else {
        println!("No dedicated runtime found at {:?}", runtime_dir);
    }

    // Clean legacy dirs if present
    let legacy_cft = runtimes_dir.join(format!("chrome-{}", get_platform_key()));
    if legacy_cft.exists() {
        let _ = std::fs::remove_dir_all(&legacy_cft);
    }

    Ok(())
}
