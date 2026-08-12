//! General jump-history/navigation primitives (lsp-integration.md Phase
//! 3): the group-tagged back stack and the "open/reuse a tab by path,
//! then place the cursor" mechanism shared by `Command::GotoDefinition`,
//! `Command::NavigateBack`, and (for the clamp helper) outline jumps.
//!
//! Same-file and cross-file navigation collapse to one code path here:
//! `LayoutMsg::OpenFileInNewTab` already reuses an already-open tab for a
//! given path (`EditorArea::find_open_file`), so there is no separate
//! "fast path" for staying in the current file — it is just the case
//! where the reused tab happens to already be the focused one.

use std::path::PathBuf;

use crate::commands::Cmd;
use crate::messages::LayoutMsg;
use crate::model::{AppModel, JumpEntry, TabContent};

use super::layout::update_layout;

/// Soft cap on the back-stack size. Flat cap + a `Vec::remove(0)` shift
/// when it overflows — fine at this size; swap to a `VecDeque` if a
/// long-running session with heavy navigation ever makes this measurable.
const JUMP_HISTORY_CAP: usize = 200;

/// Clamp a (line, col) position to the *focused* document's bounds.
/// Shared by outline jumps, goto-line, and every navigation helper below
/// — a stale position (from an outline built before an edit, or an LSP
/// response for text that has since changed) must never carry an
/// out-of-range cursor into the document.
pub(crate) fn clamp_to_document(model: &AppModel, line: usize, col: usize) -> (usize, usize) {
    let last_line = model.document().line_count().saturating_sub(1);
    let clamped_line = line.min(last_line);
    let clamped_col = col.min(model.document().line_length(clamped_line));
    (clamped_line, clamped_col)
}

/// Combines two optional `Cmd`s from a two-step navigation (open, then
/// place cursor) into one.
pub(crate) fn combine(a: Option<Cmd>, b: Option<Cmd>) -> Option<Cmd> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Cmd::Batch(vec![a, b])),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Captures the focused document's current position as a `JumpEntry`, or
/// `None` for documents with no file path — untitled/scratch buffers
/// aren't LSP-synced (design doc's "Untitled documents are not synced")
/// and have nothing a `path`-backed back stack could reopen, so they are
/// deliberately excluded from jump history too.
pub(crate) fn current_jump_entry(model: &AppModel) -> Option<JumpEntry> {
    let doc = model.try_document()?;
    let document_id = doc.id?;
    let path = doc.file_path.clone()?;
    // The user's most recently active cursor (multi-cursor editing), same
    // source `ShowHover` uses — `GotoDefinition` previously read
    // `cursors[0]` instead, so a jump could be captured/requested from a
    // different position than the one the user was actually at.
    let cursor = *model.editor().active_cursor();
    Some(JumpEntry {
        group_id: model.editor_area.focused_group_id,
        document_id,
        path,
        line: cursor.line,
        col: cursor.column,
    })
}

/// Pushes an already-built entry (the LSP go-to-definition path, which
/// captures the origin at request time — see `update/lsp.rs`).
pub(crate) fn push_history_entry(model: &mut AppModel, entry: JumpEntry) {
    model.jump_history.push(entry);
    if model.jump_history.len() > JUMP_HISTORY_CAP {
        model.jump_history.remove(0);
    }
}

/// Pushes the focused document's current position (the outline/goto-line/
/// file-finder path — captured at push time since there is no async gap
/// for those).
pub fn push_history(model: &mut AppModel) {
    if let Some(entry) = current_jump_entry(model) {
        push_history_entry(model, entry);
    }
}

/// Opens (or focuses an already-open tab for) `path` in the focused
/// group.
pub(crate) fn open_or_focus(model: &mut AppModel, path: PathBuf) -> Option<Cmd> {
    update_layout(model, LayoutMsg::OpenFileInNewTab(path))
}

/// Whether the now-focused tab is plain text — i.e. not an image or
/// binary placeholder, the design doc's "skip cursor placement" guard.
pub(crate) fn focused_tab_is_text(model: &AppModel) -> bool {
    model
        .try_editor()
        .is_some_and(|e| matches!(e.tab_content, TabContent::Text) && !e.view_mode.is_image())
}

/// Whether the now-focused tab is actually showing `path` — the guard
/// that keeps a failed or redirected open (permission denied, invalid
/// UTF-8, or a reused-in-another-group tab) from letting a caller mistake
/// the *origin* document for the target and move its cursor instead
/// (design doc: "no stale result ever moves a cursor"; `open_or_focus`
/// falling through without changing focus is exactly the case this
/// catches). Paths are canonicalized before comparing — one may come
/// from a decoded LSP URI, the other from `Document.file_path` — falling
/// back to the raw path when canonicalization fails (nonexistent file).
pub(crate) fn focused_tab_shows(model: &AppModel, path: &std::path::Path) -> bool {
    let Some(focused) = model.try_document().and_then(|d| d.file_path.as_deref()) else {
        return false;
    };
    let canonical_focused = focused.canonicalize().unwrap_or_else(|_| focused.to_path_buf());
    let canonical_target = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical_focused == canonical_target
}

/// Clamp + set cursor + center + focus on the currently focused
/// document/editor, in char coordinates — the `JumpToSymbol` body,
/// generalized so LSP jumps and jump-history navigation can reuse it
/// after opening/focusing the target tab. `None` (no-op) for a
/// non-text tab (image/binary placeholder).
pub(crate) fn place_cursor_char(model: &mut AppModel, line: usize, col: usize) -> Option<Cmd> {
    if !focused_tab_is_text(model) {
        return None;
    }
    let (line, col) = clamp_to_document(model, line, col);
    let editor = model.editor_mut();
    editor.cursors[0].line = line;
    editor.cursors[0].column = col;
    editor.cursors[0].desired_column = None;
    editor.clear_selection();
    model.ensure_cursor_visible_centered();
    model.ui.focus_editor();
    Some(Cmd::redraw_editor())
}

/// `Command::NavigateBack` / `LspMsg::NavigateBack`: pops the *focused
/// group's* most recent jump-history entry and navigates to it.
/// `document_id` is tried first (it follows a Save-As-renamed document to
/// its current path); `path` is the fallback once that document is
/// closed.
pub fn navigate_back(model: &mut AppModel) -> Option<Cmd> {
    let focused_group = model.editor_area.focused_group_id;
    let idx = model
        .jump_history
        .iter()
        .rposition(|entry| entry.group_id == focused_group)?;
    let entry = model.jump_history.remove(idx);
    let path = model
        .editor_area
        .documents
        .get(&entry.document_id)
        .and_then(|doc| doc.file_path.clone())
        .unwrap_or(entry.path);
    let open_cmd = open_or_focus(model, path.clone());
    let cursor_cmd = if focused_tab_shows(model, &path) {
        place_cursor_char(model, entry.line, entry.col)
    } else {
        None
    };
    Some(combine(open_cmd, cursor_cmd).unwrap_or(Cmd::Redraw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::editor_area::GroupId;

    fn model_with_two_files(dir: &std::path::Path) -> AppModel {
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        std::fs::write(&a, "aaa\nbbb\n").unwrap();
        std::fs::write(&b, "ccc\nddd\n").unwrap();
        let mut model = AppModel::new(800, 600, 1.0, vec![a]);
        update_layout(&mut model, LayoutMsg::OpenFileInNewTab(b));
        model
    }

    #[test]
    fn push_history_skips_untitled_documents() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        assert!(model.try_document().unwrap().file_path.is_none());
        push_history(&mut model);
        assert!(model.jump_history.is_empty());
    }

    /// Multi-cursor editing: `current_jump_entry` (and by extension
    /// `Command::GotoDefinition`, which shares this source) must capture
    /// the *active* cursor, not `cursors[0]` — otherwise a jump/request
    /// issued with a secondary cursor active would silently use the
    /// primary cursor's position instead of where the user actually was.
    #[test]
    fn current_jump_entry_uses_the_active_cursor_not_the_primary_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = model_with_two_files(dir.path());
        model.editor_mut().cursors[0].line = 0;
        model.editor_mut().cursors[0].column = 0;
        model.editor_mut().add_cursor_at(1, 2);
        assert_eq!(model.editor().active_cursor_index, 1);

        let entry = current_jump_entry(&model).unwrap();

        assert_eq!(
            (entry.line, entry.col),
            (1, 2),
            "must capture the active (secondary) cursor's position, not cursors[0]"
        );
    }

    #[test]
    fn navigate_back_returns_to_the_previous_file_and_position() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = model_with_two_files(dir.path());
        // Focused on b.txt (opened last); move the cursor before jumping.
        model.editor_mut().cursors[0].line = 1;
        model.editor_mut().cursors[0].column = 2;
        let b_path = model.document().file_path.clone().unwrap();
        push_history(&mut model);

        // Jump away to a.txt.
        let a_path = dir.path().join("a.txt");
        update_layout(&mut model, LayoutMsg::OpenFileInNewTab(a_path));
        assert_ne!(model.document().file_path.as_deref(), Some(b_path.as_path()));

        navigate_back(&mut model);
        assert_eq!(model.document().file_path.as_deref(), Some(b_path.as_path()));
        assert_eq!(model.editor().cursors[0].line, 1);
        assert_eq!(model.editor().cursors[0].column, 2);
    }

    #[test]
    fn navigate_back_is_a_no_op_when_history_is_empty() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        assert!(navigate_back(&mut model).is_none());
    }

    #[test]
    fn navigate_back_only_pops_the_focused_groups_entries() {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.jump_history.push(JumpEntry {
            group_id: GroupId(999), // some other, non-focused group
            document_id: model.document().id.unwrap(),
            path: PathBuf::from("/nonexistent/other.txt"),
            line: 0,
            col: 0,
        });
        assert!(navigate_back(&mut model).is_none());
        assert_eq!(model.jump_history.len(), 1); // untouched
    }
}
