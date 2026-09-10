//! Draft forms for open-ended preferences. Editing never mutates live config.
use std::sync::Arc;

use super::{RowKind, SettingRow};
use crate::config::{EditorConfig, LspServerOverride};
use crate::editable::{EditConstraints, EditableState, StringBuffer};
use crate::lsp::LspServerDef;

#[derive(Debug, Clone)]
pub enum SettingsChange {
    LanguageServer {
        id: &'static str,
        value: LspServerOverride,
    },
}

impl SettingsChange {
    pub fn apply(&self, config: &mut EditorConfig) {
        match self {
            Self::LanguageServer { id, value } => {
                config.lsp.servers.insert((*id).into(), value.clone());
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum FormError {
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
    pub server: &'static LspServerDef,
    pub fields: Vec<FormField>,
    pub enabled: bool,
    pub focused: Option<usize>,
    pub dragging: bool,
    pub saving: bool,
    pub status: String,
    pub executable_status: String,
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    value
        .map(|value| serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()))
        .unwrap_or_default()
}

impl SettingsForm {
    pub fn changed(&mut self) {
        self.status = "Draft · changes have not been applied".into();
        if self.focused == Some(0) {
            self.executable_status = "Executable changed · checked again when applied".into();
        }
    }

    pub fn language_server(def: &'static LspServerDef, config: &EditorConfig) -> Self {
        let value = config.lsp.servers.get(def.id).cloned().unwrap_or_default();
        let args = value
            .args
            .unwrap_or_else(|| def.args.iter().map(|arg| (*arg).into()).collect());
        let mut executable = FormField::new(
            "Executable",
            "Command on PATH or an absolute path; no shell expansion",
            value.command.as_deref().unwrap_or(def.command),
            false,
        );
        executable.browse = true;
        Self {
            session: Arc::new(()), server: def,
            fields: vec![
                executable,
                FormField::new("Arguments (JSON / YAML list)", "A list of strings, not a shell command; [] means no arguments; empty uses defaults", &json_text(Some(&serde_json::json!(args))), true),
                FormField::new("Initialization options (JSON / YAML)", "Advanced options sent as initializationOptions; empty uses defaults", &json_text(value.initialization_options.as_ref()), true),
                FormField::new("Server settings (JSON / YAML object)", "Advanced settings returned to workspace/configuration; empty uses defaults", &json_text(value.settings.as_ref()), true),
            ],
            enabled: value.enabled.unwrap_or(true), focused: Some(0), dragging: false, saving: false,
            status: "Draft · Apply & Restart saves this server only; Cancel leaves it unchanged".into(),
            executable_status: "Checking executable…".into(),
        }
    }

    pub fn entries(&self) -> Vec<SettingRow> {
        let section = "Server configuration";
        let mut rows = vec![SettingRow {
            kind: RowKind::FormEnabled,
            section,
            name: format!("{} enabled", self.server.id).into(),
            description: "The global Language servers switch still takes precedence".into(),
        }];
        rows.extend(
            self.fields
                .iter()
                .enumerate()
                .map(|(index, field)| SettingRow {
                    kind: RowKind::FormField(index),
                    section,
                    name: field.label.into(),
                    description: field.help.into(),
                }),
        );
        rows.push(SettingRow {
            kind: RowKind::FormInfo,
            section,
            name: "Resolved executable".into(),
            description: "The application's PATH may differ from an interactive shell".into(),
        });
        rows.push(SettingRow {
            kind: RowKind::ServerStatus(self.server.id),
            section,
            name: "Live server status".into(),
            description: "Current process state; open a matching document to start the server"
                .into(),
        });
        rows.push(SettingRow {
            kind: RowKind::FormActions,
            section,
            name: "Configuration".into(),
            description: "Apply & Restart, cancel the draft, or open the application log".into(),
        });
        rows
    }

    pub fn change(&self) -> Result<SettingsChange, FormError> {
        let command = self.fields[0].input.text();
        if command.trim().is_empty() || command.contains(['\n', '\r', '\0']) {
            return Err(FormError::Executable);
        }
        let arguments = self.fields[1].input.text();
        let args: Vec<String> = if arguments.trim().is_empty() {
            self.server.args.iter().map(|arg| (*arg).into()).collect()
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
        Ok(SettingsChange::LanguageServer {
            id: self.server.id,
            value: LspServerOverride {
                command: (command != self.server.command).then_some(command),
                args: (args
                    .iter()
                    .map(String::as_str)
                    .ne(self.server.args.iter().copied()))
                .then_some(args),
                enabled: Some(self.enabled),
                initialization_options,
                settings,
            },
        })
    }
}
