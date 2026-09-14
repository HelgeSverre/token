//! Shared preset selection and guidance for executable configuration forms.
use super::{FormKind, SettingsForm};
use crate::config::EditorConfig;
use crate::settings::{RowKind, SettingRow};
use crate::tooling::{self, Platform, PresetKind, Template, ToolDefinition, ToolPreset};

impl SettingsForm {
    pub fn presets(&self) -> Vec<&'static ToolPreset> {
        let kind = match self.kind {
            FormKind::LanguageServer(_) => PresetKind::Lsp,
            FormKind::Formatter(_) => PresetKind::Formatter,
            FormKind::InlineProvider(_) => return Vec::new(),
        };
        tooling::presets_for(kind, None)
    }

    pub fn source_tool(&self) -> Option<&'static ToolDefinition> {
        tooling::tool(tooling::preset(self.preset_id.as_deref()?)?.tool_id)
    }

    /// Copy the chosen template into a new independent draft. The config clone
    /// is only a constructor input; selecting a preset never persists anything.
    pub fn draft_from_preset(&self, selected: usize, config: &EditorConfig) -> Option<Self> {
        if !matches!(
            self.kind,
            FormKind::LanguageServer(None) | FormKind::Formatter(None)
        ) {
            return None;
        }
        let presets = self.presets();
        let preset = match selected.checked_sub(1) {
            Some(index) => Some(*presets.get(index)?),
            None => None,
        };
        let mut config = config.clone();
        let mut draft = match (preset.map(|preset| &preset.template), &self.kind) {
            (Some(Template::Lsp(template)), FormKind::LanguageServer(None)) => {
                let mut id = template.id.to_owned();
                let mut suffix = 2;
                while config.lsp.servers.contains_key(&id) {
                    id = format!("{}-{suffix}", template.id);
                    suffix += 1;
                }
                config
                    .lsp
                    .servers
                    .insert(id.clone(), template.configuration());
                let mut draft = Self::language_server(Some(&id), &config);
                draft.kind = FormKind::LanguageServer(None);
                draft
            }
            (Some(Template::Formatter(template)), FormKind::Formatter(None)) => {
                let language = *template.languages.first()?;
                config
                    .formatters
                    .insert(language, template.configuration(preset?.id));
                let mut draft = Self::formatter(Some(language.display_name()), &config);
                draft.kind = FormKind::Formatter(None);
                draft
            }
            (None, FormKind::LanguageServer(None)) => Self::language_server(None, &config),
            (None, FormKind::Formatter(None)) => Self::formatter(None, &config),
            _ => return None,
        };
        draft.preset = selected.checked_sub(1);
        draft.preset_id = preset.map(|preset| preset.id.into());
        draft.changed();
        Some(draft)
    }

    pub(super) fn installation_entries(&self) -> Vec<SettingRow> {
        let section = match self.kind {
            FormKind::Formatter(_) => "Formatting",
            _ => "Server configuration",
        };
        let mut rows = Vec::new();
        if let Some(tool) = self.source_tool() {
            rows.push(SettingRow {
                kind: RowKind::FormToolInfo,
                section,
                name: format!("Based on {}", tool.display_name).into(),
                description: tool.description.into(),
            });
            for (index, option) in tool.installation_options(Platform::current()).enumerate() {
                rows.push(SettingRow {
                    kind: RowKind::FormToolInfo,
                    section,
                    name: "Prerequisites".into(),
                    description: option.prerequisites.into(),
                });
                rows.push(SettingRow {
                    kind: RowKind::FormInstallGuide(index),
                    section,
                    name: option.label.into(),
                    description: option.prerequisites.into(),
                });
                for (step_index, step) in option.steps.iter().enumerate() {
                    rows.push(SettingRow {
                        kind: if step.command.is_some() {
                            RowKind::FormInstallCommand(index, step_index)
                        } else {
                            RowKind::FormToolInfo
                        },
                        section,
                        name: step.command.unwrap_or(step.explanation).into(),
                        description: if step.command.is_some() {
                            step.explanation
                        } else {
                            ""
                        }
                        .into(),
                    });
                }
            }
        } else if let Some(id) = &self.preset_id {
            rows.push(SettingRow { kind: RowKind::FormToolInfo, section, name: format!("Based on {id}").into(), description: "Installation guidance is unavailable for this preset; the saved configuration is still used.".into() });
        }
        rows.push(SettingRow {
            kind: RowKind::FormRecheck,
            section,
            name: "Executable availability".into(),
            description: "After installation, check the executable on the application's PATH"
                .into(),
        });
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::LanguageId;

    #[test]
    fn lsp_presets_copy_independent_drafts_with_unique_ids_and_custom_clears_source() {
        let config = EditorConfig::default();
        let original = config.lsp.clone();
        let form = SettingsForm::language_server(None, &config);
        let selected = form
            .presets()
            .iter()
            .position(|preset| preset.id == "ty")
            .unwrap()
            + 1;
        let mut draft = form.draft_from_preset(selected, &config).unwrap();
        assert_eq!(draft.fields[6].input.text(), "ty-2");
        assert_eq!(draft.preset_id.as_deref(), Some("ty"));
        assert_eq!(draft.fields[0].input.text(), "ty");
        assert_eq!(config.lsp, original);
        draft.fields[6].input.set_content("my-python");
        draft.fields[0].input.set_content("my-ty");
        draft.enabled = false; // Existing enabled Python assignment remains protected.
        let mut saved = config.clone();
        draft.change(&config).unwrap().apply(&mut saved);
        let reopened = SettingsForm::language_server(Some("my-python"), &saved);
        assert_eq!(reopened.source_tool().unwrap().id, "ty");
        assert_eq!(reopened.fields[0].input.text(), "my-ty");
        let custom = draft.draft_from_preset(0, &config).unwrap();
        assert!(custom.preset_id.is_none());
        assert!(custom.fields[0].input.text().is_empty());
        assert!(draft.draft_from_preset(100, &config).is_none());
    }

    #[test]
    fn formatter_presets_select_supported_language_and_retain_user_edits() {
        let mut config = EditorConfig::default();
        config.formatters.clear();
        let form = SettingsForm::formatter(None, &config);
        let mut draft = form.draft_from_preset(1, &config).unwrap();
        assert_eq!(draft.preset_id.as_deref(), Some("ruff"));
        assert_eq!(draft.fields[0].input.text(), "ruff");
        assert_eq!(draft.choices[0].labels[draft.choices[0].active], "Python");
        draft.fields[0].input.set_content("/my/ruff");
        draft.change(&config).unwrap().apply(&mut config);
        assert_eq!(config.formatters[&LanguageId::Python].command, "/my/ruff");
        let reopened = SettingsForm::formatter(Some("Python"), &config);
        assert_eq!(reopened.source_tool().unwrap().id, "ruff");
        assert!(reopened
            .installation_entries()
            .iter()
            .any(|row| matches!(row.kind, RowKind::FormInstallCommand(..))));
        assert!(draft
            .draft_from_preset(0, &config)
            .unwrap()
            .preset_id
            .is_none());
    }

    #[test]
    fn unknown_source_is_preserved_without_blocking_configuration() {
        let mut config = EditorConfig::default();
        config
            .formatters
            .get_mut(&LanguageId::Python)
            .unwrap()
            .preset_id = Some("removed-preset".into());
        let draft = SettingsForm::formatter(Some("Python"), &config);
        assert!(draft.source_tool().is_none());
        assert!(draft.change(&config).is_ok());
        assert!(draft
            .installation_entries()
            .iter()
            .any(|row| row.name.contains("removed-preset")));
    }
}
