//! Central declarations for ordinary preferences. Defaults remain in EditorConfig.
//!
//! Add a declaration here and a placement in pages.rs. The macros only generate
//! typed identities and the repetitive choice binding; no runtime registration.
use crate::config::{AutoSaveMode, EditorConfig, WordsMode};
use crate::model::{IndentStyle, LineEnding};

#[derive(Debug, Clone, Copy)]
pub(crate) enum SettingAction {
    ChooseTheme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingEffect {
    Redraw,
    FontMetrics,
}

#[derive(Debug)]
pub(crate) enum Control {
    Choice {
        labels: &'static [&'static str],
        active: fn(&EditorConfig) -> Option<usize>,
        apply: fn(&mut EditorConfig, usize),
    },
    Picker {
        labels: &'static [&'static str],
        value: fn(&EditorConfig) -> &str,
        action: SettingAction,
    },
}

#[derive(Debug)]
pub(crate) struct Descriptor {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub control: Control,
    pub effect: SettingEffect,
}

impl Descriptor {
    const fn new(
        key: &'static str,
        name: &'static str,
        description: &'static str,
        control: Control,
    ) -> Self {
        Self {
            key,
            name,
            description,
            keywords: &[],
            control,
            effect: SettingEffect::Redraw,
        }
    }

    const fn effect(mut self, effect: SettingEffect) -> Self {
        self.effect = effect;
        self
    }

    const fn keywords(mut self, keywords: &'static [&'static str]) -> Self {
        self.keywords = keywords;
        self
    }

    pub fn labels(&self) -> &'static [&'static str] {
        match self.control {
            Control::Choice { labels, .. } | Control::Picker { labels, .. } => labels,
        }
    }

    pub fn active(&self, config: &EditorConfig) -> Option<usize> {
        match self.control {
            Control::Choice { active, .. } => active(config),
            Control::Picker { .. } => None,
        }
    }

    /// Opening settings never normalizes custom values; only valid choices apply.
    pub fn apply(&self, config: &mut EditorConfig, choice: usize) -> bool {
        if choice >= self.labels().len() || self.active(config) == Some(choice) {
            return false;
        }
        match self.control {
            Control::Choice { apply, .. } => {
                apply(config, choice);
                true
            }
            Control::Picker { .. } => false,
        }
    }
}

// Labels and values are declared as pairs, then projected for the existing
// choice renderer. The field binding is checked by Rust, including enum types.
macro_rules! choice {
    ($($field:ident).+; $($label:literal => $value:expr),+ $(,)?) => {
        Control::Choice {
            labels: &[$($label),+],
            active: |config| [$($value),+].iter().position(|value| *value == config.$($field).+),
            apply: |config, choice| {
                if let Some(value) = [$($value),+].get(choice) {
                    config.$($field).+ = *value;
                }
            },
        }
    };
}

macro_rules! toggle {
    ($($field:ident).+) => { choice!($($field).+; "Off" => false, "On" => true) };
}

macro_rules! catalog {
    ($($id:ident => $descriptor:expr),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum Setting { $($id),+ }

        impl Setting {
            #[cfg(test)]
            pub const ALL: &'static [Self] = &[$(Self::$id),+];

            pub fn descriptor(self) -> &'static Descriptor {
                match self { $(Self::$id => {
                    static DESCRIPTOR: Descriptor = $descriptor;
                    &DESCRIPTOR
                }),+ }
            }
        }
    };
}

catalog! {
    ExplorerAutoReveal => Descriptor::new("explorer_auto_reveal", "Reveal active file", "Expand and scroll File Explorer when navigating to a file",
        toggle!(explorer_auto_reveal)).keywords(&["explorer", "navigation", "follow", "selection"]),
    Theme => Descriptor::new("theme", "Theme", "Color scheme · open theme picker",
        Control::Picker { labels: &["Choose…"], value: |config| &config.theme, action: SettingAction::ChooseTheme }),
    Scrollbar => Descriptor::new("show_scrollbar", "Scrollbar", "document overview",
        toggle!(show_scrollbar)),
    IndentGuides => Descriptor::new("indent_guides", "Indent guides", "vertical indentation lines",
        toggle!(indent_guides)),
    Blink => Descriptor::new("cursor_blink_ms", "Cursor blink", "caret speed",
        choice!(cursor_blink_ms; "Off" => 0, "Slow" => 1000, "Normal" => 600, "Fast" => 300)).keywords(&["caret", "blinking"]),
    Surround => Descriptor::new("auto_surround", "Auto surround", "brackets and quotes around selections",
        toggle!(auto_surround)),
    Brackets => Descriptor::new("bracket_matching", "Bracket matching", "matching pair highlights",
        toggle!(bracket_matching)),
    Hover => Descriptor::new("hover_on_mouse", "Mouse hover", "documentation tooltips",
        toggle!(hover_on_mouse)),
    HoverDelay => Descriptor::new("hover_delay_ms", "Hover delay", "tooltip timing",
        choice!(hover_delay_ms; "Fast" => 150, "Normal" => 300, "Slow" => 600)),
    InlayHints => Descriptor::new("lsp.inlay_hints", "Inlay hints", "show Sema parameter hints at line ends",
        toggle!(lsp.inlay_hints)),
    EditorConfig => Descriptor::new("editorconfig", "EditorConfig", "apply per-file rules from .editorconfig files",
        toggle!(editorconfig)),
    IndentStyle => Descriptor::new("text.indent_style", "Indent using", "fallback when no EditorConfig rule applies",
        choice!(text.indent_style; "Default" => None, "Tabs" => Some(IndentStyle::Tab), "Spaces" => Some(IndentStyle::Space))),
    IndentSize => Descriptor::new("text.indent_size", "Indent size", "columns per indentation step",
        choice!(text.indent_size; "Default" => None, "2" => Some(2), "4" => Some(4), "8" => Some(8))),
    TabWidth => Descriptor::new("text.tab_width", "Tab width", "display width of hard tabs",
        choice!(text.tab_width; "Default" => None, "2" => Some(2), "4" => Some(4), "8" => Some(8))),
    LineEnding => Descriptor::new("text.end_of_line", "Line endings", "detect or use a preferred ending",
        choice!(text.end_of_line; "Detect" => None, "LF" => Some(LineEnding::Lf), "CRLF" => Some(LineEnding::Crlf), "CR" => Some(LineEnding::Cr))),
    AutoSave => Descriptor::new("auto_save.mode", "Auto-save", "Save modified files when the window loses focus or after editing pauses",
        choice!(auto_save.mode; "Off" => AutoSaveMode::Off, "Focus loss" => AutoSaveMode::OnFocusLoss, "Idle" => AutoSaveMode::AfterDelay, "Both" => AutoSaveMode::OnFocusLossAndDelay)),
    AutoSaveDelay => Descriptor::new("auto_save.delay_ms", "Auto-save delay", "Idle time since the last edit in each file",
        choice!(auto_save.delay_ms; "0.5 s" => 500, "1 s" => 1000, "2 s" => 2000, "5 s" => 5000)),
    AutoSaveFormatting => Descriptor::new("auto_save.format_on_save", "Format on auto-save", "Apply the configured formatter or LSP before automatic saves",
        toggle!(auto_save.format_on_save)),
    AutoReload => Descriptor::new("auto_reload", "Reload external changes", "reload clean buffers; always protect local edits",
        toggle!(auto_reload)),
    FormatOnSave => Descriptor::new("format_on_save", "Format on save", "Apply the configured formatter or LSP before manual saves",
        toggle!(format_on_save)),
    StatusFont => Descriptor::new("status_bar_font_size", "Status bar font", "text size",
        choice!(status_bar_font_size; "Small" => 11.0, "Medium" => 12.0, "Large" => 13.0)).effect(SettingEffect::FontMetrics),
    SessionRestore => Descriptor::new("session.restore", "Restore saved-file tabs", "tabs, splits, selections and scroll positions",
        toggle!(session.restore)),
    SessionSave => Descriptor::new("session.save_on_exit", "Save session on exit", "metadata only, never unsaved text",
        toggle!(session.save_on_exit)),
    InlineStatistics => Descriptor::new("completion.inline.statistics", "Local completion statistics", "counts only, never source or network telemetry",
        toggle!(completion.inline.statistics)),
    CompletionEnabled => Descriptor::new("completion.enabled", "Code completion", "master switch, including manual requests",
        toggle!(completion.enabled)),
    CompletionMenu => Descriptor::new("completion.menu.enabled", "Automatic completion menu", "turn off to use only manual completion",
        toggle!(completion.menu.enabled)),
    CompletionWords => Descriptor::new("completion.menu.words", "Local word suggestions", "fallback uses words when language-server results are absent",
        choice!(completion.menu.words; "Off" => WordsMode::Disabled, "Fallback" => WordsMode::Fallback, "Always" => WordsMode::Enabled)),
    CompletionMinLength => Descriptor::new("completion.menu.min_word_length", "Minimum local word length", "shortest candidate identifier to include",
        choice!(completion.menu.min_word_length; "1" => 1, "3" => 3, "5" => 5, "8" => 8)),
    InlineEnabled => Descriptor::new("completion.inline.enabled", "AI inline suggestions", "requires a configured provider; may send source code",
        toggle!(completion.inline.enabled)),
    InlineDelay => Descriptor::new("completion.inline.debounce_ms", "Inline suggestion delay", "wait after typing before requesting a suggestion",
        choice!(completion.inline.debounce_ms; "150 ms" => 150, "300 ms" => 300, "600 ms" => 600, "1 s" => 1000)),
    InlineSuffix => Descriptor::new("completion.inline.max_line_suffix", "Text after the cursor", "limit on text after the cursor; whitespace and closers are ignored",
        choice!(completion.inline.max_line_suffix; "0" => 0, "8" => 8, "32" => 32)),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml::Value;

    // Compare whole serialized configurations with just the declared leaf
    // removed. This catches accidentally binding a key to another field.
    fn remove_leaf(value: &mut Value, key: &str) -> Option<Value> {
        let mut parts = key.split('.').peekable();
        let mut node = value;
        while let Some(part) = parts.next() {
            let map = node.as_mapping_mut()?;
            let key = Value::String(part.into());
            if parts.peek().is_none() {
                return map.remove(&key);
            }
            node = map.get_mut(&key)?;
        }
        None
    }

    #[test]
    fn settings_bindings_only_change_their_declared_yaml_key() {
        for &setting in Setting::ALL {
            let descriptor = setting.descriptor();
            if !matches!(descriptor.control, Control::Choice { .. }) {
                continue;
            }
            let mut config = EditorConfig::default();
            let mut reference = serde_yaml::to_value(&config).unwrap();
            remove_leaf(&mut reference, descriptor.key);
            let mut distinct_values = Vec::new();
            for choice in 0..descriptor.labels().len() {
                descriptor.apply(&mut config, choice);
                let mut actual = serde_yaml::to_value(&config).unwrap();
                let value = remove_leaf(&mut actual, descriptor.key);
                assert_eq!(actual, reference, "{setting:?} changed another field");
                assert!(
                    !distinct_values.contains(&value),
                    "{setting:?} has duplicate choices or an incorrect key"
                );
                distinct_values.push(value);
                assert_eq!(descriptor.active(&config), Some(choice));
            }
        }
    }
}
