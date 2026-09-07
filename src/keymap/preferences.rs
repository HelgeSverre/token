//! Pure keymap editing: platform-filtered merge, conflicts and override documents.
//! File access and installation belong to the runtime and update layers.

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

pub use super::config::{get_current_platform as platform, parse_sequence};
use super::{Command, KeyCode, KeyContext, Keybinding, KeymapError, Keystroke, Modifiers};

pub const MAX_KEYMAP_BYTES: crate::util::ByteSize = crate::util::ByteSize::mebibytes(1);
pub const MAX_CAPTURE_STROKES: usize = 4;

#[derive(Debug, Clone)]
pub enum KeymapChange {
    Base(BaseKeymap),
    Rebind {
        original: Option<Keybinding>,
        command: Command,
        strokes: Vec<Keystroke>,
    },
}

#[derive(Debug, Clone)]
pub struct KeymapSave {
    pub expected: Option<String>,
    pub change: KeymapChange,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseKeymap {
    #[default]
    Token,
    Conventional,
}

impl BaseKeymap {
    pub const LABELS: &[&str] = &["Token", "Common"];

    pub fn bindings(self) -> Vec<Keybinding> {
        let defaults = super::default_bindings();
        if self == Self::Token {
            return defaults;
        }
        super::merge_bindings(
            defaults,
            vec![
                Keybinding::new(
                    Keystroke::char_with_mods('p', Modifiers::cmd()),
                    Command::FuzzyFileFinder,
                ),
                Keybinding::new(
                    Keystroke::char_with_mods('p', Modifiers::cmd() | Modifiers::SHIFT),
                    Command::ToggleCommandPalette,
                ),
                Keybinding::new(
                    Keystroke::char_with_mods('d', Modifiers::cmd()),
                    Command::SelectNextOccurrence,
                ),
            ],
        )
    }
}

fn invalid(message: impl Into<String>) -> KeymapError {
    KeymapError::ParseError(message.into())
}

#[derive(Debug, Clone)]
pub struct KeymapSnapshot {
    /// Exact bytes-as-text observed by the worker; None means the file was absent.
    pub source: Option<String>,
    pub base: BaseKeymap,
    pub bindings: Vec<Keybinding>,
    pub conflict_counts: Vec<usize>,
    document: Mapping,
}

impl KeymapSnapshot {
    pub fn parse(source: Option<String>) -> Result<Self, KeymapError> {
        let text = source.as_deref().unwrap_or("bindings: []\n");
        if text.len() as u64 > MAX_KEYMAP_BYTES.as_u64() {
            return Err(invalid("Keymap exceeds the 1 MiB editing limit"));
        }
        let value: Value = serde_yaml::from_str(text).map_err(|e| invalid(e.to_string()))?;
        let document = value
            .as_mapping()
            .ok_or_else(|| invalid("Expected a keymap mapping"))?
            .clone();
        let base = document
            .get(Value::String("base".into()))
            .map(|v| serde_yaml::from_value::<BaseKeymap>(v.clone()))
            .transpose()
            .map_err(|e| invalid(e.to_string()))?
            .unwrap_or_default();
        let overrides = super::parse_keymap_yaml(text)?;
        let bindings = super::merge_bindings(base.bindings(), overrides);
        if bindings.len() > 2048 {
            return Err(invalid("Keymap exceeds the 2,048-binding editing limit"));
        }
        let masks: Vec<_> = bindings.iter().map(context_mask).collect();
        let mut conflict_counts = vec![0; bindings.len()];
        for (index, binding) in bindings.iter().enumerate() {
            for other in index + 1..bindings.len() {
                if (masks[index] & masks[other]) != 0
                    && (binding.keystrokes.starts_with(&bindings[other].keystrokes)
                        || bindings[other].keystrokes.starts_with(&binding.keystrokes))
                {
                    conflict_counts[index] += 1;
                    conflict_counts[other] += 1;
                }
            }
        }
        Ok(Self {
            source,
            base,
            bindings,
            conflict_counts,
            document,
        })
    }

    pub fn with_base(&self, base: BaseKeymap) -> Result<String, KeymapError> {
        let mut document = self.document.clone();
        document.insert(
            Value::String("base".into()),
            serde_yaml::to_value(base).map_err(|e| invalid(e.to_string()))?,
        );
        encode(document)
    }

    /// Rebinding applies only to this OS. A sequence-wide Unbound is followed by
    /// its still-active siblings, preserving contexts despite the legacy Unbound
    /// semantics. Global/foreign entries and unknown metadata remain in the file.
    pub fn rebind(
        &self,
        original: Option<&Keybinding>,
        command: Command,
        strokes: &[Keystroke],
    ) -> Result<String, KeymapError> {
        if strokes.is_empty() || strokes.len() > MAX_CAPTURE_STROKES {
            return Err(invalid("Capture between one and four keystrokes"));
        }
        let mut document = self.document.clone();
        let entries = document
            .get_mut(Value::String("bindings".into()))
            .and_then(Value::as_sequence_mut)
            .ok_or_else(|| invalid("Expected a bindings list"))?;
        if let Some(original) = original {
            if !self.bindings.contains(original) || original.command != command {
                return Err(invalid(
                    "The selected binding changed; reload keymap settings",
                ));
            }
            // Retire previous OS-local overrides for this sequence. Portable
            // entries must remain intact for the other platforms.
            entries.retain(|entry| {
                if entry.get("platform").and_then(Value::as_str) != Some(platform()) {
                    return true;
                }
                let sequence = entry
                    .get("key")
                    .and_then(Value::as_str)
                    .and_then(|key| parse_sequence(key).ok());
                sequence.as_deref() != Some(original.keystrokes.as_slice())
            });
            entries.push(binding_value(&Keybinding {
                keystrokes: original.keystrokes.clone(),
                command: Command::Unbound,
                when: None,
            })?);
            for sibling in self
                .bindings
                .iter()
                .filter(|binding| binding.keystrokes == original.keystrokes && *binding != original)
            {
                entries.push(binding_value(sibling)?);
            }
        }
        entries.push(binding_value(&Keybinding {
            keystrokes: strokes.to_vec(),
            command,
            when: original.and_then(|b| b.when.clone()),
        })?);
        encode(document)
    }
}

fn encode(document: Mapping) -> Result<String, KeymapError> {
    let text = serde_yaml::to_string(&document).map_err(|e| invalid(e.to_string()))?;
    KeymapSnapshot::parse(Some(text.clone()))?;
    Ok(text)
}

fn binding_value(binding: &Keybinding) -> Result<Value, KeymapError> {
    let key = binding
        .keystrokes
        .iter()
        .map(config_stroke)
        .collect::<Vec<_>>()
        .join(" ");
    if parse_sequence(&key)? != binding.keystrokes {
        return Err(invalid("This key cannot be represented in keymap.yaml"));
    }
    let mut entry = Mapping::new();
    entry.insert("key".into(), key.into());
    entry.insert("command".into(), binding.command.name().into());
    entry.insert("platform".into(), platform().into());
    if let Some(conditions) = &binding.when {
        entry.insert(
            "when".into(),
            serde_yaml::to_value(conditions).map_err(|e| invalid(e.to_string()))?,
        );
    }
    Ok(Value::Mapping(entry))
}

pub fn config_stroke(stroke: &Keystroke) -> String {
    let mut parts = Vec::new();
    for (enabled, name) in [
        (stroke.mods.ctrl(), "ctrl"),
        (stroke.mods.alt(), "alt"),
        (stroke.mods.shift(), "shift"),
        (stroke.mods.meta(), "meta"),
    ] {
        if enabled {
            parts.push(name.to_string());
        }
    }
    parts.push(match stroke.key {
        KeyCode::Char('+') => "plus".into(),
        KeyCode::Char(' ') => "literal_space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("f{n}"),
        KeyCode::NumpadAdd => "numpad_add".into(),
        KeyCode::NumpadSubtract => "numpad_subtract".into(),
        KeyCode::NumpadMultiply => "numpad_multiply".into(),
        KeyCode::NumpadDivide => "numpad_divide".into(),
        KeyCode::NumpadEnter => "numpad_enter".into(),
        KeyCode::NumpadDecimal => "numpad_decimal".into(),
        key => format!("{key:?}").to_ascii_lowercase(),
    });
    parts.join("+")
}

/// Exact collisions and prefix ambiguities are computed on typed sequences,
/// after platform filtering, and only for contexts in which both can activate.
pub fn conflicts(
    bindings: &[Keybinding],
    candidate: &Keybinding,
    skip: Option<usize>,
) -> Vec<usize> {
    let candidate_mask = context_mask(candidate);
    bindings
        .iter()
        .enumerate()
        .filter_map(|(index, binding)| {
            if skip == Some(index)
                || binding.keystrokes.is_empty()
                || candidate.keystrokes.is_empty()
            {
                return None;
            }
            let overlaps = binding.keystrokes.starts_with(&candidate.keystrokes)
                || candidate.keystrokes.starts_with(&binding.keystrokes);
            (overlaps && context_mask(binding) & candidate_mask != 0).then_some(index)
        })
        .collect()
}

fn context_mask(binding: &Keybinding) -> u128 {
    (0u8..128).fold(0, |mask, bits| {
        let flag = |bit: u8| bits & (1u8 << bit) != 0u8;
        let context = KeyContext {
            has_selection: flag(0),
            has_multiple_cursors: flag(1),
            modal_active: flag(2),
            editor_focused: flag(3),
            sidebar_focused: flag(4),
            overlay_routes_keys: flag(5),
            inline_suggestion_visible: flag(6),
        };
        if (context.editor_focused && context.sidebar_focused)
            || (context.modal_active && (context.editor_focused || context.sidebar_focused))
        {
            return mask;
        }
        if binding.is_active(Some(&context)) {
            mask | (1u128 << bits)
        } else {
            mask
        }
    })
}
