//! Outline panel update handlers

use crate::commands::Cmd;
use crate::messages::OutlineMsg;
use crate::model::AppModel;
use crate::outline::OutlineNode;
use crate::update::navigation::{clamp_to_document, push_history};
use crate::util::{visible_tree_count, visible_tree_row_at_index};
use crate::view::geometry::{DockHeaderLayout, OutlinePanelLayout, WindowLayout};

fn count_visible_items(nodes: &[OutlineNode], panel: &crate::model::OutlinePanelState) -> usize {
    visible_tree_count(nodes, |node: &OutlineNode| {
        node.is_collapsible() && !panel.is_collapsed(node)
    })
}

fn visible_node_at_index<'a>(
    nodes: &'a [OutlineNode],
    panel: &crate::model::OutlinePanelState,
    target: usize,
) -> Option<&'a OutlineNode> {
    visible_tree_row_at_index(nodes, target, |node: &OutlineNode| {
        node.is_collapsible() && !panel.is_collapsed(node)
    })
    .map(|row| row.node)
}

fn outline_visible_capacity(model: &AppModel) -> usize {
    WindowLayout::compute(model)
        .right_dock_rect
        .map(|rect| {
            let dock_layout = DockHeaderLayout::new(
                &model.dock_layout.right,
                rect,
                &model.metrics,
                model.char_width,
            );
            OutlinePanelLayout::new(dock_layout.content_rect, &model.metrics).visible_capacity()
        })
        .unwrap_or(0)
}

fn reveal_outline_selection(model: &mut AppModel) {
    let Some(selected_index) = model.outline_panel.selected_index else {
        return;
    };
    let visible_capacity = outline_visible_capacity(model);
    if visible_capacity == 0 {
        return;
    }

    let scroll_offset = model.outline_panel.scroll_offset;
    model.outline_panel.scroll_offset = if selected_index < scroll_offset {
        selected_index
    } else if selected_index >= scroll_offset.saturating_add(visible_capacity) {
        selected_index.saturating_add(1) - visible_capacity
    } else {
        scroll_offset
    };
}

/// Handle outline panel messages
/// Schedule an outline extraction for the focused document when the outline
/// panel is open and its cached outline is missing or stale. Returns `None`
/// when nothing needs doing.
///
/// This must run on every focused-document change, not just when the panel
/// is activated: a document focused *after* the panel opened has had all its
/// parses run with `extract_outline: false`, and without a refresh its
/// outline stays empty until the next edit (most visible with markdown,
/// which is often read without ever being edited).
pub(crate) fn refresh_outline_if_stale(model: &AppModel) -> Option<Cmd> {
    let dock = &model.dock_layout.right;
    if !(dock.is_open && dock.active_panel() == Some(crate::panel::PanelId::OUTLINE)) {
        return None;
    }
    let document = model.document();
    if !document.language.has_highlighting() {
        return None;
    }
    let outline_is_current = document
        .outline
        .as_ref()
        .is_some_and(|outline| outline.revision == document.revision);
    if outline_is_current {
        return None;
    }
    let document_id = document.id?;
    Some(Cmd::DebouncedSyntaxParse {
        document_id,
        revision: document.revision,
        delay_ms: 0,
    })
}

pub fn update_outline(model: &mut AppModel, msg: OutlineMsg) -> Option<Cmd> {
    match msg {
        OutlineMsg::JumpToSymbol { line, col } => {
            // Move cursor to the symbol and focus the editor. Clamp
            // defensively: a stale outline (built before the document was
            // edited) could otherwise carry an out-of-range position.
            push_history(model);
            let (clamped_line, clamped_col) = clamp_to_document(model, line, col);
            let editor = model.editor_mut();
            editor.cursors[0].line = clamped_line;
            editor.cursors[0].column = clamped_col;
            editor.cursors[0].desired_column = None;
            editor.clear_selection();
            model.ensure_cursor_visible_centered();
            model.ui.focus_editor();
            Some(Cmd::Redraw)
        }

        OutlineMsg::ToggleNode { line, name } => {
            // Find node by line+name and toggle
            let outline = model
                .editor_area
                .focused_document()
                .and_then(|doc| doc.outline.as_ref());

            if let Some(outline) = outline {
                // Find the matching node
                fn find_node<'a>(
                    nodes: &'a [OutlineNode],
                    line: usize,
                    name: &str,
                ) -> Option<&'a OutlineNode> {
                    for node in nodes {
                        if node.range.start_line == line && node.name == name {
                            return Some(node);
                        }
                        if let Some(found) = find_node(&node.children, line, name) {
                            return Some(found);
                        }
                    }
                    None
                }

                if let Some(node) = find_node(&outline.roots, line, &name) {
                    model.outline_panel.toggle_collapsed(node);
                }
            }
            Some(Cmd::Redraw)
        }

        OutlineMsg::SelectPrevious => {
            if let Some(idx) = model.outline_panel.selected_index {
                if idx > 0 {
                    model.outline_panel.selected_index = Some(idx - 1);
                }
            } else {
                model.outline_panel.selected_index = Some(0);
            }
            reveal_outline_selection(model);
            Some(Cmd::Redraw)
        }

        OutlineMsg::SelectNext => {
            let outline = model
                .editor_area
                .focused_document()
                .and_then(|doc| doc.outline.as_ref());

            if let Some(outline) = outline {
                let total = count_visible_items(&outline.roots, &model.outline_panel);
                if let Some(idx) = model.outline_panel.selected_index {
                    if idx + 1 < total {
                        model.outline_panel.selected_index = Some(idx + 1);
                    }
                } else {
                    model.outline_panel.selected_index = Some(0);
                }
            }
            reveal_outline_selection(model);
            Some(Cmd::Redraw)
        }

        OutlineMsg::ExpandSelected => {
            if let Some(idx) = model.outline_panel.selected_index {
                let outline = model
                    .editor_area
                    .focused_document()
                    .and_then(|doc| doc.outline.as_ref());

                if let Some(outline) = outline {
                    if let Some(node) =
                        visible_node_at_index(&outline.roots, &model.outline_panel, idx)
                    {
                        if node.is_collapsible() {
                            let key = crate::model::OutlinePanelState::node_key(node);
                            model.outline_panel.collapsed.remove(&key);
                        }
                    }
                    let total = count_visible_items(&outline.roots, &model.outline_panel);
                    model.outline_panel.scroll_offset = model
                        .outline_panel
                        .scroll_offset
                        .min(total.saturating_sub(1));
                }
            }
            Some(Cmd::Redraw)
        }

        OutlineMsg::CollapseSelected => {
            if let Some(idx) = model.outline_panel.selected_index {
                let outline = model
                    .editor_area
                    .focused_document()
                    .and_then(|doc| doc.outline.as_ref());

                if let Some(outline) = outline {
                    if let Some(node) =
                        visible_node_at_index(&outline.roots, &model.outline_panel, idx)
                    {
                        if node.is_collapsible() {
                            let key = crate::model::OutlinePanelState::node_key(node);
                            model.outline_panel.collapsed.insert(key);
                        }
                    }
                    let total = count_visible_items(&outline.roots, &model.outline_panel);
                    model.outline_panel.scroll_offset = model
                        .outline_panel
                        .scroll_offset
                        .min(total.saturating_sub(1));
                }
            }
            Some(Cmd::Redraw)
        }

        OutlineMsg::OpenSelected => {
            if let Some(idx) = model.outline_panel.selected_index {
                let outline = model
                    .editor_area
                    .focused_document()
                    .and_then(|doc| doc.outline.as_ref());

                if let Some(outline) = outline {
                    if let Some(node) =
                        visible_node_at_index(&outline.roots, &model.outline_panel, idx)
                    {
                        let line = node.range.start_line;
                        let col = node.range.start_col;
                        // Jump to symbol and focus editor
                        let editor = model.editor_mut();
                        editor.cursors[0].line = line;
                        editor.cursors[0].column = col;
                        editor.cursors[0].desired_column = None;
                        editor.clear_selection();
                        model.ensure_cursor_visible_centered();
                        model.ui.focus_editor();
                    }
                }
            }
            Some(Cmd::Redraw)
        }

        OutlineMsg::Scroll { lines } => {
            let offset = model.outline_panel.scroll_offset;
            if lines < 0 {
                model.outline_panel.scroll_offset = offset.saturating_sub((-lines) as usize);
            } else {
                model.outline_panel.scroll_offset = offset.saturating_add(lines as usize);
            }

            let outline = model
                .editor_area
                .focused_document()
                .and_then(|doc| doc.outline.as_ref());

            if let Some(outline) = outline {
                let total = count_visible_items(&outline.roots, &model.outline_panel);
                let visible_capacity = outline_visible_capacity(model);

                model.outline_panel.scroll_offset = if visible_capacity == 0 {
                    0
                } else {
                    model
                        .outline_panel
                        .scroll_offset
                        .min(total.saturating_sub(visible_capacity))
                };
            } else {
                model.outline_panel.scroll_offset = 0;
            }

            Some(Cmd::Redraw)
        }

        OutlineMsg::ClickRow {
            index,
            click_count,
            on_chevron,
        } => {
            let outline = model
                .editor_area
                .focused_document()
                .and_then(|doc| doc.outline.as_ref());

            if let Some(outline) = outline {
                if let Some(node) =
                    visible_node_at_index(&outline.roots, &model.outline_panel, index)
                {
                    model.outline_panel.selected_index = Some(index);
                    if on_chevron && node.is_collapsible() {
                        model.outline_panel.toggle_collapsed(node);
                    } else if click_count >= 2 {
                        let line = node.range.start_line;
                        let col = node.range.start_col;
                        push_history(model);
                        let (clamped_line, clamped_col) = clamp_to_document(model, line, col);
                        let editor = model.editor_mut();
                        editor.cursors[0].line = clamped_line;
                        editor.cursors[0].column = clamped_col;
                        editor.cursors[0].desired_column = None;
                        editor.clear_selection();
                        model.ensure_cursor_visible_centered();
                        model.ui.focus_editor();
                    }
                }
            }
            Some(Cmd::Redraw)
        }
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    use crate::commands::Cmd;
    use crate::messages::LayoutMsg;
    use crate::outline::{OutlineData, OutlineKind, OutlineRange};
    use crate::panel::PanelId;

    fn model_with_open_outline_panel() -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.document_mut().language = crate::syntax::LanguageId::Markdown;
        model.dock_layout.right.activate(PanelId::OUTLINE);
        model
    }

    fn populate_outline(model: &mut AppModel, count: usize) {
        let revision = model.document().revision;
        model.document_mut().outline = Some(OutlineData {
            revision,
            roots: (0..count)
                .map(|line| OutlineNode {
                    kind: OutlineKind::Function,
                    name: format!("symbol_{line}"),
                    range: OutlineRange {
                        start_line: line,
                        start_col: 0,
                        end_line: line,
                        end_col: 1,
                    },
                    children: Vec::new(),
                })
                .collect(),
        });
    }

    #[test]
    fn stale_outline_schedules_a_refresh() {
        let model = model_with_open_outline_panel();
        assert!(matches!(
            refresh_outline_if_stale(&model),
            Some(Cmd::DebouncedSyntaxParse { delay_ms: 0, .. })
        ));
    }

    #[test]
    fn current_outline_schedules_nothing() {
        let mut model = model_with_open_outline_panel();
        let revision = model.document().revision;
        model.document_mut().outline = Some(crate::outline::OutlineData::empty(revision));
        assert!(refresh_outline_if_stale(&model).is_none());
    }

    #[test]
    fn closed_panel_schedules_nothing() {
        let mut model = model_with_open_outline_panel();
        model.dock_layout.right.is_open = false;
        assert!(refresh_outline_if_stale(&model).is_none());
    }

    #[test]
    fn focus_change_with_open_panel_batches_the_refresh() {
        // Regression: switching to a document after the outline panel was
        // opened showed "No outline available" until the next edit —
        // tab/group focus changes never scheduled an extraction.
        let mut model = model_with_open_outline_panel();
        let group_id = model.editor_area.focused_group_id;
        let cmd = crate::update::layout::update_layout(&mut model, LayoutMsg::FocusGroup(group_id));
        let Some(Cmd::Batch(cmds)) = cmd else {
            panic!("focus change with a stale outline must batch a refresh, got {cmd:?}");
        };
        assert!(cmds
            .iter()
            .any(|c| matches!(c, Cmd::DebouncedSyntaxParse { delay_ms: 0, .. })));
    }

    #[test]
    fn keyboard_navigation_reveals_selection_minimally() {
        let mut model = model_with_open_outline_panel();
        populate_outline(&mut model, 100);
        let visible_capacity = outline_visible_capacity(&model);
        assert!(visible_capacity > 0);

        model.outline_panel.selected_index = Some(visible_capacity - 1);
        update_outline(&mut model, OutlineMsg::SelectNext);

        assert_eq!(model.outline_panel.selected_index, Some(visible_capacity));
        assert_eq!(model.outline_panel.scroll_offset, 1);

        model.outline_panel.scroll_offset = 20;
        model.outline_panel.selected_index = Some(5);
        update_outline(&mut model, OutlineMsg::SelectPrevious);

        assert_eq!(model.outline_panel.selected_index, Some(4));
        assert_eq!(model.outline_panel.scroll_offset, 4);
    }

    #[test]
    fn manual_scroll_can_move_selection_out_of_view() {
        let mut model = model_with_open_outline_panel();
        populate_outline(&mut model, 100);
        model.outline_panel.selected_index = Some(0);

        update_outline(&mut model, OutlineMsg::Scroll { lines: 10 });

        assert_eq!(model.outline_panel.scroll_offset, 10);
    }
}
