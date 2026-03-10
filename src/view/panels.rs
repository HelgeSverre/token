//! Panel rendering: sidebar file tree, dock panels, and outline panel

use crate::model::editor_area::Rect;
use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::geometry::{OutlinePanelLayout, TreeListLayout};
use super::tree_view::{render_tree, TreeRenderLayout};

enum DockContentKind {
    Outline,
    Placeholder {
        title: &'static str,
        message: &'static str,
    },
}

struct DockPaneScene {
    position: crate::panel::DockPosition,
    rect: Rect,
    border_color: u32,
    text_color: u32,
    bg_color: u32,
    content: DockContentKind,
}

impl DockPaneScene {
    fn resolve(model: &AppModel, position: crate::panel::DockPosition, rect: Rect) -> Option<Self> {
        let dock = model.dock_layout.dock(position);
        if !dock.is_open || dock.panel_ids.is_empty() {
            return None;
        }

        let theme = &model.theme.sidebar;
        let active_panel = dock
            .active_panel()
            .unwrap_or(crate::panel::PanelId::TERMINAL);
        let content = if active_panel == crate::panel::PanelId::Outline {
            DockContentKind::Outline
        } else {
            let placeholder = crate::panels::PlaceholderPanel::new(active_panel);
            DockContentKind::Placeholder {
                title: placeholder.title(),
                message: placeholder.message(),
            }
        };

        Some(Self {
            position,
            rect,
            border_color: theme.border.to_argb_u32(),
            text_color: theme.foreground.to_argb_u32(),
            bg_color: theme.background.to_argb_u32(),
            content,
        })
    }

    fn render(&self, frame: &mut Frame, painter: &mut TextPainter, model: &AppModel) {
        self.render_chrome(frame);

        match &self.content {
            DockContentKind::Outline => {
                render_outline_panel(
                    frame,
                    painter,
                    model,
                    self.rect,
                    self.text_color,
                    self.bg_color,
                );
            }
            DockContentKind::Placeholder { title, message } => {
                self.render_placeholder_content(frame, painter, model, title, message);
            }
        }
    }

    fn render_chrome(&self, frame: &mut Frame) {
        frame.fill_rect(self.rect, self.bg_color);

        match self.position {
            crate::panel::DockPosition::Left => {
                frame.fill_rect(
                    Rect::new(
                        self.rect.x + self.rect.width - 1.0,
                        self.rect.y,
                        1.0,
                        self.rect.height,
                    ),
                    self.border_color,
                );
            }
            crate::panel::DockPosition::Right => {
                frame.fill_rect(
                    Rect::new(self.rect.x, self.rect.y, 1.0, self.rect.height),
                    self.border_color,
                );
            }
            crate::panel::DockPosition::Bottom => {
                frame.fill_rect(
                    Rect::new(self.rect.x, self.rect.y, self.rect.width, 1.0),
                    self.border_color,
                );
            }
        }
    }

    fn render_placeholder_content(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        title: &str,
        message: &str,
    ) {
        let title_x = self.rect.x + model.metrics.padding_large as f32;
        let title_y = self.rect.y + model.metrics.padding_large as f32;
        painter.draw(
            frame,
            title_x as usize,
            title_y as usize,
            title,
            self.text_color,
        );

        let char_width = painter.char_width();
        let line_height = painter.line_height();
        let text_width = message.len() as f32 * char_width;
        let text_x = self.rect.x + (self.rect.width - text_width) / 2.0;
        let text_y = self.rect.y + (self.rect.height - line_height as f32) / 2.0;
        painter.draw(
            frame,
            text_x as usize,
            text_y as usize,
            message,
            self.text_color,
        );
    }
}

/// Context for sidebar rendering, holding constant values throughout tree traversal.
struct SidebarRenderContext {
    sidebar_width: usize,
    row_height: usize,
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
    layout: OutlinePanelLayout,
    selected_index: Option<usize>,
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
        row_height: metrics.file_tree_row_height,
        char_width: painter.char_width().ceil() as usize,
        tree: TreeListLayout::from_metrics(metrics),
        text_color: theme.foreground.to_argb_u32(),
        selection_bg: theme.selection_background.to_argb_u32(),
        selection_fg: theme.selection_foreground.to_argb_u32(),
        folder_icon_color: theme.folder_icon.to_argb_u32(),
    };

    render_tree(
        &workspace.file_tree.roots,
        TreeRenderLayout::new(0, sidebar_height, ctx.row_height, workspace.scroll_offset),
        |node| node.is_dir && workspace.is_expanded(&node.path),
        |row| {
            let node = row.node;
            let pos = ctx.tree.node_position(row.depth, row.row_y);

            let is_selected = workspace
                .selected_item
                .as_ref()
                .map(|p| p == &node.path)
                .unwrap_or(false);

            if is_selected {
                frame.fill_rect_blended(
                    Rect::new(
                        0.0,
                        row.row_y as f32,
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
                let indicator = if workspace.is_expanded(&node.path) {
                    "-"
                } else {
                    "+"
                };
                let icon_color = if is_selected {
                    ctx.selection_fg
                } else {
                    ctx.folder_icon_color
                };
                painter.draw(frame, icon_x, text_y, indicator, icon_color);
            }

            let fg = if is_selected {
                ctx.selection_fg
            } else {
                ctx.text_color
            };

            let available_width = ctx.tree.available_text_width(ctx.sidebar_width, text_x);
            let max_chars = if ctx.char_width > 0 {
                available_width / ctx.char_width
            } else {
                available_width / 8
            };

            let name_chars = node.name.chars().count();
            let needs_truncation = name_chars > max_chars && max_chars > 3;

            if needs_truncation {
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
        },
    );

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
    let Some(scene) = DockPaneScene::resolve(model, position, rect) else {
        return;
    };

    scene.render(frame, painter, model);
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
    let outline_layout = OutlinePanelLayout::new(rect, &model.metrics);

    // Title bar
    painter.draw(
        frame,
        outline_layout.title_x,
        outline_layout.title_y,
        "Outline",
        text_color,
    );

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
            let text_y = outline_layout.content_rect.y
                + (outline_layout.content_rect.height - line_height as f32) / 2.0;
            painter.draw(frame, text_x as usize, text_y as usize, msg, text_color);
            return;
        }
    };

    let scroll_offset = model.outline_panel.scroll_offset;
    let selected_index = model.outline_panel.selected_index;

    let ctx = OutlineRenderContext {
        layout: outline_layout,
        selected_index,
        text_color,
        selection_bg,
        selection_fg,
        icon_color: folder_icon_color,
        outline_panel: &model.outline_panel,
    };

    render_tree(
        &outline.roots,
        TreeRenderLayout::new(
            ctx.layout.content_rect.y as usize,
            ctx.layout.content_rect.height as usize,
            ctx.layout.row_height,
            scroll_offset,
        ),
        |node| node.is_collapsible() && !ctx.outline_panel.is_collapsed(node),
        |row| {
            let node = row.node;
            let pos = ctx.layout.tree.node_position(row.depth, row.row_y);
            let base_x = ctx.layout.rect.x as usize;
            let icon_x = pos.icon_x + base_x;
            let text_x = pos.text_x + base_x;
            let text_y = pos.text_y;
            let is_selected = ctx.selected_index == Some(row.index);

            if is_selected {
                frame.fill_rect_blended(
                    Rect::new(
                        ctx.layout.rect.x,
                        row.row_y as f32,
                        ctx.layout.rect.width,
                        ctx.layout.row_height as f32,
                    ),
                    ctx.selection_bg,
                );
            }

            if node.is_collapsible() {
                let indicator = if ctx.outline_panel.is_collapsed(node) {
                    "+"
                } else {
                    "-"
                };
                let icon_color = if is_selected {
                    ctx.selection_fg
                } else {
                    ctx.icon_color
                };
                painter.draw(frame, icon_x, text_y, indicator, icon_color);
            }

            let fg = if is_selected {
                ctx.selection_fg
            } else {
                ctx.text_color
            };
            let label = node.kind.label();
            let label_color = if is_selected {
                ctx.selection_fg
            } else {
                ctx.icon_color
            };
            painter.draw(frame, text_x, text_y, label, label_color);

            let name_x = text_x + (label.len() + 1) * painter.char_width() as usize;
            let container_width = ctx.layout.rect.x as usize + ctx.layout.rect.width as usize;
            let available = ctx
                .layout
                .tree
                .available_text_width(container_width, name_x);
            let char_w = painter.char_width() as usize;
            let max_chars = if char_w > 0 { available / char_w } else { 80 };

            let name_chars = node.name.chars().count();
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
        },
    );
}
