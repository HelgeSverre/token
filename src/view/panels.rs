//! Panel rendering: sidebar file tree, dock panels, and outline panel

use crate::layout::{snapshot::snap, LayoutSnapshot, RowListView, UiKey};
use crate::model::editor_area::Rect;
use crate::model::AppModel;

use super::frame::{FontRole, Frame, TextPainter};
use super::geometry::TreeRowLayout;
use super::tree_view::render_tree;

enum DockContentKind {
    Outline,
    Terminal,
    Problems,
    Usages,
    Placeholder { message: &'static str },
}

/// One dock tab resolved from the chrome snapshot: its box, its text
/// origin (the tab's padded content box), and its title.
struct DockTabScene {
    title: String,
    rect: Rect,
    text_pos: (usize, usize),
    is_active: bool,
}

struct DockPaneScene {
    position: crate::panel::DockPosition,
    dock_rect: Rect,
    header_rect: Rect,
    content_rect: Rect,
    tabs: Vec<DockTabScene>,
    border_color: u32,
    /// Physical-px chrome border thickness (`metrics.border_width`), so the
    /// header separator matches every other scaled border at HiDPI.
    border_width: usize,
    text_color: u32,
    bg_color: u32,
    active_tab_bg: u32,
    active_tab_fg: u32,
    content: DockContentKind,
}

impl DockPaneScene {
    fn resolve(
        model: &AppModel,
        position: crate::panel::DockPosition,
        chrome: &LayoutSnapshot,
    ) -> Option<Self> {
        let dock = model.dock_layout.dock(position);
        if !dock.is_open || dock.panel_ids.is_empty() {
            return None;
        }

        let dock_rect = chrome.rect(UiKey::Dock(position))?;
        let header_rect = chrome.rect(UiKey::DockHeader(position))?;

        let theme = &model.theme.sidebar;
        let active_panel = dock
            .active_panel()
            .unwrap_or(crate::panel::PanelId::TERMINAL);
        let content_rect = chrome
            .rect(UiKey::PanelContent(active_panel))
            .unwrap_or(Rect::new(
                dock_rect.x,
                dock_rect.y + header_rect.height,
                dock_rect.width,
                (dock_rect.height - header_rect.height).max(0.0),
            ));

        let active_index = dock.active_index;
        let tabs = dock
            .panel_ids
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, panel_id)| {
                let node = chrome.node(UiKey::DockTab(position, panel_id))?;
                let (tx, ty, _, _) = snap(node.content_rect);
                Some(DockTabScene {
                    title: dock_tab_title(model, panel_id),
                    rect: node.rect,
                    text_pos: (tx, ty),
                    is_active: active_index == Some(index),
                })
            })
            .collect();

        let content = match active_panel {
            crate::panel::PanelId::Outline => DockContentKind::Outline,
            crate::panel::PanelId::Terminal => DockContentKind::Terminal,
            crate::panel::PanelId::Problems => DockContentKind::Problems,
            crate::panel::PanelId::Usages => DockContentKind::Usages,
            _ => {
                let placeholder = crate::panels::PlaceholderPanel::new(active_panel);
                DockContentKind::Placeholder {
                    message: placeholder.message(),
                }
            }
        };

        Some(Self {
            position,
            dock_rect,
            header_rect,
            content_rect,
            tabs,
            border_color: theme.border.to_argb_u32(),
            border_width: model.metrics.border_width,
            text_color: theme.foreground.to_argb_u32(),
            bg_color: theme.background.to_argb_u32(),
            active_tab_bg: theme.selection_background.to_argb_u32(),
            active_tab_fg: theme.selection_foreground.to_argb_u32(),
            content,
        })
    }

    fn render(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        model: &AppModel,
        chrome: &LayoutSnapshot,
    ) {
        self.render_chrome(frame);
        {
            let mut painter = painter.with_font(FontRole::Code);
            self.render_header(frame, &mut painter);
            if matches!(self.content, DockContentKind::Terminal) {
                crate::panels::terminal::render_tabs(frame, &mut painter, model, chrome);
            }
        }

        frame.push_clip(self.content_rect);
        match &self.content {
            DockContentKind::Outline => {
                let rows = chrome.row_list(UiKey::PanelRows(crate::panel::PanelId::Outline));
                render_outline_panel(
                    frame,
                    painter,
                    model,
                    self.content_rect,
                    rows,
                    self.text_color,
                );
            }
            DockContentKind::Terminal => {
                let mut painter = painter.with_font(FontRole::Code);
                crate::panels::terminal::render_terminal_panel(
                    frame,
                    &mut painter,
                    model,
                    self.content_rect,
                );
            }
            DockContentKind::Problems => {
                let rows = chrome.row_list(UiKey::PanelRows(crate::panel::PanelId::Problems));
                render_problems_panel(
                    frame,
                    painter,
                    model,
                    self.content_rect,
                    rows,
                    self.text_color,
                );
            }
            DockContentKind::Placeholder { message } => {
                self.render_placeholder_content(frame, painter, message);
            }
            DockContentKind::Usages => {
                if let Some(rows) = chrome.row_list(UiKey::PanelRows(crate::panel::PanelId::Usages))
                {
                    render_usages_panel(frame, painter, model, rows);
                }
            }
        }
        frame.pop_clip();
    }

    fn render_chrome(&self, frame: &mut Frame) {
        let rect = self.dock_rect;
        frame.fill_rect(rect, self.bg_color);
        // Border under the header row, matching the scaled chrome border.
        let (hx, hy, hw, hh) = snap(self.header_rect);
        frame.fill_rect_px(
            hx,
            (hy + hh).saturating_sub(self.border_width),
            hw,
            self.border_width,
            self.border_color,
        );

        match self.position {
            crate::panel::DockPosition::Left => {
                frame.fill_rect(
                    Rect::new(rect.x + rect.width - 1.0, rect.y, 1.0, rect.height),
                    self.border_color,
                );
            }
            crate::panel::DockPosition::Right => {
                frame.fill_rect(
                    Rect::new(rect.x, rect.y, 1.0, rect.height),
                    self.border_color,
                );
            }
            crate::panel::DockPosition::Bottom => {
                frame.fill_rect(
                    Rect::new(rect.x, rect.y, rect.width, 1.0),
                    self.border_color,
                );
            }
        }
    }

    fn render_header(&self, frame: &mut Frame, painter: &mut TextPainter) {
        frame.push_clip(self.header_rect);
        for tab in &self.tabs {
            if tab.is_active {
                // The active-tab highlight is a translucent color (e.g. white
                // at ~10% alpha), so it must be alpha-blended over the panel
                // background. A plain `fill_rect_px` ignores alpha and would
                // paint a solid (white) block, hiding the tab title.
                let (x, y, w, h) = snap(tab.rect);
                frame.blend_rect_px(x, y, w, h, self.active_tab_bg);
            }

            let fg = if tab.is_active {
                self.active_tab_fg
            } else {
                self.text_color
            };
            painter.draw(frame, tab.text_pos.0, tab.text_pos.1, &tab.title, fg);
        }
        frame.pop_clip();
    }

    fn render_placeholder_content(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        message: &str,
    ) {
        let line_height = painter.line_height();
        let text_width = painter.measure_width(message);
        let content = self.content_rect;
        let text_x = content.x + (content.width - text_width) / 2.0;
        let text_y = content.y + (content.height - line_height as f32) / 2.0;
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
    sidebar_x: usize,
    sidebar_width: usize,
    row_height: usize,
    tree: TreeRowLayout,
    // Colors
    text_color: u32,
    selection_bg: u32,
    selection_fg: u32,
    folder_icon_color: u32,
}

/// Context for outline panel rendering, holding constant values throughout tree traversal.
struct OutlineRenderContext<'a> {
    content_rect: Rect,
    row_height: usize,
    tree: TreeRowLayout,
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
    chrome: &LayoutSnapshot,
) {
    let Some(workspace) = &model.workspace else {
        return;
    };
    let Some(rows) = chrome.row_list(UiKey::Sidebar) else {
        return;
    };
    let mut painter = painter.with_font(FontRole::Code);
    let sidebar_rect = rows.rect();
    let (sidebar_x, sidebar_y, sidebar_width, sidebar_height) = snap(sidebar_rect);

    let theme = &model.theme.sidebar;
    let metrics = &model.metrics;

    // Draw sidebar background
    let bg_color = theme.background.to_argb_u32();
    frame.fill_rect(sidebar_rect, bg_color);

    // Draw resize border on the right edge
    let border_color = theme.border.to_argb_u32();
    let border_x = sidebar_x + sidebar_width.saturating_sub(1);
    frame.fill_rect(
        Rect::new(
            border_x as f32,
            sidebar_y as f32,
            1.0,
            sidebar_height as f32,
        ),
        border_color,
    );

    // Clip all subsequent drawing to the sidebar bounds
    frame.set_clip(sidebar_rect);

    // Build render context with all constant values
    let ctx = SidebarRenderContext {
        sidebar_x,
        sidebar_width,
        row_height: rows.row_height().round() as usize,
        tree: TreeRowLayout::from_metrics(metrics),
        text_color: theme.foreground.to_argb_u32(),
        selection_bg: theme.selection_background.to_argb_u32(),
        selection_fg: theme.selection_foreground.to_argb_u32(),
        folder_icon_color: theme.folder_icon.to_argb_u32(),
    };

    render_tree(
        &workspace.file_tree.roots,
        rows,
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
                        ctx.sidebar_x as f32,
                        row.row_y as f32,
                        ctx.sidebar_width as f32,
                        ctx.row_height as f32,
                    ),
                    ctx.selection_bg,
                );
            }

            let icon_x = ctx.sidebar_x + pos.icon_x;
            let text_x = ctx.sidebar_x + pos.text_x;
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

            let sidebar_right = ctx.sidebar_x + ctx.sidebar_width;
            let available_width = ctx.tree.available_text_width(sidebar_right, text_x);
            let display_name = painter.truncate_to_width(&node.name, available_width as f32);
            painter.draw(frame, text_x, text_y, &display_name, fg);
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
    chrome: &LayoutSnapshot,
) {
    let Some(scene) = DockPaneScene::resolve(model, position, chrome) else {
        return;
    };

    scene.render(frame, painter, model, chrome);
}

/// Render the outline panel showing document symbols as a tree
pub fn render_outline_panel(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    content_rect: Rect,
    rows: Option<RowListView>,
    text_color: u32,
) {
    let theme = &model.theme.sidebar;
    let selection_bg = theme.selection_background.to_argb_u32();
    let selection_fg = theme.selection_foreground.to_argb_u32();
    let folder_icon_color = theme.folder_icon.to_argb_u32();

    let line_height = painter.line_height();
    let tree = TreeRowLayout::outline_from_metrics(&model.metrics);

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
            let text_width = painter.measure_width(msg);
            let text_x = content_rect.x + (content_rect.width - text_width) / 2.0;
            let text_y = content_rect.y + (content_rect.height - line_height as f32) / 2.0;
            painter.draw(frame, text_x as usize, text_y as usize, msg, text_color);
            return;
        }
    };

    let Some(rows) = rows else {
        return;
    };
    let selected_index = model.outline_panel.selected_index;

    let ctx = OutlineRenderContext {
        content_rect,
        row_height: rows.row_height().round() as usize,
        tree,
        selected_index,
        text_color,
        selection_bg,
        selection_fg,
        icon_color: folder_icon_color,
        outline_panel: &model.outline_panel,
    };

    render_tree(
        &outline.roots,
        rows,
        |node| node.is_collapsible() && !ctx.outline_panel.is_collapsed(node),
        |row| {
            let node = row.node;
            let pos = ctx.tree.node_position(row.depth, row.row_y);
            let base_x = ctx.content_rect.x as usize;
            let icon_x = pos.icon_x + base_x;
            let text_x = pos.text_x + base_x;
            let text_y = pos.text_y;
            let is_selected = ctx.selected_index == Some(row.index);

            if is_selected {
                frame.fill_rect_blended(
                    Rect::new(
                        ctx.content_rect.x,
                        row.row_y as f32,
                        ctx.content_rect.width,
                        ctx.row_height as f32,
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

            let name_x = text_x
                + (painter.measure_width(label) + painter.measure_width(" ")).ceil() as usize;
            let container_width = ctx.content_rect.x as usize + ctx.content_rect.width as usize;
            let available = ctx.tree.available_text_width(container_width, name_x);
            let display = painter.truncate_to_width(&node.name, available as f32);
            painter.draw(frame, name_x, text_y, &display, fg);
        },
    );
}

/// Dock tab title — `display_name` except Problems, whose title carries
/// the workspace-wide scope. Shared with the chrome layout so measured
/// and painted text agree.
pub fn dock_tab_title(model: &AppModel, panel_id: crate::panel::PanelId) -> String {
    match panel_id {
        crate::panel::PanelId::Problems => crate::update::problems::problems_panel_title(model),
        other => other.display_name().to_owned(),
    }
}

fn render_usages_panel(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    view: RowListView,
) {
    use crate::model::usages::UsagesRow;
    let rows = model.usages_panel.rows();
    let theme = &model.theme.sidebar;
    let tree = TreeRowLayout::outline_from_metrics(&model.metrics);
    for index in view.drawn_range() {
        let (Some(&row), Some(rect)) = (rows.get(index), view.row_rect(index)) else {
            continue;
        };
        let selected = model.usages_panel.selected_index == Some(index);
        if selected {
            frame.fill_rect_blended(rect, theme.selection_background.to_argb_u32());
        }
        let color = if selected {
            theme.selection_foreground.to_argb_u32()
        } else if matches!(row, UsagesRow::Summary) {
            model.theme.overlay.text_dim.to_argb_u32()
        } else {
            theme.foreground.to_argb_u32()
        };
        let depth = usize::from(matches!(row, UsagesRow::Location(_)));
        let pos = tree.node_position(depth, rect.y as usize);
        if let UsagesRow::File { collapsed, .. } = row {
            painter.draw(
                frame,
                rect.x as usize + pos.icon_x,
                pos.text_y,
                if collapsed { "▸" } else { "▾" },
                color,
            );
        }
        let x = rect.x as usize + pos.text_x;
        let available = tree.available_text_width((rect.x + rect.width) as usize, x);
        let text = crate::update::usages::row_label(model, row);
        let text = painter.truncate_to_width(&text, available as f32);
        painter.draw(frame, x, pos.text_y, &text, color);
    }
}

/// Render the Problems panel: collapsible per-file groups over
/// `model.lsp.diagnostics`, `problems_rows(model)` as the single ordering
/// authority (view, keyboard nav, and click hit-mapping all consume it).
pub fn render_problems_panel(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    content_rect: Rect,
    row_view: Option<RowListView>,
    text_color: u32,
) {
    use crate::update::problems::{problems_rows, ProblemsRow};

    let theme = &model.theme.sidebar;
    let overlay = &model.theme.overlay;
    let selection_bg = theme.selection_background.to_argb_u32();
    let selection_fg = theme.selection_foreground.to_argb_u32();
    let icon_color = theme.folder_icon.to_argb_u32();
    let error_color = overlay.severity_error.to_argb_u32();
    let warning_color = overlay.severity_warning.to_argb_u32();
    let dim_color = overlay.text_dim.to_argb_u32();
    let workspace_root = model.workspace_root();

    let line_height = painter.line_height();
    let row_height = model.metrics.file_tree_row_height;
    let tree = TreeRowLayout::outline_from_metrics(&model.metrics);
    let rows = problems_rows(model);

    if rows.is_empty() {
        let msg = crate::update::problems::problems_empty_text(model);
        let text_width = painter.measure_width(msg);
        let text_x = content_rect.x + (content_rect.width - text_width) / 2.0;
        let text_y = content_rect.y + (content_rect.height - line_height as f32) / 2.0;
        painter.draw(frame, text_x as usize, text_y as usize, msg, text_color);
        return;
    }

    let selected_index = model.problems_panel.selected_index;
    let scroll_offset = row_view
        .map(|r| r.scroll_offset())
        .unwrap_or(model.problems_panel.scroll_offset);
    let base_x = content_rect.x as usize;
    let container_width = base_x + content_rect.width as usize;
    // Ceil, not floor: draw() advances by the true fractional advance, so
    // flooring undermeasures every width and overflows budgets rightward.
    let char_w = painter.char_width().ceil() as usize;

    // Paint the drawn range (ceil — the partial bottom row is painted and
    // clipped by the panel scissor, so a clickable sliver is never blank).
    let drawn = row_view.map(|r| r.drawn_range()).unwrap_or(0..rows.len());
    let visible = rows.iter().enumerate().skip(drawn.start).take(drawn.len());

    for (index, row) in visible {
        let row_y = content_rect.y as usize + (index - scroll_offset) * row_height;
        let is_selected = selected_index == Some(index);

        if is_selected {
            frame.fill_rect_blended(
                Rect::new(
                    content_rect.x,
                    row_y as f32,
                    content_rect.width,
                    row_height as f32,
                ),
                selection_bg,
            );
        }

        let fg = if is_selected {
            selection_fg
        } else {
            text_color
        };

        match row {
            ProblemsRow::File {
                path,
                count,
                collapsed,
            } => {
                let pos = tree.node_position(0, row_y);
                let icon_x = pos.icon_x + base_x;
                let text_x = pos.text_x + base_x;
                let chevron = if *collapsed { "\u{25B8}" } else { "\u{25BE}" };
                let chevron_color = if is_selected {
                    selection_fg
                } else {
                    icon_color
                };
                painter.draw(frame, icon_x, pos.text_y, chevron, chevron_color);

                let file_icon = crate::model::FileExtension::from_path(path).icon();
                painter.draw(frame, text_x, pos.text_y, file_icon, icon_color);
                let name_x = text_x + 2 * char_w;

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let dir = path.parent().map(|p| {
                    workspace_root
                        .and_then(|root| p.strip_prefix(root).ok())
                        .unwrap_or(p)
                        .display()
                        .to_string()
                });
                let suffix = match dir.filter(|d| !d.is_empty()) {
                    Some(dir) => format!("  {dir} \u{b7} {count}"),
                    None => format!("  {count}"),
                };
                let name_available = tree.available_text_width(container_width, name_x);
                let name_display = painter.truncate_to_width(&name, name_available as f32);
                painter.draw(frame, name_x, pos.text_y, &name_display, fg);

                let suffix_x = name_x + painter.measure_width(&name_display).ceil() as usize;
                let suffix_available = tree.available_text_width(container_width, suffix_x);
                let suffix_display = painter.truncate_to_width(&suffix, suffix_available as f32);
                let dim = if is_selected { selection_fg } else { dim_color };
                painter.draw(frame, suffix_x, pos.text_y, &suffix_display, dim);
            }
            ProblemsRow::Diagnostic { path, index } => {
                let pos = tree.node_position(1, row_y);
                let icon_x = pos.icon_x + base_x;
                let text_x = pos.text_x + base_x;

                let diagnostic = model
                    .lsp
                    .diagnostics
                    .get(path)
                    .and_then(|diags| diags.get(*index));
                let Some(diagnostic) = diagnostic else {
                    continue;
                };

                let mark = crate::model::diagnostic_mark(diagnostic.severity);
                let (glyph, glyph_color) = match mark {
                    crate::model::Mark::Error => ("\u{2717}", error_color),
                    crate::model::Mark::Warning => ("\u{26A0}", warning_color),
                    _ => ("\u{2022}", icon_color),
                };
                let glyph_color = if is_selected {
                    selection_fg
                } else {
                    glyph_color
                };
                painter.draw(frame, icon_x, pos.text_y, glyph, glyph_color);

                let accessory = format!(
                    "{}:{}",
                    diagnostic.range.start.line + 1,
                    diagnostic.range.start.character + 1
                );
                // Fractional measure + right inset (symmetric with
                // available_text_width's implicit left_padding right inset).
                let accessory_width = painter.measure_width(&accessory);
                let right_inset = tree.left_padding as f32;
                let accessory_x =
                    (content_rect.x + content_rect.width - right_inset - accessory_width)
                        .max(content_rect.x) as usize;
                let accessory_color = if is_selected {
                    selection_fg
                } else {
                    icon_color
                };
                painter.draw(frame, accessory_x, pos.text_y, &accessory, accessory_color);

                let available = accessory_x.saturating_sub(text_x + tree.left_padding);
                // Multi-line LSP messages would smear their later lines
                // into the same row ('\n' renders as an empty glyph).
                let message = diagnostic.message.lines().next().unwrap_or("");
                let display = painter.truncate_to_width(message, available as f32);
                painter.draw(frame, text_x, pos.text_y, &display, fg);
            }
        }
    }
}

#[cfg(test)]
mod usages_tests {
    use super::*;

    #[test]
    fn usages_panel_renders_selected_partial_row_within_shared_clip() {
        let mut model = AppModel::new(800, 600, 1.0);
        model.usages_panel.items = (0..30)
            .map(|line| crate::update::navigation::LocationItem {
                path: "/source.rs".into(),
                position: lsp_types::Position::new(line, 3),
                preview: "usage".into(),
                route_hint: None,
            })
            .collect();
        model.dock_layout.bottom.size_logical = 137.5;
        model
            .dock_layout
            .bottom
            .activate(crate::panel::PanelId::Usages);
        model.resize(800, 600);
        let view = crate::layout::chrome::chrome(&model)
            .row_list(UiKey::PanelRows(crate::panel::PanelId::Usages))
            .unwrap();
        let last = view.drawn_range().last().unwrap();
        model.usages_panel.selected_index = Some(last);
        let rect = view.rect();
        let row = view.row_rect(last).unwrap();
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut cache = crate::view::GlyphCache::default();
        let mut painter = TextPainter::new(&font, &mut cache, 14.0, 11.0, 8.0, 18);
        let sentinel = 0xff010203;
        let mut pixels = vec![sentinel; 800 * 600];
        {
            let mut frame = Frame::new(&mut pixels, 800, 600);
            frame.push_clip(rect);
            render_usages_panel(&mut frame, &mut painter, &model, view);
            frame.pop_clip();
        }
        assert_ne!(
            pixels[row.y as usize * 800 + (row.x + row.width - 2.0) as usize],
            sentinel
        );
        for (index, pixel) in pixels.into_iter().enumerate() {
            let (x, y) = (index % 800, index / 800);
            if x < rect.x as usize
                || x >= (rect.x + rect.width) as usize
                || y < rect.y as usize
                || y >= (rect.y + rect.height) as usize
            {
                assert_eq!(
                    pixel, sentinel,
                    "paint escaped the solved panel clip at {x},{y}"
                );
            }
        }
    }
}
