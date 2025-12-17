//! Update functions for the Elm-style architecture
//!
//! All state transformations flow through these functions.

mod app;
mod csv;
mod document;
mod editor;
pub mod layout;
mod syntax;
mod ui;
mod workspace;

use crate::commands::Cmd;
use crate::messages::{CsvMsg, Direction, DocumentMsg, EditorMsg, Msg};
use crate::model::sync_status_bar;
use crate::model::AppModel;

#[cfg(debug_assertions)]
use crate::tracing::CursorSnapshot;
#[cfg(debug_assertions)]
use tracing::{debug, span, Level};

pub use app::update_app;
pub use csv::update_csv;
pub use document::update_document;
pub use editor::update_editor;
pub use layout::update_layout;
pub use syntax::{schedule_syntax_parse, update_syntax, SYNTAX_DEBOUNCE_MS};
pub use ui::update_ui;
pub use workspace::update_workspace;

/// Main update function - dispatches to sub-handlers
///
/// In debug builds, this wraps with tracing instrumentation.
/// In release builds, it's a direct dispatch with zero overhead.
#[inline]
pub fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    #[cfg(debug_assertions)]
    {
        update_traced(model, msg)
    }
    #[cfg(not(debug_assertions))]
    {
        update_inner(model, msg)
    }
}

/// Inner update logic (no tracing)
fn update_inner(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let result = match msg {
        Msg::Editor(m) => {
            // When in CSV mode, intercept navigation messages and route to CSV
            let csv_info = model
                .editor_area
                .focused_editor()
                .and_then(|e| e.view_mode.as_csv().map(|csv| (true, csv.is_editing())));

            if let Some((true, is_editing)) = csv_info {
                if let Some(csv_msg) = map_editor_to_csv(&m, is_editing) {
                    return csv::update_csv(model, csv_msg);
                }
                // For other editor messages in CSV mode, ignore them
                return None;
            }
            editor::update_editor(model, m)
        }
        Msg::Document(m) => {
            // When in CSV mode, intercept document messages for cell editing
            let csv_info = model
                .editor_area
                .focused_editor()
                .and_then(|e| e.view_mode.as_csv().map(|csv| (true, csv.is_editing())));

            if let Some((true, is_editing)) = csv_info {
                if let Some(csv_msg) = map_document_to_csv(&m, is_editing) {
                    return csv::update_csv(model, csv_msg);
                }
                // Block other document messages in CSV mode
                return None;
            }
            document::update_document(model, m)
        }
        Msg::Ui(m) => ui::update_ui(model, m),
        Msg::Layout(m) => layout::update_layout(model, m),
        Msg::App(m) => app::update_app(model, m),
        Msg::Syntax(m) => syntax::update_syntax(model, m),
        Msg::Csv(m) => csv::update_csv(model, m),
        Msg::Workspace(m) => workspace::update_workspace(model, m),
    };

    sync_status_bar(model);
    result
}

/// Map text editor movement messages to CSV navigation messages
///
/// When not editing: arrows move cell selection
/// When editing: left/right move cursor in cell, up/down confirm and navigate
fn map_editor_to_csv(editor_msg: &EditorMsg, is_editing: bool) -> Option<CsvMsg> {
    match (editor_msg, is_editing) {
        // When editing, left/right move cursor within cell
        (EditorMsg::MoveCursor(Direction::Left), true) => Some(CsvMsg::EditCursorLeft),
        (EditorMsg::MoveCursor(Direction::Right), true) => Some(CsvMsg::EditCursorRight),
        // When editing, up/down confirm edit and navigate
        (EditorMsg::MoveCursor(Direction::Up), true) => Some(CsvMsg::ConfirmEditUp),
        (EditorMsg::MoveCursor(Direction::Down), true) => Some(CsvMsg::ConfirmEdit),
        // When editing, Home/End move cursor within cell
        (EditorMsg::MoveCursorLineStart, true) => Some(CsvMsg::EditCursorHome),
        (EditorMsg::MoveCursorLineEnd, true) => Some(CsvMsg::EditCursorEnd),

        // When not editing, standard cell navigation
        (EditorMsg::MoveCursor(Direction::Up), false) => Some(CsvMsg::MoveUp),
        (EditorMsg::MoveCursor(Direction::Down), false) => Some(CsvMsg::MoveDown),
        (EditorMsg::MoveCursor(Direction::Left), false) => Some(CsvMsg::MoveLeft),
        (EditorMsg::MoveCursor(Direction::Right), false) => Some(CsvMsg::MoveRight),
        (EditorMsg::MoveCursorLineStart, false) => Some(CsvMsg::RowStart),
        (EditorMsg::MoveCursorLineEnd, false) => Some(CsvMsg::RowEnd),
        (EditorMsg::MoveCursorDocumentStart, _) => Some(CsvMsg::FirstCell),
        (EditorMsg::MoveCursorDocumentEnd, _) => Some(CsvMsg::LastCell),
        (EditorMsg::PageUp, _) => Some(CsvMsg::PageUp),
        (EditorMsg::PageDown, _) => Some(CsvMsg::PageDown),
        _ => None,
    }
}

/// Map document messages to CSV cell editing messages
///
/// When not editing: InsertNewline starts editing, InsertChar starts with that char
/// When editing: InsertNewline confirms edit, InsertChar inserts into buffer
fn map_document_to_csv(doc_msg: &DocumentMsg, is_editing: bool) -> Option<CsvMsg> {
    match (doc_msg, is_editing) {
        (DocumentMsg::InsertNewline, false) => Some(CsvMsg::StartEditing),
        (DocumentMsg::InsertNewline, true) => Some(CsvMsg::ConfirmEdit),
        (DocumentMsg::InsertChar(ch), false) => Some(CsvMsg::StartEditingWithChar(*ch)),
        (DocumentMsg::InsertChar(ch), true) => Some(CsvMsg::EditInsertChar(*ch)),
        (DocumentMsg::DeleteBackward, true) => Some(CsvMsg::EditDeleteBackward),
        (DocumentMsg::DeleteForward, true) => Some(CsvMsg::EditDeleteForward),
        _ => None,
    }
}

/// Traced update wrapper (debug builds only)
///
/// Captures before/after cursor state and logs diffs for debugging.
/// Filters out noisy messages like BlinkCursor from logging.
#[cfg(debug_assertions)]
fn update_traced(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    use crate::messages::UiMsg;

    // Skip logging for noisy periodic messages
    let is_noisy = matches!(&msg, Msg::Ui(UiMsg::BlinkCursor));

    let msg_name = msg_type_name(&msg);
    let _span = if is_noisy {
        None
    } else {
        Some(span!(Level::DEBUG, "update", msg = %msg_name).entered())
    };

    let before = model.focused_editor().map(CursorSnapshot::from_editor);

    if !is_noisy {
        debug!(target: "message", msg = %msg_name, "processing");
    }

    let result = update_inner(model, msg.clone());

    let diff = if let (Some(ref before), Some(editor)) = (&before, model.focused_editor()) {
        let after = CursorSnapshot::from_editor(editor);
        let d = before.diff(&after);
        if let Some(ref diff) = d {
            debug!(target: "cursor", %diff, "state changed");
        }
        d
    } else {
        None
    };

    if let Some(editor) = model.focused_editor() {
        editor.assert_invariants_with_context(&msg_name);
    }

    if !is_noisy {
        if let Some(ref mut overlay) = model.debug_overlay {
            overlay.record_message(msg_name.clone(), diff);
        }
    }

    result
}

/// Get a display name for a message type
///
/// Uses Debug formatting to include variant names and arguments.
/// Example outputs:
/// - `Editor::MoveCursor(Up)`
/// - `Document::InsertChar('x')`
/// - `App::Resize(1920, 1080)`
#[cfg(debug_assertions)]
fn msg_type_name(msg: &Msg) -> String {
    match msg {
        Msg::Editor(m) => format!("Editor::{:?}", m),
        Msg::Document(m) => format!("Document::{:?}", m),
        Msg::Ui(m) => format!("Ui::{:?}", m),
        Msg::Layout(m) => format!("Layout::{:?}", m),
        Msg::App(m) => format!("App::{:?}", m),
        Msg::Syntax(m) => format!("Syntax::{:?}", m),
        Msg::Csv(m) => format!("Csv::{:?}", m),
        Msg::Workspace(m) => format!("Workspace::{:?}", m),
    }
}
