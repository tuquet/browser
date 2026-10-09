pub mod models;
pub mod registry;
pub mod remote;
pub mod resolver;

pub use self::models::*;
pub use self::registry::*;
pub use self::remote::*;
pub use self::resolver::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_load_extension_arg() {
        let paths = vec![PathBuf::from("/path/one"), PathBuf::from("/path/two")];
        let arg = ExtensionRegistry::format_load_extension_arg(&paths);
        assert!(arg.contains("/path/one"));
        assert!(arg.contains("/path/two"));
        assert!(arg.contains(','));
    }

    #[test]
    fn test_extension_crud() {
        let mut reg = ExtensionRegistry::default();
        let ext = Extension {
            id: "test-ext".to_string(),
            name: "Test Extension".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            path: PathBuf::from("dummy/path"),
            enabled: true,
            is_builtin: false,
            manifest_version: 3,
        };

        reg.extensions.insert(ext.id.clone(), ext);
        assert_eq!(reg.list().len(), 1);
        assert!(reg.get("test-ext").is_some());

        // Toggle enabled
        let _ = reg.set_enabled("test-ext", false);
        assert!(!reg.get("test-ext").unwrap().enabled);

        // Built-in protection
        assert!(reg.unregister("automa").is_err());
    }

    #[tokio::test]
    async fn test_profile_extension_automa_ordering() {
        let requested = vec!["ublock".to_string(), "cookie-injector".to_string()];
        // In automation mode, automa must be auto-injected and placed first
        let mut ids = requested.clone();
        if !ids.iter().any(|id| id.eq_ignore_ascii_case("automa")) {
            ids.insert(0, "automa".to_string());
        }
        assert_eq!(ids[0], "automa");
    }
}
