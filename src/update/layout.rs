//! Layout message handlers (split views, tabs, groups)

use std::path::PathBuf;

use crate::commands::Cmd;
use crate::messages::LayoutMsg;
use crate::model::closing::CloseTarget;
use crate::model::editor::{TabContent, ViewMode};
use crate::model::ui::SplitterDragState;
use crate::model::{
    AppModel, Document, EditorGroup, EditorState, GroupId, LayoutNode, Rect, SplitContainer,
    SplitDirection, Tab, TabId,
};

use super::lsp::{close_lsp_document, open_lsp_document};
use super::syntax::schedule_syntax_parse;

/// Drag threshold in pixels before drag becomes active
const DRAG_THRESHOLD_PIXELS: f32 = 4.0;
/// Minimum pane size in pixels
const MIN_PANE_SIZE_PIXELS: f32 = 100.0;

/// Handle layout messages (split views, tabs, groups)
/// Reset the outline panel's view state for a focus/tab change and schedule
/// an outline refresh if the newly-focused document needs one (see
/// `refresh_outline_if_stale` — without this, a document focused after the
/// panel opened shows "No outline available" until its next edit).
fn on_focused_document_changed(model: &mut AppModel) -> Option<Cmd> {
    model.outline_panel.scroll_offset = 0;
    model.outline_panel.selected_index = None;
    match crate::update::outline::refresh_outline_if_stale(model) {
        Some(refresh) => Some(Cmd::Batch(vec![Cmd::redraw_editor(), refresh])),
        None => Some(Cmd::redraw_editor()),
    }
}

pub(super) fn update_layout(model: &mut AppModel, msg: LayoutMsg) -> Option<Cmd> {
    match msg {
        LayoutMsg::NewTab => {
            new_tab_in_focused_group(model);
            sync_viewports(model);
            ensure_focused_tab_visible(model);
            Some(Cmd::Redraw)
        }

        LayoutMsg::OpenFileInNewTab(path) => {
            open_file_in_group(model, path, model.editor_area.focused_group_id, None)
        }

        LayoutMsg::FilePrepared { request, result } => finish_file_open(model, request, result),

        LayoutMsg::OpenWithDefaultApp(path) => Some(Cmd::OpenInExplorer { path }),

        LayoutMsg::SplitFocused(direction) => {
            split_focused_group(model, direction);
            sync_viewports(model);
            Some(Cmd::Redraw)
        }

        LayoutMsg::SplitGroup {
            group_id,
            direction,
        } => {
            split_group(model, group_id, direction);
            sync_viewports(model);
            Some(Cmd::Redraw)
        }

        LayoutMsg::CloseGroup(group_id) => request_close_group(model, group_id),

        LayoutMsg::CloseFocusedGroup => {
            let group_id = model.editor_area.focused_group_id;
            request_close_group(model, group_id)
        }

        LayoutMsg::FocusGroup(group_id) => {
            if model.editor_area.groups.contains_key(&group_id) {
                model.editor_area.focused_group_id = group_id;
            }
            on_focused_document_changed(model)
        }

        LayoutMsg::FocusNextGroup => {
            focus_adjacent_group(model, true);
            on_focused_document_changed(model)
        }

        LayoutMsg::FocusPrevGroup => {
            focus_adjacent_group(model, false);
            on_focused_document_changed(model)
        }

        LayoutMsg::FocusGroupByIndex(index) => {
            // 1-indexed for keyboard shortcuts (Cmd+1, Cmd+2, etc.)
            let group_ids = model.editor_area.layout.group_ids();
            if index > 0 && index <= group_ids.len() {
                model.editor_area.focused_group_id = group_ids[index - 1];
            }
            on_focused_document_changed(model)
        }

        LayoutMsg::MoveTab { tab_id, to_group } => {
            move_tab(model, tab_id, to_group);
            ensure_active_tab_visible(model, to_group);
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::ReorderTab { tab_id, to_index } => {
            reorder_tab(model, tab_id, to_index);
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::ScrollTabBar { group_id, delta_px } => {
            scroll_tab_bar(model, group_id, delta_px);
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::CloseTab(tab_id) => {
            super::closing::request(model, CloseTarget::Tabs(vec![tab_id]))
        }

        LayoutMsg::CloseOtherTabs { group_id, keep } => {
            request_close_tabs(model, group_id, Some(keep))
        }

        LayoutMsg::CloseAllTabs { group_id } => request_close_tabs(model, group_id, None),

        LayoutMsg::CloseFocusedTab => {
            if let Some(tab) = model
                .editor_area
                .focused_group()
                .and_then(|g| g.active_tab())
            {
                let tab_id = tab.id;
                return super::closing::request(model, CloseTarget::Tabs(vec![tab_id]));
            }
            Some(Cmd::redraw_editor())
        }

        LayoutMsg::NextTab => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if !group.tabs.is_empty() {
                    group.active_tab_index = (group.active_tab_index + 1) % group.tabs.len();
                }
            }
            close_preview_if_not_markdown(model);
            ensure_focused_tab_visible(model);
            on_focused_document_changed(model)
        }

        LayoutMsg::PrevTab => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if !group.tabs.is_empty() {
                    group.active_tab_index = if group.active_tab_index == 0 {
                        group.tabs.len() - 1
                    } else {
                        group.active_tab_index - 1
                    };
                }
            }
            close_preview_if_not_markdown(model);
            ensure_focused_tab_visible(model);
            on_focused_document_changed(model)
        }

        LayoutMsg::SwitchToTab(index) => {
            if let Some(group) = model.editor_area.focused_group_mut() {
                if index < group.tabs.len() {
                    group.active_tab_index = index;
                }
            }
            close_preview_if_not_markdown(model);
            ensure_focused_tab_visible(model);
            on_focused_document_changed(model)
        }

        // === Splitter Dragging ===
        LayoutMsg::BeginSplitterDrag {
            splitter_index,
            position,
        } => {
            begin_splitter_drag(model, splitter_index, position);
            Some(Cmd::Redraw)
        }

        LayoutMsg::UpdateSplitterDrag { position } => {
            update_splitter_drag(model, position);
            Some(Cmd::Redraw)
        }

        LayoutMsg::EndSplitterDrag => {
            model.ui.splitter_drag = None;
            Some(Cmd::Redraw)
        }

        LayoutMsg::CancelSplitterDrag => {
            cancel_splitter_drag(model);
            Some(Cmd::Redraw)
        }
    }
}

// ============================================================================
// Tab Bar Scrolling
// ============================================================================

use crate::layout::editor::EditorTabBarLayout;

/// Scroll the tab bar of `group_id` so the active tab is fully visible,
/// and clamp the scroll offset to the current tab content width.
fn ensure_active_tab_visible(model: &mut AppModel, group_id: GroupId) {
    let char_width = model.char_width;
    let padding = model.metrics.padding_medium;
    let Some(group) = model.editor_area.groups.get(&group_id) else {
        return;
    };

    let layout = EditorTabBarLayout::new(group, model, char_width);
    let rect_w = layout.bar_rect().width.round().max(0.0) as usize;
    let total_width = layout.total_tabs_width();
    let max_scroll = total_width.saturating_sub(rect_w);
    let mut scroll = group.tab_scroll.min(max_scroll);

    if let Some((start, end)) = group
        .tabs
        .get(group.active_tab_index)
        .and_then(|tab| layout.tab_span(tab.id))
    {
        if start.saturating_sub(padding) < scroll {
            // Active tab (partially) off the left edge
            scroll = start.saturating_sub(padding);
        } else if rect_w > 0 && end + padding > scroll + rect_w {
            // Active tab (partially) off the right edge
            scroll = (end + padding).saturating_sub(rect_w);
        }
    }

    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tab_scroll = scroll;
    }
}

/// Ensure the focused group's active tab is scrolled into view.
fn ensure_focused_tab_visible(model: &mut AppModel) {
    ensure_active_tab_visible(model, model.editor_area.focused_group_id);
}

/// Scroll a group's tab bar by a pixel delta, clamped to the tab content.
fn scroll_tab_bar(model: &mut AppModel, group_id: GroupId, delta_px: i32) {
    let char_width = model.char_width;
    let Some(group) = model.editor_area.groups.get(&group_id) else {
        return;
    };

    let layout = EditorTabBarLayout::new(group, model, char_width);
    let total_width = layout.total_tabs_width();
    let rect_w = layout.bar_rect().width.round().max(0.0) as usize;
    let max_scroll = total_width.saturating_sub(rect_w) as i64;
    let new_scroll = (group.tab_scroll as i64 + delta_px as i64).clamp(0, max_scroll) as usize;

    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tab_scroll = new_scroll;
    }
}

/// Move a tab to a new index within its owning group (drag reorder),
/// keeping the same tab active.
fn reorder_tab(model: &mut AppModel, tab_id: TabId, to_index: usize) {
    let Some((group_id, from_index)) = model
        .editor_area
        .groups
        .iter()
        .find_map(|(id, g)| g.tabs.iter().position(|t| t.id == tab_id).map(|i| (*id, i)))
    else {
        return;
    };

    let Some(group) = model.editor_area.groups.get_mut(&group_id) else {
        return;
    };
    let to_index = to_index.min(group.tabs.len().saturating_sub(1));
    if from_index == to_index {
        return;
    }

    let tab = group.tabs.remove(from_index);
    group.tabs.insert(to_index, tab);

    // Keep the active tab pointing at the same tab after the shuffle
    if group.active_tab_index == from_index {
        group.active_tab_index = to_index;
    } else if from_index < group.active_tab_index && to_index >= group.active_tab_index {
        group.active_tab_index -= 1;
    } else if from_index > group.active_tab_index && to_index <= group.active_tab_index {
        group.active_tab_index += 1;
    }

    ensure_active_tab_visible(model, group_id);
}

// ============================================================================
// Layout Helper Functions
// ============================================================================

/// Create a new untitled document in the focused group
fn new_tab_in_focused_group(model: &mut AppModel) {
    let group_id = model.editor_area.focused_group_id;

    // 1. Create new untitled document
    let doc_id = model.editor_area.next_document_id();
    let untitled_name = model.editor_area.next_untitled_name();
    let mut document = Document::new();
    document.id = Some(doc_id);
    document.untitled_name = Some(untitled_name);
    model.editor_area.documents.insert(doc_id, document);

    // 2. Create new editor state for this document
    let editor_id = model.editor_area.next_editor_id();
    let mut editor = EditorState::new();
    editor.id = Some(editor_id);
    editor.document_id = Some(doc_id);
    model.editor_area.editors.insert(editor_id, editor);

    // 3. Create tab in focused group
    let tab_id = model.editor_area.next_tab_id();
    let tab = Tab {
        id: tab_id,
        editor_id,
        is_pinned: false,
        is_preview: false,
    };

    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tabs.push(tab);
        group.active_tab_index = group.tabs.len() - 1;
    }
}

/// Capture the target group and post-open action before starting disk work.
pub(super) fn open_file_in_group(
    model: &mut AppModel,
    path: PathBuf,
    group_id: GroupId,
    position: Option<crate::model::OpenPosition>,
) -> Option<Cmd> {
    begin_file_open(
        model,
        crate::model::FileOpenSource::Path(path),
        group_id,
        position,
        crate::model::FileOpenPolicy::CreateOrOpen,
    )
}

pub(super) fn open_file_for_edit(model: &mut AppModel, path: PathBuf) -> Option<Cmd> {
    begin_file_open(
        model,
        crate::model::FileOpenSource::Path(path),
        model.editor_area.focused_group_id,
        None,
        crate::model::FileOpenPolicy::ExistingText,
    )
}

pub(super) fn open_config_resource(
    model: &mut AppModel,
    resource: crate::commands::ConfigResource,
) -> Option<Cmd> {
    begin_file_open(
        model,
        crate::model::FileOpenSource::Configuration(resource),
        model.editor_area.focused_group_id,
        None,
        crate::model::FileOpenPolicy::CreateOrOpen,
    )
}

fn begin_file_open(
    model: &mut AppModel,
    source: crate::model::FileOpenSource,
    group_id: GroupId,
    position: Option<crate::model::OpenPosition>,
    policy: crate::model::FileOpenPolicy,
) -> Option<Cmd> {
    use crate::model::{PendingFileOpen, PreparedFile};
    let group = model.editor_area.groups.get(&group_id)?;
    let active_tab = group.active_tab().map(|tab| tab.id);
    let origin = (|| {
        let editor_id = group.active_editor_id()?;
        let editor = model.editor_area.editors.get(&editor_id)?;
        let document_id = editor.document_id?;
        let document = model.editor_area.documents.get(&document_id)?;
        Some(crate::model::OpenOrigin {
            editor_id,
            document_id,
            revision: document.revision,
            cursor: *editor.active_cursor(),
            selection: editor.selections.get(editor.active_cursor_index).cloned(),
        })
    })();
    let activates_tab = policy == crate::model::FileOpenPolicy::CreateOrOpen
        && !matches!(
            source,
            crate::model::FileOpenSource::Configuration(crate::commands::ConfigResource::Directory)
        );
    let target = PendingFileOpen {
        group_id,
        focus: model.ui.focus,
        active_tab,
        position,
        origin,
        policy,
        route_hint: if activates_tab {
            model.lsp.route_hint.take()
        } else {
            None
        },
    };
    let sequence = model.editor_area.file_opens.begin(target, activates_tab);
    let request = file_open_request(model, source, sequence);
    // Known original/canonical spellings need no filesystem work. Unknown
    // aliases are resolved by the worker using the same identity snapshots.
    let existing = request
        .source
        .path()
        .and_then(|path| model.editor_area.find_document_by_path(path))
        .and_then(|document_id| {
            Some(PreparedFile::Existing {
                document_id,
                path: model.editor_area.documents[&document_id]
                    .file_path
                    .clone()?,
            })
        });
    if let Some(existing) =
        existing.filter(|_| policy == crate::model::FileOpenPolicy::CreateOrOpen)
    {
        return finish_file_open(model, request, Ok(Box::new(existing)));
    }
    model.ui.is_loading = true;
    model.ui.set_status(format!("Opening: {}", request.source));
    Some(Cmd::PrepareFileOpen(request))
}

fn file_open_request(
    model: &AppModel,
    source: crate::model::FileOpenSource,
    sequence: u64,
) -> crate::model::FileOpenRequest {
    crate::model::FileOpenRequest {
        source,
        sequence,
        policy: model.editor_area.file_opens.pending[&sequence].policy,
        editorconfig: model.config.editorconfig,
        known_documents: model
            .editor_area
            .documents
            .iter()
            .filter_map(|(id, doc)| {
                Some(crate::model::KnownFile {
                    document_id: *id,
                    path: doc.file_path.clone()?,
                    identity: doc.file_identity().cloned(),
                })
            })
            .collect(),
    }
}

fn finish_file_open(
    model: &mut AppModel,
    request: crate::model::FileOpenRequest,
    result: Result<Box<crate::model::PreparedFile>, String>,
) -> Option<Cmd> {
    use crate::model::PreparedFile;
    let target = model
        .editor_area
        .file_opens
        .pending
        .remove(&request.sequence)?;
    model.ui.is_loading = !model.editor_area.file_opens.pending.is_empty()
        || model
            .editor_area
            .documents
            .values()
            .any(|doc| doc.file_io.pending(crate::model::FileRequestKind::Read));
    let Some(group) = model.editor_area.groups.get(&target.group_id) else {
        return reject_file_open(model, request.sequence, None);
    };
    let origin_unchanged = target.origin.as_ref().is_none_or(|origin| {
        model
            .editor_area
            .editors
            .get(&origin.editor_id)
            .is_some_and(|editor| {
                *editor.active_cursor() == origin.cursor
                    && editor.selections.get(editor.active_cursor_index)
                        == origin.selection.as_ref()
                    && editor.document_id == Some(origin.document_id)
                    && editor
                        .document_id
                        .and_then(|id| model.editor_area.documents.get(&id))
                        .is_some_and(|doc| doc.revision == origin.revision)
            })
    });
    let activate = target.policy == crate::model::FileOpenPolicy::CreateOrOpen
        && target.focus == model.ui.focus
        && !model.ui.has_modal()
        && model.editor_area.file_opens.latest.get(&target.group_id) == Some(&request.sequence)
        && group.active_tab().map(|tab| tab.id) == target.active_tab
        && origin_unchanged;
    let prepared = match result {
        Ok(prepared) => *prepared,
        Err(error) => {
            return reject_file_open(model, request.sequence, Some(error));
        }
    };
    let (document_id, new_document) = match prepared {
        PreparedFile::Directory { path } => {
            model.ui.set_status(format!(
                "Opened configuration directory: {}",
                path.display()
            ));
            return Some(Cmd::Batch(vec![
                Cmd::OpenInExplorer { path },
                Cmd::FileOpenFinished {
                    request_id: request.sequence,
                    document_id: None,
                },
                Cmd::redraw_status_bar(),
            ]));
        }
        PreparedFile::Existing { document_id, path } => {
            if model
                .editor_area
                .documents
                .get(&document_id)
                .is_none_or(|doc| {
                    doc.file_path.as_ref() != Some(&path)
                        || request
                            .known_documents
                            .iter()
                            .find(|known| known.document_id == document_id)
                            .and_then(|known| known.identity.as_ref())
                            .is_some_and(|old| {
                                doc.file_identity()
                                    .is_none_or(|current| old.uri() != current.uri())
                            })
                })
            {
                // The worker's document snapshot was closed or renamed. Retry
                // against current state, retaining the original group/intent.
                model
                    .editor_area
                    .file_opens
                    .pending
                    .insert(request.sequence, target);
                model.ui.is_loading = true;
                return Some(Cmd::PrepareFileOpen(file_open_request(
                    model,
                    request.source,
                    request.sequence,
                )));
            }
            (document_id, None)
        }
        PreparedFile::Loaded {
            document,
            view_mode,
            tab_content,
        } => {
            // Another request may have opened this file while disk work ran.
            let existing = document
                .file_path
                .as_deref()
                .and_then(|path| model.editor_area.find_document_by_path(path))
                .or_else(|| {
                    document.file_identity().and_then(|identity| {
                        model.editor_area.find_document_by_path(identity.path())
                    })
                });
            if let Some(id) = existing {
                if target.policy == crate::model::FileOpenPolicy::ExistingText
                    && model.editor_area.documents[&id].buffer != document.buffer
                {
                    return reject_file_open(
                        model,
                        request.sequence,
                        Some("Workspace edit target changed while loading".into()),
                    );
                }
                (id, None)
            } else {
                let id = model.editor_area.next_document_id();
                let mut document = *document;
                document.id = Some(id);
                model.editor_area.documents.insert(id, document);
                (id, Some((view_mode, tab_content)))
            }
        }
    };
    let is_new = new_document.is_some();
    let Some((editor_id, created)) =
        install_file_tab(model, target.group_id, document_id, new_document, activate)
    else {
        return reject_file_open(
            model,
            request.sequence,
            Some("Could not open tab: the source view is no longer available".into()),
        );
    };
    sync_viewports(model);
    ensure_active_tab_visible(model, target.group_id);
    let mut commands = vec![
        Cmd::Redraw,
        Cmd::FileOpenFinished {
            request_id: request.sequence,
            document_id: Some(document_id),
        },
    ];
    if is_new {
        model.record_file_opened(document_id);
        commands.push(Cmd::SaveRecentFiles {
            recent: model.recent_files.clone(),
        });
        let is_text = model
            .editor_area
            .editors
            .get(&editor_id)
            .is_some_and(|editor| editor.is_plain_text_mode());
        if is_text {
            if let Some(cmd) = schedule_syntax_parse(model, document_id) {
                commands.push(cmd);
            }
            // Keep per-open server routing attached to this completion.
            let previous_hint = std::mem::replace(&mut model.lsp.route_hint, target.route_hint);
            if let Some(cmd) = open_lsp_document(model, document_id) {
                commands.push(cmd);
            }
            model.lsp.route_hint = previous_hint;
        }
    }
    if activate || created {
        if let Some(position) = target.position {
            super::navigation::place_open_cursor(model, editor_id, position);
        }
    }
    if activate && model.editor_area.focused_group_id == target.group_id {
        model.ui.set_status(format!("Opened: {}", request.source));
        if target.position.is_some() {
            model.ui.focus_editor();
        }
    }
    commands.extend(super::text_edits::resume_workspace_opens(
        model,
        request.sequence,
        Some(document_id),
    ));
    Some(Cmd::Batch(commands))
}

fn reject_file_open(model: &mut AppModel, request_id: u64, status: Option<String>) -> Option<Cmd> {
    if let Some(status) = status {
        model.ui.set_status(status);
    }
    super::merge_cmds(
        Some(Cmd::Batch(vec![
            Cmd::redraw_status_bar(),
            Cmd::FileOpenFinished {
                request_id,
                document_id: None,
            },
        ])),
        super::text_edits::resume_workspace_opens(model, request_id, None),
    )
}

/// One installer for text, images, binary placeholders and shared documents.
/// Always prefer a tab already in the requesting group over one in another split.
fn install_file_tab(
    model: &mut AppModel,
    group_id: GroupId,
    document_id: crate::model::DocumentId,
    content: Option<(ViewMode, TabContent)>,
    activate: bool,
) -> Option<(crate::model::EditorId, bool)> {
    let group = model.editor_area.groups.get(&group_id)?;
    if let Some((index, editor_id)) = group.tabs.iter().enumerate().find_map(|(index, tab)| {
        (model.editor_area.editors.get(&tab.editor_id)?.document_id == Some(document_id))
            .then_some((index, tab.editor_id))
    }) {
        if activate {
            model
                .editor_area
                .groups
                .get_mut(&group_id)?
                .active_tab_index = index;
        }
        return Some((editor_id, false));
    }
    let (view_mode, tab_content) = content.or_else(|| {
        model
            .editor_area
            .editors
            .values()
            .find(|editor| editor.document_id == Some(document_id))
            .map(|editor| (editor.view_mode.clone(), editor.tab_content.clone()))
    })?;
    let editor_id = model.editor_area.next_editor_id();
    let mut editor = EditorState::new();
    editor.id = Some(editor_id);
    editor.document_id = Some(document_id);
    editor.view_mode = view_mode;
    editor.tab_content = tab_content;
    super::folding::offer_recent(model, &mut editor);
    if let ViewMode::Image(image) = &mut editor.view_mode {
        let group = model.editor_area.groups.get(&group_id)?;
        image.scale = crate::image::ImageState::compute_fit_scale(
            image.width,
            image.height,
            group.rect.width as u32,
            (group.rect.height as usize).saturating_sub(model.metrics.tab_bar_height) as u32,
        );
        image.offset_x = 0.0;
        image.offset_y = 0.0;
        image.user_zoomed = false;
        image.drag = None;
    }
    model.editor_area.editors.insert(editor_id, editor);
    let tab_id = model.editor_area.next_tab_id();
    let group = model.editor_area.groups.get_mut(&group_id)?;
    group.tabs.push(Tab {
        id: tab_id,
        editor_id,
        is_pinned: false,
        is_preview: false,
    });
    if activate {
        group.active_tab_index = group.tabs.len() - 1;
    }
    Some((editor_id, true))
}
/// Split the focused group in the given direction
fn split_focused_group(model: &mut AppModel, direction: SplitDirection) {
    let group_id = model.editor_area.focused_group_id;
    split_group(model, group_id, direction);
}

/// Split a specific group in the given direction
fn split_group(model: &mut AppModel, group_id: GroupId, direction: SplitDirection) {
    let Some(source) = model
        .editor_area
        .groups
        .get(&group_id)
        .and_then(|group| group.active_editor_id())
        .and_then(|id| model.editor_area.editors.get(&id))
    else {
        return;
    };
    let Some(document_id) = source.document_id else {
        return;
    };
    let content = (source.view_mode.clone(), source.tab_content.clone());
    let mut folds = source.folds.clone();
    folds.pending = None;
    let new_group_id = model.editor_area.next_group_id();
    model.editor_area.groups.insert(
        new_group_id,
        EditorGroup {
            id: new_group_id,
            tabs: Vec::new(),
            active_tab_index: 0,
            rect: Default::default(),
            attached_preview: None,
            tab_scroll: 0,
        },
    );
    if let Some((id, _)) = install_file_tab(model, new_group_id, document_id, Some(content), true) {
        if let Some(editor) = model.editor_area.editors.get_mut(&id) {
            editor.folds = folds;
        }
    }
    insert_split_in_layout(
        &mut model.editor_area.layout,
        group_id,
        new_group_id,
        direction,
    );
    model.editor_area.focused_group_id = new_group_id;
}

/// Insert a split into the layout tree, replacing the target group with a split container
fn insert_split_in_layout(
    layout: &mut LayoutNode,
    target_group: GroupId,
    new_group: GroupId,
    direction: SplitDirection,
) {
    match layout {
        LayoutNode::Empty => {}
        LayoutNode::Group(id) if *id == target_group => {
            // Replace this group with a split containing both groups
            *layout = LayoutNode::Split(SplitContainer {
                direction,
                children: vec![
                    LayoutNode::Group(target_group),
                    LayoutNode::Group(new_group),
                ],
                ratios: vec![0.5, 0.5],
                min_sizes: vec![100.0, 100.0],
            });
        }
        LayoutNode::Group(_) => {
            // Not the target group, nothing to do
        }
        LayoutNode::Preview(_) => {
            // Preview panes are not split targets
        }
        LayoutNode::Split(container) => {
            // Recursively search children
            for child in &mut container.children {
                insert_split_in_layout(child, target_group, new_group, direction);
            }
        }
    }
}

/// Close a group and remove it from the layout
fn close_group(
    model: &mut AppModel,
    group_id: GroupId,
) -> Vec<crate::model::editor_area::DocumentId> {
    // Don't close the last group
    if model.editor_area.groups.len() <= 1 {
        return vec![];
    }

    if let Some(preview_id) = model.editor_area.find_preview_for_group(group_id) {
        model.editor_area.close_preview(preview_id);
    }

    // Remove the group from the layout tree
    let removed = remove_group_from_layout(&mut model.editor_area.layout, group_id);
    if !removed {
        return vec![];
    }

    // Clean up the group's tabs and editors
    let mut released_documents = Vec::new();
    if let Some(group) = model.editor_area.groups.remove(&group_id) {
        let mut candidate_docs = Vec::new();
        for tab in group.tabs {
            super::folding::remember_closed(model, tab.editor_id);
            if let Some(editor) = model.editor_area.editors.remove(&tab.editor_id) {
                if let Some(doc_id) = editor.document_id {
                    candidate_docs.push(doc_id);
                }
            }
        }

        candidate_docs.sort_by_key(|doc_id| doc_id.0);
        candidate_docs.dedup_by_key(|doc_id| doc_id.0);

        for doc_id in candidate_docs {
            if release_document_if_unreferenced(model, doc_id) {
                released_documents.push(doc_id);
            }
        }
    }

    // If we closed the focused group, focus another group
    if model.editor_area.focused_group_id == group_id {
        let group_ids = model.editor_area.layout.group_ids();
        if let Some(&new_focus) = group_ids.first() {
            model.editor_area.focused_group_id = new_focus;
        }
    }

    released_documents
}

/// Remove a group from the layout tree, collapsing splits as needed
/// Returns true if the group was found and removed
fn remove_group_from_layout(layout: &mut LayoutNode, group_id: GroupId) -> bool {
    match layout {
        LayoutNode::Empty => false,
        LayoutNode::Group(id) => {
            // Can't remove at this level - parent needs to handle it
            *id == group_id
        }
        LayoutNode::Preview(_) => false,
        LayoutNode::Split(container) => {
            // Find and remove the group from children
            let mut found_index = None;
            for (i, child) in container.children.iter().enumerate() {
                if let LayoutNode::Group(id) = child {
                    if *id == group_id {
                        found_index = Some(i);
                        break;
                    }
                }
            }

            if let Some(index) = found_index {
                container.children.remove(index);
                container.ratios.remove(index);
                if !container.min_sizes.is_empty() {
                    container
                        .min_sizes
                        .remove(index.min(container.min_sizes.len() - 1));
                }

                // Normalize ratios
                let sum: f32 = container.ratios.iter().sum();
                if sum > 0.0 {
                    for ratio in &mut container.ratios {
                        *ratio /= sum;
                    }
                }

                // If only one child remains, collapse the split
                if container.children.len() == 1 {
                    let remaining = container.children.remove(0);
                    *layout = remaining;
                }

                return true;
            }

            // Recursively search children
            for child in &mut container.children {
                if remove_group_from_layout(child, group_id) {
                    // Check if we need to collapse after recursive removal
                    if let LayoutNode::Split(inner) = child {
                        if inner.children.len() == 1 {
                            let remaining = inner.children.remove(0);
                            *child = remaining;
                        }
                    }
                    return true;
                }
            }

            false
        }
    }
}

/// Focus the next or previous group
fn focus_adjacent_group(model: &mut AppModel, next: bool) {
    let group_ids = model.editor_area.layout.group_ids();
    if group_ids.len() <= 1 {
        return;
    }

    let current_idx = group_ids
        .iter()
        .position(|&id| id == model.editor_area.focused_group_id)
        .unwrap_or(0);

    let new_idx = if next {
        (current_idx + 1) % group_ids.len()
    } else if current_idx == 0 {
        group_ids.len() - 1
    } else {
        current_idx - 1
    };

    model.editor_area.focused_group_id = group_ids[new_idx];
}

/// Move a tab to a different group
fn move_tab(model: &mut AppModel, tab_id: TabId, to_group: GroupId) {
    // Verify target group exists before proceeding
    if !model.editor_area.groups.contains_key(&to_group) {
        return;
    }

    // Find the tab and its source group
    let mut found = None;
    for (gid, group) in &model.editor_area.groups {
        if let Some(idx) = group.tabs.iter().position(|t| t.id == tab_id) {
            found = Some((*gid, idx));
            break;
        }
    }

    let (source_group_id, tab_idx) = match found {
        Some(f) => f,
        None => return,
    };

    // Don't allow moving if it would leave the last group empty
    let source_group = match model.editor_area.groups.get(&source_group_id) {
        Some(g) => g,
        None => return,
    };

    if source_group.tabs.len() == 1 && model.editor_area.groups.len() == 1 {
        // Can't move the last tab from the last group
        return;
    }

    // Remove the tab from source group
    let tab = match model.editor_area.groups.get_mut(&source_group_id) {
        Some(group) => group.tabs.remove(tab_idx),
        None => return,
    };

    // Adjust active tab index in source group
    if let Some(source) = model.editor_area.groups.get_mut(&source_group_id) {
        if source.active_tab_index >= source.tabs.len() && !source.tabs.is_empty() {
            source.active_tab_index = source.tabs.len() - 1;
        }
    }

    // Add the tab to the target group
    if let Some(target_group) = model.editor_area.groups.get_mut(&to_group) {
        target_group.tabs.push(tab);
        target_group.active_tab_index = target_group.tabs.len() - 1;
    }

    // If source group is now empty, close it (unless it's the last group)
    if model
        .editor_area
        .groups
        .get(&source_group_id)
        .is_some_and(|g| g.tabs.is_empty())
        && model.editor_area.groups.len() > 1
    {
        close_group(model, source_group_id);
    }
}

/// Close a specific tab
fn close_tab(model: &mut AppModel, tab_id: TabId) -> Vec<crate::model::editor_area::DocumentId> {
    // Find the tab and its group
    let mut found = None;
    for (gid, group) in &model.editor_area.groups {
        if let Some(idx) = group.tabs.iter().position(|t| t.id == tab_id) {
            found = Some((*gid, idx));
            break;
        }
    }

    let (group_id, tab_idx) = match found {
        Some(f) => f,
        None => return vec![],
    };

    // Check if this is the last tab in the last group - don't allow closing it
    let group = match model.editor_area.groups.get(&group_id) {
        Some(g) => g,
        None => return vec![],
    };

    if group.tabs.len() == 1 && model.editor_area.groups.len() == 1 {
        // Can't close the last tab in the last group
        return vec![];
    }

    // Get editor_id and doc_id before removing
    let editor_id = model.editor_area.groups[&group_id].tabs[tab_idx].editor_id;
    let doc_id = model
        .editor_area
        .editors
        .get(&editor_id)
        .and_then(|e| e.document_id);

    // Remove the tab
    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.tabs.remove(tab_idx);
        if group.active_tab_index >= group.tabs.len() && !group.tabs.is_empty() {
            group.active_tab_index = group.tabs.len() - 1;
        }
    }

    // Remove the editor
    super::folding::remember_closed(model, editor_id);
    model.editor_area.editors.remove(&editor_id);

    let mut released_documents = Vec::new();
    if let Some(doc_id) = doc_id {
        if release_document_if_unreferenced(model, doc_id) {
            released_documents.push(doc_id);
        }
    }

    // If the group is now empty, close it (unless it's the last group)
    if model
        .editor_area
        .groups
        .get(&group_id)
        .is_some_and(|g| g.tabs.is_empty())
        && model.editor_area.groups.len() > 1
    {
        released_documents.extend(close_group(model, group_id));
    } else {
        model.editor_area.on_group_active_tab_changed(group_id);
    }

    released_documents
}

fn request_close_group(model: &mut AppModel, group_id: GroupId) -> Option<Cmd> {
    if model.editor_area.groups.len() <= 1 {
        return None;
    }
    request_close_tabs(model, group_id, None)
}

/// Capture the tabs, not a future mutable group. Tabs opened while saving survive.
fn request_close_tabs(model: &mut AppModel, group_id: GroupId, keep: Option<TabId>) -> Option<Cmd> {
    let tabs = model
        .editor_area
        .groups
        .get(&group_id)?
        .tabs
        .iter()
        .filter(|tab| Some(tab.id) != keep)
        .map(|tab| tab.id)
        .collect();
    super::closing::request(model, CloseTarget::Tabs(tabs))
}

/// Called only after the shared close gate approves the captured tabs.
pub(super) fn close_tabs(model: &mut AppModel, tabs: &[TabId]) -> Cmd {
    let released = closable_tabs(model, tabs)
        .into_iter()
        .flat_map(|tab| close_tab(model, tab))
        .collect();
    ensure_focused_tab_visible(model);
    with_released_documents(Cmd::Redraw, released)
}

/// Preserve one tab, in the same request order for confirmation and removal.
pub(super) fn closable_tabs(model: &AppModel, requested: &[TabId]) -> Vec<TabId> {
    let open: Vec<_> = model
        .editor_area
        .groups
        .values()
        .flat_map(|group| group.tabs.iter().map(|tab| tab.id))
        .collect();
    requested
        .iter()
        .copied()
        .filter(|id| open.contains(id))
        .take(open.len().saturating_sub(1))
        .collect()
}

// ============================================================================
// Splitter Drag Helper Functions
// ============================================================================

/// Begin dragging a splitter
fn begin_splitter_drag(model: &mut AppModel, splitter_index: usize, position: (f32, f32)) {
    // We need to find:
    // 1. The SplitterBar for this index (to get direction and local index)
    // 2. The container that owns this splitter (to get ratios and size)
    // 3. The container's size in the relevant direction

    // First compute the layout to get splitter info
    // Prefer the last rendered rect; before the first render, use the same
    // solved shell rectangle that will drive rendering and hit-testing.
    let available = model.editor_area.last_layout_rect.unwrap_or_else(|| {
        crate::layout::chrome::shell(model)
            .rect(crate::layout::UiKey::EditorArea)
            .expect("window shell always declares the editor area")
    });
    let splitters = model
        .editor_area
        .compute_layout_scaled(available, model.metrics.splitter_width);

    // Get the splitter bar
    let splitter = match splitters.get(splitter_index) {
        Some(s) => *s,
        None => return,
    };

    // Find the container and its size by traversing the layout tree
    let mut current_index = 0;
    let container_info = find_container_for_splitter(
        &model.editor_area.layout,
        splitter_index,
        &mut current_index,
        available,
    );

    let (original_ratios, container_size) = match container_info {
        Some(info) => info,
        None => return,
    };

    model.ui.splitter_drag = Some(SplitterDragState {
        splitter_index,
        local_index: splitter.index,
        start_position: position,
        original_ratios,
        direction: splitter.direction,
        container_size,
        active: false,
    });
}

/// Update splitter position during drag
fn update_splitter_drag(model: &mut AppModel, position: (f32, f32)) {
    // Extract needed fields without cloning the Vec<f32> on every frame
    let (splitter_index, local_idx, start_pos, direction, container_size, active) =
        match model.ui.splitter_drag.as_ref() {
            Some(state) => (
                state.splitter_index,
                state.local_index,
                state.start_position,
                state.direction,
                state.container_size,
                state.active,
            ),
            None => return,
        };

    // Calculate delta from start position
    let delta = match direction {
        SplitDirection::Horizontal => position.0 - start_pos.0,
        SplitDirection::Vertical => position.1 - start_pos.1,
    };

    // Check threshold if not yet active
    if !active {
        let distance =
            ((position.0 - start_pos.0).powi(2) + (position.1 - start_pos.1).powi(2)).sqrt();
        if distance < DRAG_THRESHOLD_PIXELS {
            return; // Threshold not exceeded yet
        }
        // Activate the drag
        if let Some(ref mut state) = model.ui.splitter_drag {
            state.active = true;
        }
    }

    // Get the two ratios we're adjusting (defensive: return early if indices invalid)
    let (left_ratio, right_ratio) = match model.ui.splitter_drag.as_ref() {
        Some(state) => {
            match (
                state.original_ratios.get(local_idx),
                state.original_ratios.get(local_idx + 1),
            ) {
                (Some(&l), Some(&r)) => (l, r),
                _ => return, // Layout changed underneath us; ignore this drag frame
            }
        }
        None => return,
    };

    // Calculate new ratios
    let ratio_delta = delta / container_size;
    let combined = left_ratio + right_ratio;

    // Calculate minimum ratio based on minimum pane size
    // Guard against tiny containers where 2*MIN_PANE_SIZE > container_size
    // which would cause clamp(min, max) to panic when min > max
    let raw_min_ratio = MIN_PANE_SIZE_PIXELS / container_size;
    let max_min_ratio = combined / 2.0;
    let effective_min_ratio = if raw_min_ratio <= max_min_ratio {
        raw_min_ratio
    } else {
        // Container too small to satisfy min pane size for both panes;
        // allow smaller panes but keep ratios non-negative
        0.01 // Small epsilon to prevent zero-width panes
    };

    // Apply delta with constraints
    let new_left =
        (left_ratio + ratio_delta).clamp(effective_min_ratio, combined - effective_min_ratio);
    let new_right = combined - new_left;

    // Find and update the container
    update_container_ratios_by_splitter(
        &mut model.editor_area.layout,
        splitter_index,
        local_idx,
        new_left,
        new_right,
    );
}

/// Cancel splitter drag and restore original ratios
fn cancel_splitter_drag(model: &mut AppModel) {
    let drag_state = match model.ui.splitter_drag.take() {
        Some(state) => state,
        None => return,
    };

    // Restore original ratios
    restore_container_ratios_by_splitter(
        &mut model.editor_area.layout,
        drag_state.splitter_index,
        &drag_state.original_ratios,
    );
}

// ============================================================================
// Splitter Container Traversal Helpers
// ============================================================================

/// Generic helper to visit the container owning a splitter by global index.
///
/// The layout tree is traversed depth-first. Each SplitContainer with N children
/// owns N-1 splitters (one between each pair of children). The global splitter
/// index is computed by summing splitter counts during traversal.
///
/// When the target container is found, the closure `f` is called with:
/// - A mutable reference to the container
/// - The local index within that container (which child boundary)
///
/// Returns true if the target was found and the closure was called.
fn visit_splitter_container_mut<F>(
    layout: &mut LayoutNode,
    target_index: usize,
    current_index: &mut usize,
    f: &mut F,
) -> bool
where
    F: FnMut(&mut SplitContainer, usize),
{
    match layout {
        LayoutNode::Empty => false,
        LayoutNode::Group(_) => false,
        LayoutNode::Preview(_) => false,
        LayoutNode::Split(container) => {
            let splitter_count = container.children.len().saturating_sub(1);

            // Check if target is in this container's splitter range
            if target_index >= *current_index && target_index < *current_index + splitter_count {
                let local_idx = target_index - *current_index;
                f(container, local_idx);
                return true;
            }

            *current_index += splitter_count;

            // Recurse into children
            for child in &mut container.children {
                if visit_splitter_container_mut(child, target_index, current_index, f) {
                    return true;
                }
            }

            false
        }
    }
}

/// Find the container that owns a given splitter by global index.
/// Returns (original_ratios, container_size) if found.
///
/// This needs a separate implementation because it requires calculating
/// child rects during traversal to determine container size.
fn find_container_for_splitter(
    layout: &LayoutNode,
    target_index: usize,
    current_index: &mut usize,
    rect: Rect,
) -> Option<(Vec<f32>, f32)> {
    match layout {
        LayoutNode::Empty => None,
        LayoutNode::Group(_) => None,
        LayoutNode::Preview(_) => None,
        LayoutNode::Split(container) => {
            let splitter_count = container.children.len().saturating_sub(1);

            // Check if target is in this container
            if target_index >= *current_index && target_index < *current_index + splitter_count {
                let size = match container.direction {
                    SplitDirection::Horizontal => rect.width,
                    SplitDirection::Vertical => rect.height,
                };
                return Some((container.ratios.clone(), size));
            }

            *current_index += splitter_count;

            container.child_rects(rect).find_map(|(child, child_rect)| {
                find_container_for_splitter(child, target_index, current_index, child_rect)
            })
        }
    }
}

/// Update ratios in the container that owns the target splitter.
///
/// Adjusts the two adjacent ratios (at local_idx and local_idx+1) to new values.
fn update_container_ratios_by_splitter(
    layout: &mut LayoutNode,
    target_index: usize,
    local_idx: usize,
    new_left: f32,
    new_right: f32,
) -> bool {
    let mut current_index = 0;
    visit_splitter_container_mut(
        layout,
        target_index,
        &mut current_index,
        &mut |container, _| {
            if local_idx < container.ratios.len() && local_idx + 1 < container.ratios.len() {
                container.ratios[local_idx] = new_left;
                container.ratios[local_idx + 1] = new_right;
            }
        },
    )
}

/// Restore original ratios to the container that owns the target splitter.
fn restore_container_ratios_by_splitter(
    layout: &mut LayoutNode,
    target_index: usize,
    original_ratios: &[f32],
) -> bool {
    let mut current_index = 0;
    visit_splitter_container_mut(
        layout,
        target_index,
        &mut current_index,
        &mut |container, _| {
            container.ratios = original_ratios.to_vec();
        },
    )
}

/// Close preview pane if the focused group's active tab changed.
/// Called when switching tabs to ensure preview stays relevant.
fn close_preview_if_not_markdown(model: &mut AppModel) {
    let group_id = model.editor_area.focused_group_id;
    model.editor_area.on_group_active_tab_changed(group_id);
}

fn with_released_documents(
    base: Cmd,
    released_documents: Vec<crate::model::editor_area::DocumentId>,
) -> Cmd {
    if released_documents.is_empty() {
        return base;
    }

    let mut cmds = Vec::with_capacity(1 + released_documents.len() * 2);
    cmds.push(base);
    for document_id in released_documents {
        cmds.push(Cmd::ClearSyntaxState { document_id });
        // `didClose` — never on tab close alone, only when the document
        // is actually released (refcounted across splits/groups).
        cmds.push(close_lsp_document(document_id));
    }
    Cmd::Batch(cmds)
}

fn release_document_if_unreferenced(
    model: &mut AppModel,
    doc_id: crate::model::editor_area::DocumentId,
) -> bool {
    if !model.editor_area.editors_for_document(doc_id).is_empty() {
        return false;
    }

    model.editor_area.close_previews_for_document(doc_id);
    model.editor_area.documents.remove(&doc_id).is_some()
}

/// Sync all editor viewports to their group's actual dimensions.
/// Call after creating new editors or changing group layout.
fn sync_viewports(model: &mut AppModel) {
    model.resync_viewports();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_fixture_layout(model: &mut AppModel, msg: crate::messages::LayoutMsg) -> Option<Cmd> {
        let cmd = crate::update::layout::update_layout(model, msg);
        crate::update::finish_test_file_opens(model, cmd)
    }
    use crate::messages::LayoutMsg;

    /// Opening a file already open in another split group must never
    /// steal focus into that group — it opens/reuses the document in the
    /// *focused* group instead (design doc's "a jump in split A never
    /// yanks split B").
    #[test]
    fn open_file_already_open_in_another_group_stays_in_the_focused_group() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.rs");
        let b = dir.path().join("b.rs");
        std::fs::write(&a, "fn a() {}\n").unwrap();
        std::fs::write(&b, "fn b() {}\n").unwrap();

        let mut model = AppModel::with_document(
            800,
            600,
            1.0,
            crate::model::Document::from_file(a.clone()).unwrap(),
        );
        let group_a = model.editor_area.focused_group_id;

        split_focused_group(&mut model, SplitDirection::Vertical);
        let group_b = model.editor_area.focused_group_id;
        assert_ne!(group_a, group_b, "split must focus the new group");

        // Open `b.rs` in group B, then jump back to group A and reopen it
        // from there — group A must end up showing `b.rs`, not steal focus
        // into group B.
        open_fixture_layout(&mut model, LayoutMsg::OpenFileInNewTab(b.clone()));
        assert_eq!(model.editor_area.focused_group_id, group_b);

        model.editor_area.focused_group_id = group_a;
        open_fixture_layout(&mut model, LayoutMsg::OpenFileInNewTab(b.clone()));

        assert_eq!(
            model.editor_area.focused_group_id, group_a,
            "opening a file already open elsewhere must not move focus off the requesting group"
        );
        assert_eq!(
            model
                .document()
                .file_path
                .as_deref()
                .map(|p| p.canonicalize().unwrap()),
            Some(b.canonicalize().unwrap()),
        );
    }
}
