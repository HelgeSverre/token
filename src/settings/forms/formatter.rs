//! Per-language formatter drafts using the shared Settings form controls.
use super::{FormChoice, FormError, FormKind, SettingsChange, SettingsForm};
use crate::config::{EditorConfig, FormatterConfig};
use crate::settings::{RowKind, SettingRow};
use crate::syntax::LanguageId;

pub(crate) fn formatter_ids(config: &EditorConfig) -> Vec<&'static str> {
    let mut ids: Vec<_> = config
        .formatters
        .keys()
        .map(LanguageId::display_name)
        .collect();
    ids.sort_unstable();
    ids
}

impl SettingsForm {
    pub fn formatter(id: Option<&str>, config: &EditorConfig) -> Self {
        let language = id.and_then(LanguageId::from_name);
        let value = language
            .and_then(|language| config.formatters.get(&language))
            .cloned()
            .unwrap_or_default();
        // Reuse executable and structured-argument controls (including browsing).
        let mut form = Self::language_server(None, config);
        form.kind = FormKind::Formatter(id.map(str::to_owned));
        form.fields.truncate(2);
        form.fields[0].input.set_content(&value.command);
        form.fields[1]
            .input
            .set_content(&serde_json::json!(value.args).to_string());
        form.fields[1].input.move_document_start(false);
        form.fields[1].help = "JSON / YAML string list; {file} expands to the filename. Buffer text goes to stdin; formatted text must go to stdout.";
        static LANGUAGES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
        let labels = LANGUAGES.get_or_init(|| {
            LanguageId::all()
                .map(|language| language.display_name())
                .collect()
        });
        let selected = language.or_else(|| {
            LanguageId::all().find(|language| !config.formatters.contains_key(language))
        });
        form.choices = vec![FormChoice {
            label: "Language",
            help: "One external formatter per language; selections still use LSP",
            labels,
            active: LanguageId::all()
                .position(|language| Some(language) == selected)
                .unwrap_or(0),
        }];
        form.enabled = value.enabled;
        form.preset_id = value.preset_id;
        form.focused = Some(0);
        form.status = "Draft · Save applies this formatter; Cancel leaves it unchanged".into();
        form
    }

    pub(super) fn formatter_entries(&self) -> Vec<SettingRow> {
        let section = "Formatting";
        let mut rows: Vec<_> =
            crate::settings::pages::preference_rows(crate::settings::CategoryId::Formatting)
                // Record editors use one form section without preference headings.
                .map(|mut row| {
                    row.section = section;
                    row
                })
                .collect();
        if matches!(self.kind, FormKind::Formatter(None)) {
            rows.push(SettingRow {
                kind: RowKind::FormPreset,
                section,
                name: "Start from".into(),
                description: "Copy an editable formatter preset or configure a custom command"
                    .into(),
            });
        }
        rows.push(SettingRow {
            kind: RowKind::FormEnabled,
            section,
            name: "Use external formatter".into(),
            description: "When disabled or removed, document formatting uses LSP".into(),
        });
        rows.push(SettingRow {
            kind: RowKind::FormChoice(0),
            section,
            name: self.choices[0].label.into(),
            description: self.choices[0].help.into(),
        });
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
            description: "Install the formatter separately or choose its executable".into(),
        });
        rows.extend(self.installation_entries());
        rows.push(SettingRow {
            kind: RowKind::FormActions,
            section,
            name: "Configuration".into(),
            description: "Save, cancel the draft, or remove this formatter".into(),
        });
        rows
    }

    pub(super) fn formatter_change(
        &self,
        id: Option<&str>,
        config: &EditorConfig,
    ) -> Result<SettingsChange, FormError> {
        let language = LanguageId::all()
            .nth(self.choices[0].active)
            .ok_or(FormError::MissingLanguages)?;
        let previous = id.and_then(LanguageId::from_name);
        if previous != Some(language) && config.formatters.contains_key(&language) {
            return Err(FormError::DuplicateFormatter);
        }
        let command = self.fields[0].input.text();
        if command.trim().is_empty() || command.contains(['\0', '\n', '\r']) {
            return Err(FormError::Executable);
        }
        let text = self.fields[1].input.text();
        let args: Vec<String> = if text.trim().is_empty() {
            Vec::new()
        } else {
            serde_yaml::from_str(&text).map_err(|source| FormError::Structured {
                field: "Arguments",
                source,
            })?
        };
        if args.iter().any(|arg| arg.contains('\0')) {
            return Err(FormError::Argument);
        }
        Ok(SettingsChange::Formatter {
            previous,
            language,
            value: FormatterConfig {
                preset_id: self.preset_id.clone(),
                enabled: self.enabled,
                command,
                args,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_drafts_validate_rename_remove_and_round_trip() {
        let mut config = EditorConfig::default();
        let original = config.formatters.clone();
        let mut draft = SettingsForm::formatter(Some("Python"), &config);
        draft.fields[0]
            .input
            .set_content("/path with spaces/formatter");
        draft.fields[1]
            .input
            .set_content("[\"--stdin-filename\", \"{file}\", \"-\"]");
        draft.choices[0].active = LanguageId::all()
            .position(|id| id == LanguageId::Rust)
            .unwrap();
        draft.enabled = false;
        let change = draft.change(&config).unwrap();
        assert_eq!(
            config.formatters, original,
            "draft validation must not apply changes"
        );
        change.apply(&mut config);
        draft.applied(&change);
        assert!(!config.formatters.contains_key(&LanguageId::Python));
        assert!(!config.formatters[&LanguageId::Rust].enabled);
        assert_eq!(config.formatters[&LanguageId::Rust].args[1], "{file}");
        let restored: EditorConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert_eq!(restored.formatters, config.formatters);
        draft.removal().unwrap().apply(&mut config);
        assert!(config.formatters.is_empty());
    }

    #[test]
    fn formatting_drafts_reject_duplicate_language_and_invalid_command_arguments() {
        let config = EditorConfig::default();
        let mut draft = SettingsForm::formatter(None, &config);
        draft.choices[0].active = LanguageId::all()
            .position(|id| id == LanguageId::Python)
            .unwrap();
        assert!(matches!(
            draft.change(&config),
            Err(FormError::DuplicateFormatter)
        ));
        let mut draft = SettingsForm::formatter(Some("Python"), &config);
        draft.fields[0].input.set_content("");
        assert!(matches!(draft.change(&config), Err(FormError::Executable)));
        draft.fields[0].input.set_content("ruff");
        draft.fields[1].input.set_content("--not-a-list");
        assert!(matches!(
            draft.change(&config),
            Err(FormError::Structured { .. })
        ));
    }

    #[test]
    fn formatting_cancel_reloads_saved_values_and_removal_stays_removed() {
        let mut config = EditorConfig::default();
        let mut draft = SettingsForm::formatter(Some("Python"), &config);
        draft.fields[0].input.set_content("different");
        let fresh = SettingsForm::formatter(Some("Python"), &config);
        assert_eq!(fresh.fields[0].input.text(), "ruff");
        fresh.removal().unwrap().apply(&mut config);
        let restored: EditorConfig =
            serde_yaml::from_str(&serde_yaml::to_string(&config).unwrap()).unwrap();
        assert!(restored.formatters.is_empty());
    }
}
