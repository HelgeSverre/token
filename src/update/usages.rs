//! Deterministic persistent usages interaction. The existing references request,
//! preview worker and navigation path remain the only effect pipeline.

use std::sync::Arc;

use crate::commands::Cmd;
use crate::messages::{DockMsg, ReferencesOutcome, UsagesMsg};
use crate::model::usages::{UsagesRow, MAX_REFERENCE_LOCATIONS};
use crate::model::{AppModel, DocumentId, FocusTarget};
use crate::panel::PanelId;

pub(super) fn request(model: &mut AppModel, panel: bool) -> Option<Cmd> {
    let snapshot = model
        .try_document()
        .filter(|_| model.editor().is_plain_text_mode())
        .and_then(|doc| {
            let cursor = model.editor().active_cursor().to_position();
            Some((
                doc.id?,
                doc.revision,
                doc.file_path.clone()?,
                cursor,
                crate::lsp::position_to_lsp(doc, cursor),
            ))
        });
    let (target, opened) = if panel {
        let source = snapshot
            .as_ref()
            .map(|(_, _, path, cursor, _)| {
                format!(
                    "{}:{}:{}",
                    path.display(),
                    cursor.line.saturating_add(1),
                    cursor.column.saturating_add(1)
                )
            })
            .unwrap_or_default();
        let origin = snapshot.as_ref().map(|(id, revision, ..)| (*id, *revision));
        let (token, opened) = begin(model, source, origin);
        (crate::model::usages::ReferencesTarget::Panel(token), opened)
    } else {
        cancel_pending(model);
        (crate::model::usages::ReferencesTarget::Popup, None)
    };
    let Some((document_id, revision, _, cursor, position)) = snapshot else {
        if panel {
            model.usages_panel.query = None;
            model.usages_panel.status = "Find Usages requires a saved text file".into();
        }
        return opened.or(Some(Cmd::Redraw));
    };
    super::navigation::combine(
        opened,
        Some(Cmd::LspRequestReferences {
            target,
            document_id,
            revision,
            cursor,
            position,
        }),
    )
}

fn begin(
    model: &mut AppModel,
    source: String,
    origin: Option<(DocumentId, u64)>,
) -> (Arc<()>, Option<Cmd>) {
    let token = Arc::new(());
    let panel = &mut model.usages_panel;
    panel.items.clear();
    panel.collapsed.clear();
    panel.selected_index = None;
    panel.scroll_offset = 0;
    panel.source = source;
    panel.status = "Searching…".into();
    panel.query = origin.map(
        |(document_id, revision)| crate::model::usages::UsagesQuery {
            token: Arc::clone(&token),
            document_id,
            revision,
        },
    );
    model.ui.cursor_overlay = None;
    model.ui.reference_list = None;
    let command = super::dock::update_dock(model, DockMsg::ActivatePanel(PanelId::Usages));
    (token, command)
}

/// One LSP references request may supersede another. Keep cancellation visible
/// in the persistent panel instead of leaving a loading indicator indefinitely.
fn cancel_pending(model: &mut AppModel) {
    if model.usages_panel.query.take().is_some() {
        model.usages_panel.status = "Search cancelled by a newer usages request".into();
    }
}

pub(super) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    let query = model.usages_panel.query.as_ref()?;
    let valid = model
        .editor_area
        .documents
        .get(&query.document_id)
        .is_some_and(|document| document.revision == query.revision);
    if valid {
        return None;
    }
    model.usages_panel.query = None;
    model.usages_panel.status = "Source changed or closed; run Find Usages again".into();
    Some(Cmd::Redraw)
}

pub(super) fn resolve(
    model: &mut AppModel,
    token: Arc<()>,
    document_id: DocumentId,
    revision: u64,
    mut items: Vec<super::navigation::LocationItem>,
    outcome: ReferencesOutcome,
) -> Option<Cmd> {
    if !model
        .usages_panel
        .query
        .as_ref()
        .is_some_and(|query| Arc::ptr_eq(&query.token, &token))
    {
        return None;
    }
    let unchanged = model
        .editor_area
        .documents
        .get(&document_id)
        .is_some_and(|document| document.revision == revision);
    let panel = &mut model.usages_panel;
    panel.query = None;
    panel.status = if !unchanged {
        "Source changed or closed; run Find Usages again".into()
    } else {
        match outcome {
            ReferencesOutcome::Found => {
                items.sort_by(|a, b| {
                    (&a.path, a.position.line, a.position.character).cmp(&(
                        &b.path,
                        b.position.line,
                        b.position.character,
                    ))
                });
                items.dedup_by(|a, b| a.path == b.path && a.position == b.position);
                items.truncate(MAX_REFERENCE_LOCATIONS);
                panel.items = items;
                panel.selected_index = (!panel.items.is_empty()).then_some(1);
                let count = panel.items.len();
                if count >= MAX_REFERENCE_LOCATIONS {
                    format!("Showing {count} usages (result limit {MAX_REFERENCE_LOCATIONS})")
                } else if count == 0 {
                    "No usages found".into()
                } else {
                    format!("{count} usages")
                }
            }
            ReferencesOutcome::StillIndexing => "Language server still indexing; try again".into(),
            ReferencesOutcome::NotSupported => {
                "Usages unavailable: server stopped or references not supported".into()
            }
            ReferencesOutcome::NoResult => "No usages found".into(),
            ReferencesOutcome::TimedOut => "Usages request timed out; try again".into(),
        }
    };
    // Receiving results never changes focus or reopens a dock the user closed.
    Some(Cmd::Redraw)
}

pub fn row_label(model: &AppModel, row: UsagesRow) -> String {
    let panel = &model.usages_panel;
    match row {
        UsagesRow::Summary if panel.source.is_empty() => panel.status.clone(),
        UsagesRow::Summary => format!("{} — {}", panel.status, panel.source),
        UsagesRow::File { first, count, .. } => {
            let Some(item) = panel.items.get(first) else {
                return String::new();
            };
            let path = &item.path;
            let name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy();
            let display = model
                .workspace_root()
                .and_then(|root| path.strip_prefix(root).ok())
                .unwrap_or(path);
            format!("{name} ({count}) — {}", display.display())
        }
        UsagesRow::Location(index) => {
            let Some(item) = panel.items.get(index) else {
                return String::new();
            };
            let (line, column) = item.display_position(model);
            format!(
                "{}:{}  {}",
                line.saturating_add(1),
                column.saturating_add(1),
                item.preview
            )
        }
    }
}

fn rows_view(model: &AppModel) -> Option<crate::layout::RowListView> {
    crate::layout::chrome::chrome(model).row_list(crate::layout::UiKey::PanelRows(PanelId::Usages))
}

fn reveal(model: &mut AppModel) {
    if let (Some(view), Some(selected)) = (rows_view(model), model.usages_panel.selected_index) {
        model.usages_panel.scroll_offset =
            view.scroll_to_reveal(model.usages_panel.scroll_offset, selected);
    }
}

fn selected(model: &AppModel) -> Option<UsagesRow> {
    model
        .usages_panel
        .rows()
        .get(model.usages_panel.selected_index?)
        .copied()
}

fn expand(model: &mut AppModel, expanded: bool) {
    let first = match selected(model) {
        Some(UsagesRow::File { first, .. }) | Some(UsagesRow::Location(first)) => first,
        _ => return,
    };
    let path = model.usages_panel.items[first].path.clone();
    if expanded {
        model.usages_panel.collapsed.remove(&path);
    } else {
        model.usages_panel.collapsed.insert(path.clone());
    }
    if !expanded {
        model.usages_panel.selected_index = model.usages_panel.rows().iter().position(|row| {
            matches!(row, UsagesRow::File { first, .. } if model.usages_panel.items[*first].path == path)
        });
    }
    reveal(model);
}

fn activate(model: &mut AppModel) -> Option<Cmd> {
    match selected(model) {
        Some(UsagesRow::File { collapsed, .. }) => {
            expand(model, collapsed);
            Some(Cmd::Redraw)
        }
        Some(UsagesRow::Location(index)) => {
            let item = model.usages_panel.items[index].clone();
            model.ui.focus = FocusTarget::Editor;
            super::navigation::activate_location(model, &item)
        }
        _ => None,
    }
}

pub(super) fn update_usages(model: &mut AppModel, message: UsagesMsg) -> Option<Cmd> {
    match message {
        UsagesMsg::Select { delta, page } => {
            let count = model.usages_panel.rows().len();
            if count > 1 {
                let step = if page {
                    rows_view(model).map_or(1, |view| view.visible_capacity().max(1))
                } else {
                    1
                };
                let current = model.usages_panel.selected_index.unwrap_or(1);
                let distance = (delta.unsigned_abs() as usize).saturating_mul(step);
                let next = if delta < 0 {
                    current.saturating_sub(distance)
                } else {
                    current.saturating_add(distance)
                };
                model.usages_panel.selected_index = Some(next.clamp(1, count - 1));
                reveal(model);
            }
        }
        UsagesMsg::SetExpanded(expanded) => expand(model, expanded),
        UsagesMsg::OpenSelected => return activate(model),
        UsagesMsg::Scroll { lines } => {
            if let Some(view) = rows_view(model) {
                let current = model.usages_panel.scroll_offset;
                let distance = lines.unsigned_abs() as usize;
                let next = if lines < 0 {
                    current.saturating_sub(distance)
                } else {
                    current.saturating_add(distance)
                };
                model.usages_panel.scroll_offset = view.clamp_scroll(next);
            }
        }
        UsagesMsg::ClickRow {
            index,
            click_count,
            on_chevron,
        } => {
            let rows = model.usages_panel.rows();
            if index == 0 || index >= rows.len() {
                return None;
            }
            model.usages_panel.selected_index = Some(index);
            if on_chevron || click_count >= 2 {
                return activate(model);
            }
        }
    }
    Some(Cmd::Redraw)
}
