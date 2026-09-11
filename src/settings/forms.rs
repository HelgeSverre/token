//! Draft forms for open-ended preferences. Editing never mutates live config.
use std::sync::Arc;

use super::{RowKind, SettingRow};
use crate::config::{EditorConfig, LspServerConfig};
use crate::editable::{EditConstraints, EditableState, StringBuffer};
use crate::syntax::LanguageId;
mod provider;

#[derive(Debug, Clone)]
pub enum SettingsChange {
    LanguageServer {
        previous_id: Option<String>,
        id: String,
        value: LspServerConfig,
    },
    InlineProvider {
        previous_id: Option<String>,
        id: String,
        value: crate::config::ProviderConfig,
        select: bool,
    },
    Remove {
        collection: CollectionKind,
        id: String,
    },
}

#[cfg(test)]
mod catalog_tests {
    use super::*;

    #[test]
    fn preset_records_can_be_renamed_and_removed_without_inheritance() {
        let mut config = EditorConfig::default();
        let original = config.lsp.servers["rust-analyzer"].clone();
        let mut form = SettingsForm::language_server(Some("rust-analyzer"), &config);
        form.fields[6].input.set_content("personal-rust");
        let change = form.change(&config).unwrap();
        assert!(
            config.lsp.servers.contains_key("rust-analyzer"),
            "validation is pure"
        );
        change.apply(&mut config);
        form.applied(&change);
        assert!(!config.lsp.servers.contains_key("rust-analyzer"));
        assert_eq!(config.lsp.servers["personal-rust"], original);
        form.removal().unwrap().apply(&mut config);
        let reloaded: EditorConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert!(crate::lsp::server_id_for_language(LanguageId::Rust, &reloaded.lsp).is_none());
    }

    #[test]
    fn selected_provider_rename_tracks_selection_and_removal_disables_suggestions() {
        let mut config = EditorConfig::default();
        let provider = crate::config::ProviderConfig::default();
        config
            .completion
            .providers
            .insert("old".into(), provider.clone());
        config.completion.inline.provider = "old".into();
        config.completion.inline.enabled = true;
        SettingsChange::InlineProvider {
            previous_id: Some("old".into()),
            id: "new".into(),
            value: provider,
            select: false,
        }
        .apply(&mut config);
        assert_eq!(config.completion.inline.provider, "new");
        assert!(config.completion.inline.enabled);
        assert!(!config.completion.providers.contains_key("old"));
        SettingsChange::Remove {
            collection: CollectionKind::InlineProviders,
            id: "new".into(),
        }
        .apply(&mut config);
        assert!(!config.completion.inline.enabled);
        assert!(config.completion.inline.provider.is_empty());
    }
}

/// Collections share draft navigation and persistence; their payloads stay typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionKind {
    LanguageServers,
    InlineProviders,
}

impl SettingsChange {
    pub fn apply(&self, config: &mut EditorConfig) {
        match self {
            Self::LanguageServer {
                previous_id,
                id,
                value,
            } => {
                if let Some(previous) = previous_id.as_ref().filter(|previous| *previous != id) {
                    config.lsp.servers.remove(previous);
                }
                config.lsp.servers.insert(id.clone(), value.clone());
            }
            Self::InlineProvider {
                previous_id,
                id,
                value,
                select,
            } => {
                let selected = previous_id
                    .as_ref()
                    .is_some_and(|previous| previous == &config.completion.inline.provider);
                if let Some(previous) = previous_id.as_ref().filter(|previous| *previous != id) {
                    config.completion.providers.remove(previous);
                }
                config
                    .completion
                    .providers
                    .insert(id.clone(), value.clone());
                if *select || selected {
                    config.completion.inline.provider.clone_from(id);
                }
            }
            Self::Remove {
                collection: CollectionKind::LanguageServers,
                id,
            } => {
                config.lsp.servers.remove(id);
            }
            Self::Remove {
                collection: CollectionKind::InlineProviders,
                id,
            } => {
                config.completion.providers.remove(id);
                if config.completion.inline.provider == *id {
                    config.completion.inline.provider.clear();
                    config.completion.inline.enabled = false;
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FormError {
    #[error("Choose a unique provider ID using letters, digits, dashes, underscores or dots")]
    ProviderId,
    #[error("{0}: enter a valid whole number")]
    Number(&'static str),
    #[error(transparent)]
    Provider(#[from] crate::completion::provider::ProviderError),
    #[error("Choose a unique server ID using letters, digits, dashes, underscores or dots")]
    ServerId,
    #[error("At least one supported language is required")]
    MissingLanguages,
    #[error(
        "Unknown language: {0}. Use a language name or alias, such as Rust, Python, C++, or tsx"
    )]
    Language(String),
    #[error(
        "Language assignments overlap with {0}; disable that server or change its languages first"
    )]
    AssociationConflict(String),
    #[error("Root markers must be file or directory names, without slashes, newlines or NUL")]
    RootMarker,
    #[error("Executable must not be empty or contain a newline or NUL")]
    Executable,
    #[error("{field}: {source}")]
    Structured {
        field: &'static str,
        source: serde_yaml::Error,
    },
    #[error("Server settings must be an object (or empty to use defaults)")]
    SettingsObject,
    #[error("Arguments must not contain NUL")]
    Argument,
}

#[derive(Debug, Clone)]
pub(crate) struct FormField {
    pub label: &'static str,
    pub help: &'static str,
    pub input: EditableState<StringBuffer>,
    pub browse: bool,
}

impl FormField {
    fn new(label: &'static str, help: &'static str, value: &str, multiline: bool) -> Self {
        let mut input = EditableState::new(
            if multiline {
                StringBuffer::multiline()
            } else {
                StringBuffer::new()
            },
            EditConstraints {
                allow_multiline: multiline,
                ..EditConstraints::single_line()
            },
        );
        input.set_content(value);
        Self {
            label,
            help,
            input,
            browse: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SettingsForm {
    pub session: Arc<()>,
    pub kind: FormKind,
    pub fields: Vec<FormField>,
    pub choices: Vec<FormChoice>,
    pub enabled: bool,
    pub focused: Option<usize>,
    pub dragging: bool,
    pub saving: bool,
    pub status: String,
    pub executable_status: String,
    pub remove_pending: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum FormKind {
    LanguageServer(Option<String>),
    InlineProvider(Option<String>),
}

#[derive(Debug, Clone)]
pub(crate) struct FormChoice {
    pub label: &'static str,
    pub help: &'static str,
    pub labels: &'static [&'static str],
    pub active: usize,
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    value
        .map(|value| serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()))
        .unwrap_or_default()
}

impl SettingsForm {
    pub fn title(&self) -> String {
        match &self.kind {
            FormKind::LanguageServer(Some(id)) | FormKind::InlineProvider(Some(id)) => {
                format!("{id} configuration")
            }
            FormKind::LanguageServer(None) => "Add language server".into(),
            FormKind::InlineProvider(None) => "Add AI provider".into(),
        }
    }

    pub fn actions(&self) -> &'static [&'static str] {
        if self.remove_pending {
            return &["Keep entry", "Cancel", "", "Confirm remove"];
        }
        match self.kind {
            FormKind::LanguageServer(Some(_)) => &["Save", "Cancel", "Open log", "Remove"],
            FormKind::InlineProvider(Some(_)) => &["Save", "Cancel", "Save & Use", "Remove"],
            FormKind::LanguageServer(None) => &["Save", "Cancel"],
            FormKind::InlineProvider(None) => &["Save", "Cancel", "Save & Use"],
        }
    }

    pub fn removal(&self) -> Option<SettingsChange> {
        let (collection, id) = match &self.kind {
            FormKind::LanguageServer(Some(id)) => (CollectionKind::LanguageServers, id),
            FormKind::InlineProvider(Some(id)) => (CollectionKind::InlineProviders, id),
            _ => return None,
        };
        Some(SettingsChange::Remove {
            collection,
            id: id.clone(),
        })
    }

    pub fn executable_field(&self) -> Option<usize> {
        match self.kind {
            FormKind::LanguageServer(_) => Some(0),
            FormKind::InlineProvider(_) => None,
        }
    }

    pub fn applied(&mut self, change: &SettingsChange) {
        match change {
            SettingsChange::LanguageServer { id, .. } => {
                self.kind = FormKind::LanguageServer(Some(id.clone()));
            }
            SettingsChange::InlineProvider { id, .. } => {
                self.kind = FormKind::InlineProvider(Some(id.clone()))
            }
            SettingsChange::Remove { .. } => {}
        }
        self.focused = None;
    }

    pub fn changed(&mut self) {
        self.remove_pending = false;
        self.status = "Draft · changes have not been applied".into();
        if self.focused.is_some() && self.focused == self.executable_field() {
            self.executable_status = "Executable changed · checked again when applied".into();
        }
    }

    pub fn language_server(id: Option<&str>, config: &EditorConfig) -> Self {
        let value = id
            .and_then(|id| config.lsp.servers.get(id))
            .cloned()
            .unwrap_or_default();
        let args = value.args.unwrap_or_default();
        let languages = id
            .map(|id| crate::lsp::configured_languages(id, &config.lsp))
            .unwrap_or_default()
            .iter()
            .map(LanguageId::display_name)
            .collect::<Vec<_>>()
            .join(", ");
        let markers = value.root_markers.unwrap_or_default();
        let mut executable = FormField::new(
            "Executable",
            "Command on PATH or an absolute path; no shell expansion",
            value.command.as_deref().unwrap_or_default(),
            false,
        );
        executable.browse = true;
        let mut form = Self {
            session: Arc::new(()),
            kind: FormKind::LanguageServer(id.map(str::to_owned)),
            choices: Vec::new(),
            fields: vec![
                executable,
                FormField::new(
                    "Arguments (JSON / YAML list)",
                    "A list of strings, not a shell command; empty means no arguments",
                    &json_text(Some(&serde_json::json!(args))),
                    true,
                ),
                FormField::new(
                    "Initialization options (JSON / YAML)",
                    "Advanced options sent as initializationOptions; empty sends null",
                    &json_text(value.initialization_options.as_ref()),
                    true,
                ),
                FormField::new(
                    "Server settings (JSON / YAML object)",
                    "Advanced settings returned to workspace/configuration; empty sends null",
                    &json_text(value.settings.as_ref()),
                    true,
                ),
                FormField::new(
                    "Languages",
                    "Comma-separated language names or aliases, for example C, C++, Python, tsx",
                    &languages,
                    false,
                ),
                FormField::new(
                    "Root markers (JSON / YAML list)",
                    "Nearest ancestor containing any marker; the open workspace takes precedence",
                    &json_text(Some(&serde_json::json!(markers))),
                    true,
                ),
            ],
            enabled: value.enabled.unwrap_or(true),
            focused: Some(0),
            dragging: false,
            saving: false,
            status: "Draft · Save applies this server; Cancel leaves it unchanged".into(),
            executable_status: "Checking executable…".into(),
            remove_pending: false,
        };
        form.fields.push(FormField::new(
            "Server ID",
            "Unique name, for example clangd or lua-language-server",
            id.unwrap_or_default(),
            false,
        ));
        if id.is_none() {
            form.focused = Some(6);
            form.executable_status = "Choose an installed executable".into();
        }
        form
    }

    pub fn entries(&self) -> Vec<SettingRow> {
        match &self.kind {
            FormKind::LanguageServer(server) => self.server_entries(server.as_deref()),
            FormKind::InlineProvider(_) => self.provider_entries(),
        }
    }

    fn server_entries(&self, server: Option<&str>) -> Vec<SettingRow> {
        let section = "Server configuration";
        let mut rows = vec![SettingRow {
            kind: RowKind::FormEnabled,
            section,
            name: "Server enabled".into(),
            description: "Only one enabled server can be assigned to each language".into(),
        }];
        rows.extend([6, 0, 4, 1, 5, 2, 3].into_iter().map(|index| {
            let field = &self.fields[index];
            SettingRow {
                kind: RowKind::FormField(index),
                section,
                name: field.label.into(),
                description: field.help.into(),
            }
        }));
        rows.push(SettingRow {
            kind: RowKind::FormInfo,
            section,
            name: "Resolved executable".into(),
            description: "The application's PATH may differ from an interactive shell".into(),
        });
        if let Some(id) = server {
            rows.push(SettingRow {
                kind: RowKind::ServerStatus(id.into()),
                section,
                name: "Live server status".into(),
                description: "Current process state; open a matching document to start the server"
                    .into(),
            });
        }
        rows.push(SettingRow {
            kind: RowKind::FormActions,
            section,
            name: "Configuration".into(),
            description: "Save, cancel the draft, open the log, or remove this entry".into(),
        });
        rows
    }

    pub fn change(&self, config: &EditorConfig) -> Result<SettingsChange, FormError> {
        match &self.kind {
            FormKind::LanguageServer(server) => self.server_change(server.as_deref(), config),
            FormKind::InlineProvider(id) => self.provider_change(id.as_deref(), config),
        }
    }

    fn server_change(
        &self,
        server: Option<&str>,
        config: &EditorConfig,
    ) -> Result<SettingsChange, FormError> {
        let id = self.fields[6].input.text().trim().to_owned();
        if !id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
            || !id
                .bytes()
                .all(|ch| ch.is_ascii_alphanumeric() || b"-_.".contains(&ch))
            || (server != Some(id.as_str()) && config.lsp.servers.contains_key(&id))
        {
            return Err(FormError::ServerId);
        }
        let command = self.fields[0].input.text();
        if command.trim().is_empty() || command.contains(['\n', '\r', '\0']) {
            return Err(FormError::Executable);
        }
        let arguments = self.fields[1].input.text();
        let args: Vec<String> = if arguments.trim().is_empty() {
            Vec::new()
        } else {
            serde_yaml::from_str(&arguments).map_err(|source| FormError::Structured {
                field: "Arguments",
                source,
            })?
        };
        if args.iter().any(|arg| arg.contains('\0')) {
            return Err(FormError::Argument);
        }
        let parse = |index: usize| -> Result<Option<serde_json::Value>, FormError> {
            let field = &self.fields[index];
            let text = field.input.text();
            if text.trim().is_empty() {
                return Ok(None);
            }
            serde_yaml::from_str(&text)
                .map(Some)
                .map_err(|source| FormError::Structured {
                    field: field.label,
                    source,
                })
        };
        let initialization_options = parse(2)?;
        let settings = parse(3)?;
        if settings.as_ref().is_some_and(|value| !value.is_object()) {
            return Err(FormError::SettingsObject);
        }
        let mut languages = Vec::new();
        for name in self.fields[4]
            .input
            .text()
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            let language = LanguageId::from_name(name)
                .filter(|language| crate::lsp::sync::language_id_str(*language).is_some())
                .ok_or_else(|| FormError::Language(name.into()))?;
            if !languages.contains(&language) {
                languages.push(language);
            }
        }
        if languages.is_empty() {
            return Err(FormError::MissingLanguages);
        }
        if self.enabled {
            if let Some(other) =
                crate::lsp::association_conflict(server.unwrap_or(&id), &languages, &config.lsp)
            {
                return Err(FormError::AssociationConflict(other.into()));
            }
        }
        let marker_text = self.fields[5].input.text();
        let root_markers: Vec<String> = if marker_text.trim().is_empty() {
            Vec::new()
        } else {
            serde_yaml::from_str(&marker_text).map_err(|source| FormError::Structured {
                field: "Root markers",
                source,
            })?
        };
        if root_markers.iter().any(|marker| {
            marker.is_empty()
                || marker == "."
                || marker == ".."
                || marker.contains(['/', '\\', '\n', '\r', '\0'])
        }) {
            return Err(FormError::RootMarker);
        }
        Ok(SettingsChange::LanguageServer {
            previous_id: server.map(str::to_owned),
            id,
            value: LspServerConfig {
                command: Some(command),
                args: Some(args),
                enabled: Some(self.enabled),
                initialization_options,
                settings,
                languages: Some(languages),
                root_markers: Some(root_markers),
            },
        })
    }
}
