use std::path::PathBuf;

use super::models::strip_windows_verbatim;
use super::registry::ExtensionRegistry;
use super::remote::install_remote_extension;

/// Resolves and validates absolute filesystem paths for a profile's requested extensions.
/// Handles all profile extension edge cases:
/// - Edge Case 1.1: Mandatory 'automa' injection when running in automation/workflow mode.
/// - Edge Case 1.2: Extension priority ordering (Automa first, then other extensions).
/// - Edge Case 2.1: JIT auto-provisioning from scoop catalog if missing on disk, with graceful omission on failure.
/// - Edge Case 2.2: Windows path verification (strips verbatim \\?\ prefix, guards against comma delimiter collisions).
pub async fn resolve_profile_extension_paths(
    requested_ids: &[String],
    is_automation_mode: bool,
) -> Vec<PathBuf> {
    let mut ids: Vec<String> = requested_ids.to_vec();

    // Edge Case 1.1: Enforce automa presence for automation workflows
    if is_automation_mode && !ids.iter().any(|id| id.eq_ignore_ascii_case("automa")) {
        ids.insert(0, "automa".to_string());
    }

    // Edge Case 1.2: Ensure 'automa' is always prioritized first if present
    if let Some(pos) = ids.iter().position(|id| id.eq_ignore_ascii_case("automa"))
        && pos > 0 {
            let automa_id = ids.remove(pos);
            ids.insert(0, automa_id);
        }

    let mut resolved_paths = Vec::new();

    for id in ids {
        let id_clean = id.trim().to_lowercase();
        if id_clean.is_empty() {
            continue;
        }

        let registry = ExtensionRegistry::load();
        let mut path_opt: Option<PathBuf> = None;

        if let Some(ext) = registry.get(&id_clean)
            && ext.path.exists() {
                path_opt = Some(ext.path.clone());
            }

        // Edge Case 2.1: Missing on disk -> Attempt JIT provisioning from scoop catalog
        if path_opt.is_none() {
            tracing::info!(
                "[ProfileExtension] Extension '{}' is not present on disk. Attempting JIT provisioning from scoop-bucket...",
                id_clean
            );
            if let Ok(installed) = install_remote_extension(&id_clean, false).await {
                if installed.path.exists() {
                    path_opt = Some(installed.path);
                }
            } else if id_clean == "automa" {
                // Fallback to local cli runner extension path
                let fallback_path = PathBuf::from(crate::coordinator::resolve_cli_runner_extension_path());
                if fallback_path.exists() {
                    path_opt = Some(fallback_path);
                }
            }
        }

        match path_opt {
            Some(p) => {
                let clean = strip_windows_verbatim(p);
                let p_str = clean.to_string_lossy();
                // Edge Case 2.2: Guard against comma in path breaking Chromium's delimiter
                if p_str.contains(',') {
                    tracing::error!(
                        "[ProfileExtension] Path '{}' contains a comma ',', which invalidates Chromium --load-extension. Skipping.",
                        p_str
                    );
                    continue;
                }
                if !resolved_paths.contains(&clean) {
                    resolved_paths.push(clean);
                }
            }
            None => {
                // Edge Case 2.1 Graceful fallback: warn and omit to avoid crashing Chrome
                tracing::warn!(
                    "[ProfileExtension] Warning: Extension '{}' could not be resolved or downloaded. Omitted from launch args to prevent browser crash.",
                    id_clean
                );
            }
        }
    }

    resolved_paths
}
