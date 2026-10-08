use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex as TokioMutex, RwLock};

pub fn connected_browsers() -> &'static Arc<RwLock<HashSet<String>>> {
    static BROWSERS: OnceLock<Arc<RwLock<HashSet<String>>>> = OnceLock::new();
    BROWSERS.get_or_init(|| Arc::new(RwLock::new(HashSet::new())))
}

fn launcher_mutexes() -> &'static RwLock<HashMap<String, Arc<TokioMutex<()>>>> {
    static MUTEXES: OnceLock<RwLock<HashMap<String, Arc<TokioMutex<()>>>>> = OnceLock::new();
    MUTEXES.get_or_init(|| RwLock::new(HashMap::new()))
}

pub async fn get_browser_launcher_lock(browser_id: &str) -> Arc<TokioMutex<()>> {
    let mut map = launcher_mutexes().write().await;
    map.entry(browser_id.to_string())
        .or_insert_with(|| Arc::new(TokioMutex::new(())))
        .clone()
}

pub fn resolve_cli_runner_extension_path() -> String {
    let mut possible_paths = vec![
        std::path::PathBuf::from("apps/runner/dist"),
        std::path::PathBuf::from("./apps/runner/dist"),
        std::path::PathBuf::from("../runner/dist"),
        std::path::PathBuf::from("../../apps/runner/dist"),
        std::path::PathBuf::from("../../../apps/runner/dist"),
    ];

    let ssot = crate::resolver::canonical_ssot_dir();
    possible_paths.push(ssot.join("browser").join("extensions").join("automa"));
    possible_paths.push(ssot.join("automa").join("runner"));

    if let Ok(exe_path) = std::env::current_exe()
        && let Some(exe_dir) = exe_path.parent() {
            possible_paths.push(exe_dir.join("apps/runner/dist"));
            possible_paths.push(exe_dir.join("../runner/dist"));
            possible_paths.push(exe_dir.join("../../apps/runner/dist"));
            possible_paths.push(exe_dir.join("../../../apps/runner/dist"));
        }

    if let Ok(env_ext) = std::env::var("SPECTER_EXTENSION_PATH").or_else(|_| std::env::var("AUTOMA_EXTENSION_PATH")) {
        possible_paths.insert(0, std::path::PathBuf::from(env_ext));
    }

    let mut ext_path = possible_paths.into_iter()
        .find(|p| p.exists())
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "apps/runner/dist".to_string());

    if ext_path.starts_with(r"\\?\") {
        ext_path = ext_path[4..].to_string();
    }

    ext_path
}
