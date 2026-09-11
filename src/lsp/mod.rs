//! Language Server Protocol client.
//!
//! Phase 1: pure protocol library code (`transport`, `uri`, `position`)
//! plus the per-server worker (`client`) that owns the process, the
//! handshake, and request-id correlation. Runtime ownership (spawning,
//! routing `Msg`) lives in `runtime/app.rs::LspManager`, kept out of this
//! module so `lsp/` stays free of winit/App dependencies — see
//! `docs/feature/lsp-integration.md`.

pub mod client;
pub mod document_features;
pub mod markdown;
pub mod position;
pub mod sync;
pub mod transport;
pub mod uri;
pub mod workspace_symbols;

pub use position::{lsp_to_position, position_to_lsp};
pub(crate) use uri::resolved_path_to_uri;
pub use uri::{path_to_uri, uri_to_path};

use crate::syntax::LanguageId;

/// Identifies one running (or about-to-run) language server, e.g.
/// `"rust-analyzer"`. Distinct from `LspServerDef::id` (a `&'static str`)
/// because it crosses `Msg`/`HashMap` boundaries that want an owned,
/// `Clone + Debug + Send` type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LspServerId(pub String);

impl From<&str> for LspServerId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

impl From<String> for LspServerId {
    fn from(id: String) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for LspServerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Render-only server lifecycle state, mirrored into the model by
/// `LspMsg::ServerStateChanged` (see `update/lsp.rs`). The runtime's
/// `LspManager` is authoritative; this is what the status bar and
/// automation read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    Starting,
    Indexing,
    Ready,
    Restarting { attempt: u8 },
    Failed,
    Missing,
    ShuttingDown,
}

/// A seed preset for new configurations, never a runtime fallback.
///
/// Deliberately *not* a field on `syntax::registry::LanguageDefinition` —
/// that struct is built by a positional macro with many call sites, and
/// only a subset of languages need an LSP def. A side table keyed by `LanguageId`
/// (`lsp_server_def`, below) gets the same "one place to register a
/// server" ergonomics without touching every call site or growing the
/// macro's arity.
#[derive(Debug, Clone, Copy)]
pub struct LspServerDef {
    /// Suggested ID for a new independent configuration record.
    pub id: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub languages: &'static [LanguageId],
    /// Filenames that mark a directory as this server's project root
    /// (see `client::resolve_root`).
    pub project_markers: &'static [&'static str],
}

impl LspServerDef {
    /// Copy a preset into an independent, fully editable configuration record.
    pub fn configuration(&self) -> crate::config::LspServerConfig {
        crate::config::LspServerConfig {
            command: Some(self.command.into()),
            args: Some(self.args.iter().map(|arg| (*arg).into()).collect()),
            languages: Some(self.languages.to_vec()),
            root_markers: Some(
                self.project_markers
                    .iter()
                    .map(|marker| (*marker).into())
                    .collect(),
            ),
            enabled: Some(true),
            ..Default::default()
        }
    }
}

pub static RUST_ANALYZER: LspServerDef = LspServerDef {
    id: "rust-analyzer",
    command: "rust-analyzer",
    args: &[],
    languages: &[LanguageId::Rust],
    project_markers: &["Cargo.toml"],
};

pub static TYPESCRIPT_LANGUAGE_SERVER: LspServerDef = LspServerDef {
    id: "typescript-language-server",
    command: "typescript-language-server",
    args: &["--stdio"],
    languages: &[
        LanguageId::TypeScript,
        LanguageId::Tsx,
        LanguageId::JavaScript,
        LanguageId::Jsx,
    ],
    project_markers: &["package.json"],
};

pub static PYRIGHT: LspServerDef = LspServerDef {
    id: "pyright",
    command: "pyright-langserver",
    args: &["--stdio"],
    languages: &[LanguageId::Python],
    project_markers: &["pyproject.toml"],
};

pub static GOPLS: LspServerDef = LspServerDef {
    id: "gopls",
    command: "gopls",
    args: &[],
    languages: &[LanguageId::Go],
    project_markers: &["go.work", "go.mod"],
};

pub static PHPANTOM: LspServerDef = LspServerDef {
    id: "phpantom",
    command: "phpantom_lsp",
    args: &[],
    languages: &[LanguageId::Php],
    project_markers: &["composer.json"],
};

pub static SEMA: LspServerDef = LspServerDef {
    id: "sema",
    command: "sema",
    args: &["lsp"],
    languages: &[LanguageId::Sema],
    project_markers: &["sema.toml"],
};

static ALL_SERVER_DEFS: &[&LspServerDef] = &[
    &RUST_ANALYZER,
    &TYPESCRIPT_LANGUAGE_SERVER,
    &PYRIGHT,
    &GOPLS,
    &PHPANTOM,
    &SEMA,
];

/// Built-in defaults by server ID. Use `resolve_server` for runtime configuration.
pub fn server_def_by_id(id: &str) -> Option<&'static LspServerDef> {
    ALL_SERVER_DEFS.iter().copied().find(|def| def.id == id)
}

/// Built-in defaults in a stable order. UI lists use config-aware `server_ids`.
pub fn all_server_defs() -> &'static [&'static LspServerDef] {
    ALL_SERVER_DEFS
}

/// Stable ordering for the configured catalog, without privileged preset entries.
pub fn server_ids(config: &crate::config::LspConfig) -> Vec<&str> {
    let mut ids: Vec<_> = config.servers.keys().map(String::as_str).collect();
    ids.sort_unstable();
    ids
}

/// Effective language associations, including custom servers.
pub fn configured_languages<'a>(
    id: &str,
    config: &'a crate::config::LspConfig,
) -> &'a [LanguageId] {
    config
        .servers
        .get(id)
        .and_then(|server| server.languages.as_deref())
        .unwrap_or_default()
}

/// One authoritative routing decision for startup, completion, status and menus.
/// All enabled records participate equally. Ties in hand-written config are
/// deterministic (ID order); Settings rejects competing enabled assignments.
pub fn server_id_for_language(
    language: LanguageId,
    config: &crate::config::LspConfig,
) -> Option<&str> {
    config
        .servers
        .iter()
        .filter(|(_, server)| {
            server
                .languages
                .as_ref()
                .is_some_and(|languages| languages.contains(&language))
        })
        .min_by_key(|(id, server)| (server.enabled == Some(false), id.as_str()))
        .map(|(id, _)| id.as_str())
}

/// Another enabled explicit assignment that would compete with this server.
pub fn association_conflict<'a>(
    id: &str,
    languages: &[LanguageId],
    config: &'a crate::config::LspConfig,
) -> Option<&'a str> {
    config
        .servers
        .iter()
        .filter(|(other, server)| {
            other.as_str() != id
                && server.enabled != Some(false)
                && server.languages.as_ref().is_some_and(|assigned| {
                    assigned.iter().any(|language| languages.contains(language))
                })
        })
        .map(|(id, _)| id.as_str())
        .min()
}

/// Suggested language assignments for a preset. Runtime uses the saved catalog.
pub fn languages_for_server(id: &str) -> &'static [LanguageId] {
    server_def_by_id(id).map_or(&[], |preset| preset.languages)
}

/// Looks up the default server definition for a language. `None` means
/// "no LSP support registered for this language" — not "disabled"; see
/// `config::LspConfig` for user-facing enable/disable and overrides.
///
/// TypeScript/TSX/JavaScript/JSX all map to the same
/// `typescript-language-server` def: one server instance per root
/// serves all four (matching how `typescript-language-server` itself
/// handles JSX via `languageId`).
pub fn lsp_server_def(language: LanguageId) -> Option<&'static LspServerDef> {
    ALL_SERVER_DEFS
        .iter()
        .copied()
        .find(|preset| preset.languages.contains(&language))
}

/// Executable configuration resolved from a saved server record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedServer {
    pub id: LspServerId,
    pub command: String,
    pub args: Vec<String>,
    pub root_markers: Vec<String>,
    /// `lsp.servers.<id>.initialization_options` — sent verbatim in the
    /// `initialize` request. `Null` when unconfigured.
    pub initialization_options: serde_json::Value,
    /// `lsp.servers.<id>.settings` — answers `workspace/configuration`
    /// section lookups. `Null` when unconfigured (every section then
    /// replies `null`, the pre-settings behavior).
    pub settings: serde_json::Value,
}

/// Resolve a saved record, respecting the global and local switches.
pub fn resolve_server(id: &str, config: &crate::config::LspConfig) -> Option<ResolvedServer> {
    if !config.enabled {
        return None;
    }
    let server = config.servers.get(id)?;
    if server.enabled == Some(false) {
        return None;
    }
    let command = server.command.clone()?;
    let args = server.args.clone().unwrap_or_default();
    let root_markers = server.root_markers.clone().unwrap_or_default();
    let initialization_options = server
        .initialization_options
        .clone()
        .unwrap_or(serde_json::Value::Null);
    let settings = server.settings.clone().unwrap_or(serde_json::Value::Null);
    Some(ResolvedServer {
        id: LspServerId::from(id),
        command,
        args,
        root_markers,
        initialization_options,
        settings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrated_custom_server_configuration_routes_languages_without_preset_fallbacks() {
        let mut config: crate::config::LspConfig = serde_yaml::from_str(
            r#"
servers:
  custom-rust:
    command: /installed/server
    args: [--stdio]
    languages: [rust, cpp]
    root_markers: [project.json, .git]
"#,
        )
        .unwrap();
        assert_eq!(
            server_id_for_language(LanguageId::Rust, &config),
            Some("custom-rust")
        );
        assert_eq!(
            server_id_for_language(LanguageId::Cpp, &config),
            Some("custom-rust")
        );
        assert_eq!(
            server_id_for_language(LanguageId::Go, &config),
            Some("gopls")
        );
        let resolved = resolve_server("custom-rust", &config).unwrap();
        assert_eq!(resolved.command, "/installed/server");
        assert_eq!(resolved.args, ["--stdio"]);
        assert_eq!(resolved.root_markers, ["project.json", ".git"]);
        assert!(server_ids(&config).contains(&"custom-rust"));
        let round_trip: crate::config::LspConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert_eq!(resolve_server("custom-rust", &round_trip), Some(resolved));
        config.servers.get_mut("custom-rust").unwrap().enabled = Some(false);
        assert_eq!(
            server_id_for_language(LanguageId::Rust, &config),
            Some("custom-rust")
        );
        assert!(resolve_server("custom-rust", &config).is_none());
        config.servers.get_mut("custom-rust").unwrap().enabled = Some(true);
        assert_eq!(
            association_conflict("another", &[LanguageId::Rust], &config),
            Some("custom-rust")
        );
        config.enabled = false;
        assert!(resolve_server("custom-rust", &config).is_none());
    }

    #[test]
    fn maps_every_ts_family_language_to_the_same_server_def() {
        let ts = lsp_server_def(LanguageId::TypeScript).unwrap();
        let tsx = lsp_server_def(LanguageId::Tsx).unwrap();
        let js = lsp_server_def(LanguageId::JavaScript).unwrap();
        let jsx = lsp_server_def(LanguageId::Jsx).unwrap();
        for def in [tsx, js, jsx] {
            assert_eq!(def.id, ts.id);
        }
    }

    #[test]
    fn languages_without_a_registered_server_return_none() {
        assert!(lsp_server_def(LanguageId::PlainText).is_none());
        assert!(lsp_server_def(LanguageId::Markdown).is_none());
    }

    #[test]
    fn every_def_has_a_non_empty_project_marker_list() {
        for def in all_server_defs() {
            assert!(!def.project_markers.is_empty(), "{} has no markers", def.id);
        }
    }

    #[test]
    fn master_switch_disables_every_server() {
        let config = crate::config::LspConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(resolve_server(RUST_ANALYZER.id, &config).is_none());
    }

    #[test]
    fn per_server_override_replaces_command_and_args() {
        let mut config = crate::config::LspConfig {
            enabled: true,
            ..Default::default()
        };
        config.servers.insert(
            "phpantom".to_owned(),
            crate::config::LspServerConfig {
                command: Some("laravel-lsp".to_owned()),
                args: None,
                enabled: None,
                initialization_options: None,
                settings: None,
                ..Default::default()
            },
        );
        let resolved = resolve_server(PHPANTOM.id, &config).unwrap();
        assert_eq!(resolved.command, "laravel-lsp");
        assert!(resolved.args.is_empty()); // def.args is empty and no override
        assert_eq!(resolved.initialization_options, serde_json::Value::Null);
        assert_eq!(resolved.settings, serde_json::Value::Null);
    }

    #[test]
    fn per_server_enabled_false_disables_just_that_server() {
        let mut config = crate::config::LspConfig {
            enabled: true,
            ..Default::default()
        };
        config.servers.insert(
            "pyright".to_owned(),
            crate::config::LspServerConfig {
                command: None,
                args: None,
                enabled: Some(false),
                initialization_options: None,
                settings: None,
                ..Default::default()
            },
        );
        assert!(resolve_server(PYRIGHT.id, &config).is_none());
        assert!(resolve_server(RUST_ANALYZER.id, &config).is_some());
    }

    #[test]
    fn initialization_options_and_settings_thread_through_resolve() {
        let mut config = crate::config::LspConfig {
            enabled: true,
            ..Default::default()
        };
        config.servers.insert(
            "pyright".to_owned(),
            crate::config::LspServerConfig {
                initialization_options: Some(serde_json::json!({ "python": { "pythonPath": "/usr/bin/python3" } })),
                settings: Some(serde_json::json!({ "python": { "analysis": { "typeCheckingMode": "strict" } } })),
                ..PYRIGHT.configuration()
            },
        );
        let resolved = resolve_server(PYRIGHT.id, &config).unwrap();
        assert_eq!(
            resolved.initialization_options["python"]["pythonPath"],
            serde_json::json!("/usr/bin/python3")
        );
        assert_eq!(
            resolved.settings["python"]["analysis"]["typeCheckingMode"],
            serde_json::json!("strict")
        );
    }

    #[test]
    fn all_server_defs_matches_the_registry() {
        assert_eq!(all_server_defs().len(), ALL_SERVER_DEFS.len());
        assert!(all_server_defs()
            .iter()
            .any(|def| def.id == "rust-analyzer"));
    }

    #[test]
    fn languages_for_server_covers_every_registered_def() {
        for def in all_server_defs() {
            assert!(
                !languages_for_server(def.id).is_empty(),
                "{} has no languages mapped",
                def.id
            );
            for &language in languages_for_server(def.id) {
                assert_eq!(lsp_server_def(language).unwrap().id, def.id);
            }
        }
        assert!(languages_for_server("not-a-real-server").is_empty());
    }
}
