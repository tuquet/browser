use anyhow::{anyhow, Result};
use futures::StreamExt;
use std::env;
use std::io::Write;

use super::config::{canonical_ssot_dir, set_active_version};
use super::extractor::unpack_archive_file;
use super::paths::{
    get_platform_exe_rel_path, get_platform_key, get_runtime_dir, get_runtime_exe_path,
    resolve_data_dir,
};
use super::types::{
    PINNED_CHROMIUM_REVISION, PINNED_CHROMIUM_VERSION,
};

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

/// Downloads and provisions the C++ Antidetect Engine from the curated manifest with SHA256 integrity
pub async fn download_stealth_runtime(force: bool, query: Option<&str>) -> Result<String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncWriteExt;

    let target_query = query.unwrap_or("lts");
    let manifest = crate::manifest::fetch_manifest(false).await?;
    let release = manifest.find_release(target_query).ok_or_else(|| {
        anyhow!(
            "Release '{}' not found in browser manifest. Run 'specter browser search' to see available releases.",
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
    println!(" Specter Browser - C++ Antidetect Engine Provisioning");
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
    println!(" Specter Browser - Open-Source Chromium Provisioning");
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
