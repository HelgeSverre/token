//! Keymap-tab state and row metadata; one projected order serves all consumers.
use super::{RowKind, SettingRow};
use crate::keymap::preferences::{conflicts, BaseKeymap, KeymapSnapshot};
use crate::keymap::{Command, Keybinding, Keystroke};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    General,
    Keymap,
}

#[derive(Debug, Clone)]
pub struct Capture {
    pub original: Option<Keybinding>,
    pub command: Command,
    pub strokes: Vec<Keystroke>,
    pub literal_next: bool,
}

#[derive(Debug, Clone)]
pub struct KeymapSettings {
    pub session: Arc<()>,
    pub snapshot: Option<KeymapSnapshot>,
    pub loading: bool,
    pub saving: bool,
    pub status: String,
    pub capture: Option<Capture>,
}

impl Default for KeymapSettings {
    fn default() -> Self {
        Self {
            session: Arc::new(()),
            snapshot: None,
            loading: false,
            saving: false,
            status: "Open the Keymap tab to load bindings".into(),
            capture: None,
        }
    }
}

impl KeymapSettings {
    pub(crate) fn entries(&self) -> Vec<SettingRow> {
        if let Some(capture) = &self.capture {
            return vec![SettingRow {
                kind: RowKind::CaptureActions,
                section: "Capture",
                name: format!("Rebind {}", capture.command.name()).into(),
                // The header displays the recorded sequence; the footer owns
                // validation feedback. Avoid hiding keys in clipped row detail.
                description: "".into(),
            }];
        }
        let mut rows = vec![SettingRow {
            kind: RowKind::KeymapBase,
            section: "Base keymap",
            name: "Base preset".into(),
            description: "Common changes cmd+p, cmd+shift+p and cmd+d; user overrides win".into(),
        }];
        if let Some(snapshot) = &self.snapshot {
            for (index, binding) in snapshot.bindings.iter().enumerate() {
                let count = snapshot.conflict_counts[index];
                let context = binding
                    .when
                    .as_ref()
                    .filter(|conditions| !conditions.is_empty())
                    .map(|conditions| {
                        conditions
                            .iter()
                            .map(|condition| format!("{condition:?}"))
                            .collect::<Vec<_>>()
                            .join(" + ")
                    })
                    .unwrap_or_else(|| "Any context".into());
                rows.push(SettingRow {
                    kind: RowKind::KeymapBinding(Some(index), binding.command),
                    section: "Bindings",
                    name: binding.command.name().into(),
                    description: format!(
                        "{} · {} · {}{}",
                        binding.command.display_name(),
                        binding.display_string(),
                        context,
                        if count == 0 {
                            String::new()
                        } else {
                            format!(" · {count} conflicts")
                        }
                    )
                    .into(),
                });
            }
            for &command in Command::all() {
                if command == Command::Unbound
                    || snapshot.bindings.iter().any(|b| b.command == command)
                {
                    continue;
                }
                rows.push(SettingRow {
                    kind: RowKind::KeymapBinding(None, command),
                    section: "Unassigned",
                    name: command.name().into(),
                    description: command.display_name().into(),
                });
            }
        }
        rows
    }

    pub(crate) fn update_capture_status(&mut self) {
        let (Some(capture), Some(snapshot)) = (&self.capture, &self.snapshot) else {
            return;
        };
        let binding = Keybinding {
            keystrokes: capture.strokes.clone(),
            command: capture.command,
            when: capture.original.as_ref().and_then(|b| b.when.clone()),
        };
        let skip = capture
            .original
            .as_ref()
            .and_then(|original| snapshot.bindings.iter().position(|b| b == original));
        let collisions = conflicts(&snapshot.bindings, &binding, skip);
        self.status = if capture.literal_next {
            "Next key is literal, including Escape/Ctrl+Enter/Backspace".into()
        } else if let Some(&first) = collisions.first() {
            format!(
                "{} conflicts; first: {} — review before saving",
                collisions.len(),
                snapshot.bindings[first].command.name()
            )
        } else {
            "Ctrl+Enter saves · Esc cancels · Backspace removes a stroke".into()
        };
    }

    pub(crate) fn base_index(&self) -> Option<usize> {
        self.snapshot
            .as_ref()
            .map(|snapshot| usize::from(snapshot.base == BaseKeymap::Conventional))
    }
}
