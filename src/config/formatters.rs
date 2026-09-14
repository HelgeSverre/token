//! User-owned per-language command formatters. Saved empty maps stay empty.
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::syntax::LanguageId;

/// A stdin/stdout formatter executable, invoked without a shell.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatterConfig {
    /// Optional source for installation guidance; never used to inherit settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    #[serde(default = "super::default_true")]
    pub enabled: bool,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        Self {
            preset_id: None,
            enabled: true,
            command: String::new(),
            args: Vec::new(),
        }
    }
}

/// Defaults are applied only when the entire configuration field is absent.
pub fn default_formatters() -> HashMap<LanguageId, FormatterConfig> {
    crate::tooling::default_formatters()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EditorConfig;

    #[test]
    fn absent_field_seeds_ruff_but_saved_maps_remain_authoritative() {
        let config: EditorConfig = serde_yaml::from_str("{}").unwrap();
        assert_eq!(config.formatters, default_formatters());
        let empty: EditorConfig = serde_yaml::from_str("formatters: {}").unwrap();
        let saved: EditorConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&empty).unwrap()).unwrap();
        assert!(saved.formatters.is_empty());
        let custom: EditorConfig = serde_yaml::from_str("formatters:\n  rust:\n    enabled: false\n    command: /my/rustfmt\n    args: [\"--edition=2021\"]\n").unwrap();
        let saved: EditorConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&custom).unwrap()).unwrap();
        assert_eq!(custom.formatters, saved.formatters);
        assert!(!saved.formatters.contains_key(&LanguageId::Python));
    }
}

#[cfg(test)]
mod persistence_tests {
    use crate::config::EditorConfig;
    use crate::syntax::LanguageId;

    #[test]
    fn formatting_save_removes_records_without_resurrecting_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.yaml");
        std::fs::write(
            &path,
            "custom: keep\nformatters:\n  python:\n    command: ruff\n    future_option: keep\n",
        )
        .unwrap();
        let mut config: EditorConfig =
            serde_yaml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        config.formatters.remove(&LanguageId::Python);
        config.save_to(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let saved: EditorConfig = serde_yaml::from_str(&text).unwrap();
        assert!(saved.formatters.is_empty());
        assert!(text.contains("custom: keep"));
    }

    #[test]
    fn formatting_python_defaults_to_ty_without_changing_saved_server_catalogs() {
        let config = EditorConfig::default();
        let server = crate::lsp::resolve_server("ty", &config.lsp).unwrap();
        assert_eq!(server.command, "ty");
        assert_eq!(server.args, ["server"]);
        let saved: EditorConfig = serde_yaml::from_str("lsp:\n  catalog_version: 1\n  servers:\n    pyright:\n      command: pyright-langserver\n      args: [--stdio]\n      languages: [python]\n").unwrap();
        assert!(saved.lsp.servers.contains_key("pyright"));
        assert!(!saved.lsp.servers.contains_key("ty"));
    }
}
