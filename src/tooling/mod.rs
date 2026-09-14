//! Bundled, copy-on-selection templates and installation guidance.
//!
//! Saved configuration is authoritative. This catalog is never consulted when
//! launching a configured tool. Installation steps are display text, not jobs.
pub mod presets;

use crate::config::FormatterConfig;
use crate::syntax::LanguageId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Linux,
    Windows,
    Other,
}

impl Platform {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Other
        }
    }
}

#[derive(Debug)]
pub struct InstallStep {
    pub explanation: &'static str,
    /// Copyable shell text. Never automatically executed by the application.
    pub command: Option<&'static str>,
}

#[derive(Debug)]
pub struct InstallOption {
    pub id: &'static str,
    pub label: &'static str,
    /// Empty means platform-independent guidance.
    pub platforms: &'static [Platform],
    pub prerequisites: &'static str,
    pub steps: &'static [InstallStep],
    pub source_url: &'static str,
}

impl InstallOption {
    pub fn applies_to(&self, platform: Platform) -> bool {
        self.platforms.is_empty() || self.platforms.contains(&platform)
    }
}

#[derive(Debug)]
pub struct ToolDefinition {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub homepage: &'static str,
    pub installation: &'static [InstallOption],
}

impl ToolDefinition {
    pub fn installation_options(&self, platform: Platform) -> impl Iterator<Item = &InstallOption> {
        self.installation
            .iter()
            .filter(move |option| option.applies_to(platform))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetKind {
    Lsp,
    Formatter,
}

#[derive(Debug)]
pub struct FormatterTemplate {
    pub languages: &'static [LanguageId],
    pub command: &'static str,
    pub args: &'static [&'static str],
}

impl FormatterTemplate {
    pub fn configuration(&self, preset_id: &str) -> FormatterConfig {
        FormatterConfig {
            preset_id: Some(preset_id.into()),
            enabled: true,
            command: self.command.into(),
            args: self.args.iter().map(|arg| (*arg).into()).collect(),
        }
    }
}

#[derive(Debug)]
pub enum Template {
    Lsp(&'static presets::LspTemplate),
    Formatter(FormatterTemplate),
}

#[derive(Debug)]
pub struct ToolPreset {
    pub id: &'static str,
    pub tool_id: &'static str,
    pub template: Template,
}

impl ToolPreset {
    pub fn kind(&self) -> PresetKind {
        match self.template {
            Template::Lsp(_) => PresetKind::Lsp,
            Template::Formatter(_) => PresetKind::Formatter,
        }
    }

    pub fn languages(&self) -> &'static [LanguageId] {
        match &self.template {
            Template::Lsp(template) => template.languages,
            Template::Formatter(template) => template.languages,
        }
    }

    pub fn display_name(&self) -> &'static str {
        tool(self.tool_id).map_or(self.id, |tool| tool.display_name)
    }
}

pub fn tool(id: &str) -> Option<&'static ToolDefinition> {
    TOOLS.iter().find(|tool| tool.id == id)
}

pub fn preset(id: &str) -> Option<&'static ToolPreset> {
    PRESETS.iter().find(|preset| preset.id == id)
}

/// The same ordering is used by rendering, hit testing and selection.
pub fn presets_for(kind: PresetKind, language: Option<LanguageId>) -> Vec<&'static ToolPreset> {
    let mut presets: Vec<_> = PRESETS
        .iter()
        .filter(|preset| {
            preset.kind() == kind
                && language.is_none_or(|language| preset.languages().contains(&language))
        })
        .collect();
    presets.sort_by_key(|preset| (preset.display_name(), preset.id));
    presets
}

/// Explicit default list, independent of the available formatter catalog.
pub fn default_formatters() -> std::collections::HashMap<LanguageId, FormatterConfig> {
    std::collections::HashMap::from([(LanguageId::Python, RUFF.configuration("ruff"))])
}

const RUFF: FormatterTemplate = FormatterTemplate {
    languages: &[LanguageId::Python],
    command: "ruff",
    args: &["format", "--stdin-filename", "{file}", "--quiet", "-"],
};

pub static PRESETS: &[ToolPreset] = &[
    ToolPreset {
        id: "rust-analyzer",
        tool_id: "rust-analyzer",
        template: Template::Lsp(&presets::RUST_ANALYZER),
    },
    ToolPreset {
        id: "typescript-language-server",
        tool_id: "typescript-language-server",
        template: Template::Lsp(&presets::TYPESCRIPT_LANGUAGE_SERVER),
    },
    ToolPreset {
        id: "ty",
        tool_id: "ty",
        template: Template::Lsp(&presets::TY),
    },
    ToolPreset {
        id: "gopls",
        tool_id: "gopls",
        template: Template::Lsp(&presets::GOPLS),
    },
    ToolPreset {
        id: "phpantom",
        tool_id: "phpantom",
        template: Template::Lsp(&presets::PHPANTOM),
    },
    ToolPreset {
        id: "sema",
        tool_id: "sema",
        template: Template::Lsp(&presets::SEMA),
    },
    ToolPreset {
        id: "ruff",
        tool_id: "ruff",
        template: Template::Formatter(RUFF),
    },
];

pub static TOOLS: &[ToolDefinition] = &[
    ToolDefinition {
        id: "rust-analyzer", display_name: "rust-analyzer", description: "Rust language server",
        homepage: "https://rust-analyzer.github.io/",
        installation: &[InstallOption {
            id: "rustup", label: "Install using rustup", platforms: &[],
            prerequisites: "Requires rustup and a Rust toolchain.\nUse the same toolchain as your project.",
            steps: &[InstallStep { explanation: "Install the rust-analyzer component", command: Some("rustup component add rust-analyzer") }],
            source_url: "https://rust-analyzer.github.io/book/installation.html",
        }],
    },
    ToolDefinition {
        id: "typescript-language-server", display_name: "TypeScript Language Server", description: "JavaScript and TypeScript language server",
        homepage: "https://github.com/typescript-language-server/typescript-language-server",
        installation: &[InstallOption {
            id: "npm", label: "Install using npm", platforms: &[],
            prerequisites: "Requires Node.js and npm.\nAdd npm’s global executable directory to PATH.",
            steps: &[InstallStep { explanation: "Install TypeScript and its language server", command: Some("npm install -g typescript-language-server typescript") }],
            source_url: "https://github.com/typescript-language-server/typescript-language-server#installing",
        }],
    },
    ToolDefinition {
        id: "ty", display_name: "ty", description: "Python type checker and language server",
        homepage: "https://docs.astral.sh/ty/",
        installation: &[InstallOption {
            id: "uv", label: "Install using uv", platforms: &[], prerequisites: "Requires uv.\nAdd uv’s tool executable directory to PATH.",
            steps: &[InstallStep { explanation: "Install ty globally", command: Some("uv tool install ty@latest") }],
            source_url: "https://docs.astral.sh/ty/installation/",
        }],
    },
    ToolDefinition {
        id: "gopls", display_name: "gopls", description: "Go language server",
        homepage: "https://go.dev/gopls/",
        installation: &[InstallOption {
            id: "go", label: "Install using Go", platforms: &[], prerequisites: "Requires Go.\nAdd GOBIN (or GOPATH/bin) to PATH.",
            steps: &[InstallStep { explanation: "Install gopls", command: Some("go install golang.org/x/tools/gopls@latest") }],
            source_url: "https://go.dev/gopls/",
        }],
    },
    ToolDefinition {
        id: "phpantom", display_name: "PHPantom", description: "PHP language server",
        homepage: "https://phpantom-dev.github.io/phpantom_lsp/",
        installation: &[
            InstallOption {
                id: "homebrew", label: "Install using Homebrew", platforms: &[Platform::MacOs], prerequisites: "Requires Homebrew.\nThe executable is named phpantom_lsp.",
                steps: &[InstallStep { explanation: "Install PHPantom", command: Some("brew install phpantom-lsp") }],
                source_url: "https://formulae.brew.sh/formula/phpantom-lsp",
            },
            InstallOption {
                id: "upstream", label: "Upstream installation guide", platforms: &[], prerequisites: "Follow the guide for your platform.\nThen select the installed executable.", steps: &[],
                source_url: "https://phpantom-dev.github.io/phpantom_lsp/",
            },
        ],
    },
    ToolDefinition {
        id: "sema", display_name: "Sema", description: "Sema Lisp interpreter with a built-in language server",
        homepage: "https://sema-lang.com/",
        installation: &[InstallOption {
            id: "upstream", label: "Install Sema", platforms: &[], prerequisites: "Install Sema using its official guide.\nThe server is included as sema lsp.", steps: &[],
            source_url: "https://github.com/sema-lisp/sema#installation",
        }],
    },
    ToolDefinition {
        id: "ruff", display_name: "Ruff", description: "Python formatter",
        homepage: "https://docs.astral.sh/ruff/",
        installation: &[InstallOption {
            id: "uv", label: "Install using uv", platforms: &[], prerequisites: "Requires uv.\nAdd uv’s tool executable directory to PATH.",
            steps: &[InstallStep { explanation: "Install Ruff globally", command: Some("uv tool install ruff@latest") }],
            source_url: "https://docs.astral.sh/ruff/installation/",
        }],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_references_defaults_and_platform_guidance_are_valid() {
        let mut tool_ids = HashSet::new();
        for tool in TOOLS {
            assert!(tool_ids.insert(tool.id));
            assert!(!tool.display_name.is_empty());
            assert!(tool.homepage.starts_with("https://"));
            let mut methods = HashSet::new();
            for option in tool.installation {
                assert!(methods.insert(option.id));
                assert!(option.source_url.starts_with("https://"));
                assert!(!option.label.is_empty());
                for step in option.steps {
                    assert!(!step.explanation.is_empty());
                    assert!(!step.command.is_some_and(str::is_empty));
                }
            }
            for platform in [
                Platform::MacOs,
                Platform::Linux,
                Platform::Windows,
                Platform::Other,
            ] {
                assert!(tool.installation_options(platform).next().is_some());
            }
        }
        let mut preset_ids = HashSet::new();
        for preset in PRESETS {
            assert!(preset_ids.insert(preset.id));
            assert!(tool(preset.tool_id).is_some());
            assert!(!preset.languages().is_empty());
            assert!(presets_for(preset.kind(), Some(preset.languages()[0]))
                .iter()
                .any(|item| item.id == preset.id));
        }
        for default in presets::DEFAULT_LSP_TEMPLATES {
            assert_eq!(preset(default.id).unwrap().kind(), PresetKind::Lsp);
        }
        assert_eq!(presets::DEFAULT_LSP_TEMPLATES.len(), 6);
        assert_eq!(default_formatters().len(), 1);
        assert_eq!(
            preset(
                default_formatters()[&LanguageId::Python]
                    .preset_id
                    .as_deref()
                    .unwrap()
            )
            .unwrap()
            .kind(),
            PresetKind::Formatter
        );
        let php = tool("phpantom").unwrap();
        assert_eq!(php.installation_options(Platform::MacOs).count(), 2);
        assert_eq!(php.installation_options(Platform::Windows).count(), 1);
    }

    #[test]
    fn preset_references_round_trip_without_runtime_inheritance() {
        let yaml = "lsp:\n  catalog_version: 1\n  servers:\n    renamed:\n      preset_id: ty\n      command: custom-python\n      args: [custom]\n      languages: [python]\nformatters:\n  python:\n    preset_id: retired-formatter\n    command: custom-format\n    args: [stdin]\n";
        let config: crate::config::EditorConfig = serde_yaml::from_str(yaml).unwrap();
        let restored: crate::config::EditorConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert_eq!(restored.lsp, config.lsp);
        assert_eq!(restored.formatters, config.formatters);
        let resolved = crate::lsp::resolve_server("renamed", &restored.lsp).unwrap();
        assert_eq!(resolved.command, "custom-python");
        assert_eq!(resolved.args, ["custom"]);
        assert_eq!(
            restored.formatters[&LanguageId::Python].command,
            "custom-format"
        );
        assert_eq!(restored.lsp.servers.len(), 1);
        let custom: FormatterConfig = serde_yaml::from_str("command: mine").unwrap();
        assert!(custom.preset_id.is_none());
        assert!(!serde_yaml::to_string(&custom)
            .unwrap()
            .contains("preset_id"));
    }
}
