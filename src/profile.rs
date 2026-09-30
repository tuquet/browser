use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub id: String,
    pub name: String,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub timezone: Option<String>,
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
            custom_headers: None,
        }
    }

    pub fn get_sandbox_dir(&self, base_dir: &Path) -> PathBuf {
        base_dir.join("profiles").join(&self.id)
    }
}
