use anyhow::{anyhow, Result};
use ignore::gitignore::GitignoreBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tar::{Archive, Builder};
use zstd::stream::{Decoder, Encoder};

/// Default exclusion patterns matching standard Chromium cache, IPC, and lock bloat
pub const DEFAULT_PROFILE_IGNORE: &[&str] = &[
    // Chromium caches
    "Cache/",
    "Code Cache/",
    "GPUCache/",
    "DawnCache/",
    "GrShaderCache/",
    "ShaderCache/",
    "component_crx_cache/",
    "Media Cache/",
    "Crashpad/",
    "Crash Reports/",
    "OptimizationGuidePredictionModels/",
    "segmentation_platform/",
    "Safe Browsing/",
    "Certificate Revocation Lists/",

    // Transient lock files & debug logs (preserves LevelDB/IndexedDB state)
    "Singleton*",
    "chrome_debug.log",
    "debug.log",
    "*.tmp",
    "*.tar.zst",
    "*.tar.gz",
    "*.zip",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackReport {
    pub profile_id: String,
    pub archive_path: PathBuf,
    pub file_count: usize,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub compression_ratio: f64,
    pub sha256_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnpackReport {
    pub target_dir: PathBuf,
    pub file_count: usize,
    pub restored_bytes: u64,
    pub sha256_hash: String,
}

pub struct ProfilePacker;

impl ProfilePacker {
    /// Builds standard .gitignore matcher with embedded defaults and optional .profileignore file
    pub fn build_ignore_matcher(profile_dir: &Path) -> Result<ignore::gitignore::Gitignore> {
        let mut builder = GitignoreBuilder::new(profile_dir);

        for pat in DEFAULT_PROFILE_IGNORE {
            builder.add_line(None, pat)
                .map_err(|e| anyhow!("Failed to add default ignore pattern '{}': {}", pat, e))?;
        }

        // 1. Profile-specific .profileignore
        let local_ignore = profile_dir.join(".profileignore");
        if local_ignore.exists() {
            let _ = builder.add(&local_ignore);
        }

        // 2. Global shared .profileignore in ~/.specter/browser/profiles/.profileignore
        if let Some(parent) = profile_dir.parent() {
            let global_ignore = parent.join(".profileignore");
            if global_ignore.exists() {
                let _ = builder.add(&global_ignore);
            }
        }

        builder.build().map_err(|e| anyhow!("Failed to compile .profileignore matcher: {}", e))
    }

    /// Packs a browser profile into a compressed .tar.zst payload, stripping cache bloat
    pub fn pack(profile_dir: &Path, output_archive: &Path, compression_level: Option<i32>) -> Result<PackReport> {
        if !profile_dir.exists() || !profile_dir.is_dir() {
            return Err(anyhow!("Profile directory '{}' does not exist", profile_dir.display()));
        }

        if let Some(parent) = output_archive.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let matcher = Self::build_ignore_matcher(profile_dir)?;
        let level = compression_level.unwrap_or(3);

        let out_file = File::create(output_archive)?;
        let zstd_encoder = Encoder::new(out_file, level)?;
        let mut tar_builder = Builder::new(zstd_encoder);

        let mut file_count = 0usize;
        let mut uncompressed_bytes = 0u64;

        // Recursive directory traversal with .profileignore filtering
        fn walk_and_archive(
            dir: &Path,
            root: &Path,
            matcher: &ignore::gitignore::Gitignore,
            tar: &mut Builder<Encoder<'static, File>>,
            count: &mut usize,
            total_bytes: &mut u64,
        ) -> Result<()> {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let is_dir = path.is_dir();

                // Check gitignore match against relative path or any parent
                if matcher.matched_path_or_any_parents(&path, is_dir).is_ignore() {
                    continue;
                }

                let rel_path = path.strip_prefix(root)
                    .map_err(|e| anyhow!("Failed to compute relative path: {}", e))?;

                if is_dir {
                    walk_and_archive(&path, root, matcher, tar, count, total_bytes)?;
                } else if path.is_file() {
                    let meta = entry.metadata()?;
                    *total_bytes += meta.len();
                    *count += 1;
                    tar.append_path_with_name(&path, rel_path)?;
                }
            }
            Ok(())
        }

        walk_and_archive(profile_dir, profile_dir, &matcher, &mut tar_builder, &mut file_count, &mut uncompressed_bytes)?;

        let zstd_encoder = tar_builder.into_inner()?;
        let mut out_file = zstd_encoder.finish()?;
        out_file.flush()?;

        // Compute SHA256 and size of the generated archive
        let mut archive_file = File::open(output_archive)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut compressed_bytes = 0u64;

        loop {
            let n = archive_file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            compressed_bytes += n as u64;
            hasher.update(&buffer[..n]);
        }

        let sha256_hash = hex::encode(hasher.finalize());
        let compression_ratio = if uncompressed_bytes > 0 {
            (1.0 - (compressed_bytes as f64 / uncompressed_bytes as f64)) * 100.0
        } else {
            0.0
        };

        let profile_id = profile_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(PackReport {
            profile_id,
            archive_path: output_archive.to_path_buf(),
            file_count,
            uncompressed_bytes,
            compressed_bytes,
            compression_ratio,
            sha256_hash,
        })
    }

    /// Unpacks a .tar.zst profile archive into the target profile sandbox directory
    pub fn unpack(archive_path: &Path, target_profile_dir: &Path, expected_hash: Option<&str>) -> Result<UnpackReport> {
        if !archive_path.exists() {
            return Err(anyhow!("Archive file '{}' does not exist", archive_path.display()));
        }

        // Verify SHA256 integrity if hash is supplied
        let mut archive_file = File::open(archive_path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let n = archive_file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let calculated_hash = hex::encode(hasher.finalize());

        if let Some(expected) = expected_hash.filter(|exp| !calculated_hash.eq_ignore_ascii_case(exp.trim())) {
            return Err(anyhow!(
                "SHA-256 integrity check failed! Expected: {}, Found: {}",
                expected, calculated_hash
            ));
        }

        std::fs::create_dir_all(target_profile_dir)?;

        let archive_file = File::open(archive_path)?;
        let zstd_decoder = Decoder::new(archive_file)?;
        let mut tar_archive = Archive::new(zstd_decoder);

        let mut file_count = 0usize;
        let mut restored_bytes = 0u64;

        for entry in tar_archive.entries()? {
            let mut entry = entry?;
            let size = entry.header().size()?;
            restored_bytes += size;
            file_count += 1;
            entry.unpack_in(target_profile_dir)?;
        }

        Ok(UnpackReport {
            target_dir: target_profile_dir.to_path_buf(),
            file_count,
            restored_bytes,
            sha256_hash: calculated_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_pack_unpack_cache_filtering() {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let temp_root = std::env::temp_dir().join(format!("specter_pack_test_{}", now));
        let src_dir = temp_root.join("source_profile");
        let dst_dir = temp_root.join("restored_profile");
        let archive_path = temp_root.join("snapshot.tar.zst");

        std::fs::create_dir_all(src_dir.join("Default").join("Local Storage")).unwrap();
        std::fs::create_dir_all(src_dir.join("Default").join("Cache")).unwrap();
        std::fs::create_dir_all(src_dir.join("Default").join("GPUCache")).unwrap();

        // Essential state files
        std::fs::write(src_dir.join("profile.json"), r#"{"name":"test"}"#).unwrap();
        std::fs::write(src_dir.join("Default").join("Cookies"), "test-cookies-payload").unwrap();
        std::fs::write(src_dir.join("Default").join("Local Storage").join("leveldb.log"), "db-data").unwrap();

        // Bloat files (MUST BE IGNORED)
        std::fs::write(src_dir.join("Default").join("Cache").join("data_0"), "huge-cache-file").unwrap();
        std::fs::write(src_dir.join("Default").join("GPUCache").join("index"), "gpu-cache-data").unwrap();
        std::fs::write(src_dir.join("SingletonLock"), "process-lock").unwrap();
        std::fs::write(src_dir.join("temp.tmp"), "temp-data").unwrap();

        // 1. Pack
        let pack_res = ProfilePacker::pack(&src_dir, &archive_path, Some(3)).expect("Pack failed");
        assert!(pack_res.file_count >= 3);
        assert!(pack_res.compressed_bytes > 0);
        assert!(!pack_res.sha256_hash.is_empty());

        // 2. Unpack
        let unpack_res = ProfilePacker::unpack(&archive_path, &dst_dir, Some(&pack_res.sha256_hash)).expect("Unpack failed");
        assert_eq!(unpack_res.file_count, pack_res.file_count);
        assert_eq!(unpack_res.sha256_hash, pack_res.sha256_hash);

        // 3. Verify Essential Files Exist
        assert!(dst_dir.join("profile.json").exists());
        assert!(dst_dir.join("Default").join("Cookies").exists());
        assert!(dst_dir.join("Default").join("Local Storage").join("leveldb.log").exists());

        // 4. Verify Bloat Files Were Stripped
        assert!(!dst_dir.join("Default").join("Cache").exists());
        assert!(!dst_dir.join("Default").join("GPUCache").exists());
        assert!(!dst_dir.join("SingletonLock").exists());
        assert!(!dst_dir.join("temp.tmp").exists());

        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
