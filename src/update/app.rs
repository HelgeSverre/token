//! App message handlers (file operations, window events)

use crate::commands::Cmd;
use crate::messages::AppMsg;
use crate::model::AppModel;

/// Handle app messages (file operations, window events)
pub fn update_app(model: &mut AppModel, msg: AppMsg) -> Option<Cmd> {
    match msg {
        AppMsg::Resize(width, height) => {
            model.resize(width, height);
            Some(Cmd::Redraw)
        }

        AppMsg::SaveFile => {
            let file_path = model.document().file_path.clone();
            match file_path {
                Some(path) => {
                    let content = model.document().buffer.to_string();
                    model.ui.is_saving = true;
                    model.ui.set_status("Saving...");
                    Some(Cmd::SaveFile { path, content })
                }
                None => {
                    model.ui.set_status("No file path - cannot save");
                    Some(Cmd::Redraw)
                }
            }
        }

        AppMsg::LoadFile(path) => {
            model.ui.is_loading = true;
            model.ui.set_status("Loading...");
            Some(Cmd::LoadFile { path })
        }

        AppMsg::NewFile => {
            // TODO: Implement new file
            model.ui.set_status("New file not yet implemented");
            Some(Cmd::Redraw)
        }

        AppMsg::SaveCompleted(result) => {
            model.ui.is_saving = false;
            match result {
                Ok(_) => {
                    model.document_mut().is_modified = false;
                    if let Some(path) = &model.document().file_path {
                        model.ui.set_status(format!("Saved: {}", path.display()));
                    }
                }
                Err(e) => {
                    model.ui.set_status(format!("Error: {}", e));
                }
            }
            Some(Cmd::Redraw)
        }

        AppMsg::FileLoaded { path, result } => {
            model.ui.is_loading = false;
            match result {
                Ok(content) => {
                    model.document_mut().buffer = ropey::Rope::from(content);
                    model.document_mut().file_path = Some(path.clone());
                    model.document_mut().is_modified = false;
                    model.document_mut().undo_stack.clear();
                    model.document_mut().redo_stack.clear();
                    *model.editor_mut().cursor_mut() = Default::default();
                    model.ui.set_status(format!("Loaded: {}", path.display()));
                }
                Err(e) => {
                    model.ui.set_status(format!("Error: {}", e));
                }
            }
            Some(Cmd::Redraw)
        }

        AppMsg::Quit => {
            // Handled by the event loop
            None
        }
    }
}
