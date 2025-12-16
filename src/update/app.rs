//! App message handlers (file operations, window events)

use std::path::PathBuf;

use crate::commands::{Cmd, CommandId};
use crate::config_paths;
use crate::keymap::get_default_keymap_yaml;
use crate::messages::{AppMsg, DocumentMsg, LayoutMsg, UiMsg};
use crate::model::{AppModel, ModalId, SplitDirection};
use crate::syntax::LanguageId;

use super::{update_document, update_layout, update_ui, SYNTAX_DEBOUNCE_MS};

/// Handle app messages (file operations, window events)
pub fn update_app(model: &mut AppModel, msg: AppMsg) -> Option<Cmd> {
    match msg {
        AppMsg::Resize(width, height) => {
            model.resize(width, height);
            Some(Cmd::Redraw)
        }

        AppMsg::ScaleFactorChanged(scale_factor) => {
            model.set_scale_factor(scale_factor);
            // Reinitialize renderer (creates new glyph cache) and force redraw
            Some(Cmd::Batch(vec![Cmd::ReinitializeRenderer, Cmd::Redraw]))
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
                    // Detect language from file extension
                    let language = LanguageId::from_path(&path);

                    let doc = model.document_mut();
                    doc.buffer = ropey::Rope::from(content);
                    doc.file_path = Some(path.clone());
                    doc.is_modified = false;
                    doc.undo_stack.clear();
                    doc.redo_stack.clear();
                    doc.language = language;
                    doc.syntax_highlights = None;
                    doc.revision = doc.revision.wrapping_add(1);

                    *model.editor_mut().primary_cursor_mut() = Default::default();
                    model.ui.set_status(format!("Loaded: {}", path.display()));

                    // Trigger syntax parsing if language has highlighting
                    if language.has_highlighting() {
                        if let Some(doc_id) = model.document().id {
                            let revision = model.document().revision;
                            return Some(Cmd::Batch(vec![
                                Cmd::Redraw,
                                Cmd::DebouncedSyntaxParse {
                                    document_id: doc_id,
                                    revision,
                                    delay_ms: SYNTAX_DEBOUNCE_MS,
                                },
                            ]));
                        }
                    }
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

        // =====================================================================
        // File Dialog Messages
        // =====================================================================
        AppMsg::SaveFileAs => {
            let suggested = model.document().file_path.clone();
            Some(Cmd::ShowSaveFileDialog {
                suggested_path: suggested,
            })
        }

        AppMsg::SaveFileAsDialogResult { path } => {
            if let Some(path) = path {
                model.document_mut().file_path = Some(path.clone());
                let content = model.document().buffer.to_string();
                model.ui.is_saving = true;
                model.ui.set_status("Saving...");
                Some(Cmd::SaveFile { path, content })
            } else {
                model.ui.set_status("Save cancelled");
                Some(Cmd::Redraw)
            }
        }

        AppMsg::OpenFileDialog => {
            let start_dir = model
                .document()
                .file_path
                .as_ref()
                .and_then(|p| p.parent().map(PathBuf::from))
                .or_else(|| model.workspace_root.clone());
            Some(Cmd::ShowOpenFileDialog {
                allow_multi: true,
                start_dir,
            })
        }

        AppMsg::OpenFileDialogResult { paths } => {
            if paths.is_empty() {
                model.ui.set_status("Open cancelled");
                return Some(Cmd::Redraw);
            }

            // Open each file as a new tab
            for path in paths {
                update_layout(model, LayoutMsg::OpenFileInNewTab(path));
            }
            Some(Cmd::Redraw)
        }

        AppMsg::OpenFolderDialog => {
            let start_dir = model.workspace_root.clone();
            Some(Cmd::ShowOpenFolderDialog { start_dir })
        }

        AppMsg::OpenFolderDialogResult { folder } => {
            if let Some(root) = folder {
                model.workspace_root = Some(root.clone());
                model
                    .ui
                    .set_status(format!("Workspace: {}", root.display()));
            } else {
                model.ui.set_status("Open folder cancelled");
            }
            Some(Cmd::Redraw)
        }
    }
}

/// Execute a command from the command palette
pub fn execute_command(model: &mut AppModel, cmd_id: CommandId) -> Option<Cmd> {
    match cmd_id {
        CommandId::NewFile => update_layout(model, LayoutMsg::NewTab),
        CommandId::OpenFile => update_app(model, AppMsg::OpenFileDialog),
        CommandId::OpenFolder => update_app(model, AppMsg::OpenFolderDialog),
        CommandId::SaveFile => update_app(model, AppMsg::SaveFile),
        CommandId::SaveFileAs => update_app(model, AppMsg::SaveFileAs),
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
                        model
                            .ui
                            .set_status(format!("Failed to create keymap: {}", e));
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
