//! Shared ownership for delayed and visible documentation.

use super::{AppModel, DocumentId, EditorId, FocusTarget, Position};

/// The editor interaction that a documentation request belongs to. Comparing
/// this at both dwell and response time also covers split panes of the same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoverAnchor {
    pub document_id: DocumentId,
    pub revision: u64,
    editor_id: EditorId,
    caret: Position,
    selection: (Position, Position),
    scroll: (u64, u64),
}

impl HoverAnchor {
    pub fn capture(model: &AppModel) -> Option<Self> {
        if model.ui.focus != FocusTarget::Editor || model.ui.has_modal() {
            return None;
        }
        let editor = model.editor_area.focused_editor()?;
        if !editor.is_plain_text_mode() {
            return None;
        }
        let document = model.try_document()?;
        let selection = editor.active_selection();
        let (x, y) = editor.pixel_scroll_position();
        Some(Self {
            document_id: document.id?,
            revision: document.revision,
            editor_id: editor.id?,
            caret: editor.active_cursor().to_position(),
            selection: (selection.anchor, selection.head),
            scroll: (x.to_bits(), y.to_bits()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverOrigin {
    Mouse,
    Keyboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoverRequest {
    pub anchor: HoverAnchor,
    pub position: Position,
    pub origin: HoverOrigin,
}

/// Preserve the actual hovered cell (and diagnostics at that cell), while
/// treating movement within one word as the same intent. Unlike moving the
/// anchor to the word's start, this also works for partially scrolled/wrapped words.
pub fn same_hover_target(document: &super::Document, a: Position, b: Position) -> bool {
    if a == b {
        return true;
    }
    if a.line != b.line {
        return false;
    }
    use crate::util::text::{char_type, CharType};
    document
        .buffer
        .get_line(a.line)
        .and_then(|line| line.get_slice(a.column.min(b.column)..=a.column.max(b.column)))
        .is_some_and(|text| text.chars().all(|ch| char_type(ch) == CharType::WordChar))
}

/// Mouse documentation never competes with an editing/selection interaction or
/// another documentation surface. Explicit quick documentation bypasses this.
pub fn mouse_hover_allowed(model: &AppModel) -> bool {
    model.config.hover_on_mouse
        && model.ui.cursor_overlay.is_none()
        && model.ui.signature_help.is_none()
        && HoverAnchor::capture(model).is_some()
        && model.editor().cursors.len() == 1
        && model.editor().active_selection().is_empty()
}
