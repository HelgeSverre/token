//! Edit constraints for the unified text editing system.
//!
//! Constraints define what operations are allowed in different editing contexts.

/// Character filter function type
pub type CharFilter = fn(char) -> bool;

/// Constraints that limit what operations are allowed in an editing context.
#[derive(Debug, Clone)]
pub struct EditConstraints {
    /// Allow multiple lines (Enter inserts newline vs confirms)
    pub allow_multiline: bool,

    /// Allow multiple cursors
    pub allow_multi_cursor: bool,

    /// Allow text selection
    pub allow_selection: bool,

    /// Enable undo/redo tracking
    pub enable_undo: bool,

    /// Maximum length in characters (None = unlimited)
    pub max_length: Option<usize>,

    /// Character filter (None = all characters allowed)
    /// Returns true if character is allowed
    pub char_filter: Option<CharFilter>,
}

impl Default for EditConstraints {
    fn default() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }
}

impl EditConstraints {
    /// Full editor constraints (all features enabled)
    pub fn editor() -> Self {
        Self {
            allow_multiline: true,
            allow_multi_cursor: true,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }

    /// Single-line input constraints (command palette, find query)
    pub fn single_line() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }

    /// Numeric input constraints (digits only)
    pub fn numeric() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: Some(10), // Max reasonable line number
            char_filter: Some(|c| c.is_ascii_digit()),
        }
    }

    /// Go to line constraints (digits and colon for line:col format)
    pub fn goto_line() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: Some(20), // line:col format
            char_filter: Some(|c| c.is_ascii_digit() || c == ':'),
        }
    }

    /// CSV cell constraints (single-line, no multi-cursor)
    pub fn csv_cell() -> Self {
        Self {
            allow_multiline: false,
            allow_multi_cursor: false,
            allow_selection: true,
            enable_undo: true,
            max_length: None,
            char_filter: None,
        }
    }

    /// Check if a character passes the filter (if any)
    pub fn is_char_allowed(&self, ch: char) -> bool {
        match self.char_filter {
            Some(filter) => filter(ch),
            None => true,
        }
    }

    /// Check if inserting text would exceed max length
    pub fn would_exceed_max_length(&self, current_len: usize, insert_len: usize) -> bool {
        if let Some(max) = self.max_length {
            current_len + insert_len > max
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_constraints() {
        let c = EditConstraints::editor();
        assert!(c.allow_multiline);
        assert!(c.allow_multi_cursor);
        assert!(c.allow_selection);
        assert!(c.enable_undo);
        assert!(c.is_char_allowed('a'));
        assert!(c.is_char_allowed('\n'));
    }

    #[test]
    fn test_single_line_constraints() {
        let c = EditConstraints::single_line();
        assert!(!c.allow_multiline);
        assert!(!c.allow_multi_cursor);
        assert!(c.allow_selection);
    }

    #[test]
    fn test_numeric_constraints() {
        let c = EditConstraints::numeric();
        assert!(c.is_char_allowed('0'));
        assert!(c.is_char_allowed('9'));
        assert!(!c.is_char_allowed('a'));
        assert!(!c.is_char_allowed('-'));
    }

    #[test]
    fn test_max_length() {
        let c = EditConstraints::numeric();
        assert!(!c.would_exceed_max_length(5, 3));
        assert!(c.would_exceed_max_length(8, 5));
    }
}
