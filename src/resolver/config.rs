use anyhow::{anyhow, Result};
use std::env;
use std::path::PathBuf;

use super::types::{
    BrowserConfigFile, InstalledRuntimeInfo, DEFAULT_SSOT_DIR_NAME,
    PINNED_STEALTH_CHROMIUM_VERSION,
};

/// Resolves the canonical SSOT root directory (~/.specter/ or $SPECTER_HOME)
pub fn canonical_ssot_dir() -> PathBuf {
    if let Ok(dir) = env::var(crate::constants::ENV_SPECTER_HOME) {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(DEFAULT_SSOT_DIR_NAME)
}

pub fn get_browser_config_path() -> PathBuf {
    canonical_ssot_dir().join("browser").join("browser.json")
}

pub fn get_active_version() -> String {
    let cfg_path = get_browser_config_path();
    if cfg_path.exists()
        && let Ok(content) = std::fs::read_to_string(&cfg_path)
        && let Ok(cfg) = serde_json::from_str::<BrowserConfigFile>(&content)
    {
        if let Some(ref ver) = cfg.active_version {
            return ver.clone();
        }
        if let Some(ref rt) = cfg.active_runtime
            && rt.contains('.')
        {
            return rt.trim_start_matches('v').to_string();
        }
    }
    PINNED_STEALTH_CHROMIUM_VERSION.to_string()
}

pub fn get_active_runtime_name() -> String {
    let cfg_path = get_browser_config_path();
    if cfg_path.exists()
        && let Ok(content) = std::fs::read_to_string(&cfg_path)
        && let Ok(cfg) = serde_json::from_str::<BrowserConfigFile>(&content)
    {
        if let Some(ref rt) = cfg.active_runtime {
            return rt.clone();
        }
        if let Some(ref ver) = cfg.active_version {
            return format!("v{}", ver);
        }
    }
    "stealth".to_string()
}

pub fn set_active_version(query: &str) -> Result<InstalledRuntimeInfo> {
    let q = query.trim();
    let stripped = q.strip_prefix('v').unwrap_or(q);

    let installed = super::detector::list_installed_runtimes();
    if installed.is_empty() {
        return Err(anyhow!(
            "No antidetect browser runtimes installed. Run 'specter browser install' first."
        ));
    }

    let matched = installed.iter().find(|r| {
        r.version == stripped
            || r.major_version == stripped
            || r.id == q
            || r.id == stripped
            || ((stripped.eq_ignore_ascii_case("lts") || stripped.eq_ignore_ascii_case("stealth")) && r.version == PINNED_STEALTH_CHROMIUM_VERSION)
            || r.channel.eq_ignore_ascii_case(stripped)
    }).cloned();

    if let Some(target) = matched {
        let cfg_path = get_browser_config_path();
        if let Some(parent) = cfg_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut cfg = if cfg_path.exists() {
            std::fs::read_to_string(&cfg_path)
                .ok()
                .and_then(|c| serde_json::from_str::<BrowserConfigFile>(&c).ok())
                .unwrap_or_default()
        } else {
            BrowserConfigFile::default()
        };
        cfg.active_version = Some(target.version.clone());
        cfg.active_runtime = Some(target.id.clone());
        let json = serde_json::to_string_pretty(&cfg)?;
        std::fs::write(cfg_path, json)?;
        return Ok(target);
    }

    let manifest = crate::manifest::BrowserManifest::get_embedded();
    if let Some(rel) = manifest.find_release(query) {
        Err(anyhow!(
            "Version '{}' ({}) is available in upstream manifest but not installed on this machine.\n  Run 'specter browser install {}' to install it.",
            query, rel.version, query
        ))
    } else {
        Err(anyhow!(
            "Version '{}' was not found in manifest or local runtimes.\n  Run 'specter browser search' to see all available releases.",
            query
        ))
    }
}

pub fn set_active_runtime(runtime_id: &str) -> Result<()> {
    set_active_version(runtime_id).map(|_| ())
}
