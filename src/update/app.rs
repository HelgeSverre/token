//! App message handlers (file operations, window events)

use std::path::PathBuf;

use crate::commands::{Cmd, CommandId};
use crate::config::EditorConfig;
use crate::config_paths;
use crate::keymap::get_default_keymap_yaml;
use crate::messages::{AppMsg, DockMsg, DocumentMsg, LayoutMsg, UiMsg};
use crate::model::{AppModel, ModalId, SplitDirection};
use crate::panel::PanelId;
use crate::syntax::LanguageId;
use crate::theme::{load_theme, Theme};

use super::{update_document, update_layout, update_ui, SYNTAX_DEBOUNCE_MS};

/// Handle app messages (file operations, window events)
pub fn update_app(model: &mut AppModel, msg: AppMsg) -> Option<Cmd> {
    match msg {
        AppMsg::Resize(width, height) => {
            model.resize(width, height);

            // Update CSV viewport size if in CSV mode
            if let Some(editor) = model.editor_area.focused_editor_mut() {
                if let Some(csv) = editor.view_mode.as_csv_mut() {
                    let line_height = model.line_height.max(1);
                    let tab_bar_height = model.metrics.tab_bar_height;
                    let status_bar_height = line_height;
                    let col_header_height = line_height;
                    let content_height = (height as usize)
                        .saturating_sub(tab_bar_height)
                        .saturating_sub(status_bar_height)
                        .saturating_sub(col_header_height);
                    let visible_rows = content_height / line_height;
                    csv.set_viewport_size(visible_rows.max(1), csv.viewport.visible_cols);
                }
            }

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
                    Some(Cmd::redraw_status_bar())
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
            Some(Cmd::redraw_status_bar())
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
            Some(Cmd::redraw_status_bar())
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

                    model.editor_mut().collapse_to_primary();
                    model.ui.set_status(format!("Loaded: {}", path.display()));

                    // Trigger syntax parsing if language has highlighting
                    if language.has_highlighting() {
                        if let Some(doc_id) = model.document().id {
                            let revision = model.document().revision;
                            return Some(Cmd::Batch(vec![
                                Cmd::redraw_editor(),
                                Cmd::DebouncedSyntaxParse {
                                    document_id: doc_id,
                                    revision,
                                    delay_ms: SYNTAX_DEBOUNCE_MS,
                                },
                            ]));
                        }
                    }
                    Some(Cmd::redraw_editor())
                }
                Err(e) => {
                    model.ui.set_status(format!("Error: {}", e));
                    Some(Cmd::redraw_status_bar())
                }
            }
        }

        AppMsg::Quit => Some(Cmd::Quit),

        AppMsg::ReloadConfiguration => {
            use crate::config::ReloadResult;

            let (new_config, result) = EditorConfig::reload();
            let new_theme = load_theme(&new_config.theme).unwrap_or_else(|_| Theme::default());
            model.config = new_config;
            model.theme = new_theme;

            let msg = match result {
                ReloadResult::Loaded => "Configuration reloaded",
                ReloadResult::FileNotFound => "Config file not found, using defaults",
                ReloadResult::ParseError(ref e) => {
                    tracing::warn!("Config parse error: {}", e);
                    "Config file invalid, using defaults"
                }
                ReloadResult::ReadError(ref e) => {
                    tracing::warn!("Config read error: {}", e);
                    "Could not read config file, using defaults"
                }
                ReloadResult::NoConfigDir => "No config directory, using defaults",
            };
            model.ui.set_status(msg);
            Some(Cmd::redraw_status_bar())
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
                Some(Cmd::redraw_status_bar())
            }
        }

        AppMsg::OpenFileDialog => {
            let start_dir = model
                .document()
                .file_path
                .as_ref()
                .and_then(|p| p.parent().map(PathBuf::from))
                .or_else(|| model.workspace_root().cloned());
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

        // TODO: Remove OpenFolderDialog - combine with OpenFileDialog using auto-detection
        // After dialog returns: if path.is_dir() -> open workspace, else -> open file in tab
        // See docs/feature/workspace-management.md for design
        AppMsg::OpenFolderDialog => {
            let start_dir = model.workspace_root().cloned();
            Some(Cmd::ShowOpenFolderDialog { start_dir })
        }

        AppMsg::OpenFolderDialogResult { folder } => {
            if let Some(root) = folder {
                model.open_workspace(root);
            } else {
                model.ui.set_status("Open folder cancelled");
            }
            Some(Cmd::redraw_status_bar())
        }
    }
}

/// Execute a command from the command palette
pub fn execute_command(model: &mut AppModel, cmd_id: CommandId) -> Option<Cmd> {
    match cmd_id {
        CommandId::NewFile => update_layout(model, LayoutMsg::NewTab),
        CommandId::OpenFile => update_app(model, AppMsg::OpenFileDialog),
        CommandId::FuzzyFileFinder => update_ui(model, UiMsg::OpenFuzzyFileFinder),
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
                Some(Cmd::redraw_status_bar())
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
        CommandId::ToggleCsvView => super::csv::update_csv(model, crate::messages::CsvMsg::Toggle),
        CommandId::ToggleMarkdownPreview => {
            super::preview::update_preview(model, crate::messages::PreviewMsg::Toggle)
        }
        CommandId::OpenLogFile => {
            if let Some(log_path) = config_paths::log_file() {
                // Ensure logs dir exists
                let _ = config_paths::ensure_logs_dir();
                Some(Cmd::OpenFileInEditor { path: log_path })
            } else {
                model.ui.set_status("Could not determine log file path");
                Some(Cmd::Redraw)
            }
        }
        CommandId::ReloadConfiguration => update_app(model, AppMsg::ReloadConfiguration),
        CommandId::OpenFolder => update_app(model, AppMsg::OpenFolderDialog),
        CommandId::ToggleFileExplorer => {
            // Command palette uses focus-agnostic toggle (pure open/close)
            super::dock::update_dock(model, DockMsg::TogglePanel(PanelId::FILE_EXPLORER))
        }
        CommandId::ToggleTerminal => {
            // Command palette uses focus-agnostic toggle (pure open/close)
            super::dock::update_dock(model, DockMsg::TogglePanel(PanelId::TERMINAL))
        }
        CommandId::ToggleOutline => {
            // Command palette uses focus-agnostic toggle (pure open/close)
            super::dock::update_dock(model, DockMsg::TogglePanel(PanelId::OUTLINE))
        }
        CommandId::CloseFocusedDock => super::dock::update_dock(model, DockMsg::CloseFocusedDock),
        CommandId::RevealInFinder => {
            if let Some(path) = model.document().file_path.clone() {
                Some(Cmd::Batch(vec![
                    Cmd::RevealFileInFinder { path },
                    Cmd::Redraw,
                ]))
            } else {
                model.ui.set_status("No file path (unsaved)");
                Some(Cmd::Redraw)
            }
        }
        CommandId::CopyAbsolutePath => {
            if let Some(path) = model.document().file_path.clone() {
                let text = path.display().to_string();
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text);
                    model.ui.set_status(format!("Copied: {}", text));
                } else {
                    model.ui.set_status("Failed to access clipboard");
                }
                Some(Cmd::Redraw)
            } else {
                model.ui.set_status("No file path (unsaved)");
                Some(Cmd::Redraw)
            }
        }
        CommandId::CopyRelativePath => {
            if let Some(path) = model.document().file_path.clone() {
                let text = if let Some(root) = model.workspace_root() {
                    path.strip_prefix(root)
                        .map(|rel| rel.display().to_string())
                        .unwrap_or_else(|_| path.display().to_string())
                } else {
                    path.display().to_string()
                };
                if let Ok(mut clipboard) = arboard::Clipboard::new() {
                    let _ = clipboard.set_text(&text);
                    model.ui.set_status(format!("Copied: {}", text));
                } else {
                    model.ui.set_status("Failed to access clipboard");
                }
                Some(Cmd::Redraw)
            } else {
                model.ui.set_status("No file path (unsaved)");
                Some(Cmd::Redraw)
            }
        }
        CommandId::Quit => update_app(model, AppMsg::Quit),
        #[cfg(debug_assertions)]
        CommandId::TogglePerfOverlay => Some(Cmd::TogglePerfOverlay),
        #[cfg(debug_assertions)]
        CommandId::ToggleDebugOverlay => {
            if let Some(ref mut overlay) = model.debug_overlay {
                overlay.toggle();
            }
            Some(Cmd::Redraw)
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
