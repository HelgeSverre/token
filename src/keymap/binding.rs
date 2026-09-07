//! Keybinding struct representing a mapping from keystroke(s) to command

use super::command::Command;
use super::context::{Condition, KeyContext};
use super::types::Keystroke;

/// A single keybinding mapping one or more keystrokes to a command
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybinding {
    /// The keystroke sequence (usually 1, sometimes 2 for chords)
    pub keystrokes: Vec<Keystroke>,
    /// The command to execute
    pub command: Command,
    /// Conditions that must be true for this binding to activate
    pub when: Option<Vec<Condition>>,
}

impl Keybinding {
    /// Create a single-keystroke binding
    pub fn new(keystroke: Keystroke, command: Command) -> Self {
        Self {
            keystrokes: vec![keystroke],
            command,
            when: None,
        }
    }

    /// Create a chord binding (multi-keystroke sequence)
    pub fn chord(keystrokes: Vec<Keystroke>, command: Command) -> Self {
        Self {
            keystrokes,
            command,
            when: None,
        }
    }

    /// Add conditions to this binding (builder pattern)
    pub fn when(mut self, conditions: Vec<Condition>) -> Self {
        self.when = Some(conditions);
        self
    }

    /// Add a single condition to this binding
    pub fn when_single(mut self, condition: Condition) -> Self {
        self.when = Some(vec![condition]);
        self
    }

    /// Conditional bindings require a context, including while a chord is only
    /// partially entered. Unconditional bindings remain eligible without one.
    pub(super) fn is_active(&self, context: Option<&KeyContext>) -> bool {
        self.when.as_ref().is_none_or(|conditions| {
            context.is_some_and(|context| Condition::evaluate_all(conditions, context))
        })
    }

    /// Check if this binding matches a single keystroke (not a chord)
    pub fn matches_single(&self, keystroke: &Keystroke) -> bool {
        self.keystrokes.len() == 1 && self.keystrokes[0] == *keystroke
    }

    /// Check if this binding starts with the given keystroke
    pub fn starts_with(&self, keystroke: &Keystroke) -> bool {
        self.keystrokes.first() == Some(keystroke)
    }

    /// Check if this is a chord (multi-keystroke) binding
    pub fn is_chord(&self) -> bool {
        self.keystrokes.len() > 1
    }

    /// Get display string for this keybinding
    pub fn display_string(&self) -> String {
        self.keystrokes
            .iter()
            .map(|k| k.display_string())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::types::{KeyCode, Modifiers};

    #[test]
    fn test_single_binding() {
        let stroke = Keystroke::new(KeyCode::Char('s'), Modifiers::CTRL);
        let binding = Keybinding::new(stroke, Command::SaveFile);

        assert!(!binding.is_chord());
        assert!(binding.matches_single(&stroke));
    }

    #[test]
    fn chord_binding_eligibility_requires_all_conditions_and_a_context() {
        let binding = Keybinding::new(Keystroke::key(KeyCode::Tab), Command::Copy);
        assert!(binding.is_active(None));
        assert!(binding.is_active(Some(&KeyContext::default())));
        let binding = binding.when(vec![Condition::HasSelection, Condition::EditorFocused]);
        assert!(!binding.is_active(None));
        let mut context = KeyContext::editor_default();
        assert!(!binding.is_active(Some(&context)));
        context.has_selection = true;
        assert!(binding.is_active(Some(&context)));
        context.editor_focused = false;
        assert!(!binding.is_active(Some(&context)));
    }

    #[test]
    fn test_chord_binding() {
        let stroke1 = Keystroke::new(KeyCode::Char('k'), Modifiers::CTRL);
        let stroke2 = Keystroke::new(KeyCode::Char('c'), Modifiers::CTRL);
        let binding = Keybinding::chord(vec![stroke1, stroke2], Command::Copy);

        assert!(binding.is_chord());
        assert!(!binding.matches_single(&stroke1));
        assert!(binding.starts_with(&stroke1));
    }
}
