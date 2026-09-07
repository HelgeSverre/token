//! Preset metadata. Config values remain authoritative, including off-preset values.
use crate::config::{EditorConfig, WordsMode};

#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Theme(String),
    Bool(bool),
    U64(u64),
    Usize(usize),
    F32(f32),
    Words(WordsMode),
}

#[derive(Debug)]
pub struct SettingDescriptor {
    pub id: &'static str,
    pub section: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub keywords: &'static str,
    pub choices: &'static [(&'static str, SettingValue)],
    pub read: fn(&EditorConfig) -> SettingValue,
    pub apply: fn(&mut EditorConfig, SettingValue),
}

impl SettingDescriptor {
    pub fn active_choice(&self, config: &EditorConfig) -> Option<usize> {
        let current = (self.read)(config);
        self.choices.iter().position(|(_, value)| *value == current)
    }

    /// Apply only a declared preset. Returns false for an invalid index or no change.
    pub fn select(&self, config: &mut EditorConfig, index: usize) -> bool {
        let Some((_, value)) = self.choices.get(index) else {
            return false;
        };
        if (self.read)(config) == *value {
            return false;
        }
        (self.apply)(config, value.clone());
        true
    }
}

pub static DESCRIPTORS: &[SettingDescriptor] = &[
    // The open-ended theme registry uses the existing picker, as allowed by
    // the settings spec. An empty preset list identifies that affordance.
    SettingDescriptor {
        id: "theme",
        section: "Appearance",
        name: "Theme",
        description: "Open the theme picker",
        keywords: "color scheme appearance",
        choices: &[],
        read: |config| SettingValue::Theme(config.theme.clone()),
        apply: |config, value| {
            if let SettingValue::Theme(theme) = value {
                config.theme = theme;
            }
        },
    },
    SettingDescriptor {
        id: "cursor_blink_ms",
        section: "Appearance",
        name: "Cursor Blink",
        description: "Caret blink interval",
        keywords: "caret animation",
        choices: &[
            ("Off", SettingValue::U64(0)),
            ("Slow", SettingValue::U64(1000)),
            ("Normal", SettingValue::U64(600)),
            ("Fast", SettingValue::U64(300)),
        ],
        read: |config| SettingValue::U64(config.cursor_blink_ms),
        apply: |config, value| {
            if let SettingValue::U64(value) = value {
                config.cursor_blink_ms = value;
            }
        },
    },
    SettingDescriptor {
        id: "show_scrollbar",
        section: "Appearance",
        name: "Scrollbars",
        description: "Show scrollbars in editor panes",
        keywords: "scroll visibility",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.show_scrollbar),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.show_scrollbar = value;
            }
        },
    },
    SettingDescriptor {
        id: "auto_surround",
        section: "Editor",
        name: "Auto Surround",
        description: "Surround selected text with brackets and quotes",
        keywords: "selection parentheses quotes",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.auto_surround),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.auto_surround = value;
            }
        },
    },
    SettingDescriptor {
        id: "bracket_matching",
        section: "Editor",
        name: "Bracket Matching",
        description: "Highlight the matching bracket",
        keywords: "parentheses braces highlight",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.bracket_matching),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.bracket_matching = value;
            }
        },
    },
    SettingDescriptor {
        id: "format_on_save",
        section: "Editor",
        name: "Format on Save",
        description: "Format the document before saving",
        keywords: "formatting save",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.format_on_save),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.format_on_save = value;
            }
        },
    },
    SettingDescriptor {
        id: "hover_on_mouse",
        section: "Editor",
        name: "Mouse Hover",
        description: "Show language information under the mouse",
        keywords: "tooltip documentation",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.hover_on_mouse),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.hover_on_mouse = value;
            }
        },
    },
    SettingDescriptor {
        id: "hover_delay_ms",
        section: "Editor",
        name: "Hover Delay",
        description: "Wait before showing mouse hover information",
        keywords: "tooltip delay",
        choices: &[
            ("Fast", SettingValue::U64(150)),
            ("Normal", SettingValue::U64(300)),
            ("Slow", SettingValue::U64(600)),
        ],
        read: |config| SettingValue::U64(config.hover_delay_ms),
        apply: |config, value| {
            if let SettingValue::U64(value) = value {
                config.hover_delay_ms = value;
            }
        },
    },
    SettingDescriptor {
        id: "status_bar_font_size",
        section: "Status Bar",
        name: "Status Bar Font Size",
        description: "Size of status bar text",
        keywords: "text appearance size",
        choices: &[
            ("Small", SettingValue::F32(11.0)),
            ("Medium", SettingValue::F32(12.0)),
            ("Large", SettingValue::F32(13.0)),
        ],
        read: |config| SettingValue::F32(config.status_bar_font_size),
        apply: |config, value| {
            if let SettingValue::F32(value) = value {
                config.status_bar_font_size = value;
            }
        },
    },
    SettingDescriptor {
        id: "completion.enabled",
        section: "Completion",
        name: "Autocomplete",
        description: "Offer completion suggestions while typing",
        keywords: "suggestions dropdown",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.completion.enabled),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.completion.enabled = value;
            }
        },
    },
    SettingDescriptor {
        id: "completion.words",
        section: "Completion",
        name: "Buffer Words",
        description: "Include words from the current document",
        keywords: "autocomplete fallback",
        choices: &[
            ("On", SettingValue::Words(WordsMode::Enabled)),
            ("Fallback", SettingValue::Words(WordsMode::Fallback)),
            ("Off", SettingValue::Words(WordsMode::Disabled)),
        ],
        read: |config| SettingValue::Words(config.completion.words),
        apply: |config, value| {
            if let SettingValue::Words(value) = value {
                config.completion.words = value;
            }
        },
    },
    SettingDescriptor {
        id: "completion.inline.enabled",
        section: "Completion",
        name: "Inline Suggestions",
        description: "Show ghost text from the configured provider",
        keywords: "ai autocomplete ghost",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.completion.inline.enabled),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.completion.inline.enabled = value;
            }
        },
    },
    SettingDescriptor {
        id: "completion.inline.debounce_ms",
        section: "Completion",
        name: "Inline Delay",
        description: "Quiet time before requesting an inline suggestion",
        keywords: "ai debounce latency",
        choices: &[
            ("Fast", SettingValue::U64(150)),
            ("Normal", SettingValue::U64(300)),
            ("Slow", SettingValue::U64(600)),
        ],
        read: |config| SettingValue::U64(config.completion.inline.debounce_ms),
        apply: |config, value| {
            if let SettingValue::U64(value) = value {
                config.completion.inline.debounce_ms = value;
            }
        },
    },
    SettingDescriptor {
        id: "completion.inline.max_line_suffix",
        section: "Completion",
        name: "Inline Suffix Limit",
        description: "Maximum trailing characters for automatic suggestions",
        keywords: "ai suffix tail",
        choices: &[
            ("Short", SettingValue::Usize(0)),
            ("Normal", SettingValue::Usize(8)),
            ("Long", SettingValue::Usize(16)),
        ],
        read: |config| SettingValue::Usize(config.completion.inline.max_line_suffix),
        apply: |config, value| {
            if let SettingValue::Usize(value) = value {
                config.completion.inline.max_line_suffix = value;
            }
        },
    },
    SettingDescriptor {
        id: "lsp.enabled",
        section: "LSP",
        name: "Language Servers",
        description: "Enable language server support",
        keywords: "intelligence diagnostics master",
        choices: &[
            ("Off", SettingValue::Bool(false)),
            ("On", SettingValue::Bool(true)),
        ],
        read: |config| SettingValue::Bool(config.lsp.enabled),
        apply: |config, value| {
            if let SettingValue::Bool(value) = value {
                config.lsp.enabled = value;
            }
        },
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_round_trips_through_config() {
        for descriptor in DESCRIPTORS {
            if descriptor.id == "theme" {
                assert!(descriptor.choices.is_empty());
                continue;
            }
            assert!(
                (2..=5).contains(&descriptor.choices.len()),
                "{}",
                descriptor.id
            );
            let mut config = EditorConfig::default();
            for (index, (_, value)) in descriptor.choices.iter().enumerate() {
                (descriptor.apply)(&mut config, value.clone());
                assert_eq!((descriptor.read)(&config), *value, "{}", descriptor.id);
                assert_eq!(
                    descriptor.active_choice(&config),
                    Some(index),
                    "{}",
                    descriptor.id
                );
            }
        }
    }

    #[test]
    fn off_preset_values_have_no_active_choice_and_reads_do_not_change_them() {
        let mut config = EditorConfig {
            cursor_blink_ms: 777,
            status_bar_font_size: 12.5,
            ..EditorConfig::default()
        };
        let before = serde_yaml::to_string(&config).unwrap();
        for id in ["cursor_blink_ms", "status_bar_font_size"] {
            let descriptor = DESCRIPTORS.iter().find(|d| d.id == id).unwrap();
            assert_eq!(descriptor.active_choice(&config), None);
            assert!(!descriptor.select(&mut config, usize::MAX));
        }
        assert_eq!(serde_yaml::to_string(&config).unwrap(), before);
    }
}
