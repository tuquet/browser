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
        }
    }

    pub fn get_sandbox_dir(&self, base_dir: &Path) -> PathBuf {
        base_dir.join("profiles").join(&self.id)
    }

    /// Resolves effective extension IDs, guaranteeing core 'automa' engine is present for automation
    pub fn get_effective_extensions(&self) -> Vec<String> {
        let mut list = self.extensions.clone().unwrap_or_else(|| vec!["automa".to_string()]);
        if !list.iter().any(|e| e.eq_ignore_ascii_case("automa")) {
            list.insert(0, "automa".to_string());
        }
        list
    }
}
