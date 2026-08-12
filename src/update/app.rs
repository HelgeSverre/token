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
                    let status_bar_height = model.status_bar_height;
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
            let mut lsp_cmd = None;
            match result {
                Ok(_) => {
                    let doc = model.document_mut();
                    doc.is_modified = false;
                    doc.saved_revision = Some(doc.undo_stack.len());
                    if let Some(path) = &model.document().file_path {
                        model.ui.set_status(format!("Saved: {}", path.display()));
                    }
                    if let Some(doc_id) = model.document().id {
                        lsp_cmd = Some(super::save_lsp_document(doc_id));
                    }
                }
                Err(e) => {
                    model.ui.set_status(format!("Error: {}", e));
                }
            }
            match lsp_cmd {
                Some(cmd) => Some(Cmd::Batch(vec![Cmd::redraw_status_bar(), cmd])),
                None => Some(Cmd::redraw_status_bar()),
            }
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
                    let old_path = model.document().file_path.clone();

                    let doc = model.document_mut();
                    doc.buffer = ropey::Rope::from(content);
                    doc.file_path = Some(path.clone());
                    doc.is_modified = false;
                    doc.undo_stack.clear();
                    doc.redo_stack.clear();
                    doc.saved_revision = Some(0);
                    doc.language = language;
                    doc.syntax_highlights = None;
                    doc.syntax_tree = None;
                    doc.revision = doc.revision.wrapping_add(1);

                    // External reload = revision bump = normal didChange
                    // (see design doc's Document Synchronization). If the
                    // path itself changed (tab reused for a different
                    // file), the LSP identity changes too: didClose(old) +
                    // didOpen(new), same as Save As.
                    let mut lsp_cmds = Vec::new();
                    if let Some(doc_id) = model.document().id {
                        if old_path.as_deref() == Some(path.as_path()) {
                            lsp_cmds.extend(super::schedule_lsp_did_change(model, doc_id));
                        } else {
                            if old_path.is_some() {
                                lsp_cmds.push(super::close_lsp_document(doc_id));
                            }
                            lsp_cmds.extend(super::open_lsp_document(model, doc_id));
                        }
                    }

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
                            let mut cmds = vec![
                                Cmd::redraw_editor(),
                                Cmd::DebouncedSyntaxParse {
                                    document_id: doc_id,
                                    revision,
                                    delay_ms: SYNTAX_DEBOUNCE_MS,
                                },
                                Cmd::SaveRecentFiles {
                                    recent: model.recent_files.clone(),
                                },
                            ];
                            cmds.extend(lsp_cmds);
                            return Some(Cmd::Batch(cmds));
                        }
                    }
                    let mut cmds = vec![
                        Cmd::redraw_editor(),
                        Cmd::SaveRecentFiles {
                            recent: model.recent_files.clone(),
                        },
                    ];
                    cmds.extend(lsp_cmds);
                    Some(Cmd::Batch(cmds))
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
            // SyncStatusBarMetrics re-derives the bar height in case
            // `status_bar_font_size` changed.
            Some(Cmd::Batch(vec![Cmd::SyncStatusBarMetrics, Cmd::Redraw]))
        }

        AppMsg::RestartLanguageServer => {
            let language = model.document().language;
            match crate::lsp::lsp_server_def(language)
                .map(|def| crate::lsp::LspServerId::from(def.id))
            {
                Some(server_id) if model.lsp.servers.contains_key(&server_id) => {
                    super::update_lsp(model, crate::messages::LspMsg::RestartServer { server_id })
                }
                Some(server_id) => {
                    model
                        .ui
                        .set_status(format!("{server_id}: no running server to restart"));
                    Some(Cmd::redraw_status_bar())
                }
                None => {
                    model.ui.set_status("No language server for this file");
                    Some(Cmd::redraw_status_bar())
                }
            }
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
            if let Some(new_path) = path {
                // Save As is a didClose(old) + didOpen(new) pair, not a
                // rename (design doc's Document Synchronization) — but the
                // file doesn't exist at `new_path` yet, and `path_to_uri`
                // canonicalizes only once it does, so `file_path` and the
                // LSP traffic wait for `SaveAsCompleted` rather than firing
                // here against a URI the write will invalidate.
                let old_path = model.document().file_path.clone();
                let Some(document_id) = model.document().id else {
                    model.ui.set_status("Save cancelled");
                    return Some(Cmd::redraw_status_bar());
                };
                let content = model.document().buffer.to_string();
                model.ui.is_saving = true;
                model.ui.set_status("Saving...");
                Some(Cmd::SaveFileAs {
                    document_id,
                    old_path,
                    new_path,
                    content,
                })
            } else {
                model.ui.set_status("Save cancelled");
                Some(Cmd::redraw_status_bar())
            }
        }

        AppMsg::SaveAsCompleted {
            document_id,
            old_path,
            new_path,
            result,
        } => {
            model.ui.is_saving = false;
            match result {
                Ok(_) => {
                    let mut had_marks = false;
                    if let Some(doc) = model.editor_area.documents.get_mut(&document_id) {
                        doc.file_path = Some(new_path.clone());
                        doc.is_modified = false;
                        doc.saved_revision = Some(doc.undo_stack.len());
                        // The old path's diagnostics no longer describe
                        // this document's identity — Save As is otherwise
                        // the one didClose/didOpen pair with no
                        // `LanguageChanged`-style projection clear (that
                        // handler's the only existing sibling).
                        had_marks = !doc.diagnostics.is_empty();
                        doc.diagnostics.clear();
                    }
                    if had_marks {
                        model.resync_viewports();
                    }
                    model
                        .ui
                        .set_status(format!("Saved: {}", new_path.display()));
                    let mut cmds = vec![Cmd::redraw_editor()];
                    if old_path.is_some() {
                        cmds.push(super::close_lsp_document(document_id));
                    }
                    if let Some(open_cmd) = super::open_lsp_document(model, document_id) {
                        cmds.push(open_cmd);
                    }
                    Some(Cmd::Batch(cmds))
                }
                Err(e) => {
                    model.ui.set_status(format!("Error: {}", e));
                    Some(Cmd::redraw_status_bar())
                }
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

            super::document::update_document(model, crate::messages::DocumentMsg::InsertText(text))
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
        CommandId::GotoDefinition => {
            crate::update::update_lsp(model, crate::messages::LspMsg::GotoDefinition)
        }
        CommandId::NavigateBack => {
            crate::update::update_lsp(model, crate::messages::LspMsg::NavigateBack)
        }
        CommandId::ShowHover => {
            crate::update::update_lsp(model, crate::messages::LspMsg::ShowHover)
        }
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
        CommandId::RevealInSidebar => super::workspace::update_workspace(
            model,
            crate::messages::WorkspaceMsg::RevealActiveFile,
        ),
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
        CommandId::TriggerCompletionMenu => {
            crate::update::update_completion(model, crate::messages::CompletionMsg::TriggerMenu)
        }
        CommandId::RestartLanguageServer => update_app(model, AppMsg::RestartLanguageServer),
        CommandId::ToggleLsp => crate::update::lsp::toggle_lsp_enabled(model),
        CommandId::ManageLanguageServers => {
            update_ui(model, UiMsg::ToggleModal(ModalId::LspServers))
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
        #[cfg(debug_assertions)]
        CommandId::CycleCursorOverlayDemo => {
            use crate::model::{CursorOverlayKind, CursorOverlayState};
            model.ui.cursor_overlay = match model.ui.cursor_overlay {
                None => Some(CursorOverlayState::new(CursorOverlayKind::DebugCompletion)),
                Some(state) if state.kind == CursorOverlayKind::DebugCompletion => {
                    Some(CursorOverlayState::new(CursorOverlayKind::DebugHover))
                }
                Some(_) => None,
            };
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
        let window_layout = WindowLayout::compute(model);
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

    // lsp-integration.md's "every rope mutation bumps `Document.revision`"
    // invariant: an external file reload (`AppMsg::FileLoaded`) must bump
    // the revision too — the LSP sync test for this in this same commit
    // (`push_edit` already covers the ordinary-edit path in
    // `model/document.rs`'s own tests).
    #[test]
    fn file_loaded_bumps_revision_forcing_a_resync() {
        let mut model = test_model();
        // Same path before and after: a reload, not an identity change —
        // must resync via a normal didChange, not didClose/didOpen.
        model.document_mut().file_path = Some(PathBuf::from("/tmp/reloaded.rs"));
        let before = model.document().revision;

        let cmd = update_app(
            &mut model,
            AppMsg::FileLoaded {
                path: PathBuf::from("/tmp/reloaded.rs"),
                result: Ok("fn main() {}".to_owned()),
            },
        )
        .expect("FileLoaded should produce a command");

        assert_eq!(model.document().revision, before.wrapping_add(1));
        let Cmd::Batch(cmds) = cmd else {
            panic!("expected a Batch including LspScheduleDidChange");
        };
        assert!(
            cmds.iter()
                .any(|c| matches!(c, Cmd::LspScheduleDidChange { .. })),
            "expected LspScheduleDidChange in {cmds:?}"
        );
    }

    #[test]
    fn file_loaded_into_a_reused_tab_with_a_different_path_emits_lsp_close_open_pair() {
        let mut model = test_model();
        // Tab previously showed a different file (e.g. Open Log File reusing
        // the focused tab) — the LSP identity changes, same as Save As.
        model.document_mut().file_path = Some(PathBuf::from("/tmp/old.rs"));
        let doc_id = model.document().id;

        let cmd = update_app(
            &mut model,
            AppMsg::FileLoaded {
                path: PathBuf::from("/tmp/new.rs"),
                result: Ok("fn main() {}".to_owned()),
            },
        )
        .expect("FileLoaded should produce a command");

        let Cmd::Batch(cmds) = cmd else {
            panic!("expected a Batch including LspDidClose/LspDidOpen");
        };
        assert!(cmds.iter().any(
            |c| matches!(c, Cmd::LspDidClose { document_id } if Some(*document_id) == doc_id)
        ));
        assert!(cmds.iter().any(|c| match c {
            Cmd::Batch(inner) => inner.iter().any(|c| matches!(c, Cmd::LspDidOpen { .. })),
            _ => false,
        }));
    }

    #[test]
    fn save_file_as_dialog_result_defers_the_path_swap_to_completion() {
        let mut model = test_model();
        // Give the document a path first so Save As has an "old" URI to
        // close — the design doc's "Save As = didClose(old) + didOpen(new)".
        model.document_mut().file_path = Some(PathBuf::from("/tmp/old.rs"));
        let doc_id = model.document().id.unwrap();

        let cmd = update_app(
            &mut model,
            AppMsg::SaveFileAsDialogResult {
                path: Some(PathBuf::from("/tmp/new.rs")),
            },
        )
        .expect("Save As should produce a command");

        // `file_path` (and thus every `path_to_uri` call) must not change
        // until the write actually lands — computing the URI against a
        // not-yet-existing path yields a non-canonical URI that a later
        // canonicalizing lookup (`find_document_by_uri`) would never match
        // (see design doc's URIs and Paths section).
        assert_eq!(
            model.document().file_path,
            Some(PathBuf::from("/tmp/old.rs")),
            "file_path must stay unchanged until SaveAsCompleted"
        );
        assert!(matches!(
            cmd,
            Cmd::SaveFileAs { document_id, old_path: Some(_), new_path, .. }
                if document_id == doc_id && new_path.as_path() == std::path::Path::new("/tmp/new.rs")
        ));
    }

    #[test]
    fn save_as_completed_swaps_path_and_emits_lsp_close_open_pair() {
        let mut model = test_model();
        model.document_mut().file_path = Some(PathBuf::from("/tmp/old.rs"));
        let doc_id = model.document().id.unwrap();
        model.document_mut().diagnostics = vec![lsp_types::Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: "stale".to_owned(),
            ..Default::default()
        }];

        let cmd = update_app(
            &mut model,
            AppMsg::SaveAsCompleted {
                document_id: doc_id,
                old_path: Some(PathBuf::from("/tmp/old.rs")),
                new_path: PathBuf::from("/tmp/new.rs"),
                result: Ok(()),
            },
        )
        .expect("SaveAsCompleted should produce a command");

        assert_eq!(
            model.document().file_path,
            Some(PathBuf::from("/tmp/new.rs"))
        );
        assert!(
            model.document().diagnostics.is_empty(),
            "the old file's diagnostics must not survive onto the new path"
        );
        let Cmd::Batch(cmds) = cmd else {
            panic!("expected a Batch of [redraw, LspDidClose, ...LspDidOpen batch]");
        };
        assert!(cmds
            .iter()
            .any(|c| matches!(c, Cmd::LspDidClose { document_id } if *document_id == doc_id)));
        assert!(cmds.iter().any(|c| match c {
            Cmd::Batch(inner) => inner.iter().any(|c| matches!(c, Cmd::LspDidOpen { .. })),
            _ => false,
        }));
    }

    #[test]
    fn save_as_completed_reports_the_error_and_leaves_path_unchanged_on_failure() {
        let mut model = test_model();
        model.document_mut().file_path = Some(PathBuf::from("/tmp/old.rs"));
        let doc_id = model.document().id.unwrap();

        update_app(
            &mut model,
            AppMsg::SaveAsCompleted {
                document_id: doc_id,
                old_path: Some(PathBuf::from("/tmp/old.rs")),
                new_path: PathBuf::from("/tmp/new.rs"),
                result: Err("permission denied".to_owned()),
            },
        );

        assert_eq!(
            model.document().file_path,
            Some(PathBuf::from("/tmp/old.rs")),
            "a failed write must not swap the document onto the unwritten path"
        );
    }
}
