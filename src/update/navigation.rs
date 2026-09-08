//! General jump-history/navigation primitives (lsp-integration.md Phase
//! 3): the group-tagged back stack and the "open/reuse a tab by path,
//! then place the cursor" mechanism shared by `Command::GotoDefinition`,
//! `Command::NavigateBack`, and (for the clamp helper) outline jumps.
//!
//! Opening captures a target group and typed cursor continuation. Cursor
//! conversion/placement runs only after that destination has been installed.

use std::path::{Path, PathBuf};

use crate::commands::Cmd;
#[cfg(test)]
use crate::messages::LayoutMsg;
use crate::model::{AppModel, JumpEntry, TabContent};

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
/// captures the origin at request time — see `update/lsp.rs`). A *new*
/// jump invalidates the group's forward chain, standard back/forward
/// semantics — `navigate_back`/`navigate_forward` use the raw push
/// instead, since moving along the chain must not clear it.
pub(crate) fn push_history_entry(model: &mut AppModel, entry: JumpEntry) {
    let group = entry.group_id;
    model.forward_history.retain(|e| e.group_id != group);
    push_back_raw(model, entry);
}

fn push_back_raw(model: &mut AppModel, entry: JumpEntry) {
    model.jump_history.push(entry);
    if model.jump_history.len() > JUMP_HISTORY_CAP {
        model.jump_history.remove(0);
    }
}

fn push_forward_raw(model: &mut AppModel, entry: JumpEntry) {
    model.forward_history.push(entry);
    if model.forward_history.len() > JUMP_HISTORY_CAP {
        model.forward_history.remove(0);
    }
}

/// Pushes the focused document's current position (the outline/goto-line/
/// file-finder path — captured at push time since there is no async gap
/// for those).
pub(crate) fn push_history(model: &mut AppModel) {
    if let Some(entry) = current_jump_entry(model) {
        push_history_entry(model, entry);
    }
}

/// CLI/automation coordinates are one-indexed; defer conversion/placement until
/// the actual destination has loaded, just like LSP and history navigation.
pub fn open_path_at(
    model: &mut AppModel,
    path: PathBuf,
    line: Option<usize>,
    column: Option<usize>,
) -> Option<Cmd> {
    let position = line.map(|line| crate::model::OpenPosition::Char {
        line: line.saturating_sub(1),
        column: column.unwrap_or(1).saturating_sub(1),
    });
    super::layout::open_file_in_group(model, path, model.editor_area.focused_group_id, position)
}

/// Complete a navigation against its actual destination, after loading and
/// viewport synchronization. Never borrows the subsequently focused document.
pub(super) fn place_open_cursor(
    model: &mut AppModel,
    editor_id: crate::model::EditorId,
    position: crate::model::OpenPosition,
) {
    let Some(editor) = model.editor_area.editors.get_mut(&editor_id) else {
        return;
    };
    if !matches!(editor.tab_content, TabContent::Text) || editor.view_mode.is_image() {
        return;
    }
    let Some(document) = editor
        .document_id
        .and_then(|id| model.editor_area.documents.get(&id))
    else {
        return;
    };
    let (line, column) = match position {
        crate::model::OpenPosition::Char { line, column } => (line, column),
        crate::model::OpenPosition::Lsp(position) => {
            let position = crate::lsp::lsp_to_position(document, position);
            (position.line, position.column)
        }
    };
    let line = line.min(document.line_count().saturating_sub(1));
    editor.cursors[0].line = line;
    editor.cursors[0].column = column.min(document.line_length(line));
    editor.cursors[0].desired_column = None;
    editor.clear_selection();
    editor.ensure_cursor_visible_with_mode(document, crate::model::ScrollRevealMode::Centered);
}

/// One row of any confirmable location list (usages popup, problems
/// panel, future pickers). Positions remain in LSP UTF-16 coordinates until
/// the destination is available; display conversion is best-effort only.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationItem {
    pub path: PathBuf,
    /// Raw LSP position (UTF-16 column) — converted to editor char
    /// coordinates only at jump time, once the target document is open
    /// (`jump_to_location`). Storing one coordinate space ends the
    /// open-at-build-time ambiguity that misplaced cursors in unopened
    /// multi-byte files.
    pub position: lsp_types::Position,
    pub preview: String,
    /// The server/root that resolved this location, set only when `path`
    /// is outside every workspace root — the same route hint
    /// `DefinitionResolved`'s single-location branch sets before jumping
    /// (lsp-integration.md "never spawn a new server rooted in a
    /// toolchain directory"). `None` inside the workspace, where the
    /// generic open path already derives the right root. Popup/list
    /// activation (`ActivateReference`, the single-item collapse in
    /// `open_location_list_popup`) must set `model.lsp.route_hint` from
    /// this before calling `jump_to_location`.
    pub route_hint: Option<(crate::lsp::LspServerId, PathBuf)>,
}

impl LocationItem {
    /// Best-effort display coordinates (0-based char line/col): converted
    /// through the document when it's open, raw UTF-16 otherwise — the
    /// same best-effort contract as `preview`. Display-only; jumps always
    /// convert post-open via `jump_to_location`.
    pub fn display_position(&self, model: &AppModel) -> (usize, usize) {
        model
            .editor_area
            .find_open_file(&self.path)
            .and_then(|(doc_id, _, _)| model.editor_area.documents.get(&doc_id))
            .map(|doc| {
                let p = crate::lsp::lsp_to_position(doc, self.position);
                (p.line, p.column)
            })
            .unwrap_or((
                self.position.line as usize,
                self.position.character as usize,
            ))
    }
}

/// Location-list activation shares route hints, history and deferred UTF-16
/// placement between popup and persistent usages results.
pub(crate) fn activate_location(model: &mut AppModel, item: &LocationItem) -> Option<Cmd> {
    if let Some((server_id, root)) = &item.route_hint {
        model.lsp.route_hint = Some((item.path.clone(), server_id.clone(), root.clone()));
    }
    jump_to_location(model, None, &item.path, item.position)
}

/// The shared activate handler: push jump history and capture a typed post-open
/// cursor action in the origin group. `origin`: pre-captured entry for async flows
/// (definition/references resolve), or capture-now for sync ones.
/// `target` is a raw LSP position (UTF-16 column), converted against the
/// target document *after* it opens — the only moment conversion is
/// always possible (the pre-refactor code converted correctly only for
/// already-open files and passed raw UTF-16 columns through otherwise).
pub(crate) fn jump_to_location(
    model: &mut AppModel,
    origin: Option<JumpEntry>,
    path: &Path,
    target: lsp_types::Position,
) -> Option<Cmd> {
    let origin = origin.or_else(|| current_jump_entry(model));
    let group_id = origin
        .as_ref()
        .map(|entry| entry.group_id)
        .unwrap_or(model.editor_area.focused_group_id);
    if let Some(entry) = origin {
        push_history_entry(model, entry);
    }
    super::layout::open_file_in_group(
        model,
        path.to_path_buf(),
        group_id,
        Some(crate::model::OpenPosition::Lsp(target)),
    )
}

/// `Command::NavigateBack` / `LspMsg::NavigateBack`: pops the *focused
/// group's* most recent jump-history entry and navigates to it.
/// `document_id` is tried first (it follows a Save-As-renamed document to
/// its current path); `path` is the fallback once that document is
/// closed.
pub(crate) fn navigate_back(model: &mut AppModel) -> Option<Cmd> {
    let focused_group = model.editor_area.focused_group_id;
    let idx = model
        .jump_history
        .iter()
        .rposition(|entry| entry.group_id == focused_group)?;
    let entry = model.jump_history.remove(idx);
    // The position we're leaving becomes forward-navigable.
    if let Some(current) = current_jump_entry(model) {
        push_forward_raw(model, current);
    }
    navigate_to_entry(model, entry)
}

/// `Command::NavigateForward`: the mirror of `navigate_back` — pops the
/// focused group's most recent forward entry, pushing the position being
/// left back onto the back stack (without clearing forward).
pub(crate) fn navigate_forward(model: &mut AppModel) -> Option<Cmd> {
    let focused_group = model.editor_area.focused_group_id;
    let idx = model
        .forward_history
        .iter()
        .rposition(|entry| entry.group_id == focused_group)?;
    let entry = model.forward_history.remove(idx);
    if let Some(current) = current_jump_entry(model) {
        push_back_raw(model, current);
    }
    navigate_to_entry(model, entry)
}

fn navigate_to_entry(model: &mut AppModel, entry: JumpEntry) -> Option<Cmd> {
    let path = model
        .editor_area
        .documents
        .get(&entry.document_id)
        .and_then(|doc| doc.file_path.clone())
        .unwrap_or(entry.path);
    super::layout::open_file_in_group(
        model,
        path,
        entry.group_id,
        Some(crate::model::OpenPosition::Char {
            line: entry.line,
            column: entry.col,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_fixture_layout(model: &mut AppModel, msg: crate::messages::LayoutMsg) -> Option<Cmd> {
        let cmd = crate::update::layout::update_layout(model, msg);
        crate::update::finish_test_file_opens(model, cmd)
    }

    fn jump_fixture(
        model: &mut AppModel,
        origin: Option<JumpEntry>,
        path: &Path,
        position: lsp_types::Position,
    ) {
        let cmd = jump_to_location(model, origin, path, position);
        crate::update::finish_test_file_opens(model, cmd);
    }
    use crate::model::editor_area::GroupId;

    fn model_with_two_files(dir: &std::path::Path) -> AppModel {
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        std::fs::write(&a, "aaa\nbbb\n").unwrap();
        std::fs::write(&b, "ccc\nddd\n").unwrap();
        let mut model =
            AppModel::with_document(800, 600, 1.0, crate::model::Document::from_file(a).unwrap());
        open_fixture_layout(&mut model, LayoutMsg::OpenFileInNewTab(b));
        model
    }

    #[test]
    fn push_history_skips_untitled_documents() {
        let mut model = AppModel::new(800, 600, 1.0);
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
    fn jump_to_location_with_no_origin_captures_the_current_position_for_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = model_with_two_files(dir.path());
        model.editor_mut().cursors[0].line = 1;
        model.editor_mut().cursors[0].column = 2;
        let b_path = model.document().file_path.clone().unwrap();

        let a_path = dir.path().join("a.txt");
        jump_fixture(
            &mut model,
            None,
            &a_path,
            lsp_types::Position {
                line: 0,
                character: 1,
            },
        );

        assert_eq!(
            model.document().file_path.as_deref(),
            Some(a_path.as_path())
        );
        assert_eq!(model.editor().cursors[0].line, 0);
        assert_eq!(model.editor().cursors[0].column, 1);

        // The pre-jump position on b.txt must be in history, same as any
        // other jump.
        navigate_back(&mut model);
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(b_path.as_path())
        );
        assert_eq!(model.editor().cursors[0].line, 1);
        assert_eq!(model.editor().cursors[0].column, 2);
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
        open_fixture_layout(&mut model, LayoutMsg::OpenFileInNewTab(a_path));
        assert_ne!(
            model.document().file_path.as_deref(),
            Some(b_path.as_path())
        );

        navigate_back(&mut model);
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(b_path.as_path())
        );
        assert_eq!(model.editor().cursors[0].line, 1);
        assert_eq!(model.editor().cursors[0].column, 2);
    }

    #[test]
    fn navigate_back_is_a_no_op_when_history_is_empty() {
        let mut model = AppModel::new(800, 600, 1.0);
        assert!(navigate_back(&mut model).is_none());
    }

    #[test]
    fn navigate_back_only_pops_the_focused_groups_entries() {
        let mut model = AppModel::new(800, 600, 1.0);
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

    #[test]
    fn lsp_jump_into_an_unopened_multibyte_file_converts_utf16_columns() {
        // Regression: LocationItem carried raw UTF-16 columns for files
        // not open at build time, and jump_to_location placed them
        // blindly — landing the cursor between/after the wrong chars
        // whenever multi-byte text preceded the target. Conversion now
        // happens after the open, when the document is always available.
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("uni.rs");
        // "a🎉bc": char cols a=0 🎉=1 b=2; UTF-16 units a=0 🎉=1..2 b=3.
        std::fs::write(&target, "a\u{1F389}bc\n").unwrap();
        let mut model = model_with_two_files(dir.path());
        assert!(
            model.editor_area.find_open_file(&target).is_none(),
            "target must not be open before the jump"
        );

        jump_fixture(
            &mut model,
            None,
            &target,
            lsp_types::Position {
                line: 0,
                character: 3, // 'b' in UTF-16 units
            },
        );

        assert_eq!(
            model.document().file_path.as_deref(),
            Some(target.as_path())
        );
        assert_eq!(
            model.editor().cursors[0].column,
            2,
            "UTF-16 col 3 is char col 2 ('b') — raw passthrough would give 3"
        );
    }

    #[test]
    fn forward_returns_after_back_and_new_jumps_clear_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = model_with_two_files(dir.path());
        let origin_path = model.document().file_path.clone().unwrap();

        // Jump: push origin, open a.txt.
        push_history(&mut model);
        let a_path = dir.path().join("a.txt");
        open_fixture_layout(&mut model, LayoutMsg::OpenFileInNewTab(a_path.clone()));

        // Back returns to the origin and arms forward.
        navigate_back(&mut model);
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(origin_path.as_path())
        );
        assert_eq!(model.forward_history.len(), 1);

        // Forward returns to a.txt.
        navigate_forward(&mut model).expect("forward entry should exist");
        assert_eq!(
            model.document().file_path.as_deref(),
            Some(a_path.as_path())
        );
        assert!(model.forward_history.is_empty());

        // Back again, then a NEW jump clears the forward chain.
        navigate_back(&mut model);
        assert_eq!(model.forward_history.len(), 1);
        push_history(&mut model);
        assert!(
            model.forward_history.is_empty(),
            "a new jump must invalidate the forward chain"
        );
        assert!(navigate_forward(&mut model).is_none());
    }
}
