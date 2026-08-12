//! Panel rendering: sidebar file tree, dock panels, and outline panel

use crate::model::editor_area::Rect;
use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::geometry::{DockHeaderLayout, OutlinePanelLayout, TreeListLayout};
use super::tree_view::{render_tree, TreeRenderLayout};

enum DockContentKind {
    Outline,
    Terminal,
    Problems,
    Placeholder { message: &'static str },
}

struct DockPaneScene {
    position: crate::panel::DockPosition,
    layout: DockHeaderLayout,
    border_color: u32,
    text_color: u32,
    bg_color: u32,
    active_tab_bg: u32,
    active_tab_fg: u32,
    content: DockContentKind,
}

impl DockPaneScene {
    fn resolve(model: &AppModel, position: crate::panel::DockPosition, rect: Rect) -> Option<Self> {
        let dock = model.dock_layout.dock(position);
        if !dock.is_open || dock.panel_ids.is_empty() {
            return None;
        }

        let theme = &model.theme.sidebar;
        let layout = DockHeaderLayout::new(dock, rect, &model.metrics, model.char_width);
        let active_panel = dock
            .active_panel()
            .unwrap_or(crate::panel::PanelId::TERMINAL);
        let content = match active_panel {
            crate::panel::PanelId::Outline => DockContentKind::Outline,
            crate::panel::PanelId::Terminal => DockContentKind::Terminal,
            crate::panel::PanelId::Problems => DockContentKind::Problems,
            _ => {
                let placeholder = crate::panels::PlaceholderPanel::new(active_panel);
                DockContentKind::Placeholder {
                    message: placeholder.message(),
                }
            }
        };

        Some(Self {
            position,
            layout,
            border_color: theme.border.to_argb_u32(),
            text_color: theme.foreground.to_argb_u32(),
            bg_color: theme.background.to_argb_u32(),
            active_tab_bg: theme.selection_background.to_argb_u32(),
            active_tab_fg: theme.selection_foreground.to_argb_u32(),
            content,
        })
    }

    fn render(&self, frame: &mut Frame, painter: &mut TextPainter, model: &AppModel) {
        self.render_chrome(frame);
        self.render_header(frame, painter, model);

        match &self.content {
            DockContentKind::Outline => {
                render_outline_panel(
                    frame,
                    painter,
                    model,
                    self.layout.content_rect,
                    self.text_color,
                );
            }
            DockContentKind::Terminal => {
                crate::panels::terminal::render_terminal_panel(
                    frame,
                    painter,
                    model,
                    self.layout.content_rect,
                );
            }
            DockContentKind::Problems => {
                render_problems_panel(
                    frame,
                    painter,
                    model,
                    self.layout.content_rect,
                    self.text_color,
                );
            }
            DockContentKind::Placeholder { message } => {
                self.render_placeholder_content(frame, painter, message);
            }
        }
    }

    fn render_chrome(&self, frame: &mut Frame) {
        let rect = Rect::new(
            self.layout.rect_x as f32,
            self.layout.rect_y as f32,
            self.layout.rect_w as f32,
            self.layout.rect_h as f32 + self.layout.content_rect.height,
        );
        frame.fill_rect(rect, self.bg_color);
        frame.fill_rect_px(
            self.layout.rect_x,
            self.layout.border_y,
            self.layout.rect_w,
            1,
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

    fn render_header(&self, frame: &mut Frame, painter: &mut TextPainter, model: &AppModel) {
        for tab in &self.layout.tabs {
            if tab.is_active {
                // The active-tab highlight is a translucent color (e.g. white
                // at ~10% alpha), so it must be alpha-blended over the panel
                // background. A plain `fill_rect_px` ignores alpha and would
                // paint a solid (white) block, hiding the tab title.
                frame.blend_rect_px(tab.x, tab.y, tab.width, tab.height, self.active_tab_bg);
            }

            let fg = if tab.is_active {
                self.active_tab_fg
            } else {
                self.text_color
            };
            painter.draw(frame, tab.text_x, tab.text_y, tab.title, fg);

            // "PROBLEMS ✗3 ⚠2" — counts drawn right after the tab title,
            // reusing the gutter/status-bar severity colors (no new theme
            // keys per the design doc's non-goals).
            if tab.panel_id == crate::panel::PanelId::Problems {
                let (errors, warnings) = crate::update::problems::severity_counts(model);
                if errors > 0 || warnings > 0 {
                    let char_w = painter.char_width();
                    let mut x = tab.text_x + (tab.title.len() as f32 * char_w) as usize + 6;
                    let overlay = &model.theme.overlay;
                    if errors > 0 {
                        let text = format!("\u{2717}{errors}");
                        painter.draw(
                            frame,
                            x,
                            tab.text_y,
                            &text,
                            overlay.severity_error.to_argb_u32(),
                        );
                        x += (text.len() as f32 * char_w) as usize + 6;
                    }
                    if warnings > 0 {
                        let text = format!("\u{26A0}{warnings}");
                        painter.draw(
                            frame,
                            x,
                            tab.text_y,
                            &text,
                            overlay.severity_warning.to_argb_u32(),
                        );
                    }
                }
            }
        }
    }

    fn render_placeholder_content(
        &self,
        frame: &mut Frame,
        painter: &mut TextPainter,
        message: &str,
    ) {
        let char_width = painter.char_width();
        let line_height = painter.line_height();
        let text_width = message.len() as f32 * char_width;
        let content = self.layout.content_rect;
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

/// Truncate `name` to at most `max_chars` characters (including the trailing
/// ellipsis), returning it unchanged if it already fits.
///
/// Operates on chars, not bytes, so multi-byte UTF-8 sequences are never cut
/// mid-codepoint.
fn truncate_with_ellipsis(name: &str, max_chars: usize) -> std::borrow::Cow<'_, str> {
    let char_count = name.chars().count();
    if char_count <= max_chars || max_chars == 0 {
        return std::borrow::Cow::Borrowed(name);
    }

    let truncated: String = name
        .chars()
        .take(max_chars.saturating_sub(1))
        .chain(std::iter::once('\u{2026}'))
        .collect();
    std::borrow::Cow::Owned(truncated)
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
            let max_chars = available_width
                .checked_div(ctx.char_width)
                .unwrap_or(available_width / 8);

            let display_name = truncate_with_ellipsis(&node.name, max_chars);
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
) {
    let theme = &model.theme.sidebar;
    let selection_bg = theme.selection_background.to_argb_u32();
    let selection_fg = theme.selection_foreground.to_argb_u32();
    let folder_icon_color = theme.folder_icon.to_argb_u32();

    let line_height = painter.line_height();
    let outline_layout = OutlinePanelLayout::new(rect, &model.metrics);

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

    let selected_index = model.outline_panel.selected_index;
    let scroll_offset = model.outline_panel.scroll_offset;

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
            let base_x = ctx.layout.content_rect.x as usize;
            let icon_x = pos.icon_x + base_x;
            let text_x = pos.text_x + base_x;
            let text_y = pos.text_y;
            let is_selected = ctx.selected_index == Some(row.index);

            if is_selected {
                frame.fill_rect_blended(
                    Rect::new(
                        ctx.layout.content_rect.x,
                        row.row_y as f32,
                        ctx.layout.content_rect.width,
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
            let container_width =
                ctx.layout.content_rect.x as usize + ctx.layout.content_rect.width as usize;
            let available = ctx
                .layout
                .tree
                .available_text_width(container_width, name_x);
            let char_w = painter.char_width() as usize;
            let max_chars = available.checked_div(char_w).unwrap_or(80);

            let display = truncate_with_ellipsis(&node.name, max_chars);
            painter.draw(frame, name_x, text_y, &display, fg);
        },
    );
}

/// Render the Problems panel: collapsible per-file groups over
/// `model.lsp.diagnostics`, `problems_rows(model)` as the single ordering
/// authority (view, keyboard nav, and click hit-mapping all consume it).
pub fn render_problems_panel(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    rect: Rect,
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

    let line_height = painter.line_height();
    let layout = OutlinePanelLayout::new(rect, &model.metrics);
    let rows = problems_rows(model);

    if rows.is_empty() {
        let msg = "No problems";
        let char_width = painter.char_width();
        let text_width = msg.len() as f32 * char_width;
        let text_x = rect.x + (rect.width - text_width) / 2.0;
        let text_y =
            layout.content_rect.y + (layout.content_rect.height - line_height as f32) / 2.0;
        painter.draw(frame, text_x as usize, text_y as usize, msg, text_color);
        return;
    }

    let selected_index = model.problems_panel.selected_index;
    let scroll_offset = model.problems_panel.scroll_offset;
    let base_x = layout.content_rect.x as usize;
    let container_width = base_x + layout.content_rect.width as usize;
    let char_w = painter.char_width() as usize;

    let visible = rows
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(layout.visible_capacity());

    for (index, row) in visible {
        let row_y = layout.content_rect.y as usize + (index - scroll_offset) * layout.row_height;
        let is_selected = selected_index == Some(index);

        if is_selected {
            frame.fill_rect_blended(
                Rect::new(
                    layout.content_rect.x,
                    row_y as f32,
                    layout.content_rect.width,
                    layout.row_height as f32,
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
                let pos = layout.tree.node_position(0, row_y);
                let icon_x = pos.icon_x + base_x;
                let text_x = pos.text_x + base_x;
                let chevron = if *collapsed { "\u{25B8}" } else { "\u{25BE}" };
                let chevron_color = if is_selected {
                    selection_fg
                } else {
                    icon_color
                };
                painter.draw(frame, icon_x, pos.text_y, chevron, chevron_color);

                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string());
                let dir = path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let label = if dir.is_empty() {
                    format!("{name}  ({count})")
                } else {
                    format!("{name}  {dir}  ({count})")
                };
                let available = layout.tree.available_text_width(container_width, text_x);
                let max_chars = available.checked_div(char_w).unwrap_or(80);
                let display = truncate_with_ellipsis(&label, max_chars);
                painter.draw(frame, text_x, pos.text_y, &display, fg);
            }
            ProblemsRow::Diagnostic { path, index } => {
                let pos = layout.tree.node_position(1, row_y);
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
                let accessory_width = (accessory.len() * char_w) as f32;
                let accessory_x =
                    (layout.content_rect.x + layout.content_rect.width - accessory_width) as usize;
                let accessory_color = if is_selected {
                    selection_fg
                } else {
                    icon_color
                };
                painter.draw(frame, accessory_x, pos.text_y, &accessory, accessory_color);

                let available = accessory_x.saturating_sub(text_x + layout.tree.left_padding);
                let max_chars = available.checked_div(char_w).unwrap_or(80);
                let display = truncate_with_ellipsis(&diagnostic.message, max_chars);
                painter.draw(frame, text_x, pos.text_y, &display, fg);
            }
        }
    }
}

#[cfg(test)]
mod truncate_with_ellipsis_tests {
    use super::truncate_with_ellipsis;

    #[test]
    fn leaves_short_names_unchanged() {
        let result = truncate_with_ellipsis("short.rs", 20);
        assert_eq!(result, "short.rs");
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn truncates_long_names_with_ellipsis() {
        // "a_very_long_filename.rs" is 24 chars; requesting 10 chars should
        // keep 9 chars of the original plus a trailing ellipsis.
        let result = truncate_with_ellipsis("a_very_long_filename.rs", 10);
        assert_eq!(result, "a_very_lo\u{2026}");
        assert_eq!(result.chars().count(), 10);
    }

    #[test]
    fn truncates_multibyte_names_on_char_boundaries() {
        // "café_very_long_name" contains a multi-byte 'é' (2 bytes in UTF-8).
        // Truncation must count characters, not bytes, or this would panic
        // or split the 'é' mid-codepoint.
        let name = "café_very_long_name";
        assert_eq!(name.chars().count(), 19);
        let result = truncate_with_ellipsis(name, 6);
        assert_eq!(result, "café_\u{2026}");
        assert_eq!(result.chars().count(), 6);
    }

    #[test]
    fn does_not_truncate_when_max_chars_is_zero() {
        assert_eq!(truncate_with_ellipsis("anything", 0), "anything");
    }
}
