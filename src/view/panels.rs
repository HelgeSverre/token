//! Panel rendering: sidebar file tree, dock panels, and outline panel

use crate::model::editor_area::Rect;
use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::geometry::TreeListLayout;

/// Context for sidebar rendering, holding constant values throughout tree traversal.
struct SidebarRenderContext {
    sidebar_width: usize,
    sidebar_height: usize,
    row_height: usize,
    scroll_offset: usize,
    char_width: usize,
    tree: TreeListLayout,
    // Colors
    text_color: u32,
    selection_bg: u32,
    selection_fg: u32,
    folder_icon_color: u32,
}

/// Context for outline panel rendering, holding constant values throughout tree traversal.
struct OutlineRenderContext<'a> {
    rect: Rect,
    max_y: usize,
    row_height: usize,
    scroll_offset: usize,
    selected_index: Option<usize>,
    tree: TreeListLayout,
    text_color: u32,
    selection_bg: u32,
    selection_fg: u32,
    icon_color: u32,
    outline_panel: &'a crate::model::OutlinePanelState,
}

/// Render the sidebar (file tree) for a workspace.
pub fn render_sidebar(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    sidebar_width: usize,
    sidebar_height: usize,
) {
    let Some(workspace) = &model.workspace else {
        return;
    };

    let theme = &model.theme.sidebar;
    let metrics = &model.metrics;

    // Draw sidebar background
    let bg_color = theme.background.to_argb_u32();
    frame.fill_rect(
        Rect::new(0.0, 0.0, sidebar_width as f32, sidebar_height as f32),
        bg_color,
    );

    // Draw resize border on the right edge
    let border_color = theme.border.to_argb_u32();
    let border_x = sidebar_width.saturating_sub(1);
    frame.fill_rect(
        Rect::new(border_x as f32, 0.0, 1.0, sidebar_height as f32),
        border_color,
    );

    // Clip all subsequent drawing to the sidebar bounds
    frame.set_clip(Rect::new(
        0.0,
        0.0,
        sidebar_width as f32,
        sidebar_height as f32,
    ));

    // Build render context with all constant values
    let ctx = SidebarRenderContext {
        sidebar_width,
        sidebar_height,
        row_height: metrics.file_tree_row_height,
        scroll_offset: workspace.scroll_offset,
        char_width: painter.char_width().ceil() as usize,
        tree: TreeListLayout::from_metrics(metrics),
        text_color: theme.foreground.to_argb_u32(),
        selection_bg: theme.selection_background.to_argb_u32(),
        selection_fg: theme.selection_foreground.to_argb_u32(),
        folder_icon_color: theme.folder_icon.to_argb_u32(),
    };

    let mut y = 0usize;
    let mut visible_index = 0usize;

    // Helper function to render a tree node recursively.
    // visible_index tracks the global flattened index of items.
    // Items before scroll_offset are counted but not drawn.
    // y only advances for items that are actually drawn.
    #[allow(clippy::too_many_arguments)]
    fn render_node(
        frame: &mut Frame,
        painter: &mut TextPainter,
        node: &crate::model::FileNode,
        workspace: &crate::model::Workspace,
        ctx: &SidebarRenderContext,
        y: &mut usize,
        visible_index: &mut usize,
        depth: usize,
    ) {
        // If we've started rendering and filled the viewport, bail out
        if *visible_index >= ctx.scroll_offset && *y >= ctx.sidebar_height {
            return;
        }

        // Capture this node's global index, then advance the counter
        let idx = *visible_index;
        *visible_index += 1;

        // Only draw if this item is at or after scroll_offset
        let is_visible_row = idx >= ctx.scroll_offset;

        if is_visible_row {
            // If we're beyond the viewport height, stop drawing
            if *y >= ctx.sidebar_height {
                return;
            }

            let pos = ctx.tree.node_position(depth, *y);

            // Check if this item is selected
            let is_selected = workspace
                .selected_item
                .as_ref()
                .map(|p| p == &node.path)
                .unwrap_or(false);

            // Draw selection background with alpha blending
            if is_selected {
                frame.fill_rect_blended(
                    Rect::new(
                        0.0,
                        *y as f32,
                        ctx.sidebar_width as f32,
                        ctx.row_height as f32,
                    ),
                    ctx.selection_bg,
                );
            }

            let icon_x = pos.icon_x;
            let text_x = pos.text_x;
            let text_y = pos.text_y;

            if node.is_dir {
                let is_expanded = workspace.is_expanded(&node.path);
                // Use +/- indicators: - for expanded, + for collapsed
                let indicator = if is_expanded { "-" } else { "+" };
                let icon_color = if is_selected {
                    ctx.selection_fg
                } else {
                    ctx.folder_icon_color
                };
                painter.draw(frame, icon_x, text_y, indicator, icon_color);
            }

            // Draw file/folder name, truncating if too long
            let fg = if is_selected {
                ctx.selection_fg
            } else {
                ctx.text_color
            };

            // Calculate available width for text
            let available_width = ctx.tree.available_text_width(ctx.sidebar_width, text_x);

            // Use actual char width from font metrics
            let max_chars = if ctx.char_width > 0 {
                available_width / ctx.char_width
            } else {
                available_width / 8
            };

            let name_chars = node.name.chars().count();
            let needs_truncation = name_chars > max_chars && max_chars > 3;

            if needs_truncation {
                // Use char_indices for safe UTF-8 boundary slicing
                let truncate_at = max_chars.saturating_sub(1);
                let byte_end = node
                    .name
                    .char_indices()
                    .nth(truncate_at)
                    .map(|(i, _)| i)
                    .unwrap_or(node.name.len());
                let mut display_name = String::with_capacity(byte_end + 3);
                display_name.push_str(&node.name[..byte_end]);
                display_name.push('\u{2026}');
                painter.draw(frame, text_x, text_y, &display_name, fg);
            } else {
                painter.draw(frame, text_x, text_y, &node.name, fg);
            }

            // Only advance y for items that are actually drawn
            *y += ctx.row_height;
        }

        // Always recurse into children if expanded (even if parent is above viewport)
        // Children may scroll into view even when their parent folder header is not visible
        if node.is_dir && workspace.is_expanded(&node.path) {
            for child in &node.children {
                render_node(
                    frame,
                    painter,
                    child,
                    workspace,
                    ctx,
                    y,
                    visible_index,
                    depth + 1,
                );
            }
        }
    }

    // Render all root nodes
    for node in &workspace.file_tree.roots {
        render_node(
            frame,
            painter,
            node,
            workspace,
            &ctx,
            &mut y,
            &mut visible_index,
            0,
        );
        // Early exit if viewport is filled
        if visible_index >= ctx.scroll_offset && y >= ctx.sidebar_height {
            break;
        }
    }

    frame.clear_clip();
}

/// Render a dock panel (right or bottom dock with placeholder content)
pub fn render_dock(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    position: crate::panel::DockPosition,
    rect: Rect,
) {
    let dock = model.dock_layout.dock(position);
    if !dock.is_open || dock.panel_ids.is_empty() {
        return;
    }

    let theme = &model.theme.sidebar; // Use sidebar theme for now
    let border_color = theme.border.to_argb_u32();
    let text_color = theme.foreground.to_argb_u32();
    let bg_color = theme.background.to_argb_u32();

    // Fill background
    frame.fill_rect(rect, bg_color);

    // Draw border on edge facing the editor
    match position {
        crate::panel::DockPosition::Left => {
            // Border on right edge
            frame.fill_rect(
                Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height),
                border_color,
            );
        }
        crate::panel::DockPosition::Right => {
            // Border on left edge
            frame.fill_rect(Rect::new(rect.x, rect.y, 1.0, rect.height), border_color);
        }
        crate::panel::DockPosition::Bottom => {
            // Border on top edge
            frame.fill_rect(Rect::new(rect.x, rect.y, rect.width, 1.0), border_color);
        }
    }

    let active_panel = dock.active_panel();

    // Dispatch to panel-specific rendering
    if active_panel == Some(crate::panel::PanelId::Outline) {
        render_outline_panel(frame, painter, model, rect, text_color, bg_color);
    } else {
        // Placeholder for other panels
        let placeholder = active_panel
            .map(crate::panels::PlaceholderPanel::new)
            .unwrap_or_else(|| {
                crate::panels::PlaceholderPanel::new(crate::panel::PanelId::TERMINAL)
            });

        let title = placeholder.title();
        let title_x = rect.x + model.metrics.padding_large as f32;
        let title_y = rect.y + model.metrics.padding_large as f32;
        painter.draw(frame, title_x as usize, title_y as usize, title, text_color);

        let message = placeholder.message();
        let char_width = painter.char_width();
        let line_height = painter.line_height();
        let text_width = message.len() as f32 * char_width;
        let text_x = rect.x + (rect.width - text_width) / 2.0;
        let text_y = rect.y + (rect.height - line_height as f32) / 2.0;
        painter.draw(frame, text_x as usize, text_y as usize, message, text_color);
    }
}

/// Render the outline panel showing document symbols as a tree
pub fn render_outline_panel(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    rect: Rect,
    text_color: u32,
    _bg_color: u32,
) {
    let theme = &model.theme.sidebar;
    let selection_bg = theme.selection_background.to_argb_u32();
    let selection_fg = theme.selection_foreground.to_argb_u32();
    let folder_icon_color = theme.folder_icon.to_argb_u32();

    let line_height = painter.line_height();
    let row_height = model.metrics.file_tree_row_height;

    // Title bar
    let title_x = rect.x + model.metrics.padding_large as f32;
    let title_y = rect.y + model.metrics.padding_medium as f32;
    painter.draw(
        frame,
        title_x as usize,
        title_y as usize,
        "Outline",
        text_color,
    );

    let content_y = rect.y + row_height as f32 + model.metrics.padding_medium as f32;
    let content_height = rect.height - row_height as f32 - model.metrics.padding_medium as f32;

    // Get outline from the focused document
    let outline = model
        .editor_area
        .focused_document()
        .and_then(|doc| doc.outline.as_ref());

    let outline = match outline {
        Some(o) if !o.is_empty() => o,
        _ => {
            // Show "No outline available" centered
            let msg = "No outline available";
            let char_width = painter.char_width();
            let text_width = msg.len() as f32 * char_width;
            let text_x = rect.x + (rect.width - text_width) / 2.0;
            let text_y = content_y + (content_height - line_height as f32) / 2.0;
            painter.draw(frame, text_x as usize, text_y as usize, msg, text_color);
            return;
        }
    };

    let scroll_offset = model.outline_panel.scroll_offset;
    let selected_index = model.outline_panel.selected_index;

    let mut y = content_y as usize;
    let mut visible_index: usize = 0;
    let max_y = (content_y + content_height) as usize;

    // Recursive render function for outline nodes
    fn render_outline_node(
        frame: &mut Frame,
        painter: &mut TextPainter,
        node: &crate::outline::OutlineNode,
        ctx: &OutlineRenderContext,
        y: &mut usize,
        visible_index: &mut usize,
        depth: usize,
    ) {
        if *visible_index >= ctx.scroll_offset && *y >= ctx.max_y {
            return;
        }

        let idx = *visible_index;
        *visible_index += 1;

        let is_visible_row = idx >= ctx.scroll_offset;

        if is_visible_row {
            if *y >= ctx.max_y {
                return;
            }

            let pos = ctx.tree.node_position(depth, *y);
            // Offset by the rect's x position for outline panels embedded in docks
            let base_x = ctx.rect.x as usize;
            let icon_x = pos.icon_x + base_x;
            let text_x = pos.text_x + base_x;
            let text_y = pos.text_y;
            let is_selected = ctx.selected_index == Some(idx);

            if is_selected {
                frame.fill_rect_blended(
                    Rect::new(ctx.rect.x, *y as f32, ctx.rect.width, ctx.row_height as f32),
                    ctx.selection_bg,
                );
            }

            if node.is_collapsible() {
                let is_collapsed = ctx.outline_panel.is_collapsed(node);
                let indicator = if is_collapsed { "+" } else { "-" };
                let icon_color = if is_selected {
                    ctx.selection_fg
                } else {
                    ctx.icon_color
                };
                painter.draw(frame, icon_x, text_y, indicator, icon_color);
            }

            // Draw kind label + name
            let fg = if is_selected {
                ctx.selection_fg
            } else {
                ctx.text_color
            };
            let label = node.kind.label();

            // Draw label in dimmer color, then name
            let label_color = if is_selected {
                ctx.selection_fg
            } else {
                ctx.icon_color
            };
            painter.draw(frame, text_x, text_y, label, label_color);

            let name_x = text_x + (label.len() + 1) * painter.char_width() as usize;

            // Truncate name if needed
            let container_width = ctx.rect.x as usize + ctx.rect.width as usize;
            let available = ctx.tree.available_text_width(container_width, name_x);
            let char_w = painter.char_width() as usize;
            let max_chars = if char_w > 0 { available / char_w } else { 80 };

            let name_chars: usize = node.name.chars().count();
            if name_chars > max_chars && max_chars > 1 {
                let display: String = node
                    .name
                    .chars()
                    .take(max_chars.saturating_sub(1))
                    .chain(std::iter::once('\u{2026}'))
                    .collect();
                painter.draw(frame, name_x, text_y, &display, fg);
            } else {
                painter.draw(frame, name_x, text_y, &node.name, fg);
            }

            *y += ctx.row_height;
        }

        // Recurse into children if expanded
        if node.is_collapsible() && !ctx.outline_panel.is_collapsed(node) {
            for child in &node.children {
                render_outline_node(frame, painter, child, ctx, y, visible_index, depth + 1);
            }
        }
    }

    let ctx = OutlineRenderContext {
        rect,
        max_y,
        row_height,
        scroll_offset,
        selected_index,
        tree: TreeListLayout::outline_from_metrics(&model.metrics),
        text_color,
        selection_bg,
        selection_fg,
        icon_color: folder_icon_color,
        outline_panel: &model.outline_panel,
    };

    for node in &outline.roots {
        render_outline_node(frame, painter, node, &ctx, &mut y, &mut visible_index, 0);
        if visible_index >= scroll_offset && y >= max_y {
            break;
        }
    }
}
