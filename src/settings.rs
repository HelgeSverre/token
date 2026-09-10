//! Preset settings metadata and the shared search/section ordering authority.
use crate::config::EditorConfig;
use crate::editable::{EditConstraints, EditableState, StringBuffer};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::borrow::Cow;
pub mod keymap;

/// Categories in the separate Settings page, derived from the form metadata.
pub fn categories() -> Vec<Option<&'static str>> {
    let mut categories = vec![None];
    for descriptor in DESCRIPTORS {
        let category = Some(descriptor.section);
        if !categories.contains(&category) {
            categories.push(category);
        }
    }
    categories.extend([Some("LSP"), Some("Keymap")]);
    categories
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Setting {
    Theme,
    Blink,
    Surround,
    Brackets,
    Scrollbar,
    IndentGuides,
    StatusFont,
    Hover,
    HoverDelay,
    InlayHints,
    FormatOnSave,
    AutoReload,
    AutoSave,
    AutoSaveDelay,
    AutoSaveFormatting,
    IndentStyle,
    IndentSize,
    EditorConfig,
    TabWidth,
    LineEnding,
    SessionRestore,
    SessionSave,
    CompletionEnabled,
    CompletionMenu,
    CompletionWords,
    CompletionMinLength,
    InlineEnabled,
    InlineDelay,
    InlineSuffix,
    InlineStatistics,
}

pub(crate) struct Descriptor {
    pub setting: Setting,
    pub section: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub labels: &'static [&'static str],
}

const BOOL_LABELS: &[&str] = &["Off", "On"];
const BLINK: &[u64] = &[0, 1000, 600, 300];
const FONT: &[f32] = &[11.0, 12.0, 13.0];
const HOVER_DELAY: &[u64] = &[150, 300, 600];
const AUTO_SAVE_DELAY: &[u64] = &[500, 1000, 2000, 5000];
const WORD_LENGTHS: &[usize] = &[1, 3, 5, 8];
const INLINE_DELAYS: &[u64] = &[150, 300, 600, 1000];
const INLINE_SUFFIXES: &[usize] = &[0, 8, 32];
const WORD_MODES: &[crate::config::WordsMode] = &[
    crate::config::WordsMode::Disabled,
    crate::config::WordsMode::Fallback,
    crate::config::WordsMode::Enabled,
];
const AUTO_SAVE_MODES: &[crate::config::AutoSaveMode] = &[
    crate::config::AutoSaveMode::Off,
    crate::config::AutoSaveMode::OnFocusLoss,
    crate::config::AutoSaveMode::AfterDelay,
    crate::config::AutoSaveMode::OnFocusLossAndDelay,
];

pub(crate) static DESCRIPTORS: &[Descriptor] = &[
    Descriptor {
        setting: Setting::Theme,
        section: "Appearance",
        name: "Theme",
        description: "Color scheme · open theme picker",
        labels: &["Choose…"],
    },
    Descriptor {
        setting: Setting::Scrollbar,
        section: "Appearance",
        name: "Scrollbar",
        description: "show_scrollbar · document overview",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::IndentGuides,
        section: "Appearance",
        name: "Indent guides",
        description: "indent_guides · vertical indentation lines",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::Blink,
        section: "Editor",
        name: "Cursor blink",
        description: "cursor_blink_ms · caret speed",
        labels: &["Off", "Slow", "Normal", "Fast"],
    },
    Descriptor {
        setting: Setting::Surround,
        section: "Editor",
        name: "Auto surround",
        description: "auto_surround · brackets and quotes around selections",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::Brackets,
        section: "Editor",
        name: "Bracket matching",
        description: "bracket_matching · matching pair highlights",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::Hover,
        section: "Editor",
        name: "Mouse hover",
        description: "hover_on_mouse · documentation tooltips",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::HoverDelay,
        section: "Editor",
        name: "Hover delay",
        description: "hover_delay_ms · tooltip timing",
        labels: &["Fast", "Normal", "Slow"],
    },
    Descriptor {
        setting: Setting::InlayHints,
        section: "Editor",
        name: "Inlay hints",
        description: "lsp.inlay_hints · show Sema parameter hints at line ends",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::EditorConfig,
        section: "Editor",
        name: "EditorConfig",
        description: "editorconfig · apply per-file rules from .editorconfig files",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::IndentStyle,
        section: "Editor",
        name: "Indent using",
        description: "text.indent_style · fallback when no EditorConfig rule applies",
        labels: &["Default", "Tabs", "Spaces"],
    },
    Descriptor {
        setting: Setting::IndentSize,
        section: "Editor",
        name: "Indent size",
        description: "text.indent_size · columns per indentation step",
        labels: &["Default", "2", "4", "8"],
    },
    Descriptor {
        setting: Setting::TabWidth,
        section: "Editor",
        name: "Tab width",
        description: "text.tab_width · display width of hard tabs",
        labels: &["Default", "2", "4", "8"],
    },
    Descriptor {
        setting: Setting::LineEnding,
        section: "Editor",
        name: "Line endings",
        description: "text.end_of_line · detect or use a preferred ending",
        labels: &["Detect", "LF", "CRLF", "CR"],
    },
    Descriptor {
        setting: Setting::AutoSave,
        section: "Editor",
        name: "Auto-save",
        description: "Save modified files when the window loses focus or after editing pauses",
        labels: &["Off", "Focus loss", "Idle", "Both"],
    },
    Descriptor {
        setting: Setting::AutoSaveDelay,
        section: "Editor",
        name: "Auto-save delay",
        description: "Idle time since the last edit in each file",
        labels: &["0.5 s", "1 s", "2 s", "5 s"],
    },
    Descriptor {
        setting: Setting::AutoSaveFormatting,
        section: "Editor",
        name: "Format on auto-save",
        description: "Apply language server formatting before automatic saves",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::AutoReload,
        section: "Editor",
        name: "Reload external changes",
        description: "auto_reload · reload clean buffers; always protect local edits",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::FormatOnSave,
        section: "Editor",
        name: "Format on save",
        description: "format_on_save · language server formatting",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::StatusFont,
        section: "Status Bar",
        name: "Status bar font",
        description: "status_bar_font_size · text size",
        labels: &["Small", "Medium", "Large"],
    },
    Descriptor {
        setting: Setting::SessionRestore,
        section: "Session",
        name: "Restore saved-file tabs",
        description: "session.restore · tabs, splits, selections and scroll positions",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::SessionSave,
        section: "Session",
        name: "Save session on exit",
        description: "session.save_on_exit · metadata only, never unsaved text",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::InlineStatistics,
        section: "Completion",
        name: "Local completion statistics",
        description:
            "completion.inline.statistics · counts only, never source or network telemetry",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::CompletionEnabled,
        section: "Completion",
        name: "Code completion",
        description: "completion.enabled · master switch, including manual requests",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::CompletionMenu,
        section: "Completion",
        name: "Automatic completion menu",
        description: "completion.menu.enabled · turn off to use only manual completion",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::CompletionWords,
        section: "Completion",
        name: "Local word suggestions",
        description: "completion.menu.words · fallback uses words when language-server results are absent",
        labels: &["Off", "Fallback", "Always"],
    },
    Descriptor {
        setting: Setting::CompletionMinLength,
        section: "Completion",
        name: "Minimum local word length",
        description: "completion.menu.min_word_length · shortest candidate identifier to include",
        labels: &["1", "3", "5", "8"],
    },
    Descriptor {
        setting: Setting::InlineEnabled,
        section: "Completion",
        name: "AI inline suggestions",
        description: "completion.inline.enabled · requires a configured provider; may send source code",
        labels: BOOL_LABELS,
    },
    Descriptor {
        setting: Setting::InlineDelay,
        section: "Completion",
        name: "Inline suggestion delay",
        description: "completion.inline.debounce_ms · wait after typing before requesting a suggestion",
        labels: &["150 ms", "300 ms", "600 ms", "1 s"],
    },
    Descriptor {
        setting: Setting::InlineSuffix,
        section: "Completion",
        name: "Text after the cursor",
        description: "completion.inline.max_line_suffix · limit on text after the cursor; whitespace and closers are ignored",
        labels: &["0", "8", "32"],
    },
];

impl Descriptor {
    pub fn active(&self, config: &EditorConfig) -> Option<usize> {
        Some(match self.setting {
            Setting::Theme => return None,
            Setting::Blink => return BLINK.iter().position(|&v| v == config.cursor_blink_ms),
            Setting::StatusFont => {
                return FONT.iter().position(|&v| v == config.status_bar_font_size)
            }
            Setting::HoverDelay => {
                return HOVER_DELAY.iter().position(|&v| v == config.hover_delay_ms)
            }
            Setting::Surround => usize::from(config.auto_surround),
            Setting::Brackets => usize::from(config.bracket_matching),
            Setting::Scrollbar => usize::from(config.show_scrollbar),
            Setting::IndentGuides => usize::from(config.indent_guides),
            Setting::EditorConfig => usize::from(config.editorconfig),
            Setting::Hover => usize::from(config.hover_on_mouse),
            Setting::InlayHints => usize::from(config.lsp.inlay_hints),
            Setting::FormatOnSave => usize::from(config.format_on_save),
            Setting::AutoReload => usize::from(config.auto_reload),
            Setting::AutoSave => {
                return AUTO_SAVE_MODES
                    .iter()
                    .position(|&mode| mode == config.auto_save.mode)
            }
            Setting::AutoSaveDelay => {
                return AUTO_SAVE_DELAY
                    .iter()
                    .position(|&delay| delay == config.auto_save.delay_ms)
            }
            Setting::AutoSaveFormatting => usize::from(config.auto_save.format_on_save),
            Setting::IndentStyle => [
                None,
                Some(crate::model::IndentStyle::Tab),
                Some(crate::model::IndentStyle::Space),
            ]
            .iter()
            .position(|value| *value == config.text.indent_style)?,
            Setting::IndentSize => [None, Some(2), Some(4), Some(8)]
                .iter()
                .position(|value| *value == config.text.indent_size)?,
            Setting::TabWidth => [None, Some(2), Some(4), Some(8)]
                .iter()
                .position(|value| *value == config.text.tab_width)?,
            Setting::LineEnding => [
                None,
                Some(crate::model::LineEnding::Lf),
                Some(crate::model::LineEnding::Crlf),
                Some(crate::model::LineEnding::Cr),
            ]
            .iter()
            .position(|value| *value == config.text.end_of_line)?,
            Setting::SessionRestore => usize::from(config.session.restore),
            Setting::SessionSave => usize::from(config.session.save_on_exit),
            Setting::CompletionEnabled => usize::from(config.completion.enabled),
            Setting::CompletionMenu => usize::from(config.completion.menu.enabled),
            Setting::CompletionWords => WORD_MODES
                .iter()
                .position(|&mode| mode == config.completion.menu.words)?,
            Setting::CompletionMinLength => WORD_LENGTHS
                .iter()
                .position(|&length| length == config.completion.menu.min_word_length)?,
            Setting::InlineEnabled => usize::from(config.completion.inline.enabled),
            Setting::InlineDelay => INLINE_DELAYS
                .iter()
                .position(|&delay| delay == config.completion.inline.debounce_ms)?,
            Setting::InlineSuffix => INLINE_SUFFIXES
                .iter()
                .position(|&length| length == config.completion.inline.max_line_suffix)?,
            Setting::InlineStatistics => usize::from(config.completion.inline.statistics),
        })
    }

    /// Mutate only an explicit, valid preset; opening the UI never normalizes values.
    pub fn apply(&self, config: &mut EditorConfig, choice: usize) -> bool {
        if choice >= self.labels.len() || self.active(config) == Some(choice) {
            return false;
        }
        match self.setting {
            Setting::Theme => return false,
            Setting::Blink => config.cursor_blink_ms = BLINK[choice],
            Setting::StatusFont => config.status_bar_font_size = FONT[choice],
            Setting::HoverDelay => config.hover_delay_ms = HOVER_DELAY[choice],
            Setting::Surround => config.auto_surround = choice != 0,
            Setting::Brackets => config.bracket_matching = choice != 0,
            Setting::Scrollbar => config.show_scrollbar = choice != 0,
            Setting::IndentGuides => config.indent_guides = choice != 0,
            Setting::EditorConfig => config.editorconfig = choice != 0,
            Setting::Hover => config.hover_on_mouse = choice != 0,
            Setting::InlayHints => config.lsp.inlay_hints = choice != 0,
            Setting::FormatOnSave => config.format_on_save = choice != 0,
            Setting::AutoReload => config.auto_reload = choice != 0,
            Setting::AutoSave => config.auto_save.mode = AUTO_SAVE_MODES[choice],
            Setting::AutoSaveDelay => config.auto_save.delay_ms = AUTO_SAVE_DELAY[choice],
            Setting::AutoSaveFormatting => config.auto_save.format_on_save = choice != 0,
            Setting::IndentStyle => {
                config.text.indent_style = [
                    None,
                    Some(crate::model::IndentStyle::Tab),
                    Some(crate::model::IndentStyle::Space),
                ][choice]
            }
            Setting::IndentSize => {
                config.text.indent_size = [None, Some(2), Some(4), Some(8)][choice]
            }
            Setting::TabWidth => config.text.tab_width = [None, Some(2), Some(4), Some(8)][choice],
            Setting::LineEnding => {
                config.text.end_of_line = [
                    None,
                    Some(crate::model::LineEnding::Lf),
                    Some(crate::model::LineEnding::Crlf),
                    Some(crate::model::LineEnding::Cr),
                ][choice]
            }
            Setting::SessionRestore => config.session.restore = choice != 0,
            Setting::SessionSave => config.session.save_on_exit = choice != 0,
            Setting::CompletionEnabled => config.completion.enabled = choice != 0,
            Setting::CompletionMenu => config.completion.menu.enabled = choice != 0,
            Setting::CompletionWords => config.completion.menu.words = WORD_MODES[choice],
            Setting::CompletionMinLength => {
                config.completion.menu.min_word_length = WORD_LENGTHS[choice]
            }
            Setting::InlineEnabled => config.completion.inline.enabled = choice != 0,
            Setting::InlineDelay => config.completion.inline.debounce_ms = INLINE_DELAYS[choice],
            Setting::InlineSuffix => {
                config.completion.inline.max_line_suffix = INLINE_SUFFIXES[choice]
            }
            Setting::InlineStatistics => config.completion.inline.statistics = choice != 0,
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RowKind {
    KeymapBase,
    KeymapBinding(Option<usize>, crate::keymap::Command),
    CaptureActions,
    Preset(usize),
    LspMaster,
    ServerEnabled(&'static str),
    ServerCommand(&'static crate::lsp::LspServerDef),
    ServerStatus(&'static str),
}

#[derive(Debug, Clone)]
pub(crate) struct SettingRow {
    pub kind: RowKind,
    pub section: &'static str,
    pub name: Cow<'static, str>,
    description: Cow<'static, str>,
}

impl SettingRow {
    pub fn choices(&self) -> &'static [&'static str] {
        match self.kind {
            RowKind::KeymapBase => crate::keymap::preferences::BaseKeymap::LABELS,
            RowKind::KeymapBinding(..) => &[],
            RowKind::CaptureActions => &["Save", "Cancel", "Literal"],
            RowKind::Preset(index) => DESCRIPTORS[index].labels,
            RowKind::LspMaster | RowKind::ServerEnabled(_) => BOOL_LABELS,
            RowKind::ServerCommand(_) => &["Configure…"],
            RowKind::ServerStatus(_) => &[],
        }
    }

    pub fn active(&self, config: &EditorConfig) -> Option<usize> {
        match self.kind {
            RowKind::KeymapBase | RowKind::KeymapBinding(..) | RowKind::CaptureActions => None,
            RowKind::Preset(index) => DESCRIPTORS[index].active(config),
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
        if let RowKind::ServerCommand(def) = self.kind {
            return match config
                .lsp
                .servers
                .get(def.id)
                .and_then(|o| o.command.as_deref())
            {
                Some("") => Cow::Borrowed("(empty override)"),
                Some(command) => Cow::Borrowed(command),
                None => Cow::Owned(format!("{} (default)", def.command)),
            };
        }
        Cow::Borrowed(&self.description)
    }

    pub fn status(&self, model: &crate::model::AppModel) -> Option<Cow<'static, str>> {
        let RowKind::ServerStatus(id) = self.kind else {
            return None;
        };
        use crate::lsp::ServerState;
        Some(
            match model.lsp.servers.get(&crate::lsp::LspServerId::from(id)) {
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

fn settings_rows() -> Vec<SettingRow> {
    let mut rows: Vec<_> = DESCRIPTORS
        .iter()
        .enumerate()
        .map(|(index, d)| SettingRow {
            kind: RowKind::Preset(index),
            section: d.section,
            name: d.name.into(),
            description: d.description.into(),
        })
        .collect();
    rows.push(SettingRow {
        kind: RowKind::LspMaster,
        section: "LSP",
        name: "Language servers".into(),
        description: "lsp.enabled · enable language intelligence".into(),
    });
    for def in crate::lsp::all_server_defs() {
        let languages = crate::lsp::languages_for_server(def.id)
            .iter()
            .map(|language| language.display_name())
            .collect::<Vec<_>>()
            .join(", ");
        rows.extend([
            SettingRow {
                kind: RowKind::ServerEnabled(def.id),
                section: "LSP",
                name: format!("{} enabled", def.id).into(),
                description: format!("lsp.servers.{}.enabled · {languages}", def.id).into(),
            },
            SettingRow {
                kind: RowKind::ServerCommand(def),
                section: "LSP",
                name: format!("{} executable", def.id).into(),
                description: format!(
                    "lsp.servers.{}.command · {languages} · opens config.yaml",
                    def.id
                )
                .into(),
            },
            SettingRow {
                kind: RowKind::ServerStatus(def.id),
                section: "LSP",
                name: format!("{} status", def.id).into(),
                description: format!("{} live process state · {languages} · read-only", def.id)
                    .into(),
            },
        ]);
    }
    rows
}

#[derive(Debug, Clone)]
pub struct SettingsState {
    pub category: usize,
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
        let entries = settings_rows();
        let rows = (0..entries.len()).collect();
        Self {
            category: 0,
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
    pub(crate) fn refresh_entries(&mut self) {
        self.entries = match self.tab {
            keymap::SettingsTab::General => settings_rows(),
            keymap::SettingsTab::Keymap => self.keymap.entries(),
        };
        self.resolve_rows();
    }
    /// Current search input, for read-only automation snapshots.
    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Selected index in filtered-row order.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Row labels and sections in the same order used by rendering and input.
    pub fn filtered_rows(&self) -> impl Iterator<Item = (&str, &'static str)> + '_ {
        self.rows
            .iter()
            .map(|&id| (self.entries[id].name.as_ref(), self.entries[id].section))
    }

    pub(crate) fn resolve_rows(&mut self) {
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
        let category = categories().get(self.category).copied().flatten();
        self.rows = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, d)| {
                if self.tab == keymap::SettingsTab::General
                    && category.is_some_and(|category| d.section != category)
                {
                    return None;
                }
                let text = format!("{} {} {}", d.section, d.name, d.description).to_lowercase();
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

    pub(crate) fn sections(&self) -> Vec<(&'static str, std::ops::Range<usize>)> {
        let mut sections: Vec<(&str, std::ops::Range<usize>)> = Vec::new();
        for (index, &id) in self.rows.iter().enumerate() {
            let section = self.entries[id].section;
            if let Some((title, range)) = sections.last_mut() {
                if *title == section {
                    range.end = index + 1;
                    continue;
                }
            }
            sections.push((section, index..index + 1));
        }
        sections
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsp_settings_rows_follow_registry_and_resolve_current_command_values() {
        let state = SettingsState::default();
        assert_eq!(
            state.rows.len(),
            DESCRIPTORS.len() + 1 + 3 * crate::lsp::all_server_defs().len()
        );
        let mut config = EditorConfig::default();
        for (def, group) in crate::lsp::all_server_defs()
            .iter()
            .zip(state.entries[DESCRIPTORS.len() + 1..].as_chunks::<3>().0)
        {
            assert_eq!(group[0].active(&config), Some(1));
            assert_eq!(group[1].name, format!("{} executable", def.id));
            assert_eq!(
                group[1].detail(&config),
                format!("{} (default)", def.command)
            );
            assert_eq!(group[1].choices(), &["Configure…"]);
            assert!(group[2].choices().is_empty());
            config.lsp.servers.entry(def.id.into()).or_default().command =
                Some("/an/overridden/command".into());
            assert_eq!(group[1].detail(&config), "/an/overridden/command");
            config.lsp.servers.get_mut(def.id).unwrap().command = Some(String::new());
            assert_eq!(group[1].detail(&config), "(empty override)");
        }
    }

    #[test]
    fn settings_presets_roundtrip_and_reject_invalid_choices() {
        for d in DESCRIPTORS.iter().filter(|d| d.setting != Setting::Theme) {
            for choice in 0..d.labels.len() {
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
        for d in DESCRIPTORS.iter().filter(|d| {
            matches!(
                d.setting,
                Setting::Blink | Setting::StatusFont | Setting::HoverDelay
            )
        }) {
            assert_eq!(d.active(&config), None);
        }
    }

    #[test]
    fn settings_fuzzy_search_sections_share_flat_order() {
        let mut state = SettingsState {
            category: categories()
                .iter()
                .position(|&category| category == Some("Editor"))
                .unwrap(),
            ..SettingsState::default()
        };
        state.editable.set_content("brackets");
        state.resolve_rows();
        assert_eq!(state.rows.len(), 2);
        assert!(state
            .rows
            .iter()
            .any(|&id| DESCRIPTORS[id].setting == Setting::Brackets));
        assert_eq!(state.sections(), vec![("Editor", 0..2)]);
        state.editable.set_content("zzzz-no-setting");
        state.resolve_rows();
        assert!(state.sections().is_empty());
    }
}
