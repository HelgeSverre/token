use crate::syntax::LanguageId;

/// A seed preset for new configurations, never a runtime fallback.
#[derive(Debug, Clone, Copy)]
pub struct LspTemplate {
    /// Suggested ID for a new independent configuration record.
    pub id: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub languages: &'static [LanguageId],
    pub initialization_options: Option<&'static serde_json::Value>,
    pub settings: Option<&'static serde_json::Value>,
    /// Filenames that mark a directory as this server's project root.
    pub project_markers: &'static [&'static str],
}

impl LspTemplate {
    /// Copy a preset into an independent, fully editable configuration record.
    pub fn configuration(&self) -> crate::config::LspServerConfig {
        crate::config::LspServerConfig {
            preset_id: Some(self.id.into()),
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
            initialization_options: self.initialization_options.cloned(),
            settings: self.settings.cloned(),
        }
    }
}

pub static RUST_ANALYZER: LspTemplate = LspTemplate {
    id: "rust-analyzer",
    command: "rust-analyzer",
    args: &[],
    languages: &[LanguageId::Rust],
    initialization_options: None,
    settings: None,
    project_markers: &["Cargo.toml"],
};

pub static TYPESCRIPT_LANGUAGE_SERVER: LspTemplate = LspTemplate {
    id: "typescript-language-server",
    command: "typescript-language-server",
    args: &["--stdio"],
    languages: &[
        LanguageId::TypeScript,
        LanguageId::Tsx,
        LanguageId::JavaScript,
        LanguageId::Jsx,
    ],
    initialization_options: None,
    settings: None,
    project_markers: &["package.json"],
};

pub static TY: LspTemplate = LspTemplate {
    id: "ty",
    command: "ty",
    args: &["server"],
    languages: &[LanguageId::Python],
    initialization_options: None,
    settings: None,
    project_markers: &["pyproject.toml"],
};

pub static GOPLS: LspTemplate = LspTemplate {
    id: "gopls",
    command: "gopls",
    args: &[],
    languages: &[LanguageId::Go],
    initialization_options: None,
    settings: None,
    project_markers: &["go.work", "go.mod"],
};

pub static PHPANTOM: LspTemplate = LspTemplate {
    id: "phpantom",
    command: "phpantom_lsp",
    args: &[],
    languages: &[LanguageId::Php],
    initialization_options: None,
    settings: None,
    project_markers: &["composer.json"],
};

pub static SEMA: LspTemplate = LspTemplate {
    id: "sema",
    command: "sema",
    args: &["lsp"],
    languages: &[LanguageId::Sema],
    initialization_options: None,
    settings: None,
    project_markers: &["sema.toml"],
};

pub static DEFAULT_LSP_TEMPLATES: &[&LspTemplate] = &[
    &RUST_ANALYZER,
    &TYPESCRIPT_LANGUAGE_SERVER,
    &TY,
    &GOPLS,
    &PHPANTOM,
    &SEMA,
];
