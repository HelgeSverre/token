//! Settings state and the shared projection used by rendering, input and search.
use crate::config::EditorConfig;
use crate::editable::{EditConstraints, EditableState, StringBuffer};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::borrow::Cow;
pub(crate) mod catalog;
pub mod pages;
pub(crate) use catalog::Setting;
pub use pages::{categories, CategoryId};
pub mod forms;
pub mod keymap;

const BOOL_LABELS: &[&str] = &["Off", "On"];

#[derive(Debug, Clone)]
pub(crate) enum RowKind {
    AddServer,
    AddProvider,
    Provider(String),
    FormField(usize),
    FormChoice(usize),
    FormEnabled,
    FormAdvanced,
    FormPreset,
    FormActions,
    FormInfo,
    FormToolInfo,
    FormInstallCommand(usize, usize),
    FormInstallGuide(usize),
    FormRecheck,
    KeymapBase,
    KeymapBinding(Option<usize>, crate::keymap::Command),
    CaptureActions,
    Preset(Setting),
    LspMaster,
    ServerEnabled(String),
    ServerCommand(String),
    ServerStatus(String),
}

#[derive(Debug, Clone)]
pub(crate) struct SettingRow {
    pub kind: RowKind,
    pub section: &'static str,
    pub name: Cow<'static, str>,
    description: Cow<'static, str>,
}

pub(crate) struct PickerValue<'a> {
    pub label: &'static str,
    pub value: &'a str,
}

impl SettingRow {
    pub(crate) fn category(&self) -> Option<CategoryId> {
        match self.kind {
            RowKind::Preset(setting) => pages::placement(setting).map(|(category, _)| category),
            RowKind::AddProvider | RowKind::Provider(_) => Some(CategoryId::AiProviders),
            RowKind::AddServer
            | RowKind::LspMaster
            | RowKind::ServerEnabled(_)
            | RowKind::ServerCommand(_)
            | RowKind::ServerStatus(_) => Some(CategoryId::LanguageServers),
            _ => None,
        }
    }

    fn group(&self) -> Option<pages::GroupId> {
        match self.kind {
            RowKind::Preset(setting) => pages::placement(setting).map(|(_, group)| group.id),
            _ => None,
        }
    }

    pub(crate) fn picker<'a>(&self, config: &'a EditorConfig) -> Option<PickerValue<'a>> {
        match self.kind {
            RowKind::Preset(setting) => match setting.descriptor().control {
                catalog::Control::Picker { labels, value, .. } => Some(PickerValue {
                    label: labels[0],
                    value: value(config),
                }),
                catalog::Control::Toggle { .. } | catalog::Control::Choice { .. } => None,
            },
            _ => None,
        }
    }

    fn search_text(&self) -> String {
        let mut text = format!(
            "{} {} {} {}",
            self.category().map_or("", CategoryId::label),
            self.section,
            self.name,
            self.description
        );
        if let RowKind::Preset(setting) = self.kind {
            let descriptor = setting.descriptor();
            text.push(' ');
            text.push_str(descriptor.key);
            for keyword in descriptor.keywords {
                text.push(' ');
                text.push_str(keyword);
            }
        }
        text.to_lowercase()
    }

    pub fn choices(&self) -> &'static [&'static str] {
        match self.kind {
            RowKind::AddServer => &["Add language server…"],
            RowKind::AddProvider => &["Add AI provider…"],
            RowKind::Provider(_) => &["Configure…"],
            RowKind::FormField(_) | RowKind::FormChoice(_) | RowKind::FormInfo => &[],
            RowKind::FormEnabled => BOOL_LABELS,
            RowKind::FormAdvanced => &["Show"],
            RowKind::FormPreset | RowKind::FormToolInfo => &[],
            RowKind::FormInstallCommand(..) => &["Copy command"],
            RowKind::FormInstallGuide(_) => &["Open installation guide"],
            RowKind::FormRecheck => &["Check executable again"],
            // Form-owned actions depend on the draft's kind and confirmation state.
            RowKind::FormActions => &[],
            RowKind::KeymapBase => crate::keymap::preferences::BaseKeymap::LABELS,
            RowKind::KeymapBinding(..) => &[],
            RowKind::CaptureActions => &["Save", "Cancel", "Literal"],
            RowKind::Preset(setting) => setting.descriptor().labels(),
            RowKind::LspMaster | RowKind::ServerEnabled(_) => BOOL_LABELS,
            RowKind::ServerCommand(_) => &["Configure…"],
            RowKind::ServerStatus(_) => &[],
        }
    }

    pub(crate) fn choice_presentation(&self) -> crate::view::overlay_surface::ChoicePresentation {
        use crate::view::overlay_surface::ChoicePresentation;
        match self.kind {
            RowKind::FormEnabled | RowKind::LspMaster | RowKind::ServerEnabled(_) => {
                ChoicePresentation::Checkbox
            }
            RowKind::FormAdvanced => ChoicePresentation::Disclosure,
            RowKind::FormChoice(_) => ChoicePresentation::Select,
            RowKind::Preset(setting) => match setting.descriptor().control {
                catalog::Control::Toggle { .. } => ChoicePresentation::Checkbox,
                catalog::Control::Choice { .. } => ChoicePresentation::Buttons,
                catalog::Control::Picker { .. } => ChoicePresentation::Buttons,
            },
            _ => ChoicePresentation::Buttons,
        }
    }

    pub fn active(&self, config: &EditorConfig) -> Option<usize> {
        match &self.kind {
            RowKind::AddServer | RowKind::AddProvider | RowKind::Provider(_) => None,
            RowKind::FormField(_)
            | RowKind::FormChoice(_)
            | RowKind::FormEnabled
            | RowKind::FormAdvanced
            | RowKind::FormPreset
            | RowKind::FormActions
            | RowKind::FormInfo
            | RowKind::FormToolInfo
            | RowKind::FormInstallCommand(..)
            | RowKind::FormInstallGuide(_)
            | RowKind::FormRecheck => None,
            RowKind::KeymapBase | RowKind::KeymapBinding(..) | RowKind::CaptureActions => None,
            RowKind::Preset(setting) => setting.descriptor().active(config),
            RowKind::LspMaster => Some(usize::from(config.lsp.enabled)),
            RowKind::ServerEnabled(id) => Some(usize::from(
                config
                    .lsp
                    .servers
                    .get(id)
                    .and_then(|o| o.enabled)
                    .unwrap_or(true),
            )),
            RowKind::ServerCommand(_) | RowKind::ServerStatus(_) => None,
        }
    }

    /// Open-ended commands use the row's clipped detail slot, never an unbounded
    /// accessory that could overwrite the label or paint outside the panel.
    pub fn detail<'a>(&'a self, config: &'a EditorConfig) -> Cow<'a, str> {
        if let RowKind::ServerCommand(id) = &self.kind {
            return match config
                .lsp
                .servers
                .get(id)
                .and_then(|o| o.command.as_deref())
            {
                Some("") => Cow::Borrowed("(empty command)"),
                Some(command) => Cow::Borrowed(command),
                None => Cow::Borrowed("No executable configured"),
            };
        }
        Cow::Borrowed(&self.description)
    }

    pub fn status(&self, model: &crate::model::AppModel) -> Option<Cow<'static, str>> {
        let RowKind::ServerStatus(id) = &self.kind else {
            return None;
        };
        use crate::lsp::ServerState;
        Some(
            match model
                .lsp
                .servers
                .get(&crate::lsp::LspServerId::from(id.as_str()))
            {
                None => Cow::Borrowed("Not started"),
                Some(ServerState::Starting) => Cow::Borrowed("Starting"),
                Some(ServerState::Indexing) => Cow::Borrowed("Indexing"),
                Some(ServerState::Ready) => Cow::Borrowed("Ready"),
                Some(ServerState::Restarting { attempt }) => {
                    Cow::Owned(format!("Restarting ({attempt})"))
                }
                Some(ServerState::Failed) => Cow::Borrowed("Failed"),
                Some(ServerState::Missing) => Cow::Borrowed("Missing"),
                Some(ServerState::ShuttingDown) => Cow::Borrowed("Shutting down"),
            },
        )
    }
}

fn settings_rows(config: &EditorConfig) -> Vec<SettingRow> {
    let mut rows: Vec<_> = categories()
        .iter()
        .flat_map(|&category| pages::preference_rows(category))
        .collect();
    rows.push(SettingRow {
        kind: RowKind::AddProvider,
        section: "AI",
        name: "AI providers".into(),
        description: "Connect an existing service or manage a local llama-server model".into(),
    });
    let mut providers: Vec<_> = config.completion.providers.keys().collect();
    providers.sort_unstable();
    for id in providers {
        rows.push(SettingRow {
            kind: RowKind::Provider(id.clone()),
            section: "AI",
            name: format!("{id} AI provider").into(),
            description: if *id == config.completion.inline.provider {
                "Selected for inline completion · inline suggestions have a separate enable switch"
                    .into()
            } else {
                "Configure this provider; choose Save & Use to select it for inline completion"
                    .into()
            },
        });
    }
    rows.push(SettingRow {
        kind: RowKind::LspMaster,
        section: "LSP",
        name: "Language servers".into(),
        description: "lsp.enabled · enable language intelligence".into(),
    });
    rows.push(SettingRow {
        kind: RowKind::AddServer,
        section: "LSP",
        name: "Custom language server".into(),
        description: "Add an installed server and choose the languages it handles".into(),
    });
    for id in crate::lsp::server_ids(&config.lsp) {
        let languages = crate::lsp::configured_languages(id, &config.lsp)
            .iter()
            .map(|language| language.display_name())
            .collect::<Vec<_>>()
            .join(", ");
        rows.extend([
            SettingRow {
                kind: RowKind::ServerEnabled(id.into()),
                section: "LSP",
                name: format!("{id} enabled").into(),
                description: format!("lsp.servers.{id}.enabled · {languages}").into(),
            },
            SettingRow {
                kind: RowKind::ServerCommand(id.into()),
                section: "LSP",
                name: format!("{id} executable").into(),
                description: format!(
                    "lsp.servers.{id}.command · {languages} · configure this server"
                )
                .into(),
            },
            SettingRow {
                kind: RowKind::ServerStatus(id.into()),
                section: "LSP",
                name: format!("{id} status").into(),
                description: format!("{id} live process state · {languages} · read-only").into(),
            },
        ]);
    }
    rows.sort_by_key(|row| row.category().map(CategoryId::index));
    rows
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub(crate) form: Option<forms::SettingsForm>,
    pub category: CategoryId,
    pub tab: keymap::SettingsTab,
    pub keymap: keymap::KeymapSettings,
    pub(crate) editable: EditableState<StringBuffer>,
    pub(crate) selected_index: usize,
    /// Physical-pixel offset in the Settings form, including section headings.
    pub(crate) scroll_offset_px: usize,
    /// Stable metadata. Values and live status are read from the model, not cached.
    pub(crate) entries: Vec<SettingRow>,
    /// Entry indices, grouped by section in table/registry order. Input and view share this.
    pub(crate) rows: Vec<usize>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self::new(&EditorConfig::default())
    }
}

impl SettingsState {
    pub fn new(config: &EditorConfig) -> Self {
        let entries = settings_rows(config);
        let rows = (0..entries.len()).collect();
        Self {
            form: None,
            category: CategoryId::All,
            tab: keymap::SettingsTab::General,
            keymap: keymap::KeymapSettings::default(),
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            selected_index: 0,
            scroll_offset_px: 0,
            entries,
            rows,
        }
    }
}

impl SettingsState {
    pub fn saving(&self) -> bool {
        self.form
            .as_ref()
            .map_or(self.keymap.saving, |form| form.saving)
    }

    pub fn editing_field(&self) -> bool {
        self.form
            .as_ref()
            .is_some_and(|form| form.focused.is_some() && !form.saving)
    }

    pub fn selection_drag_row(&self) -> Option<usize> {
        self.form
            .as_ref()
            .filter(|form| form.dragging && !form.saving)
            .map(|_| self.selected_index)
    }

    pub(crate) fn focused_input_mut(&mut self) -> Option<&mut EditableState<StringBuffer>> {
        match &mut self.form {
            Some(form) if !form.saving => {
                let index = form.focused?;
                Some(&mut form.fields.get_mut(index)?.input)
            }
            Some(_) => None,
            None => Some(&mut self.editable),
        }
    }
    pub(crate) fn refresh_entries(&mut self, config: &EditorConfig) {
        if let Some(form) = &self.form {
            self.entries = form.entries();
            self.rows = (0..self.entries.len()).collect();
            self.selected_index = self.selected_index.min(self.rows.len().saturating_sub(1));
            return;
        }
        self.entries = match self.tab {
            keymap::SettingsTab::General => settings_rows(config),
            keymap::SettingsTab::Keymap => self.keymap.entries(),
        };
        self.resolve_rows();
    }
    /// Current search input, for read-only automation snapshots.
    pub fn input(&self) -> String {
        if let Some(form) = &self.form {
            return form
                .focused
                .and_then(|index| form.fields.get(index))
                .map_or_else(String::new, |field| field.input.text());
        }
        self.editable.text()
    }

    /// Persistent feedback for the active explicit-save workflow.
    pub fn status(&self) -> Option<&str> {
        self.form
            .as_ref()
            .map(|form| form.status.as_str())
            .or_else(|| {
                (self.tab == keymap::SettingsTab::Keymap).then_some(self.keymap.status.as_str())
            })
    }

    /// Selected index in filtered-row order.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Row labels and sections in the same order used by rendering and input.
    pub fn filtered_rows(&self) -> impl Iterator<Item = (&str, &'static str)> + '_ {
        self.rows.iter().map(|&id| {
            (
                self.entries[id].name.as_ref(),
                self.section_title(&self.entries[id]),
            )
        })
    }

    pub(crate) fn resolve_rows(&mut self) {
        if self.form.is_some() {
            return;
        }
        if self.tab == keymap::SettingsTab::Keymap && self.keymap.capture.is_some() {
            self.rows = (0..self.entries.len()).collect();
            self.selected_index = 0;
            self.scroll_offset_px = 0;
            return;
        }
        let query = self.editable.text().to_lowercase();
        let mut matcher = Matcher::new(Config::DEFAULT);
        let mut needle_buf = Vec::new();
        let needle = Utf32Str::new(&query, &mut needle_buf);
        let category = self.category;
        self.rows = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, d)| {
                if self.tab == keymap::SettingsTab::General
                    && category != CategoryId::All
                    && d.category() != Some(category)
                {
                    return None;
                }
                let text = d.search_text();
                let mut haystack = Vec::new();
                (query.is_empty()
                    || matcher
                        .fuzzy_match(Utf32Str::new(&text, &mut haystack), needle)
                        .is_some())
                .then_some(index)
            })
            .collect();
        self.selected_index = 0;
        self.scroll_offset_px = 0;
    }

    fn section_title(&self, row: &SettingRow) -> &'static str {
        if self.form.is_none()
            && self.tab == keymap::SettingsTab::General
            && self.category == CategoryId::All
        {
            row.category().map_or(row.section, CategoryId::label)
        } else {
            row.section
        }
    }

    pub(crate) fn sections(&self) -> Vec<(&'static str, std::ops::Range<usize>)> {
        let mut sections: Vec<(&str, std::ops::Range<usize>)> = Vec::new();
        let mut previous_group = None;
        for (index, &id) in self.rows.iter().enumerate() {
            let section = self.section_title(&self.entries[id]);
            if let Some((title, range)) = sections.last_mut() {
                if *title == section
                    && (self.category == CategoryId::All
                        || self.form.is_some()
                        || previous_group == self.entries[id].group())
                {
                    range.end = index + 1;
                    continue;
                }
            }
            previous_group = self.entries[id].group();
            sections.push((section, index..index + 1));
        }
        sections
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_all_and_search_keep_each_category_contiguous() {
        let mut state = SettingsState::default();
        for query in ["", "save", "format", "completion"] {
            state.editable.set_content(query);
            state.resolve_rows();
            let sections = state.sections();
            let titles: std::collections::HashSet<_> =
                sections.iter().map(|(title, _)| *title).collect();
            assert_eq!(
                titles.len(),
                sections.len(),
                "repeated heading for {query:?}"
            );
            let categories: Vec<_> = state
                .rows
                .iter()
                .filter_map(|&id| state.entries[id].category())
                .map(CategoryId::index)
                .collect();
            assert!(categories.is_sorted());
            assert_eq!(
                sections.iter().map(|(_, rows)| rows.len()).sum::<usize>(),
                state.rows.len()
            );
        }
    }

    #[test]
    fn settings_search_uses_catalog_keys_and_keywords_after_regrouping() {
        let mut state = SettingsState::default();
        for (query, expected) in [
            ("cursor_blink_ms", Setting::Blink),
            ("blinking", Setting::Blink),
            ("auto_save.delay_ms", Setting::AutoSaveDelay),
        ] {
            state.editable.set_content(query);
            state.resolve_rows();
            assert!(state.rows.iter().any(|&id| matches!(state.entries[id].kind, RowKind::Preset(setting) if setting == expected)), "{query}");
        }
    }

    #[test]
    fn settings_formatter_preferences_share_one_unheaded_form_section() {
        let config = EditorConfig::default();
        let mut state = SettingsState::new(&config);
        state.category = CategoryId::Formatting;
        state.form = Some(forms::SettingsForm::formatter(None, &config));
        state.refresh_entries(&config);
        assert_eq!(state.sections(), vec![("Formatting", 0..state.rows.len())]);
    }

    #[test]
    fn lsp_settings_rows_follow_registry_and_resolve_current_command_values() {
        let state = SettingsState::default();
        let lsp_rows: Vec<_> = state
            .entries
            .iter()
            .filter(|row| row.section == "LSP")
            .collect();
        assert_eq!(lsp_rows.len(), 2 + 3 * crate::lsp::all_server_defs().len());
        let mut config = EditorConfig::default();
        let ids: Vec<_> = crate::lsp::server_ids(&config.lsp)
            .into_iter()
            .map(str::to_owned)
            .collect();
        for (id, group) in ids.iter().zip(lsp_rows[2..].as_chunks::<3>().0) {
            assert_eq!(group[0].active(&config), Some(1));
            assert_eq!(group[1].name, format!("{id} executable"));
            assert_eq!(
                group[1].detail(&config),
                config.lsp.servers[id].command.as_deref().unwrap()
            );
            assert_eq!(group[1].choices(), &["Configure…"]);
            assert!(group[2].choices().is_empty());
            config.lsp.servers.entry(id.clone()).or_default().command =
                Some("/an/overridden/command".into());
            assert_eq!(group[1].detail(&config), "/an/overridden/command");
            config.lsp.servers.get_mut(id).unwrap().command = Some(String::new());
            assert_eq!(group[1].detail(&config), "(empty command)");
        }
    }

    #[test]
    fn settings_presets_roundtrip_and_reject_invalid_choices() {
        for d in Setting::ALL.iter().map(|id| id.descriptor()).filter(|d| {
            matches!(
                d.control,
                catalog::Control::Toggle { .. } | catalog::Control::Choice { .. }
            )
        }) {
            for choice in 0..d.labels().len() {
                let mut config = EditorConfig::default();
                d.apply(&mut config, choice);
                assert_eq!(d.active(&config), Some(choice), "{}", d.name);
                assert!(!d.apply(&mut config, choice));
                assert!(!d.apply(&mut config, usize::MAX));
                assert_eq!(d.active(&config), Some(choice));
            }
        }
    }

    #[test]
    fn settings_off_preset_values_have_no_active_choice() {
        let config = EditorConfig {
            cursor_blink_ms: 777,
            status_bar_font_size: 12.5,
            hover_delay_ms: 444,
            ..EditorConfig::default()
        };
        for d in [Setting::Blink, Setting::StatusFont, Setting::HoverDelay].map(Setting::descriptor)
        {
            assert_eq!(d.active(&config), None);
        }
    }

    #[test]
    fn control_presentation_is_semantic_not_derived_from_labels() {
        use crate::view::overlay_surface::ChoicePresentation;

        let toggle = SettingRow {
            kind: RowKind::Preset(Setting::CompletionEnabled),
            section: "Completion",
            name: "Enabled".into(),
            description: "".into(),
        };
        let choice_with_off_label = SettingRow {
            kind: RowKind::Preset(Setting::Blink),
            section: "Editor",
            name: "Cursor blink".into(),
            description: "".into(),
        };

        assert_eq!(toggle.choice_presentation(), ChoicePresentation::Checkbox);
        assert_eq!(
            choice_with_off_label.choice_presentation(),
            ChoicePresentation::Buttons
        );
        assert_eq!(choice_with_off_label.choices().first(), Some(&"Off"));
    }

    #[test]
    fn settings_fuzzy_search_sections_share_flat_order() {
        let mut state = SettingsState {
            category: CategoryId::Editor,
            ..SettingsState::default()
        };
        state.editable.set_content("brackets");
        state.resolve_rows();
        assert_eq!(state.rows.len(), 2);
        assert!(state
            .rows
            .iter()
            .any(|&id| matches!(state.entries[id].kind, RowKind::Preset(Setting::Brackets))));
        assert_eq!(state.sections(), vec![("Editing assistance", 0..2)]);
        state.editable.set_content("zzzz-no-setting");
        state.resolve_rows();
        assert!(state.sections().is_empty());
    }
}
