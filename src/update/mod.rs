//! Update functions for the Elm-style architecture
//!
//! All state transformations flow through these functions.

mod app;
mod document;
mod editor;
mod layout;
mod ui;

use crate::commands::Cmd;
use crate::messages::Msg;
use crate::model::sync_status_bar;
use crate::model::AppModel;

#[cfg(debug_assertions)]
use crate::tracing::CursorSnapshot;
#[cfg(debug_assertions)]
use tracing::{debug, span, Level};

pub use app::update_app;
pub use document::update_document;
pub use editor::update_editor;
pub use layout::update_layout;
pub use ui::update_ui;

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
        Msg::Editor(m) => editor::update_editor(model, m),
        Msg::Document(m) => document::update_document(model, m),
        Msg::Ui(m) => ui::update_ui(model, m),
        Msg::Layout(m) => layout::update_layout(model, m),
        Msg::App(m) => app::update_app(model, m),
    };

    sync_status_bar(model);
    result
}

/// Traced update wrapper (debug builds only)
///
/// Captures before/after cursor state and logs diffs for debugging.
#[cfg(debug_assertions)]
fn update_traced(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let msg_name = msg_type_name(&msg);
    let _span = span!(Level::DEBUG, "update", msg = %msg_name).entered();

    let before = model.focused_editor().map(CursorSnapshot::from_editor);

    debug!(target: "message", msg = %msg_name, "processing");

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

    if let Some(ref mut overlay) = model.debug_overlay {
        overlay.record_message(msg_name.clone(), diff);
    }

    result
}

/// Get a display name for a message type
#[cfg(debug_assertions)]
fn msg_type_name(msg: &Msg) -> String {
    match msg {
        Msg::Editor(m) => format!("Editor::{:?}", std::mem::discriminant(m)),
        Msg::Document(m) => format!("Document::{:?}", std::mem::discriminant(m)),
        Msg::Ui(m) => format!("Ui::{:?}", std::mem::discriminant(m)),
        Msg::Layout(m) => format!("Layout::{:?}", std::mem::discriminant(m)),
        Msg::App(m) => format!("App::{:?}", std::mem::discriminant(m)),
    }
}
