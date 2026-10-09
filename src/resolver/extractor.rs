use anyhow::{anyhow, Result};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Helper to unpack archive files (.zip, .tar.xz, .tar.gz) into destination directory
pub fn unpack_archive_file(archive_path: &Path, extract_to: &Path) -> Result<()> {
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
