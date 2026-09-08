//! Keymap struct for storing and looking up keybindings

use std::collections::HashMap;

use super::binding::Keybinding;
use super::command::Command;
use super::context::KeyContext;
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
    /// Without a context, only unconditional bindings are eligible.
    pub fn handle_keystroke(&mut self, keystroke: Keystroke) -> KeyAction {
        self.handle_keystroke_with_context([keystroke], None)
    }

    /// Handle ordered interpretations of one key event with conditional bindings.
    /// The first matching interpretation wins; chord state advances only once.
    ///
    /// Bindings are checked in order; first matching binding wins.
    /// Bindings with conditions are checked before unconditional ones.
    pub fn handle_keystroke_with_context(
        &mut self,
        candidates: impl IntoIterator<Item = Keystroke>,
        context: Option<&KeyContext>,
    ) -> KeyAction {
        self.handle_keystroke_filtered(candidates, context, |_| true)
    }

    /// Resolve only commands the caller can dispatch in its current focus mode.
    /// Filtering before matching keeps unavailable commands from claiming a
    /// single key or chord prefix, or shadowing an available binding.
    pub fn handle_keystroke_filtered(
        &mut self,
        candidates: impl IntoIterator<Item = Keystroke>,
        context: Option<&KeyContext>,
        accepts: impl Fn(Command) -> bool,
    ) -> KeyAction {
        let is_active =
            |binding: &Keybinding| binding.is_active(context) && accepts(binding.command);
        for keystroke in candidates {
            let action = self.action_for(keystroke, &is_active);
            match action {
                KeyAction::NoMatch => continue,
                KeyAction::AwaitMore => self.pending_chord.push(keystroke),
                KeyAction::Execute(_) => self.reset(),
            }
            return action;
        }
        self.reset();
        KeyAction::NoMatch
    }

    fn action_for(
        &self,
        keystroke: Keystroke,
        is_active: &impl Fn(&Keybinding) -> bool,
    ) -> KeyAction {
        if !self.pending_chord.is_empty() {
            let mut pending = self.pending_chord.clone();
            pending.push(keystroke);
            return self.chord_action(&pending, is_active);
        }

        // Try single-keystroke binding
        if let Some(indices) = self.single_lookup.get(&keystroke) {
            if let Some(command) = self.find_matching_binding(indices, is_active) {
                return KeyAction::Execute(command);
            }
        }

        // Check if this starts a chord
        if self
            .chord_prefixes
            .get(&keystroke)
            .is_some_and(|indices| indices.iter().any(|&idx| is_active(&self.bindings[idx])))
        {
            return KeyAction::AwaitMore;
        }

        KeyAction::NoMatch
    }

    /// Find first binding that matches the context
    fn find_matching_binding(
        &self,
        indices: &[usize],
        is_active: &impl Fn(&Keybinding) -> bool,
    ) -> Option<Command> {
        // First pass: find bindings with conditions that match
        for &idx in indices {
            let binding = &self.bindings[idx];
            if binding.when.is_some() && is_active(binding) {
                return Some(binding.command);
            }
        }

        // Second pass: find unconditional bindings
        for &idx in indices {
            let binding = &self.bindings[idx];
            if binding.when.is_none() && is_active(binding) {
                return Some(binding.command);
            }
        }

        None
    }

    /// Resolve without mutating chord state, shared by dispatch and hints.
    fn chord_action(
        &self,
        pending: &[Keystroke],
        is_active: &impl Fn(&Keybinding) -> bool,
    ) -> KeyAction {
        let Some(&first) = pending.first() else {
            return KeyAction::NoMatch;
        };

        // Get all bindings that start with the first keystroke
        let Some(indices) = self.chord_prefixes.get(&first) else {
            return KeyAction::NoMatch;
        };

        // Preserve dispatch's existing chord precedence: first eligible exact match.
        for &idx in indices {
            let binding = &self.bindings[idx];
            if binding.keystrokes == pending && is_active(binding) {
                return KeyAction::Execute(binding.command);
            }
        }

        // Check if any binding could still match (prefix match)
        let could_match = indices.iter().any(|&idx| {
            let binding = &self.bindings[idx];
            binding.keystrokes.len() > pending.len()
                && &binding.keystrokes[..pending.len()] == pending
                && is_active(binding)
        });

        if could_match {
            KeyAction::AwaitMore
        } else {
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
        self.find_matching_binding(indices, &|binding| binding.is_active(context))
    }

    /// Get all bindings
    pub fn bindings(&self) -> &[Keybinding] {
        &self.bindings
    }

    /// First binding whose entire sequence actually dispatches this command.
    /// This rejects false conditions, shadowed bindings and unreachable chords.
    pub fn binding_for(&self, command: Command, context: &KeyContext) -> Option<&Keybinding> {
        self.bindings.iter().find(|binding| {
            if binding.command != command {
                return false;
            }
            let first = binding.keystrokes[0];
            if let Some(winner) = self.lookup_with_context(&first, Some(context)) {
                return binding.keystrokes.len() == 1 && winner == command;
            }
            for length in 2..=binding.keystrokes.len() {
                let action = self.chord_action(&binding.keystrokes[..length], &|binding| {
                    binding.is_active(Some(context))
                });
                if length == binding.keystrokes.len() {
                    return action == KeyAction::Execute(command);
                }
                if action != KeyAction::AwaitMore {
                    return false;
                }
            }
            false
        })
    }

    /// Get display string for a command's keybinding
    pub fn display_for(&self, command: Command, context: &KeyContext) -> Option<String> {
        self.binding_for(command, context)
            .map(|b| b.display_string())
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
    use crate::keymap::context::Condition;
    use crate::keymap::types::{KeyCode, Modifiers};

    fn chord_context_step(
        keymap: &mut Keymap,
        stroke: Keystroke,
        context: &KeyContext,
    ) -> KeyAction {
        keymap.handle_keystroke_with_context([stroke], Some(context))
    }

    #[test]
    fn chord_context_inactive_prefix_does_not_capture_input() {
        let mut keymap = Keymap::with_bindings(vec![Keybinding::chord(
            vec![ctrl_k(), ctrl_c()],
            Command::Copy,
        )
        .when_single(Condition::HasSelection)]);
        let mut context = KeyContext::editor_default();
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_k(), &context),
            KeyAction::NoMatch
        );
        assert!(!keymap.has_pending_chord());
        assert_eq!(keymap.handle_keystroke(ctrl_k()), KeyAction::NoMatch);
        context.has_selection = true;
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_k(), &context),
            KeyAction::AwaitMore
        );
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_c(), &context),
            KeyAction::Execute(Command::Copy)
        );
        assert!(!keymap.has_pending_chord());
    }

    #[test]
    fn chord_context_rechecks_each_prefix_and_preserves_eligible_branches() {
        let mut keymap = Keymap::with_bindings(vec![
            Keybinding::chord(vec![ctrl_k(), ctrl_c(), ctrl_s()], Command::Copy)
                .when_single(Condition::HasSelection),
            Keybinding::chord(vec![ctrl_k(), ctrl_s()], Command::SaveFile),
        ]);
        let mut context = KeyContext::editor_default();
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_k(), &context),
            KeyAction::AwaitMore
        );
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_c(), &context),
            KeyAction::NoMatch
        );
        assert!(!keymap.has_pending_chord());
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_k(), &context),
            KeyAction::AwaitMore
        );
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_s(), &context),
            KeyAction::Execute(Command::SaveFile)
        );
        context.has_selection = true;
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_k(), &context),
            KeyAction::AwaitMore
        );
        context.has_selection = false;
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_c(), &context),
            KeyAction::NoMatch
        );
        assert!(!keymap.has_pending_chord());
        context.has_selection = true;
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_k(), &context),
            KeyAction::AwaitMore
        );
        assert_eq!(
            chord_context_step(&mut keymap, ctrl_c(), &context),
            KeyAction::AwaitMore
        );
        assert_eq!(keymap.handle_keystroke(ctrl_s()), KeyAction::NoMatch);
        assert!(!keymap.has_pending_chord());
    }

    #[test]
    fn key_interpretations_prefer_logical_bindings_and_preserve_unbound_input() {
        let logical = Keystroke::new(KeyCode::Char(']'), Modifiers::ALT);
        let base = Keystroke::new(KeyCode::Char('9'), Modifiers::ALT);
        let mut keymap = Keymap::with_bindings(vec![
            Keybinding::new(logical, Command::NextInlineSuggestion)
                .when_single(Condition::InlineSuggestionVisible),
            Keybinding::new(base, Command::Copy),
        ]);
        let mut context = KeyContext {
            inline_suggestion_visible: true,
            ..Default::default()
        };
        assert_eq!(
            keymap.handle_keystroke_with_context([logical, base], Some(&context)),
            KeyAction::Execute(Command::NextInlineSuggestion)
        );
        context.inline_suggestion_visible = false;
        assert_eq!(
            keymap.handle_keystroke_with_context([logical, base], Some(&context)),
            KeyAction::Execute(Command::Copy)
        );
        let composed = Keystroke::new(KeyCode::Char('’'), Modifiers::ALT);
        assert_eq!(
            keymap.handle_keystroke_with_context([composed, logical], Some(&context)),
            KeyAction::NoMatch
        );
        context.inline_suggestion_visible = true;
        assert_eq!(
            keymap.handle_keystroke_with_context([composed, logical], Some(&context)),
            KeyAction::Execute(Command::NextInlineSuggestion)
        );
        assert!(!keymap.has_pending_chord());
    }

    #[test]
    fn key_interpretations_resolve_against_the_same_pending_chord() {
        let mut keymap = Keymap::with_bindings(vec![
            Keybinding::chord(vec![ctrl_k(), ctrl_c(), ctrl_s()], Command::SaveFile),
            Keybinding::new(ctrl_s(), Command::Copy),
        ]);
        assert_eq!(
            keymap.handle_keystroke_with_context([ctrl_k(), ctrl_s()], None),
            KeyAction::AwaitMore
        );
        assert_eq!(
            keymap.handle_keystroke_with_context([ctrl_s(), ctrl_c()], None),
            KeyAction::AwaitMore
        );
        assert_eq!(keymap.pending_chord, vec![ctrl_k(), ctrl_c()]);
        assert_eq!(
            keymap.handle_keystroke_with_context([ctrl_c(), ctrl_s()], None),
            KeyAction::Execute(Command::SaveFile)
        );
        assert!(!keymap.has_pending_chord());
        assert_eq!(keymap.handle_keystroke(ctrl_k()), KeyAction::AwaitMore);
        assert_eq!(
            keymap.handle_keystroke_with_context([ctrl_s(), ctrl_s()], None),
            KeyAction::NoMatch
        );
        assert!(!keymap.has_pending_chord());
    }

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

        let binding = keymap.binding_for(Command::SaveFile, &KeyContext::editor_default());
        assert!(binding.is_some());
        assert_eq!(binding.unwrap().command, Command::SaveFile);
    }

    #[test]
    fn test_display_for_command() {
        let keymap = Keymap::with_bindings(vec![Keybinding::new(ctrl_s(), Command::SaveFile)]);

        let display = keymap.display_for(Command::SaveFile, &KeyContext::editor_default());
        assert!(display.is_some());
        // Display format depends on platform
        let s = display.unwrap();
        assert!(s.contains('S') || s.contains('s'));
    }

    #[test]
    fn shortcut_hints_follow_conditions_shadowing_and_unbinding() {
        let bindings = vec![
            Keybinding::new(ctrl_s(), Command::SaveFile),
            Keybinding::new(ctrl_s(), Command::Copy).when_single(Condition::HasSelection),
        ];
        let keymap = Keymap::with_bindings(bindings.clone());
        let mut context = KeyContext::editor_default();
        assert_eq!(
            keymap.display_for(Command::SaveFile, &context),
            Some(ctrl_s().display_string())
        );
        assert_eq!(keymap.display_for(Command::Copy, &context), None);
        context.has_selection = true;
        assert_eq!(keymap.display_for(Command::SaveFile, &context), None);
        assert_eq!(
            keymap.display_for(Command::Copy, &context),
            Some(ctrl_s().display_string())
        );
        let unbound = super::super::merge_bindings(
            bindings,
            vec![Keybinding::new(ctrl_s(), Command::Unbound)],
        );
        let keymap = Keymap::with_bindings(unbound);
        assert_eq!(keymap.display_for(Command::SaveFile, &context), None);
        assert_eq!(keymap.display_for(Command::Copy, &context), None);
    }

    #[test]
    fn shortcut_hints_resolve_chords_without_mutating_pending_input() {
        let mut keymap = Keymap::with_bindings(vec![
            Keybinding::chord(vec![ctrl_k(), ctrl_c()], Command::Copy)
                .when_single(Condition::HasSelection),
            Keybinding::chord(vec![ctrl_k(), ctrl_s()], Command::SaveFile),
        ]);
        let mut context = KeyContext::editor_default();
        assert_eq!(keymap.display_for(Command::Copy, &context), None);
        context.has_selection = true;
        assert_eq!(
            keymap.handle_keystroke_with_context([ctrl_k()], Some(&context)),
            KeyAction::AwaitMore
        );
        let pending = keymap.pending_chord_display();
        assert_eq!(
            keymap.display_for(Command::Copy, &context),
            Some(format!(
                "{} {}",
                ctrl_k().display_string(),
                ctrl_c().display_string()
            ))
        );
        assert_eq!(keymap.pending_chord_display(), pending);
        assert_eq!(
            keymap.handle_keystroke_with_context([ctrl_c()], Some(&context)),
            KeyAction::Execute(Command::Copy)
        );
        keymap.add_binding(Keybinding::new(ctrl_k(), Command::OpenFile));
        assert_eq!(keymap.display_for(Command::Copy, &context), None);
        assert_eq!(keymap.display_for(Command::SaveFile, &context), None);
    }

    #[test]
    fn shortcut_hints_reject_shadowed_and_shorter_chord_prefixes() {
        let context = KeyContext::editor_default();
        let keymap = Keymap::with_bindings(vec![
            Keybinding::chord(vec![ctrl_k(), ctrl_c()], Command::Copy),
            Keybinding::chord(vec![ctrl_k(), ctrl_c()], Command::SaveFile),
            Keybinding::chord(vec![ctrl_k(), ctrl_c(), ctrl_s()], Command::OpenFile),
        ]);
        assert!(keymap.display_for(Command::Copy, &context).is_some());
        assert!(keymap.display_for(Command::SaveFile, &context).is_none());
        assert!(keymap.display_for(Command::OpenFile, &context).is_none());
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
