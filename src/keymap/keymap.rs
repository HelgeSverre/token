//! Keymap struct for storing and looking up keybindings

use std::collections::HashMap;

use super::binding::Keybinding;
use super::command::Command;
use super::context::{Condition, KeyContext};
use super::types::Keystroke;

/// Result of handling a keystroke
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    /// Execute this command
    Execute(Command),
    /// Keystroke is part of a chord, await more input
    AwaitMore,
    /// No binding matches this keystroke
    NoMatch,
}

/// The keymap stores all keybindings and handles lookup
#[derive(Debug, Clone)]
pub struct Keymap {
    /// All registered bindings
    bindings: Vec<Keybinding>,
    /// Fast lookup for single-keystroke bindings (indices into bindings)
    /// Multiple bindings can share the same keystroke with different conditions
    single_lookup: HashMap<Keystroke, Vec<usize>>,
    /// Keystrokes that start a chord sequence
    chord_prefixes: HashMap<Keystroke, Vec<usize>>, // indices into bindings
    /// Current chord state (pending keystrokes)
    pending_chord: Vec<Keystroke>,
}

impl Keymap {
    /// Create an empty keymap
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            single_lookup: HashMap::new(),
            chord_prefixes: HashMap::new(),
            pending_chord: Vec::new(),
        }
    }

    /// Create a keymap with the given bindings
    pub fn with_bindings(bindings: Vec<Keybinding>) -> Self {
        let mut keymap = Self::new();
        for binding in bindings {
            keymap.add_binding(binding);
        }
        keymap
    }

    /// Add a binding to the keymap
    pub fn add_binding(&mut self, binding: Keybinding) {
        if binding.keystrokes.is_empty() {
            return;
        }

        let first_stroke = binding.keystrokes[0];
        let idx = self.bindings.len();

        if binding.is_chord() {
            // Track chord prefix
            self.chord_prefixes
                .entry(first_stroke)
                .or_default()
                .push(idx);
        } else {
            // Single keystroke - add to fast lookup
            self.single_lookup
                .entry(first_stroke)
                .or_default()
                .push(idx);
        }

        self.bindings.push(binding);
    }

    /// Clear pending chord state
    pub fn reset(&mut self) {
        self.pending_chord.clear();
    }

    /// Handle a keystroke and return the action to take
    ///
    /// This handles both single-keystroke bindings and chord sequences.
    /// Pass `None` for context to skip condition checking (matches any binding).
    pub fn handle_keystroke(&mut self, keystroke: Keystroke) -> KeyAction {
        self.handle_keystroke_with_context(keystroke, None)
    }

    /// Handle a keystroke with context for conditional bindings
    ///
    /// Bindings are checked in order; first matching binding wins.
    /// Bindings with conditions are checked before unconditional ones.
    pub fn handle_keystroke_with_context(
        &mut self,
        keystroke: Keystroke,
        context: Option<&KeyContext>,
    ) -> KeyAction {
        // If we have a pending chord, try to complete it
        if !self.pending_chord.is_empty() {
            self.pending_chord.push(keystroke);
            return self.try_complete_chord(context);
        }

        // Try single-keystroke binding
        if let Some(indices) = self.single_lookup.get(&keystroke) {
            if let Some(command) = self.find_matching_binding(indices, context) {
                return KeyAction::Execute(command);
            }
        }

        // Check if this starts a chord
        if self.chord_prefixes.contains_key(&keystroke) {
            self.pending_chord.push(keystroke);
            return KeyAction::AwaitMore;
        }

        KeyAction::NoMatch
    }

    /// Find first binding that matches the context
    fn find_matching_binding(
        &self,
        indices: &[usize],
        context: Option<&KeyContext>,
    ) -> Option<Command> {
        // First pass: find bindings with conditions that match
        for &idx in indices {
            let binding = &self.bindings[idx];
            if let Some(ref conditions) = binding.when {
                if let Some(ctx) = context {
                    if Condition::evaluate_all(conditions, ctx) {
                        return Some(binding.command);
                    }
                }
                // If no context provided but binding has conditions, skip it
            }
        }

        // Second pass: find unconditional bindings
        for &idx in indices {
            let binding = &self.bindings[idx];
            if binding.when.is_none() {
                return Some(binding.command);
            }
        }

        None
    }

    /// Try to complete a pending chord sequence
    fn try_complete_chord(&mut self, context: Option<&KeyContext>) -> KeyAction {
        let first = self.pending_chord[0];

        // Get all bindings that start with the first keystroke
        let Some(indices) = self.chord_prefixes.get(&first) else {
            self.reset();
            return KeyAction::NoMatch;
        };

        // Check for exact match (conditional bindings first)
        for &idx in indices {
            let binding = &self.bindings[idx];
            if binding.keystrokes == self.pending_chord {
                // Check conditions if present
                if let Some(ref conditions) = binding.when {
                    if let Some(ctx) = context {
                        if Condition::evaluate_all(conditions, ctx) {
                            let command = binding.command;
                            self.reset();
                            return KeyAction::Execute(command);
                        }
                    }
                    // Has conditions but no context or doesn't match - continue
                    continue;
                }
                // No conditions - match
                let command = binding.command;
                self.reset();
                return KeyAction::Execute(command);
            }
        }

        // Check if any binding could still match (prefix match)
        let could_match = indices.iter().any(|&idx| {
            let binding = &self.bindings[idx];
            binding.keystrokes.len() > self.pending_chord.len()
                && binding.keystrokes[..self.pending_chord.len()] == self.pending_chord
        });

        if could_match {
            KeyAction::AwaitMore
        } else {
            self.reset();
            KeyAction::NoMatch
        }
    }

    /// Look up a single keystroke without chord handling
    ///
    /// Use this for simple lookups when you don't need chord support.
    /// Returns first unconditional binding that matches.
    pub fn lookup(&self, keystroke: &Keystroke) -> Option<Command> {
        self.lookup_with_context(keystroke, None)
    }

    /// Look up a single keystroke with context
    pub fn lookup_with_context(
        &self,
        keystroke: &Keystroke,
        context: Option<&KeyContext>,
    ) -> Option<Command> {
        let indices = self.single_lookup.get(keystroke)?;
        self.find_matching_binding(indices, context)
    }

    /// Get all bindings
    pub fn bindings(&self) -> &[Keybinding] {
        &self.bindings
    }

    /// Get the keybinding for a command (first match)
    pub fn binding_for(&self, command: Command) -> Option<&Keybinding> {
        self.bindings.iter().find(|b| b.command == command)
    }

    /// Get display string for a command's keybinding
    pub fn display_for(&self, command: Command) -> Option<String> {
        self.binding_for(command).map(|b| b.display_string())
    }

    /// Check if any chord is in progress
    pub fn has_pending_chord(&self) -> bool {
        !self.pending_chord.is_empty()
    }

    /// Get the pending chord keystrokes (for status bar display)
    pub fn pending_chord_display(&self) -> Option<String> {
        if self.pending_chord.is_empty() {
            None
        } else {
            Some(
                self.pending_chord
                    .iter()
                    .map(|k| k.display_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::types::{KeyCode, Modifiers};

    fn ctrl_s() -> Keystroke {
        Keystroke::new(KeyCode::Char('s'), Modifiers::CTRL)
    }

    fn ctrl_k() -> Keystroke {
        Keystroke::new(KeyCode::Char('k'), Modifiers::CTRL)
    }

    fn ctrl_c() -> Keystroke {
        Keystroke::new(KeyCode::Char('c'), Modifiers::CTRL)
    }

    #[test]
    fn test_single_binding_lookup() {
        let keymap = Keymap::with_bindings(vec![Keybinding::new(ctrl_s(), Command::SaveFile)]);

        assert_eq!(keymap.lookup(&ctrl_s()), Some(Command::SaveFile));
        assert_eq!(keymap.lookup(&ctrl_k()), None);
    }

    #[test]
    fn test_handle_single_keystroke() {
        let mut keymap = Keymap::with_bindings(vec![Keybinding::new(ctrl_s(), Command::SaveFile)]);

        assert_eq!(
            keymap.handle_keystroke(ctrl_s()),
            KeyAction::Execute(Command::SaveFile)
        );
        assert_eq!(keymap.handle_keystroke(ctrl_k()), KeyAction::NoMatch);
    }

    #[test]
    fn test_chord_await_more() {
        let mut keymap = Keymap::with_bindings(vec![Keybinding::chord(
            vec![ctrl_k(), ctrl_c()],
            Command::Copy,
        )]);

        // First keystroke should await more
        assert_eq!(keymap.handle_keystroke(ctrl_k()), KeyAction::AwaitMore);
        assert!(keymap.has_pending_chord());

        // Second keystroke should complete
        assert_eq!(
            keymap.handle_keystroke(ctrl_c()),
            KeyAction::Execute(Command::Copy)
        );
        assert!(!keymap.has_pending_chord());
    }

    #[test]
    fn test_chord_mismatch_resets() {
        let mut keymap = Keymap::with_bindings(vec![Keybinding::chord(
            vec![ctrl_k(), ctrl_c()],
            Command::Copy,
        )]);

        assert_eq!(keymap.handle_keystroke(ctrl_k()), KeyAction::AwaitMore);
        // Wrong second keystroke
        assert_eq!(keymap.handle_keystroke(ctrl_s()), KeyAction::NoMatch);
        assert!(!keymap.has_pending_chord());
    }

    #[test]
    fn test_binding_for_command() {
        let keymap = Keymap::with_bindings(vec![
            Keybinding::new(ctrl_s(), Command::SaveFile),
            Keybinding::new(ctrl_c(), Command::Copy),
        ]);

        let binding = keymap.binding_for(Command::SaveFile);
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().command, Command::SaveFile);
    }

    #[test]
    fn test_display_for_command() {
        let keymap = Keymap::with_bindings(vec![Keybinding::new(ctrl_s(), Command::SaveFile)]);

        let display = keymap.display_for(Command::SaveFile);
        assert!(display.is_some());
        // Display format depends on platform
        let s = display.unwrap();
        assert!(s.contains('S') || s.contains('s'));
    }

    #[test]
    fn test_reset_clears_pending() {
        let mut keymap = Keymap::with_bindings(vec![Keybinding::chord(
            vec![ctrl_k(), ctrl_c()],
            Command::Copy,
        )]);

        keymap.handle_keystroke(ctrl_k());
        assert!(keymap.has_pending_chord());

        keymap.reset();
        assert!(!keymap.has_pending_chord());
    }
}
