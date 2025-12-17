//! Context system for conditional keybindings
//!
//! Enables bindings that only activate under certain conditions,
//! such as "Tab indents when there's a selection".

use serde::Deserialize;

/// Context extracted from the application model for keybinding evaluation
#[derive(Debug, Clone, Default)]
pub struct KeyContext {
    /// Whether there's an active text selection
    pub has_selection: bool,
    /// Whether multiple cursors are active
    pub has_multiple_cursors: bool,
    /// Whether a modal dialog is open
    pub modal_active: bool,
    /// Whether the editor (vs modal input) has focus
    pub editor_focused: bool,
    /// Whether the sidebar file tree has focus
    pub sidebar_focused: bool,
}

impl KeyContext {
    /// Create context indicating editor is focused with no special state
    pub fn editor_default() -> Self {
        Self {
            has_selection: false,
            has_multiple_cursors: false,
            modal_active: false,
            editor_focused: true,
            sidebar_focused: false,
        }
    }

    /// Create context for when a modal is active
    pub fn modal() -> Self {
        Self {
            has_selection: false,
            has_multiple_cursors: false,
            modal_active: true,
            editor_focused: false,
            sidebar_focused: false,
        }
    }
}

/// Conditions that can be attached to keybindings
///
/// Multiple conditions on a binding are ANDed together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    /// Binding only active when text is selected
    HasSelection,
    /// Binding only active when no text is selected
    NoSelection,
    /// Binding only active with multiple cursors
    HasMultipleCursors,
    /// Binding only active with single cursor
    SingleCursor,
    /// Binding only active when a modal is open
    ModalActive,
    /// Binding only active when no modal is open
    ModalInactive,
    /// Binding only active when editor has focus
    EditorFocused,
    /// Binding only active when sidebar has focus
    SidebarFocused,
}

impl Condition {
    /// Evaluate this condition against the current context
    pub fn evaluate(self, ctx: &KeyContext) -> bool {
        match self {
            Condition::HasSelection => ctx.has_selection,
            Condition::NoSelection => !ctx.has_selection,
            Condition::HasMultipleCursors => ctx.has_multiple_cursors,
            Condition::SingleCursor => !ctx.has_multiple_cursors,
            Condition::ModalActive => ctx.modal_active,
            Condition::ModalInactive => !ctx.modal_active,
            Condition::EditorFocused => ctx.editor_focused,
            Condition::SidebarFocused => ctx.sidebar_focused,
        }
    }

    /// Evaluate all conditions (AND logic)
    pub fn evaluate_all(conditions: &[Condition], ctx: &KeyContext) -> bool {
        conditions.iter().all(|c| c.evaluate(ctx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_context() {
        let ctx = KeyContext::default();
        assert!(!ctx.has_selection);
        assert!(!ctx.has_multiple_cursors);
        assert!(!ctx.modal_active);
        assert!(!ctx.editor_focused);
    }

    #[test]
    fn test_editor_default_context() {
        let ctx = KeyContext::editor_default();
        assert!(!ctx.has_selection);
        assert!(ctx.editor_focused);
    }

    #[test]
    fn test_condition_has_selection() {
        let mut ctx = KeyContext::default();
        assert!(!Condition::HasSelection.evaluate(&ctx));
        assert!(Condition::NoSelection.evaluate(&ctx));

        ctx.has_selection = true;
        assert!(Condition::HasSelection.evaluate(&ctx));
        assert!(!Condition::NoSelection.evaluate(&ctx));
    }

    #[test]
    fn test_condition_multi_cursor() {
        let mut ctx = KeyContext::default();
        assert!(!Condition::HasMultipleCursors.evaluate(&ctx));
        assert!(Condition::SingleCursor.evaluate(&ctx));

        ctx.has_multiple_cursors = true;
        assert!(Condition::HasMultipleCursors.evaluate(&ctx));
        assert!(!Condition::SingleCursor.evaluate(&ctx));
    }

    #[test]
    fn test_condition_modal() {
        let ctx = KeyContext::modal();
        assert!(Condition::ModalActive.evaluate(&ctx));
        assert!(!Condition::ModalInactive.evaluate(&ctx));
        assert!(!Condition::EditorFocused.evaluate(&ctx));
    }

    #[test]
    fn test_evaluate_all_empty() {
        let ctx = KeyContext::default();
        assert!(Condition::evaluate_all(&[], &ctx));
    }

    #[test]
    fn test_evaluate_all_and_logic() {
        let mut ctx = KeyContext::editor_default();
        ctx.has_selection = true;

        // Both conditions true
        let conditions = vec![Condition::HasSelection, Condition::EditorFocused];
        assert!(Condition::evaluate_all(&conditions, &ctx));

        // One condition false
        let conditions = vec![Condition::HasSelection, Condition::HasMultipleCursors];
        assert!(!Condition::evaluate_all(&conditions, &ctx));
    }
}
