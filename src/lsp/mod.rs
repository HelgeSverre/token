//! Language Server Protocol client.
//!
//! Phase 1: pure protocol library code (`transport`, `uri`, `position`)
//! plus the per-server worker (`client`) that owns the process, the
//! handshake, and request-id correlation. Runtime ownership (spawning,
//! routing `Msg`) lives in `runtime/app.rs::LspManager`, kept out of this
//! module so `lsp/` stays free of winit/App dependencies — see
//! `docs/feature/lsp-integration.md`.

pub mod client;
pub mod position;
pub mod transport;
pub mod uri;

pub use position::{lsp_to_position, position_to_lsp};
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

/// One entry in the compile-time server registry: how to spawn a server
/// for a language and how to find its project root.
///
/// Deliberately *not* a field on `syntax::registry::LanguageDefinition` —
/// that struct is built by a positional macro with ~85 call sites, and
/// only 5 languages need an LSP def. A side table keyed by `LanguageId`
/// (`lsp_server_def`, below) gets the same "one place to register a
/// server" ergonomics without touching every call site or growing the
/// macro's arity.
#[derive(Debug, Clone, Copy)]
pub struct LspServerDef {
    /// Stable id used in config overrides, `HashMap` keys, and logs.
    pub id: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    /// Filenames that mark a directory as this server's project root
    /// (see `client::resolve_root`).
    pub project_markers: &'static [&'static str],
}

pub static RUST_ANALYZER: LspServerDef = LspServerDef {
    id: "rust-analyzer",
    command: "rust-analyzer",
    args: &[],
    project_markers: &["Cargo.toml"],
};

pub static TYPESCRIPT_LANGUAGE_SERVER: LspServerDef = LspServerDef {
    id: "typescript-language-server",
    command: "typescript-language-server",
    args: &["--stdio"],
    project_markers: &["package.json"],
};

pub static PYRIGHT: LspServerDef = LspServerDef {
    id: "pyright",
    command: "pyright-langserver",
    args: &["--stdio"],
    project_markers: &["pyproject.toml"],
};

pub static PHPANTOM: LspServerDef = LspServerDef {
    id: "phpantom",
    command: "phpantom_lsp",
    args: &[],
    project_markers: &["composer.json"],
};

pub static SEMA: LspServerDef = LspServerDef {
    id: "sema",
    command: "sema",
    args: &["lsp"],
    project_markers: &["sema.toml"],
};

static ALL_SERVER_DEFS: &[&LspServerDef] = &[
    &RUST_ANALYZER,
    &TYPESCRIPT_LANGUAGE_SERVER,
    &PYRIGHT,
    &PHPANTOM,
    &SEMA,
];

/// Looks up a registered def by `LspServerDef::id` (e.g. `"pyright"`) —
/// used to respawn a server on restart, when only its id (not the
/// triggering language) is known.
pub fn server_def_by_id(id: &str) -> Option<&'static LspServerDef> {
    ALL_SERVER_DEFS.iter().copied().find(|def| def.id == id)
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
    use LanguageId::*;
    match language {
        Rust => Some(&RUST_ANALYZER),
        TypeScript | Tsx | JavaScript | Jsx => Some(&TYPESCRIPT_LANGUAGE_SERVER),
        Python => Some(&PYRIGHT),
        Php => Some(&PHPANTOM),
        Sema => Some(&SEMA),
        _ => None,
    }
}

/// A server definition with config overrides applied: what to actually
/// execute, or `None` if the master switch or a per-server `enabled:
/// false` disables it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedServer {
    pub id: LspServerId,
    pub command: String,
    pub args: Vec<String>,
}

/// Applies `config.yaml`'s `lsp:` overrides to a registry def.
pub fn resolve_server(
    def: &'static LspServerDef,
    config: &crate::config::LspConfig,
) -> Option<ResolvedServer> {
    if !config.enabled {
        return None;
    }
    let over = config.servers.get(def.id);
    if over.is_some_and(|o| o.enabled == Some(false)) {
        return None;
    }
    let command = over
        .and_then(|o| o.command.clone())
        .unwrap_or_else(|| def.command.to_owned());
    let args = over
        .and_then(|o| o.args.clone())
        .unwrap_or_else(|| def.args.iter().map(|s| s.to_string()).collect());
    Some(ResolvedServer {
        id: LspServerId::from(def.id),
        command,
        args,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for def in [
            &RUST_ANALYZER,
            &TYPESCRIPT_LANGUAGE_SERVER,
            &PYRIGHT,
            &PHPANTOM,
            &SEMA,
        ] {
            assert!(!def.project_markers.is_empty(), "{} has no markers", def.id);
        }
    }

    #[test]
    fn master_switch_disables_every_server() {
        let config = crate::config::LspConfig {
            enabled: false,
            servers: Default::default(),
        };
        assert!(resolve_server(&RUST_ANALYZER, &config).is_none());
    }

    #[test]
    fn per_server_override_replaces_command_and_args() {
        let mut config = crate::config::LspConfig {
            enabled: true,
            servers: Default::default(),
        };
        config.servers.insert(
            "phpantom".to_owned(),
            crate::config::LspServerOverride {
                command: Some("laravel-lsp".to_owned()),
                args: None,
                enabled: None,
            },
        );
        let resolved = resolve_server(&PHPANTOM, &config).unwrap();
        assert_eq!(resolved.command, "laravel-lsp");
        assert!(resolved.args.is_empty()); // def.args is empty and no override
    }

    #[test]
    fn per_server_enabled_false_disables_just_that_server() {
        let mut config = crate::config::LspConfig {
            enabled: true,
            servers: Default::default(),
        };
        config.servers.insert(
            "pyright".to_owned(),
            crate::config::LspServerOverride {
                command: None,
                args: None,
                enabled: Some(false),
            },
        );
        assert!(resolve_server(&PYRIGHT, &config).is_none());
        assert!(resolve_server(&RUST_ANALYZER, &config).is_some());
    }
}
