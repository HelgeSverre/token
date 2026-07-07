//! App message handlers (file operations, window events)

use std::path::PathBuf;

use crate::commands::{Cmd, CommandId};
use crate::config::EditorConfig;
use crate::config_paths;
use crate::keymap::get_default_keymap_yaml;
use crate::messages::{AppMsg, DockMsg, DocumentMsg, LayoutMsg, TerminalMsg, UiMsg};
use crate::model::{AppModel, ModalId, SplitDirection};
use crate::panel::{DockPosition, PanelId};
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

            Some(super::dock::with_terminal_sync(model, Cmd::Redraw))
        }

        AppMsg::ScaleFactorChanged(scale_factor) => {
            model.set_scale_factor(scale_factor);
            // Reinitialize renderer (creates new glyph cache) and force redraw
            Some(super::dock::with_terminal_sync(
                model,
                Cmd::Batch(vec![Cmd::ReinitializeRenderer, Cmd::Redraw]),
            ))
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
                    let doc = model.document_mut();
                    doc.is_modified = false;
                    doc.saved_revision = Some(doc.undo_stack.len());
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

        AppMsg::KeymapCreated { path, result } => match result {
            Ok(_) => Some(Cmd::OpenFileInEditor { path }),
            Err(e) => {
                model
                    .ui
                    .set_status(format!("Failed to create keymap: {}", e));
                Some(Cmd::redraw_status_bar())
            }
        },

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
                    doc.saved_revision = Some(0);
                    doc.language = language;
                    doc.syntax_highlights = None;
                    doc.revision = doc.revision.wrapping_add(1);

                    // Cmd::OpenFileInEditor (e.g. OpenKeybindings/OpenLogFile)
                    // reuses the focused tab regardless of what it was
                    // previously showing, so a non-text tab (image/CSV/binary)
                    // could otherwise end up rendering text content with the
                    // wrong renderer still active.
                    let editor = model.editor_mut();
                    editor.view_mode = crate::model::editor::ViewMode::Text;
                    editor.tab_content = crate::model::editor::TabContent::Text;
                    editor.collapse_to_primary();
                    model.ui.set_status(format!("Loaded: {}", path.display()));

                    // Record in recent files
                    model.record_file_opened(path.clone());

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
                                Cmd::SaveRecentFiles {
                                    recent: model.recent_files.clone(),
                                },
                            ]));
                        }
                    }
                    Some(Cmd::Batch(vec![
                        Cmd::redraw_editor(),
                        Cmd::SaveRecentFiles {
                            recent: model.recent_files.clone(),
                        },
                    ]))
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
            // A theme/config reload can change colors across the whole
            // window, not just the status bar, so it needs a full redraw to
            // actually appear before the next unrelated event triggers one.
            Some(Cmd::Redraw)
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

            // Open each file as a new tab, preserving any commands each open
            // produces (e.g. debounced syntax parsing, recent-files saves)
            // instead of discarding all but the final redraw. Flatten nested
            // batches so callers can scan the result with a single pass.
            let mut cmds: Vec<Cmd> = Vec::new();
            for path in paths {
                if let Some(cmd) = update_layout(model, LayoutMsg::OpenFileInNewTab(path)) {
                    match cmd {
                        Cmd::Batch(inner) => cmds.extend(inner),
                        other => cmds.push(other),
                    }
                }
            }
            cmds.push(Cmd::Redraw);
            Some(Cmd::Batch(cmds))
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

        AppMsg::PasteFromClipboard(text) => {
            if model.ui.active_modal.is_some() {
                return super::ui::update_ui(
                    model,
                    crate::messages::UiMsg::Modal(crate::messages::ModalMsg::PasteText(text)),
                );
            }

            let csv_info = model
                .editor_area
                .focused_editor()
                .and_then(|e| e.view_mode.as_csv().map(|csv| (true, csv.is_editing())));
            if let Some((true, true)) = csv_info {
                return super::csv::update_csv(model, crate::messages::CsvMsg::EditPasteText(text));
            }

            if is_terminal_dock_focused(model) {
                return super::terminal::update_terminal(model, TerminalMsg::Paste(text));
            }

            super::document::update_document(model, crate::messages::DocumentMsg::PasteText(text))
        }
    }
}

fn is_terminal_dock_focused(model: &AppModel) -> bool {
    if model.ui.focused_dock() != Some(DockPosition::Bottom) {
        return false;
    }

    let bottom_dock = model.dock_layout.dock(DockPosition::Bottom);
    bottom_dock.is_open && bottom_dock.active_panel() == Some(PanelId::TERMINAL)
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
                    Some(Cmd::CreateDefaultKeymapFile { path: keymap_path })
                } else {
                    Some(Cmd::OpenFileInEditor { path: keymap_path })
                }
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
                model.ui.set_status(format!("Copied: {}", text));
                Some(Cmd::Batch(vec![Cmd::CopyToClipboard(text), Cmd::Redraw]))
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
                model.ui.set_status(format!("Copied: {}", text));
                Some(Cmd::Batch(vec![Cmd::CopyToClipboard(text), Cmd::Redraw]))
            } else {
                model.ui.set_status("No file path (unsaved)");
                Some(Cmd::Redraw)
            }
        }
        CommandId::OpenRecentFiles => update_ui(model, UiMsg::ToggleModal(ModalId::RecentFiles)),
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

pub fn create_default_keymap_file(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    std::fs::write(path, get_default_keymap_yaml())
        .map_err(|e| format!("Failed to write file: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AppModel;
    use crate::panels::terminal::grid_size_for_rect;
    use crate::terminal::{PtyHandle, TerminalSession};
    use crate::view::geometry::{DockHeaderLayout, WindowLayout};
    use std::sync::mpsc;

    fn test_model() -> AppModel {
        AppModel::new(800, 600, 1.0, vec![])
    }

    fn focused_terminal_model() -> (AppModel, mpsc::Receiver<Vec<u8>>) {
        let mut model = test_model();
        model.dock_layout.bottom.activate(PanelId::TERMINAL);
        model.ui.focus_dock(DockPosition::Bottom);

        let (pty, pty_rx) = PtyHandle::new_for_test();
        let (msg_tx, _msg_rx) = mpsc::channel();
        model
            .terminal
            .sessions
            .push(TerminalSession::new(11, 24, 80, pty, msg_tx));

        (model, pty_rx)
    }

    fn expected_terminal_grid_size(model: &AppModel) -> crate::panels::terminal::TerminalGridSize {
        let window_layout = WindowLayout::compute(model, model.line_height);
        let dock_rect = window_layout
            .bottom_dock_rect
            .expect("terminal dock should be open");
        let content_rect = DockHeaderLayout::new(
            &model.dock_layout.bottom,
            dock_rect,
            &model.metrics,
            model.char_width,
        )
        .content_rect;

        grid_size_for_rect(content_rect, model.char_width, model.line_height)
    }

    #[test]
    fn resizing_window_with_open_terminal_panel_spawns_to_resolved_grid_size() {
        let mut model = test_model();
        model.dock_layout.bottom.activate(PanelId::TERMINAL);

        let cmd = update_app(&mut model, AppMsg::Resize(900, 700));
        let expected = expected_terminal_grid_size(&model);

        let Some(Cmd::Batch(cmds)) = cmd else {
            panic!("expected resize to return a batched terminal spawn + redraw command");
        };

        assert!(cmds.iter().any(|cmd| matches!(
            cmd,
            Cmd::SpawnTerminal {
                session_id: 0,
                rows,
                cols,
            } if *rows == expected.rows && *cols == expected.cols
        )));
        assert!(cmds.iter().any(|cmd| matches!(cmd, Cmd::Redraw)));
    }

    #[test]
    fn paste_from_clipboard_routes_to_focused_terminal() {
        let (mut model, pty_rx) = focused_terminal_model();
        let document_before = model.document().buffer.to_string();

        let cmd = update_app(
            &mut model,
            AppMsg::PasteFromClipboard("terminal paste".to_string()),
        );

        assert!(cmd.is_none());
        assert_eq!(model.document().buffer.to_string(), document_before);
        assert_eq!(pty_rx.try_recv().unwrap(), b"terminal paste".to_vec());
    }
}
