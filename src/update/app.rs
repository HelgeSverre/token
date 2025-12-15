//! App message handlers (file operations, window events)

use crate::commands::{Cmd, CommandId};
use crate::config_paths;
use crate::keymap::get_default_keymap_yaml;
use crate::messages::{AppMsg, DocumentMsg, LayoutMsg, UiMsg};
use crate::model::{AppModel, ModalId, SplitDirection};

use super::{update_document, update_layout, update_ui};

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
                    *model.editor_mut().primary_cursor_mut() = Default::default();
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

/// Execute a command from the command palette
pub fn execute_command(model: &mut AppModel, cmd_id: CommandId) -> Option<Cmd> {
    match cmd_id {
        CommandId::NewFile => update_layout(model, LayoutMsg::NewTab),
        CommandId::SaveFile => update_app(model, AppMsg::SaveFile),
        CommandId::Undo => update_document(model, DocumentMsg::Undo),
        CommandId::Redo => update_document(model, DocumentMsg::Redo),
        CommandId::Cut => update_document(model, DocumentMsg::Cut),
        CommandId::Copy => update_document(model, DocumentMsg::Copy),
        CommandId::Paste => update_document(model, DocumentMsg::Paste),
        CommandId::SelectAll => {
            // SelectAll is an EditorMsg, so we need to dispatch through update
            crate::update::update_editor(model, crate::messages::EditorMsg::SelectAll)
        }
        CommandId::GotoLine => update_ui(model, UiMsg::ToggleModal(ModalId::GotoLine)),
        CommandId::SplitHorizontal => {
            update_layout(model, LayoutMsg::SplitFocused(SplitDirection::Horizontal))
        }
        CommandId::SplitVertical => {
            update_layout(model, LayoutMsg::SplitFocused(SplitDirection::Vertical))
        }
        CommandId::CloseGroup => update_layout(model, LayoutMsg::CloseFocusedGroup),
        CommandId::NextTab => update_layout(model, LayoutMsg::NextTab),
        CommandId::PrevTab => update_layout(model, LayoutMsg::PrevTab),
        CommandId::CloseTab => update_layout(model, LayoutMsg::CloseFocusedTab),
        CommandId::Find => update_ui(model, UiMsg::ToggleModal(ModalId::FindReplace)),
        CommandId::ShowCommandPalette => {
            update_ui(model, UiMsg::ToggleModal(ModalId::CommandPalette))
        }
        CommandId::SwitchTheme => update_ui(model, UiMsg::ToggleModal(ModalId::ThemePicker)),
        CommandId::OpenConfigDirectory => {
            if let Some(config_dir) = config_paths::config_dir() {
                config_paths::ensure_all_config_dirs();
                Some(Cmd::OpenInExplorer { path: config_dir })
            } else {
                model.ui.set_status("Could not determine config directory");
                Some(Cmd::Redraw)
            }
        }
        CommandId::OpenKeybindings => {
            if let Some(keymap_path) = config_paths::keymap_file() {
                config_paths::ensure_all_config_dirs();
                if !keymap_path.exists() {
                    if let Err(e) = create_default_keymap_file(&keymap_path) {
                        model.ui.set_status(format!("Failed to create keymap: {}", e));
                        return Some(Cmd::Redraw);
                    }
                }
                Some(Cmd::OpenFileInEditor { path: keymap_path })
            } else {
                model.ui.set_status("Could not determine keymap path");
                Some(Cmd::Redraw)
            }
        }
    }
}

fn create_default_keymap_file(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    std::fs::write(path, get_default_keymap_yaml())
        .map_err(|e| format!("Failed to write file: {}", e))
}
