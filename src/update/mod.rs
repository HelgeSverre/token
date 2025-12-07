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

pub use app::update_app;
pub use document::update_document;
pub use editor::update_editor;
pub use layout::update_layout;
pub use ui::update_ui;

/// Main update function - dispatches to sub-handlers
pub fn update(model: &mut AppModel, msg: Msg) -> Option<Cmd> {
    let result = match msg {
        Msg::Editor(m) => editor::update_editor(model, m),
        Msg::Document(m) => document::update_document(model, m),
        Msg::Ui(m) => ui::update_ui(model, m),
        Msg::Layout(m) => layout::update_layout(model, m),
        Msg::App(m) => app::update_app(model, m),
    };

    // Sync status bar segments after state changes
    sync_status_bar(model);

    result
}
