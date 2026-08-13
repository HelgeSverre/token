//! Update functions for the Elm-style architecture
//!
//! All state transformations flow through these functions.

mod app;
mod completion;
pub mod context_menu;
mod csv;
mod dock;
mod document;
mod editor;
mod image;
pub mod layout;
mod lsp;
pub mod navigation;
pub mod outline;
mod preview;
pub mod problems;
mod syntax;
mod terminal;
mod text_edit;
mod ui;
mod workspace;

use crate::commands::Cmd;
use crate::messages::{CsvMsg, Direction, DocumentMsg, EditorMsg, Msg};
use crate::model::sync_status_bar;
use crate::model::AppModel;
use crate::util::text::{char_type, CharType};

#[cfg(debug_assertions)]
use crate::tracing::CursorSnapshot;
#[cfg(debug_assertions)]
use tracing::{debug, span, Level};

pub use app::{create_default_keymap_file, execute_command, update_app};
pub use completion::update_completion;
pub use context_menu::update_context_menu;
pub use csv::update_csv;
pub use dock::update_dock;
pub use document::update_document;
pub use editor::update_editor;
pub use layout::update_layout;
pub use lsp::{
    close_lsp_document, open_lsp_document, save_lsp_document, schedule_lsp_did_change, update_lsp,
};
pub use outline::update_outline;
pub use preview::update_preview;
pub use syntax::{schedule_syntax_parse, update_syntax, SYNTAX_DEBOUNCE_MS};
pub use terminal::update_terminal;
pub use text_edit::{apply_text_edit_msg, update_text_edit};
pub use ui::{resolve_palette_rows, search_everywhere_sections, update_ui, ALL_TAB_GROUP_CAP};
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
            // Any cursor/selection movement invalidates the completion
            // query (autocomplete.md dismiss rule: "cursor line change").
            // Up/Down/Enter/Tab/Escape never reach here while the menu is
            // open — they're claimed by the pre-keymap cursor-overlay
            // branch in runtime/input.rs before an `EditorMsg` is ever
            // built, so this only fires for keys that should dismiss. Most
            // editor messages already redraw, but some (e.g. a no-op
            // cursor move) return `None`, so the dismiss needs its own
            // redraw merged in or the now-stale popup stays on screen.
            let was_dismissed = completion::dismiss(model);
            let redraw_for_dismiss = was_dismissed.then(Cmd::redraw_editor);

            // Block editor messages in image mode and binary placeholder mode
            let is_non_text = model.editor_area.focused_editor().is_some_and(|e| {
                e.view_mode.is_image()
                    || matches!(
                        e.tab_content,
                        crate::model::editor::TabContent::BinaryPlaceholder(_)
                    )
            });
            if is_non_text {
                return redraw_for_dismiss;
            }

            // When in CSV mode, intercept navigation messages and route to CSV
            let csv_info = model
                .editor_area
                .focused_editor()
                .and_then(|e| e.view_mode.as_csv().map(|csv| (true, csv.is_editing())));

            if let Some((true, is_editing)) = csv_info {
                if let Some(csv_msg) = map_editor_to_csv(&m, is_editing) {
                    return merge_cmds(csv::update_csv(model, csv_msg), redraw_for_dismiss);
                }
                return redraw_for_dismiss;
            }
            merge_cmds(editor::update_editor(model, m), redraw_for_dismiss)
        }
        Msg::Document(m) => {
            // Block document messages in image mode and binary placeholder mode
            let is_non_text = model.editor_area.focused_editor().is_some_and(|e| {
                e.view_mode.is_image()
                    || matches!(
                        e.tab_content,
                        crate::model::editor::TabContent::BinaryPlaceholder(_)
                    )
            });
            if is_non_text {
                return None;
            }

            // When in CSV mode, intercept document messages for cell editing
            let csv_info = model
                .editor_area
                .focused_editor()
                .and_then(|e| e.view_mode.as_csv().map(|csv| (true, csv.is_editing())));

            if let Some((true, is_editing)) = csv_info {
                if let Some(csv_msg) = map_document_to_csv(&m, is_editing) {
                    return csv::update_csv(model, csv_msg);
                }
                return None;
            }
            // Only these two facts of `m` matter to the completion sync
            // below; read them before `m` moves into `update_document`
            // rather than cloning the whole message (cheap for most
            // variants, but `InsertText(String)` carries a full paste/IME
            // payload that a clone would copy for nothing).
            let is_copy = matches!(m, DocumentMsg::Copy);
            let opens_on_word_char =
                matches!(&m, DocumentMsg::InsertChar(ch) if char_type(*ch) == CharType::WordChar);
            let result = document::update_document(model, m);
            let completion_cmd =
                completion::sync_after_document_edit(model, is_copy, opens_on_word_char);
            merge_cmds(result, completion_cmd)
        }
        Msg::Ui(m) => ui::update_ui(model, m),
        Msg::Layout(m) => layout::update_layout(model, m),
        Msg::App(m) => app::update_app(model, m),
        Msg::Syntax(m) => syntax::update_syntax(model, m),
        Msg::Csv(m) => csv::update_csv(model, m),
        Msg::Image(m) => image::update_image(model, m),
        Msg::Preview(m) => preview::update_preview(model, m),
        Msg::Workspace(m) => workspace::update_workspace(model, m),
        Msg::Dock(m) => dock::update_dock(model, m),
        Msg::Outline(m) => outline::update_outline(model, m),
        Msg::Problems(m) => problems::update_problems(model, m),
        Msg::TextEdit(context, m) => text_edit::update_text_edit(model, context, m),
        Msg::Terminal(m) => terminal::update_terminal(model, m),
        Msg::Completion(m) => completion::update_completion(model, m),
        Msg::Lsp(m) => lsp::update_lsp(model, m),
        Msg::ContextMenu(m) => context_menu::update_context_menu(model, m),
    };

    // Any focus change away from the editor (modal open, dock/sidebar
    // focus, ...) must dismiss the completion popup: `cursor_overlay` is
    // claimed pre-keymap purely on `.is_some()` regardless of focus
    // (runtime/app.rs), so a still-open menu would hijack Up/Down/Enter/Tab
    // meant for whatever now actually has focus. This single check covers
    // every message that can move focus (`Msg::Ui` modals, `Msg::Dock`,
    // `Msg::App`, ...) instead of chasing each one individually.
    let result =
        if model.ui.focus != crate::model::FocusTarget::Editor && completion::dismiss(model) {
            merge_cmds(result, Some(Cmd::redraw_editor()))
        } else {
            result
        };

    sync_status_bar(model);
    result
}

/// Combine two independently-produced `Cmd`s into one, batching when both
/// are present. Used where a message can both perform its own effect (e.g.
/// document edit + redraw/syntax-parse) and trigger a side effect elsewhere
/// (e.g. the completion menu opening/closing) that also wants to redraw.
fn merge_cmds(a: Option<Cmd>, b: Option<Cmd>) -> Option<Cmd> {
    match (a, b) {
        (Some(a), Some(b)) => Some(Cmd::Batch(vec![a, b])),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
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

    let result = update_inner(model, msg);

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
        Msg::Image(m) => format!("Image::{:?}", m),
        Msg::Preview(m) => format!("Preview::{:?}", m),
        Msg::Workspace(m) => format!("Workspace::{:?}", m),
        Msg::Dock(m) => format!("Dock::{:?}", m),
        Msg::Outline(m) => format!("Outline::{:?}", m),
        Msg::Problems(m) => format!("Problems::{:?}", m),
        Msg::TextEdit(ctx, m) => format!("TextEdit::{:?}::{:?}", ctx, m),
        Msg::Terminal(m) => format!("Terminal::{:?}", m),
        Msg::Completion(m) => format!("Completion::{:?}", m),
        Msg::Lsp(m) => format!("Lsp::{:?}", m),
        Msg::ContextMenu(m) => format!("ContextMenu::{:?}", m),
    }
}
