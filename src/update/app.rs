//! App message handlers (file operations, window events)

use std::path::PathBuf;

use crate::commands::{Cmd, CommandId, ConfigResource};
use crate::messages::{AppMsg, DockMsg, LayoutMsg, TerminalMsg, UiMsg};
use crate::model::{AppModel, DocumentId, FileRequestKind, ModalId, SaveIntent, SaveReason};
use crate::panel::PanelId;
use crate::syntax::LanguageId;

use super::{layout::update_layout, ui::update_ui};

/// Handle app messages (file operations, window events)
pub(super) fn update_app(model: &mut AppModel, msg: AppMsg) -> Option<Cmd> {
    let before = (model.ui.is_saving, model.ui.is_loading);
    let result = update_app_inner(model, msg);
    model.ui.is_saving = model
        .editor_area
        .documents
        .values()
        .any(|doc| doc.file_io.pending(FileRequestKind::Write));
    model.ui.is_loading = !model.editor_area.file_opens.pending.is_empty()
        || model
            .editor_area
            .documents
            .values()
            .any(|doc| doc.file_io.pending(FileRequestKind::Read));
    if before != (model.ui.is_saving, model.ui.is_loading)
        && result.as_ref().is_none_or(|cmd| !cmd.needs_redraw())
    {
        super::merge_cmds(result, Some(Cmd::redraw_status_bar()))
    } else {
        result
    }
}

fn update_app_inner(model: &mut AppModel, msg: AppMsg) -> Option<Cmd> {
    match msg {
        AppMsg::ScrollAnimationTick { seconds } => model
            .advance_scroll_animations(seconds)
            .then_some(Cmd::redraw_editor()),
        AppMsg::Resize(width, height) => {
            model.resize(width, height);

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
            if model.document().external_change.is_some() {
                return super::file_change::show_focused(model, true);
            }
            save_document(model)
        }
        AppMsg::AutoSave(request) => super::auto_save::save(model, request),

        AppMsg::LoadFile(path) => {
            let target = model
                .document_mut()
                .begin_file_request(FileRequestKind::Read)?;
            model.ui.is_loading = true;
            model.ui.set_status("Loading...");
            Some(Cmd::LoadFile { target, path })
        }

        AppMsg::FilesChanged(paths) => {
            super::file_policy::changed(model, &paths);
            super::file_change::changed(model, &paths)
        }
        AppMsg::FilePolicyResolved { request, result } => {
            super::file_policy::resolved(model, request, result)
        }
        AppMsg::FileObserved { target, observed } => {
            super::file_change::observed(model, target, observed)
        }
        AppMsg::ResolveFileChange => super::file_change::show_focused(model, true),

        AppMsg::NewFile => {
            // TODO: Implement new file
            model.ui.set_status("New file not yet implemented");
            Some(Cmd::redraw_status_bar())
        }

        AppMsg::SaveCompleted {
            target,
            path,
            content,
            identity,
            result,
        } => finish_save(model, target, path, content, identity, result),

        AppMsg::OpenConfigResource(resource) => {
            super::layout::open_config_resource(model, resource)
        }

        AppMsg::FileLoaded {
            target,
            path,
            identity,
            result,
        } => finish_load(model, target, path, identity, result),

        AppMsg::Quit => super::closing::request(model, crate::model::closing::CloseTarget::Quit),

        AppMsg::ReloadConfiguration => Some(Cmd::ReloadConfiguration),
        AppMsg::ThemeLoaded {
            id,
            persist,
            result,
        } => {
            match result {
                Ok(theme) => {
                    model.theme = *theme;
                    if persist {
                        model.config.theme = id;
                        return Some(Cmd::Batch(vec![
                            Cmd::SaveConfiguration {
                                config: Box::new(model.config.clone()),
                            },
                            Cmd::Redraw,
                        ]));
                    }
                }
                Err(error) => model
                    .ui
                    .set_status(format!("Could not load theme: {error}")),
            }
            Some(Cmd::Redraw)
        }
        AppMsg::ConfigurationSaved(result) => {
            if let Err(error) = result {
                model
                    .ui
                    .set_status(format!("Could not save configuration: {error}"));
                return Some(Cmd::redraw_status_bar());
            }
            None
        }
        AppMsg::ConfigurationLoaded {
            config,
            theme,
            result,
        } => {
            use crate::config::ReloadResult;

            model.config = *config;
            model.theme = *theme;

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
            // Fonts and their layout metrics are applied together by runtime.
            Some(Cmd::Batch(vec![Cmd::SyncFontMetrics, Cmd::Redraw]))
        }

        AppMsg::RestartLanguageServer => {
            let language = model.document().language;
            match crate::lsp::server_id_for_language(language, &model.config.lsp)
                .map(crate::lsp::LspServerId::from)
            {
                Some(server_id) if model.lsp.servers.contains_key(&server_id) => {
                    super::lsp::update_lsp(
                        model,
                        crate::messages::LspMsg::RestartServer { server_id },
                    )
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
        AppMsg::SaveFileAs => request_save_as(model, model.document().id?),

        AppMsg::SaveFileAsDialogResult { target, path } => {
            let doc = model.editor_area.documents.get_mut(&target.document_id)?;
            if !doc.file_io.finish(&target, FileRequestKind::SaveDialog)
                || doc.file_path != target.source_path
            {
                return None;
            }
            if let Some(path) = path {
                request_save(model, target.document_id, path, SaveReason::SaveAs)
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
                group_id: model.editor_area.focused_group_id,
                allow_multi: true,
                start_dir,
            })
        }

        AppMsg::OpenFileDialogResult { group_id, paths } => {
            if !model.editor_area.groups.contains_key(&group_id) {
                return None;
            }
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
                if let Some(cmd) = super::layout::open_file_in_group(model, path, group_id, None) {
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
            if model.ui.active_modal.is_some()
                || model.ui.focus == crate::model::FocusTarget::FindBar
            {
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

pub(super) fn is_terminal_dock_focused(model: &AppModel) -> bool {
    model
        .dock_layout
        .active_panel_position(PanelId::TERMINAL)
        .is_some_and(|position| model.ui.focused_dock() == Some(position))
}

/// Execute a command from the command palette
/// Writes the focused document to its path (`Cmd::SaveFile`, completed by
/// `AppMsg::SaveCompleted`). Shared by `AppMsg::SaveFile` and the
/// `format_on_save` chain in `update/lsp.rs`, which must not re-enter the
/// formatting gate.
pub(super) fn save_document(model: &mut AppModel) -> Option<Cmd> {
    let doc = model.try_document()?;
    let document_id = doc.id?;
    match doc.file_path.clone() {
        Some(path) => request_save(model, document_id, path, SaveReason::Manual),
        None => request_save_as(model, document_id),
    }
}

pub(super) fn request_save_as(model: &mut AppModel, document_id: DocumentId) -> Option<Cmd> {
    if !can_save_document(model, document_id) {
        return Some(Cmd::redraw_status_bar());
    }
    let target = model
        .editor_area
        .documents
        .get_mut(&document_id)?
        .begin_file_request(FileRequestKind::SaveDialog)?;
    Some(Cmd::ShowSaveFileDialog {
        suggested_path: target.source_path.clone(),
        target,
    })
}

/// Prepare a specific document without moving focus or taking its final snapshot
/// until any formatter has completed. The token also invalidates older replies.
pub(super) fn request_save(
    model: &mut AppModel,
    document_id: DocumentId,
    path: PathBuf,
    reason: SaveReason,
) -> Option<Cmd> {
    if !can_save_document(model, document_id) {
        return Some(Cmd::redraw_status_bar());
    }
    let doc = model.editor_area.documents.get_mut(&document_id)?;
    if doc.external_change.is_some() && reason != SaveReason::SaveAs {
        return None;
    }
    let automatic_policy = reason
        .is_automatic()
        .then(|| model.config.auto_save.clone());
    let intent = SaveIntent::new(doc, document_id, path, reason, automatic_policy);
    doc.pending_save = Some(intent.clone());
    if let Some(command) = super::file_policy::prepare_save(model, &intent) {
        return Some(command);
    }
    prepare_resolved_save(model, intent)
}

pub(super) fn prepare_resolved_save(model: &mut AppModel, intent: SaveIntent) -> Option<Cmd> {
    let document_id = intent.document_id;
    let reason = intent.reason;
    let doc = model.editor_area.documents.get_mut(&document_id)?;
    if !intent.is_current(doc) {
        return None;
    }
    if reason.is_automatic()
        && (doc.revision != intent.revision
            || intent.automatic_policy.as_ref() != Some(&model.config.auto_save))
    {
        doc.pending_save = None;
        return None;
    }
    let format = if reason.is_automatic() {
        model.config.auto_save.format_on_save
    } else {
        model.config.format_on_save
    };
    let same_language =
        reason != SaveReason::SaveAs || LanguageId::from_path(&intent.path) == doc.language;
    doc.pending_save = Some(intent.clone());
    if format && model.config.lsp.enabled && doc.file_path.is_some() && same_language {
        Some(Cmd::LspRequestFormatting {
            document_id,
            revision: doc.revision,
            range: None,
            options: super::lsp::formatting_options(intent.settings),
            save: Some(intent),
        })
    } else {
        finish_preparing_save(model, intent)
    }
}

pub(super) fn finish_save_formatting(
    model: &mut AppModel,
    intent: SaveIntent,
    revision: u64,
    edits: Option<Vec<(lsp_types::Range, String)>>,
) -> Option<Cmd> {
    let doc = model.editor_area.documents.get(&intent.document_id)?;
    if !intent.is_current(doc) {
        return None;
    }
    if !doc
        .pending_save
        .as_ref()
        .is_some_and(|pending| pending.resolution_generation == intent.resolution_generation)
    {
        return None;
    }
    let intent = doc.pending_save.clone()?;
    if let Some(command) = super::file_policy::prepare_save(model, &intent) {
        return Some(command);
    }
    let doc = model.editor_area.documents.get(&intent.document_id)?;
    let current_policy = intent
        .automatic_policy
        .as_ref()
        .is_none_or(|policy| policy == &model.config.auto_save);
    if !current_policy || (intent.reason.is_automatic() && doc.revision != intent.revision) {
        model
            .editor_area
            .documents
            .get_mut(&intent.document_id)?
            .pending_save = None;
        return None;
    }
    let mut effects = None;
    if doc.revision == revision
        && doc.language == intent.language
        && doc.text_policy_generation == intent.text_policy_generation
        && doc.external_change.is_none()
    {
        if let Some(edits) = edits.as_deref() {
            let planned = super::text_edits::plan_text_edits(doc, edits);
            effects = super::text_edits::apply_planned_edits(
                model,
                intent.document_id,
                &planned,
                super::text_edits::EditCarets::Preserve,
            );
        }
    }
    let save = finish_preparing_save(model, intent);
    if edits.is_none() && save.is_some() {
        model
            .ui
            .set_status("Formatter unavailable, saved unformatted");
    }
    super::merge_cmds(effects, save)
}

fn finish_preparing_save(model: &mut AppModel, intent: SaveIntent) -> Option<Cmd> {
    let doc = model.editor_area.documents.get_mut(&intent.document_id)?;
    if !intent.is_current(doc) {
        return None;
    }
    doc.pending_save = None;
    if doc.external_change.is_some() && intent.reason != SaveReason::SaveAs {
        return None;
    }
    let settings = intent
        .destination_policy
        .as_ref()
        .filter(|_| model.config.editorconfig)
        .map_or(doc.text_settings, |policy| {
            policy.settings(model.config.text)
        });
    let cleanup = super::save_cleanup::apply(model, intent.document_id, settings);
    let mut command = begin_save(model, intent.document_id, intent.path.clone())?;
    if let Cmd::SaveFile { target, .. } = &mut command {
        target.file_policy = intent.destination_policy.clone();
    }
    super::merge_cmds(cleanup, Some(command))
}

pub(super) fn begin_save(
    model: &mut AppModel,
    document_id: crate::model::DocumentId,
    path: PathBuf,
) -> Option<Cmd> {
    if !can_save_document(model, document_id) {
        return Some(Cmd::redraw_status_bar());
    }
    let doc = model.editor_area.documents.get_mut(&document_id)?;
    let mut target = doc.begin_file_request(FileRequestKind::Write)?;
    if !doc.matches_file_path(&path) {
        // A different destination returned by the native Save As dialog is
        // explicitly chosen; aliases of this document retain conflict checks.
        target.write_guard.save_as = true;
    }
    let content = doc.buffer.clone();
    doc.file_io.queue_write(path.clone(), content.clone());
    model.ui.is_saving = true;
    model.ui.set_status("Saving...");
    Some(Cmd::SaveFile {
        target,
        path,
        content,
    })
}

fn can_save_document(model: &mut AppModel, document_id: crate::model::DocumentId) -> bool {
    let placeholder = model.editor_area.editors.values().any(|editor| {
        editor.document_id == Some(document_id)
            && (editor.view_mode.is_image()
                || matches!(
                    editor.tab_content,
                    crate::model::editor::TabContent::BinaryPlaceholder(_)
                ))
    });
    if placeholder {
        model
            .ui
            .set_status("Saving image/binary tabs is not supported");
    }
    !placeholder
}

fn finish_save(
    model: &mut AppModel,
    target: crate::model::FileRequest,
    path: PathBuf,
    content: ropey::Rope,
    identity: Option<crate::util::FileIdentity>,
    result: Result<(), String>,
) -> Option<Cmd> {
    let document_id = target.document_id;
    let doc = model.editor_area.documents.get_mut(&document_id)?;
    if !doc.file_io.finish(&target, FileRequestKind::Write) {
        return None;
    }
    if let Err(error) = result {
        doc.save_error = Some((target.revision, error.clone()));
        doc.file_io.check_again = true;
        model
            .ui
            .set_status(format!("Error saving {}: {error}", path.display()));
        return Some(Cmd::redraw_status_bar());
    }
    doc.file_io.saved(&target);
    doc.file_io.check_again = true;
    let old_uri = doc.file_identity().map(|identity| identity.uri().clone());
    let old_path = doc.file_path.replace(path.clone());
    doc.set_file_identity(identity);
    if model.config.editorconfig {
        if let Some(policy) = target.file_policy {
            let source = doc
                .file_identity()
                .map_or(path.as_path(), |id| id.path())
                .to_path_buf();
            doc.file_policy.enabled = true;
            doc.file_policy.install(source, (*policy).clone());
            doc.file_text_preferences = doc.file_policy.resolved.as_ref()?.preferences;
            doc.resolve_text_settings(model.config.text);
        }
    }
    let renamed = old_path.as_ref() != Some(&path)
        || old_uri
            .as_ref()
            .zip(doc.file_identity())
            .is_some_and(|(old, current)| old != current.uri());
    doc.record_saved_buffer(content.clone());
    let language = LanguageId::from_path(&path);
    let relanguage = renamed && !doc.language_pinned && language != doc.language;
    let had_marks = renamed && !doc.diagnostics.is_empty();
    if renamed {
        doc.diagnostics.clear();
    }
    if had_marks {
        model.resync_viewports();
    }
    model.ui.set_status(format!("Saved: {}", path.display()));
    let mut cmds = vec![Cmd::redraw_editor()];
    if renamed {
        if relanguage {
            cmds.extend(
                super::syntax::apply_language(model, document_id, language).unwrap_or_default(),
            );
        }
        if old_path.is_some() {
            cmds.push(super::close_lsp_document(document_id));
        }
        cmds.extend(super::open_lsp_document(model, document_id));
    } else {
        cmds.push(Cmd::LspDidSave {
            document_id,
            saved_text: content,
        });
    }
    Some(Cmd::Batch(cmds))
}

pub(super) fn finish_load(
    model: &mut AppModel,
    target: crate::model::FileRequest,
    path: PathBuf,
    identity: Option<crate::util::FileIdentity>,
    result: Result<String, String>,
) -> Option<Cmd> {
    let external = target.external_reload;
    let document_id = target.document_id;
    let pending_cell_edit =
        external && super::file_change::has_pending_cell_edit(model, document_id);
    let doc = model.editor_area.documents.get_mut(&document_id)?;
    if !doc.file_io.finish(&target, FileRequestKind::Read) {
        return None;
    }
    if doc.revision != target.revision || doc.file_path != target.source_path || pending_cell_edit {
        doc.file_io.check_again |= external;
        model.ui.set_status(format!(
            "Load discarded: {} changed while loading",
            path.display()
        ));
        return Some(Cmd::redraw_status_bar());
    }
    let content = match result {
        Ok(content) => content,
        Err(error) => {
            doc.file_io.check_again |= external;
            model
                .ui
                .set_status(format!("Error loading {}: {error}", path.display()));
            return Some(Cmd::redraw_status_bar());
        }
    };
    let old_uri = doc.file_identity().map(|identity| identity.uri().clone());
    let old_path = doc.file_path.replace(path.clone());
    doc.set_file_identity(identity);
    let renamed = old_path.as_ref() != Some(&path)
        || old_uri
            .as_ref()
            .zip(doc.file_identity())
            .is_some_and(|(old, current)| old != current.uri());
    doc.detected_line_ending = crate::model::LineEnding::detect(&content);
    doc.buffer = ropey::Rope::from(content);
    doc.folds = None;
    doc.record_saved_buffer(doc.buffer.clone());
    doc.undo_stack.clear();
    doc.redo_stack.clear();
    doc.file_io.invalidate();
    if !doc.language_pinned {
        doc.language = LanguageId::from_path(&path);
    }
    doc.syntax_highlights = None;
    doc.syntax_tree = None;
    doc.outline = None;
    doc.revision = doc.revision.wrapping_add(1);
    if renamed {
        doc.diagnostics.clear();
    }

    // External reloads retain each pane's view and position. Explicit loads of
    // another file retain the existing reset-to-text behavior.
    for editor in model
        .editor_area
        .editors
        .values_mut()
        .filter(|editor| editor.document_id == Some(document_id))
    {
        editor.clear_selection_history();
        if external {
            if let Some(csv) = editor.view_mode.as_csv_mut() {
                match crate::csv::parse_csv(&doc.buffer.to_string(), csv.delimiter) {
                    Ok(data) => {
                        let mut replacement = crate::csv::CsvState::new(data, csv.delimiter);
                        replacement.has_header_row = csv.has_header_row;
                        replacement.selected_cell = csv.selected_cell;
                        replacement.viewport = csv.viewport.clone();
                        replacement.clamp_selection();
                        replacement.viewport.top_row = replacement.viewport.top_row.min(
                            replacement
                                .data
                                .row_count()
                                .saturating_sub(replacement.viewport.visible_rows),
                        );
                        replacement.viewport.left_col = replacement.viewport.left_col.min(
                            replacement
                                .data
                                .column_count()
                                .saturating_sub(replacement.viewport.visible_cols),
                        );
                        *csv = replacement;
                    }
                    Err(_) => editor.view_mode = crate::model::ViewMode::Text,
                }
            }
        } else {
            editor.view_mode = crate::model::ViewMode::Text;
            editor.tab_content = crate::model::TabContent::Text;
            editor.collapse_to_primary();
        }
        for cursor in &mut editor.cursors {
            cursor.line = cursor.line.min(doc.line_count().saturating_sub(1));
            cursor.column = cursor.column.min(doc.line_length(cursor.line));
            cursor.desired_column = None;
        }
        if external {
            for selection in &mut editor.selections {
                for position in [&mut selection.anchor, &mut selection.head] {
                    position.line = position.line.min(doc.line_count().saturating_sub(1));
                    position.column = position.column.min(doc.line_length(position.line));
                }
            }
            editor.ensure_wrap_cache(doc);
            let (x, y) = editor.pixel_scroll_position();
            editor.set_pixel_scroll(doc, x, y);
        } else {
            editor.collapse_selections_to_cursors();
        }
    }
    let mut cmds = vec![Cmd::Redraw];
    if !renamed {
        cmds.extend(super::schedule_lsp_did_change(model, document_id));
    } else {
        if old_path.is_some() {
            cmds.push(super::close_lsp_document(document_id));
        }
        cmds.extend(super::open_lsp_document(model, document_id));
    }
    cmds.extend(super::schedule_syntax_parse(model, document_id));
    model.resync_viewports();
    model.ui.set_status(format!("Loaded: {}", path.display()));
    if !external {
        model.record_file_opened(document_id);
        cmds.push(Cmd::SaveRecentFiles {
            recent: model.recent_files.clone(),
        });
    }
    Some(Cmd::Batch(cmds))
}

pub fn execute_command(model: &mut AppModel, cmd_id: CommandId) -> Option<Cmd> {
    match cmd_id {
        CommandId::ResolveFileChange => super::file_change::show_focused(model, true),
        CommandId::ShowContextMenu => {
            crate::update::context_menu::open_editor_menu_at_caret(model, false)
        }
        CommandId::CloseGroup => update_layout(model, LayoutMsg::CloseFocusedGroup),
        CommandId::SwitchTheme => update_ui(model, UiMsg::ToggleModal(ModalId::ThemePicker)),
        CommandId::OpenConfigDirectory => {
            update_app(model, AppMsg::OpenConfigResource(ConfigResource::Directory))
        }
        CommandId::OpenKeybindings => update_app(
            model,
            AppMsg::OpenConfigResource(ConfigResource::Keybindings),
        ),
        CommandId::OpenLogFile => {
            update_app(model, AppMsg::OpenConfigResource(ConfigResource::Log))
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
        CommandId::ToggleProblems => {
            // Command palette uses focus-agnostic toggle (pure open/close)
            super::dock::update_dock(model, DockMsg::TogglePanel(PanelId::PROBLEMS))
        }
        CommandId::ToggleProblemsScope => {
            super::problems::update_problems(model, crate::messages::ProblemsMsg::ToggleScope)
        }
        CommandId::ToggleUsages => {
            super::dock::update_dock(model, DockMsg::TogglePanel(PanelId::Usages))
        }
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
        CommandId::ToggleLsp => crate::update::lsp::toggle_lsp_enabled(model),
        CommandId::ToggleAutocomplete => crate::update::completion::toggle_enabled(model),
        CommandId::ManageLanguageServers => {
            update_ui(model, UiMsg::ToggleModal(ModalId::LspServers))
        }
        CommandId::SetLanguage => update_ui(model, UiMsg::ToggleModal(ModalId::LanguagePicker)),
        CommandId::ShowFileTextSettings => super::file_policy::show_details(model),
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
        // Shared actions use the same message dispatch as keyboard bindings.
        id => {
            let action = id.to_keymap_command()?;
            let mut cmds: Vec<_> = action
                .to_msgs()
                .into_iter()
                .filter_map(|msg| super::update(model, msg))
                .collect();
            match cmds.len() {
                0 => None,
                1 => cmds.pop(),
                _ => Some(Cmd::Batch(cmds)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AppModel;

    fn complete_test_load(
        model: &mut AppModel,
        path: PathBuf,
        result: Result<String, String>,
    ) -> Option<Cmd> {
        let target = model
            .document_mut()
            .begin_file_request(FileRequestKind::Read)
            .unwrap();
        update_app(
            model,
            AppMsg::FileLoaded {
                identity: None,
                target,
                path,
                result,
            },
        )
    }

    fn complete_test_save(
        model: &mut AppModel,
        path: PathBuf,
        result: Result<(), String>,
    ) -> Option<Cmd> {
        let target = model
            .document_mut()
            .begin_file_request(FileRequestKind::Write)
            .unwrap();
        let content = model.document().buffer.clone();
        update_app(
            model,
            AppMsg::SaveCompleted {
                identity: None,
                target,
                path,
                content,
                result,
            },
        )
    }
    use crate::panel::DockPosition;
    use crate::panels::terminal::grid_size_for_rect;
    use crate::terminal::{PtyHandle, TerminalSession};
    use std::sync::mpsc;

    fn test_model() -> AppModel {
        AppModel::new(800, 600, 1.0)
    }

    fn file_backed_model() -> (tempfile::TempDir, AppModel) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.rs");
        std::fs::write(&path, "fn main() {}\n").unwrap();
        (
            dir,
            AppModel::with_document(
                800,
                600,
                1.0,
                crate::model::Document::from_file(path).unwrap(),
            ),
        )
    }

    #[test]
    fn save_file_with_format_on_save_requests_formatting_instead_of_saving() {
        let (_dir, mut model) = file_backed_model();
        model.config.format_on_save = true;
        let cmd = update_app(&mut model, AppMsg::SaveFile);
        assert!(
            matches!(cmd, Some(Cmd::LspRequestFormatting { save: Some(_), .. })),
            "got {cmd:?}"
        );
        assert!(!model.ui.is_saving);
    }

    #[test]
    fn save_file_without_format_on_save_saves_directly() {
        let (_dir, mut model) = file_backed_model();
        let cmd = update_app(&mut model, AppMsg::SaveFile);
        assert!(matches!(cmd, Some(Cmd::SaveFile { .. })), "got {cmd:?}");
        assert!(model.ui.is_saving);
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
        let content_rect = crate::layout::chrome::chrome(model)
            .rect(crate::layout::UiKey::PanelContent(
                crate::panel::PanelId::Terminal,
            ))
            .expect("terminal dock should be open");

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

    #[test]
    fn paste_routes_to_terminal_after_it_moves_to_the_right_dock() {
        let (mut model, pty_rx) = focused_terminal_model();
        model
            .dock_layout
            .bottom
            .panel_ids
            .retain(|&panel| panel != PanelId::TERMINAL);
        model.dock_layout.bottom.active_index = Some(0);
        model.dock_layout.right.register_panel(PanelId::TERMINAL);
        model.dock_layout.right.activate(PanelId::TERMINAL);
        model.ui.focus_dock(DockPosition::Right);

        let cmd = update_app(
            &mut model,
            AppMsg::PasteFromClipboard("moved terminal".to_owned()),
        );

        assert!(cmd.is_none());
        assert_eq!(pty_rx.try_recv().unwrap(), b"moved terminal".to_vec());
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

        let cmd = complete_test_load(
            &mut model,
            PathBuf::from("/tmp/reloaded.rs"),
            Ok("fn main() {}".to_owned()),
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

        let cmd = complete_test_load(
            &mut model,
            PathBuf::from("/tmp/new.rs"),
            Ok("fn main() {}".to_owned()),
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

        let target = model
            .document_mut()
            .begin_file_request(FileRequestKind::SaveDialog)
            .unwrap();
        let cmd = update_app(
            &mut model,
            AppMsg::SaveFileAsDialogResult {
                target,
                path: Some(PathBuf::from("/tmp/new.rs")),
            },
        )
        .expect("Save As should produce a command");
        let Cmd::ResolveFilePolicy(request) = cmd else {
            panic!("destination policy request")
        };
        let cmd = update_app(
            &mut model,
            AppMsg::FilePolicyResolved {
                result: Ok(crate::editorconfig::ResolvedFilePolicy {
                    path: request.path.clone(),
                    ..Default::default()
                }),
                request,
            },
        )
        .unwrap();

        // `file_path` (and thus every `path_to_uri` call) must not change
        // until the write actually lands — computing the URI against a
        // not-yet-existing path yields a non-canonical URI that a later
        // canonicalizing lookup (`find_document_by_uri`) would never match
        // (see design doc's URIs and Paths section).
        assert_eq!(
            model.document().file_path,
            Some(PathBuf::from("/tmp/old.rs")),
            "file_path must stay unchanged until SaveCompleted"
        );
        assert!(matches!(
            cmd,
            Cmd::SaveFile { target, path, .. }
                if target.document_id == doc_id && path.as_path() == std::path::Path::new("/tmp/new.rs")
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

        let cmd = complete_test_save(&mut model, PathBuf::from("/tmp/new.rs"), Ok(()))
            .expect("SaveCompleted should produce a command");

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

        complete_test_save(
            &mut model,
            PathBuf::from("/tmp/new.rs"),
            Err("permission denied".to_owned()),
        );

        assert_eq!(
            model.document().file_path,
            Some(PathBuf::from("/tmp/old.rs")),
            "a failed write must not swap the document onto the unwritten path"
        );
    }

    #[test]
    fn file_loaded_keeps_a_pinned_language_but_redetects_an_unpinned_one() {
        for (pinned, expected) in [(true, LanguageId::Rust), (false, LanguageId::PlainText)] {
            let mut model = test_model();
            model.document_mut().file_path = Some(PathBuf::from("/tmp/notes.txt"));
            model.document_mut().language = LanguageId::Rust;
            model.document_mut().language_pinned = pinned;

            complete_test_load(
                &mut model,
                PathBuf::from("/tmp/notes.txt"),
                Ok("fn main() {}".to_owned()),
            );

            assert_eq!(model.document().language, expected, "pinned={pinned}");
        }
    }

    #[test]
    fn save_as_completed_follows_the_new_extension_unless_pinned() {
        for (pinned, expected) in [(false, LanguageId::Rust), (true, LanguageId::PlainText)] {
            let mut model = test_model();
            model.document_mut().file_path = Some(PathBuf::from("/tmp/old.txt"));
            model.document_mut().language_pinned = pinned;

            let cmd = complete_test_save(&mut model, PathBuf::from("/tmp/new.rs"), Ok(()))
                .expect("SaveCompleted should produce a command");

            assert_eq!(model.document().language, expected, "pinned={pinned}");
            let Cmd::Batch(cmds) = cmd else {
                panic!("expected a Batch");
            };
            assert_eq!(
                cmds.iter()
                    .any(|c| matches!(c, Cmd::ClearSyntaxState { .. })),
                !pinned,
                "the worker cache is cleared exactly when the language switches"
            );
            assert_eq!(
                cmds.iter()
                    .filter(|c| matches!(c, Cmd::LspDidClose { .. }))
                    .count(),
                1,
                "the language switch must not double the Save As didClose"
            );
        }
    }
}
