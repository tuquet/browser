use anyhow::{anyhow, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Pinned stable Long-Term-Support (LTS) release of official Open-Source Chromium
pub const PINNED_CHROMIUM_REVISION: &str = "1148";
pub const PINNED_CHROMIUM_VERSION: &str = "131.0.6778.33";

/// Pinned stable Long-Term-Support (LTS) release of C++ Antidetect Chromium
pub const PINNED_STEALTH_CHROMIUM_VERSION: &str = "148.0.7778.215";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
/// Dedicated browser runtime descriptor for Tuquet (Zero Host Scanning)
pub struct DetectedHostBrowser {
    /// Browser engine type (always chromium in phase 1)
    pub browser_type: String,
    /// Human-friendly display label
    pub name: String,
    /// Absolute filesystem path to browser executable
    pub executable_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    pub installed: bool,
    pub platform: String,
    pub executable_path: String,
    pub directory: String,
    pub pinned_version: String,
    pub size_mb: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRuntimeInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub major_version: String,
    pub channel: String,
    pub status_badge: String,
    pub path: String,
    pub is_active: bool,
    pub size_mb: Option<f64>,
}

pub const DEFAULT_SSOT_DIR_NAME: &str = ".specter";

/// Resolves the canonical SSOT root directory (~/.specter/ or $SPECTER_HOME)
pub fn canonical_ssot_dir() -> PathBuf {
    if let Ok(dir) = env::var("SPECTER_HOME") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = env::var("USERPROFILE")
        .or_else(|_| env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(DEFAULT_SSOT_DIR_NAME)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserConfigFile {
    pub active_version: Option<String>,
    pub active_runtime: Option<String>,
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

    let installed = list_installed_runtimes();
    if installed.is_empty() {
        return Err(anyhow!(
            "No antidetect browser runtimes installed. Run 'tuquet browser install' first."
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
            "Version '{}' ({}) is available in upstream manifest but not installed on this machine.\n  Run 'tuquet browser install {}' to install it.",
            query, rel.version, query
        ))
    } else {
        Err(anyhow!(
            "Version '{}' was not found in manifest or local runtimes.\n  Run 'tuquet browser search' to see all available releases.",
            query
        ))
    }
}

pub fn set_active_runtime(runtime_id: &str) -> Result<()> {
    set_active_version(runtime_id).map(|_| ())
}

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
                if detect_version_from_dir(&path).as_deref() == Some(&active_ver)
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

/// Constructs the list of official high-speed CDN download mirror URLs for Open-Source Chromium
pub fn get_download_urls(revision: &str) -> Vec<String> {
    let platform_asset = match get_platform_key() {
        "win64" => "chromium-win64.zip",
        "linux64" => "chromium-linux.zip",
        "mac-arm64" => "chromium-mac-arm64.zip",
        "mac-x64" => "chromium-mac.zip",
        _ => "chromium-win64.zip",
    };
    vec![
        // Direct Microsoft Azure Blob CDN (avoids 307 redirect and edge instability)
        format!(
            "https://playwright.download.prss.microsoft.com/dbazure/download/playwright/builds/chromium/{}/{}",
            revision, platform_asset
        ),
        // Azure Edge CDN
        format!(
            "https://playwright.azureedge.net/builds/chromium/{}/{}",
            revision, platform_asset
        ),
    ]
}

/// Constructs the primary CDN download URL for Open-Source Chromium
pub fn get_download_url(revision: &str) -> String {
    get_download_urls(revision).into_iter().next().unwrap()
}

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

/// Recursively calculates directory size in bytes
fn calculate_dir_size(dir: &Path) -> Option<u64> {
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

/// Helper to unpack archive files (.zip, .tar.xz, .tar.gz) into destination directory
fn unpack_archive_file(archive_path: &Path, extract_to: &Path) -> Result<()> {
    use std::fs::File;
    use std::io::BufReader;

    let path_str = archive_path.to_string_lossy();
    if path_str.ends_with(".tar.xz") || path_str.ends_with(".txz") {
        let temp_tar_path = extract_to.join("_temp_decompressed.tar");
        {
            let f = File::open(archive_path)?;
            let mut reader = BufReader::new(f);
            let out_f = File::create(&temp_tar_path)?;
            let mut writer = std::io::BufWriter::new(out_f);
            lzma_rs::xz_decompress(&mut reader, &mut writer)
                .map_err(|e| anyhow!("Failed to decompress .tar.xz archive: {}", e))?;
        }
        {
            let tar_f = File::open(&temp_tar_path)?;
            let mut tar_archive = tar::Archive::new(tar_f);
            tar_archive.unpack(extract_to)
                .map_err(|e| anyhow!("Failed to unpack tar archive: {}", e))?;
        }
        let _ = std::fs::remove_file(&temp_tar_path);
    } else if path_str.ends_with(".zip") {
        let archive_file = File::open(archive_path)?;
        let mut archive = zip::ZipArchive::new(archive_file)?;
        archive.extract(extract_to)?;
    } else {
        // Fallback: try zip first, if fails try xz
        let archive_file = File::open(archive_path)?;
        if let Ok(mut archive) = zip::ZipArchive::new(archive_file) {
            archive.extract(extract_to)?;
        } else {
            let temp_tar_path = extract_to.join("_temp_decompressed.tar");
            {
                let f = File::open(archive_path)?;
                let mut reader = BufReader::new(f);
                let out_f = File::create(&temp_tar_path)?;
                let mut writer = std::io::BufWriter::new(out_f);
                lzma_rs::xz_decompress(&mut reader, &mut writer)
                    .map_err(|e| anyhow!("Failed to decompress archive as zip or xz: {}", e))?;
            }
            {
                let tar_f = File::open(&temp_tar_path)?;
                let mut tar_archive = tar::Archive::new(tar_f);
                tar_archive.unpack(extract_to)
                    .map_err(|e| anyhow!("Failed to unpack tar archive: {}", e))?;
            }
            let _ = std::fs::remove_file(&temp_tar_path);
        }
    }
    Ok(())
}

/// Downloads and provisions the C++ Antidetect Engine from the curated manifest with SHA256 integrity
pub async fn download_stealth_runtime(force: bool, query: Option<&str>) -> Result<String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncWriteExt;

    let target_query = query.unwrap_or("lts");
    let manifest = crate::manifest::fetch_manifest(false).await?;
    let release = manifest.find_release(target_query).ok_or_else(|| {
        anyhow!(
            "Release '{}' not found in browser manifest. Run 'tuquet browser search' to see available releases.",
            target_query
        )
    })?;

    let platform_key = get_platform_key();
    let asset = release.platforms.get(platform_key).ok_or_else(|| {
        anyhow!(
            "Release v{} does not have a precompiled binary for platform '{}'",
            release.version,
            platform_key
        )
    })?;

    let runtimes_root = canonical_ssot_dir().join("browser").join("runtimes");
    let _ = std::fs::create_dir_all(&runtimes_root);

    let is_lts = target_query == "lts"
        || release.version == manifest.channels.get("lts").map(|s| s.as_str()).unwrap_or("148.0.7778.215");

    let target_dir = if (is_lts || target_query == "stealth") && runtimes_root.join("stealth").exists() {
        runtimes_root.join("stealth")
    } else {
        runtimes_root.join(format!("v{}", release.version))
    };

    let target_exe = target_dir.join(get_platform_exe_rel_path());
    if !force && target_exe.exists() {
        return Ok(target_exe.to_string_lossy().to_string());
    }

    if release.status == "buggy" {
        eprintln!("\n⚠️ WARNING: Release v{} is marked as BUGGY in the manifest!", release.version);
        eprintln!("  Notes: {}\n", release.notes);
    }

    println!("============================================================");
    println!(" Tuquet Browser - C++ Antidetect Engine Provisioning");
    println!("============================================================");
    println!(" Version:    v{} (Channel: {}, Status: {})", release.version, release.channel, release.status);
    println!(" Platform:   {}", platform_key);
    println!(" Asset:      {}", asset.asset_name);
    println!(" Source:     {}", asset.url);
    println!(" Target:     {}", target_exe.display());
    println!("------------------------------------------------------------");

    // Client builder with proxy awareness
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(600));

    let proxy_candidate = env::var("ALL_PROXY")
        .or_else(|_| env::var("all_proxy"))
        .or_else(|_| env::var("HTTPS_PROXY"))
        .or_else(|_| env::var("https_proxy"))
        .ok();

    if let Some(proxy_str) = proxy_candidate
        && let Ok(proxy) = reqwest::Proxy::all(&proxy_str) {
            builder = builder.proxy(proxy);
        }

    let client = builder.build()?;
    let extension = if asset.asset_name.ends_with(".tar.xz") {
        "tar.xz"
    } else if asset.asset_name.ends_with(".zip") {
        "zip"
    } else {
        asset.asset_name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("archive")
    };
    let temp_archive_path = std::env::temp_dir().join(format!("stealth_chromium_{}_{}.{}", platform_key, release.version, extension));

    let mut file = tokio::fs::File::create(&temp_archive_path).await?;
    let response = client.get(&asset.url).send().await?;
    if !response.status().is_success() {
        return Err(anyhow!("Failed to download {}: HTTP {}", asset.url, response.status()));
    }

    let total_bytes = response.content_length().unwrap_or(asset.size);
    let mut downloaded_bytes: u64 = 0;
    let mut stream = response.bytes_stream();
    let mut last_reported = std::time::Instant::now();
    let mut hasher = Sha256::new();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded_bytes += chunk.len() as u64;

        if last_reported.elapsed().as_millis() >= 300 || (total_bytes > 0 && downloaded_bytes >= total_bytes) {
            let pct = if total_bytes > 0 { (downloaded_bytes as f64 / total_bytes as f64) * 100.0 } else { 0.0 };
            print!(
                "\r[StealthDownloader] {:>5.1}% ({:.1} MB / {:.1} MB)...",
                pct.min(100.0),
                downloaded_bytes as f64 / 1_048_576.0,
                total_bytes as f64 / 1_048_576.0
            );
            let _ = std::io::stdout().flush();
            last_reported = std::time::Instant::now();
        }
    }

    file.flush().await?;
    drop(file);

    let calculated_hash = hex::encode(hasher.finalize());
    println!(
        "\n[StealthDownloader] Download completed ({:.1} MB). SHA256: {}",
        downloaded_bytes as f64 / 1_048_576.0,
        calculated_hash
    );

    if let Some(expected_sha) = &asset.sha256 {
        if !calculated_hash.eq_ignore_ascii_case(expected_sha) {
            let _ = tokio::fs::remove_file(&temp_archive_path).await;
            return Err(anyhow!(
                "SHA256 checksum mismatch!\nExpected: {}\nComputed: {}",
                expected_sha,
                calculated_hash
            ));
        }
        println!("[StealthDownloader] ✓ SHA256 checksum verified successfully.");
    }

    println!("[StealthDownloader] Extracting archive...");
    let archive_clone = temp_archive_path.clone();
    let temp_extract = runtimes_root.join(format!("temp_extract_{}", release.version));
    let _ = std::fs::remove_dir_all(&temp_extract);
    std::fs::create_dir_all(&temp_extract)?;

    let temp_extract_clone = temp_extract.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        unpack_archive_file(&archive_clone, &temp_extract_clone)
    })
    .await??;

    let _ = tokio::fs::remove_file(&temp_archive_path).await;

    // Discover unpacked root folder
    let mut source_dir = temp_extract.clone();
    if let Ok(entries) = std::fs::read_dir(&temp_extract) {
        let items: Vec<_> = entries.flatten().collect();
        if items.len() == 1 && items[0].path().is_dir() {
            source_dir = items[0].path();
        }
    }

    if target_dir.exists() {
        let _ = std::fs::remove_dir_all(&target_dir);
    }
    if source_dir != temp_extract {
        std::fs::rename(&source_dir, &target_dir)?;
        let _ = std::fs::remove_dir_all(&temp_extract);
    } else {
        std::fs::rename(&temp_extract, &target_dir)?;
    }

    let target_exe = target_dir.join(get_platform_exe_rel_path());
    let effective_exe = if target_exe.exists() {
        target_exe
    } else {
        let mut found = None;
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let p = entry.path().join(get_platform_exe_rel_path());
                if p.exists() {
                    found = Some(p);
                    break;
                }
            }
        }
        found.unwrap_or(target_exe)
    };

    if !effective_exe.exists() {
        return Err(anyhow!("Extraction completed but executable not found at {:?}", effective_exe));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(entries) = std::fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let file_name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if (file_name == "chrome"
                        || file_name == "chrome-sandbox"
                        || file_name == "chrome_crashpad_handler"
                        || file_name.ends_with(".so"))
                        && let Ok(meta) = std::fs::metadata(&p)
                    {
                        let mut perms = meta.permissions();
                        perms.set_mode(0o755);
                        let _ = std::fs::set_permissions(&p, perms);
                    }
                }
            }
        }
        if let Ok(meta) = std::fs::metadata(&effective_exe) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&effective_exe, perms);
        }
    }

    let _ = set_active_version(&release.version);

    println!("✓ Successfully installed C++ Antidetect Chromium v{} at: {}", release.version, effective_exe.display());
    Ok(effective_exe.to_string_lossy().to_string())
}

/// Downloads and installs official Open-Source Chromium into <data_dir>/runtimes/chromium-<platform>/
pub async fn download_chromium_runtime(force: bool, custom_revision: Option<&str>) -> Result<String> {
    use tokio::io::AsyncWriteExt;

    let exe_path = get_runtime_exe_path();
    if !force && exe_path.exists() {
        let abs_path = if !exe_path.is_absolute() {
            std::env::current_dir().map(|cwd| cwd.join(&exe_path)).unwrap_or_else(|_| exe_path.clone())
        } else {
            exe_path.clone()
        };
        return Ok(abs_path.to_string_lossy().to_string());
    }

    let revision = custom_revision.unwrap_or(PINNED_CHROMIUM_REVISION);
    let mirrors = get_download_urls(revision);
    let platform = get_platform_key();

    println!("============================================================");
    println!(" Tuquet Browser - Open-Source Chromium Provisioning");
    println!("============================================================");
    println!(" Engine:     Chromium (Pure Open Source - BSD 3-Clause)");
    println!(" Version:    v{} (Revision {})", PINNED_CHROMIUM_VERSION, revision);
    println!(" Platform:   {}", platform);
    println!(" Primary:    {}", mirrors[0]);
    println!(" Target:     {}", exe_path.display());
    println!("------------------------------------------------------------");

    // Configure proxy-aware HTTP client
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(600));

    let proxy_candidate = env::var("ALL_PROXY")
        .or_else(|_| env::var("all_proxy"))
        .or_else(|_| env::var("HTTPS_PROXY"))
        .or_else(|_| env::var("https_proxy"))
        .or_else(|_| env::var("HTTP_PROXY"))
        .or_else(|_| env::var("http_proxy"))
        .ok();

    if let Some(proxy_str) = proxy_candidate
        && let Ok(proxy) = reqwest::Proxy::all(&proxy_str) {
            builder = builder.proxy(proxy);
        }

    let client = builder.build()?;

    let temp_zip_path = std::env::temp_dir().join(format!(
        "chromium_oss_{}_{}.zip",
        platform, revision
    ));

    let mut downloaded_bytes: u64 = if temp_zip_path.exists() {
        std::fs::metadata(&temp_zip_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let mut total_bytes: u64 = 0;
    let mut mirror_idx = 0;
    let mut attempts = 0;
    let max_attempts = 10;

    while attempts < max_attempts {
        attempts += 1;
        let url = &mirrors[mirror_idx % mirrors.len()];

        let mut req = client.get(url);
        if downloaded_bytes > 0 {
            req = req.header("Range", format!("bytes={}-", downloaded_bytes));
        }

        let response = match req.send().await {
            Ok(res) => res,
            Err(e) => {
                eprintln!("\n[ChromiumDownloader] Mirror {} connection error: {}. Retrying with next mirror...", url, e);
                mirror_idx += 1;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        let status = response.status();
        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            // Resuming download
            if total_bytes == 0 {
                if let Some(cr) = response.headers().get("Content-Range").and_then(|h| h.to_str().ok())
                    && let Some(slash_idx) = cr.rfind('/')
                        && let Ok(tot) = cr[slash_idx + 1..].trim().parse::<u64>() {
                            total_bytes = tot;
                        }
                if total_bytes == 0 {
                    total_bytes = downloaded_bytes + response.content_length().unwrap_or(0);
                }
            }
        } else if status.is_success() {
            // Fresh download (server didn't accept Range or fresh start)
            downloaded_bytes = 0;
            total_bytes = response.content_length().unwrap_or(0);
        } else if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            // Already fully downloaded!
            break;
        } else {
            eprintln!("\n[ChromiumDownloader] Server returned HTTP {}. Switching mirror...", status);
            mirror_idx += 1;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            continue;
        }

        let mut file = match tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(downloaded_bytes > 0)
            .truncate(downloaded_bytes == 0)
            .open(&temp_zip_path)
            .await {
                Ok(f) => f,
                Err(e) => return Err(anyhow!("Failed to open temp archive {:?}: {}", temp_zip_path, e)),
            };

        let mut stream = response.bytes_stream();
        let mut stream_failed = false;
        let mut last_reported = std::time::Instant::now();

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(chunk) => {
                    if let Err(e) = file.write_all(&chunk).await {
                        eprintln!("\n[ChromiumDownloader] Error writing chunk to disk: {}", e);
                        stream_failed = true;
                        break;
                    }
                    downloaded_bytes += chunk.len() as u64;

                    if last_reported.elapsed().as_millis() >= 300 || (total_bytes > 0 && downloaded_bytes >= total_bytes) {
                        if total_bytes > 0 {
                            let pct = (downloaded_bytes as f64 / total_bytes as f64) * 100.0;
                            let mb_down = downloaded_bytes as f64 / 1_048_576.0;
                            let mb_tot = total_bytes as f64 / 1_048_576.0;
                            print!(
                                "\r[ChromiumDownloader] {:>5.1}% ({:.1} MB / {:.1} MB)...",
                                pct.min(100.0), mb_down, mb_tot
                            );
                        } else {
                            let mb_down = downloaded_bytes as f64 / 1_048_576.0;
                            print!("\r[ChromiumDownloader] Downloaded {:.1} MB...", mb_down);
                        }
                        let _ = std::io::stdout().flush();
                        last_reported = std::time::Instant::now();
                    }
                }
                Err(e) => {
                    eprintln!(
                        "\n[ChromiumDownloader] Stream interrupted at {:.1} MB ({}). Automatically resuming...",
                        downloaded_bytes as f64 / 1_048_576.0,
                        e
                    );
                    stream_failed = true;
                    break;
                }
            }
        }

        let _ = file.flush().await;
        drop(file);

        if !stream_failed
            && (total_bytes == 0 || downloaded_bytes >= total_bytes) {
                break; // Complete!
            }

        mirror_idx += 1;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    if total_bytes > 0 && downloaded_bytes < total_bytes {
        return Err(anyhow!(
            "Failed to complete Chromium download after {} attempts. Downloaded {:.1} MB of {:.1} MB.",
            max_attempts,
            downloaded_bytes as f64 / 1_048_576.0,
            total_bytes as f64 / 1_048_576.0
        ));
    }

    println!(
        "\n[ChromiumDownloader] Download completed ({:.1} MB). Extracting archive...",
        downloaded_bytes as f64 / 1_048_576.0
    );

    let data_dir = resolve_data_dir();
    let runtimes_dir = data_dir.join("runtimes");
    if !runtimes_dir.exists() {
        std::fs::create_dir_all(&runtimes_dir)?;
    }

    let extract_dir = runtimes_dir.clone();
    let zip_clone = temp_zip_path.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        use std::fs::File;
        let archive_file = File::open(&zip_clone)?;
        let mut archive = zip::ZipArchive::new(archive_file)?;
        archive.extract(&extract_dir)?;
        Ok(())
    })
    .await??;

    let _ = tokio::fs::remove_file(&temp_zip_path).await;

    // Open-Source Chromium archives extract to chrome-win, chrome-linux, or chrome-mac
    let raw_folder_name = match platform {
        "win64" => "chrome-win",
        "linux64" => "chrome-linux",
        _ => "chrome-mac",
    };

    let raw_extracted_path = runtimes_dir.join(raw_folder_name);
    let target_dir = get_runtime_dir();

    if target_dir.exists() {
        let _ = std::fs::remove_dir_all(&target_dir);
    }

    if raw_extracted_path.exists() {
        std::fs::rename(&raw_extracted_path, &target_dir)
            .map_err(|e| anyhow!("Failed to rename extracted folder {:?} to {:?}: {}", raw_extracted_path, target_dir, e))?;
    }

    let target_exe = get_runtime_exe_path();
    if !target_exe.exists() {
        return Err(anyhow!(
            "Extraction completed but executable not found at {:?}",
            target_exe
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&target_exe) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(&target_exe, perms);
        }
    }

    println!(
        "[ChromiumDownloader] Dedicated Open-Source Chromium provisioned at: {:?}",
        target_exe
    );
    println!("============================================================");

    let abs_path = if !target_exe.is_absolute() {
        std::env::current_dir()
            .map(|cwd| cwd.join(&target_exe))
            .unwrap_or_else(|_| target_exe)
    } else {
        target_exe
    };
    Ok(abs_path.to_string_lossy().to_string())
}

/// Resolves the executable path of the automation browser.
///
/// Priority (Zero Host Scanning Standard):
/// 1. AUTOMA_BROWSER_PATH / CHROME_EXECUTABLE_PATH environment variable override.
/// 2. Dedicated standalone C++ Antidetect Chromium in <data_dir>/runtimes/<version>/
/// 3. Auto-provision Golden LTS dedicated antidetect runtime on first execution.
pub async fn resolve_executable_path(_default_browser: &str) -> Result<String> {
    if let Ok(path) = env::var("AUTOMA_BROWSER_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }
    if let Ok(path) = env::var("CHROME_EXECUTABLE_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
    }

    let cached_exe = get_runtime_exe_path();
    if cached_exe.exists() {
        let abs_path = if !cached_exe.is_absolute() {
            std::env::current_dir()
                .map(|cwd| cwd.join(&cached_exe))
                .unwrap_or_else(|_| cached_exe.clone())
        } else {
            cached_exe.clone()
        };
        return Ok(abs_path.to_string_lossy().to_string());
    }

    // Auto-provision dedicated C++ Antidetect Chromium on first run
    println!("Dedicated C++ Antidetect Chromium runtime not found. Auto-provisioning Golden LTS...");
    download_stealth_runtime(false, Some("lts")).await
}
