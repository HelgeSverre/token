//! Outline panel update handlers

use crate::commands::Cmd;
use crate::messages::OutlineMsg;
use crate::model::AppModel;
use crate::outline::OutlineNode;

/// Count total visible items in the outline tree (for navigation bounds)
fn count_visible_items(
    nodes: &[OutlineNode],
    panel: &crate::model::OutlinePanelState,
) -> usize {
    let mut count = 0;
    for node in nodes {
        count += 1;
        if node.is_collapsible() && !panel.is_collapsed(node) {
            count += count_visible_items(&node.children, panel);
        }
    }
    count
}

/// Get the node at a given flattened visible index
fn node_at_index<'a>(
    nodes: &'a [OutlineNode],
    panel: &crate::model::OutlinePanelState,
    target: usize,
    current: &mut usize,
) -> Option<&'a OutlineNode> {
    for node in nodes {
        if *current == target {
            return Some(node);
        }
        *current += 1;
        if node.is_collapsible() && !panel.is_collapsed(node) {
            if let Some(found) = node_at_index(&node.children, panel, target, current) {
                return Some(found);
            }
        }
    }
    None
}

/// Handle outline panel messages
pub fn update_outline(model: &mut AppModel, msg: OutlineMsg) -> Option<Cmd> {
    match msg {
        OutlineMsg::JumpToSymbol { line, col } => {
            // Move cursor to the symbol and focus the editor
            let editor = model.editor_mut();
            editor.cursors[0].line = line;
            editor.cursors[0].column = col;
            editor.cursors[0].desired_column = None;
            editor.clear_selection();
            model.ensure_cursor_visible();
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
            Some(Cmd::Redraw)
        }

        OutlineMsg::ExpandSelected => {
            if let Some(idx) = model.outline_panel.selected_index {
                let outline = model
                    .editor_area
                    .focused_document()
                    .and_then(|doc| doc.outline.as_ref());

                if let Some(outline) = outline {
                    let mut current = 0;
                    if let Some(node) =
                        node_at_index(&outline.roots, &model.outline_panel, idx, &mut current)
                    {
                        if node.is_collapsible() {
                            let key = crate::model::OutlinePanelState::node_key(node);
                            model.outline_panel.collapsed.remove(&key);
                        }
                    }
                    let total = count_visible_items(&outline.roots, &model.outline_panel);
                    model.outline_panel.scroll_offset =
                        model.outline_panel.scroll_offset.min(total.saturating_sub(1));
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
                    let mut current = 0;
                    if let Some(node) =
                        node_at_index(&outline.roots, &model.outline_panel, idx, &mut current)
                    {
                        if node.is_collapsible() {
                            let key = crate::model::OutlinePanelState::node_key(node);
                            model.outline_panel.collapsed.insert(key);
                        }
                    }
                    let total = count_visible_items(&outline.roots, &model.outline_panel);
                    model.outline_panel.scroll_offset =
                        model.outline_panel.scroll_offset.min(total.saturating_sub(1));
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
                    let mut current = 0;
                    if let Some(node) =
                        node_at_index(&outline.roots, &model.outline_panel, idx, &mut current)
                    {
                        let line = node.range.start_line;
                        let col = node.range.start_col;
                        // Jump to symbol and focus editor
                        let editor = model.editor_mut();
                        editor.cursors[0].line = line;
                        editor.cursors[0].column = col;
                        editor.cursors[0].desired_column = None;
                        editor.clear_selection();
                        model.ensure_cursor_visible();
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
                model.outline_panel.scroll_offset = offset + lines as usize;
            }

            let outline = model
                .editor_area
                .focused_document()
                .and_then(|doc| doc.outline.as_ref());

            if let Some(outline) = outline {
                let total = count_visible_items(&outline.roots, &model.outline_panel);
                let dock_height = model.dock_layout.right.size(model.metrics.scale_factor);
                let title_height = model.metrics.file_tree_row_height as f32 + 4.0;
                let visible_capacity =
                    ((dock_height - title_height) / model.metrics.file_tree_row_height as f32)
                        .max(0.0) as usize;
                model.outline_panel.scroll_offset = model
                    .outline_panel
                    .scroll_offset
                    .min(total.saturating_sub(visible_capacity));
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
            model.outline_panel.selected_index = Some(index);

            let outline = model
                .editor_area
                .focused_document()
                .and_then(|doc| doc.outline.as_ref());

            if let Some(outline) = outline {
                let mut current = 0;
                if let Some(node) =
                    node_at_index(&outline.roots, &model.outline_panel, index, &mut current)
                {
                    if on_chevron && node.is_collapsible() {
                        model.outline_panel.toggle_collapsed(node);
                    } else if click_count >= 2 {
                        let line = node.range.start_line;
                        let col = node.range.start_col;
                        let editor = model.editor_mut();
                        editor.cursors[0].line = line;
                        editor.cursors[0].column = col;
                        editor.cursors[0].desired_column = None;
                        editor.clear_selection();
                        model.ensure_cursor_visible();
                        model.ui.focus_editor();
                    }
                }
            }
            Some(Cmd::Redraw)
        }
    }
}
