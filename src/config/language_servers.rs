//! Persistent server records. Presets seed new/legacy configurations only;
//! runtime routing never resurrects a removed entry or applies preset overrides.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::syntax::LanguageId;

/// User-owned language-server catalog. An empty saved catalog stays empty.
#[derive(Debug, Clone, PartialEq)]
pub struct LspConfig {
    pub enabled: bool,
    /// Optional parameter annotations; independent of server configuration.
    pub inlay_hints: bool,
    pub servers: HashMap<String, LspServerConfig>,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            inlay_hints: false,
            servers: preset_servers(),
        }
    }
}

/// One ordinary editable server, whether entered manually or made from a preset.
/// Missing arguments/root markers mean empty lists, not implicit preset values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LspServerConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub languages: Option<Vec<LanguageId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_markers: Option<Vec<String>>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub enabled: Option<bool>,
    /// Sent as initializationOptions without interpreting server-specific keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<serde_json::Value>,
    /// Returned through workspace/configuration section lookups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<serde_json::Value>,
}

fn preset_servers() -> HashMap<String, LspServerConfig> {
    crate::lsp::all_server_defs()
        .iter()
        .map(|preset| (preset.id.to_owned(), preset.configuration()))
        .collect()
}

impl Serialize for LspConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Catalog<'a> {
            catalog_version: u32,
            enabled: bool,
            inlay_hints: bool,
            servers: &'a HashMap<String, LspServerConfig>,
        }
        Catalog {
            catalog_version: 1,
            enabled: self.enabled,
            inlay_hints: self.inlay_hints,
            servers: &self.servers,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LspConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Stored {
            #[serde(default)]
            catalog_version: Option<u32>,
            #[serde(default = "super::default_true")]
            enabled: bool,
            #[serde(default)]
            inlay_hints: bool,
            #[serde(default)]
            servers: HashMap<String, LspServerConfig>,
        }
        let stored = Stored::deserialize(deserializer)?;
        let servers = match stored.catalog_version {
            None => migrate_overrides(stored.servers),
            Some(1) => stored.servers,
            Some(version) => {
                return Err(serde::de::Error::custom(format!(
                    "unsupported language-server catalog version {version}"
                )))
            }
        };
        Ok(Self {
            enabled: stored.enabled,
            inlay_hints: stored.inlay_hints,
            servers,
        })
    }
}

/// Preserve the old override format's effective values and explicit routing
/// precedence once, at the input boundary. Subsequent saves are complete catalogs.
fn migrate_overrides(
    overrides: HashMap<String, LspServerConfig>,
) -> HashMap<String, LspServerConfig> {
    let mut servers = preset_servers();
    let mut explicit: Vec<_> = overrides
        .iter()
        .filter(|(_, server)| server.enabled != Some(false))
        .collect();
    explicit.sort_unstable_by_key(|(id, _)| id.as_str());
    let mut owners = HashMap::new();
    for (id, server) in explicit {
        for language in server.languages.as_deref().unwrap_or_default() {
            owners.entry(*language).or_insert_with(|| id.clone());
        }
    }
    for (id, mut server) in overrides {
        if let Some(defaults) = servers.remove(&id) {
            server.command = server.command.or(defaults.command);
            server.args = server.args.or(defaults.args);
            server.languages = server.languages.or(defaults.languages);
            server.root_markers = server.root_markers.or(defaults.root_markers);
        }
        servers.insert(id, server);
    }
    for (id, server) in &mut servers {
        if let Some(languages) = &mut server.languages {
            languages.retain(|language| owners.get(language).is_none_or(|owner| owner == id));
        }
    }
    servers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_overrides_materialize_defaults_and_routing_without_runtime_inheritance() {
        let config: LspConfig = serde_yaml::from_str(
            "servers:\n  rust-analyzer:\n    command: /installed/ra\n  my-python:\n    command: pylsp\n    languages: [python]\n",
        )
        .unwrap();
        assert_eq!(
            config.servers["rust-analyzer"].command.as_deref(),
            Some("/installed/ra")
        );
        assert_eq!(
            config.servers["rust-analyzer"].root_markers.as_deref(),
            Some(&["Cargo.toml".to_owned()][..])
        );
        assert_eq!(
            crate::lsp::server_id_for_language(LanguageId::Python, &config),
            Some("my-python")
        );
        assert!(config.servers["pyright"]
            .languages
            .as_ref()
            .unwrap()
            .is_empty());
        let saved = serde_yaml::to_string(&config).unwrap();
        assert!(saved.contains("catalog_version: 1"));
        assert_eq!(serde_yaml::from_str::<LspConfig>(&saved).unwrap(), config);
    }

    #[test]
    fn removed_presets_and_empty_catalogs_stay_removed_after_round_trip() {
        let mut config = LspConfig::default();
        config.servers.remove("rust-analyzer");
        let reloaded: LspConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert!(!crate::lsp::server_ids(&reloaded).contains(&"rust-analyzer"));
        assert!(crate::lsp::server_id_for_language(LanguageId::Rust, &reloaded).is_none());
        assert!(crate::lsp::resolve_server("rust-analyzer", &reloaded).is_none());
        config.servers.clear();
        let reloaded: LspConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert!(reloaded.servers.is_empty());
        assert!(serde_yaml::from_str::<LspConfig>("catalog_version: 2").is_err());
    }
}
