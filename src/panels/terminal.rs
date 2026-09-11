//! Terminal dock panel rendering helpers.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::SelectionRange;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};

use crate::layout::{
    Dir, ElementDecl, LayoutSnapshot, Padding, ScrollDecl, Sizing, SizingAxes, UiKey, UiTree,
};
use crate::model::editor_area::Rect;
use crate::model::AppModel;
use crate::terminal::TabAction;
use crate::view::frame::{Frame, TextPainter};

/// Declare terminal controls in the same snapshot used for dock hit testing
/// and PTY sizing. The tab viewport clips overflow; cycle buttons stay visible.
pub(crate) fn declare_tabs(tree: &mut UiTree, model: &AppModel) {
    let height = model.metrics.tab_bar_height as f32;
    tree.node(
        ElementDecl {
            key: Some(UiKey::TerminalTabs),
            dir: Dir::Row,
            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height)),
            clip: true,
            ..Default::default()
        },
        |tree| {
            let button = |tree: &mut UiTree, action| {
                tree.leaf(ElementDecl {
                    key: Some(UiKey::TerminalAction(action)),
                    sizing: SizingAxes::fixed(height, height),
                    ..Default::default()
                });
            };
            button(tree, TabAction::Previous);
            button(tree, TabAction::Next);
            tree.node(
                ElementDecl {
                    key: Some(UiKey::TerminalTabViewport),
                    sizing: SizingAxes::grow(),
                    clip: true,
                    ..Default::default()
                },
                |tree| {
                    tree.node(
                        ElementDecl {
                            dir: Dir::Row,
                            sizing: SizingAxes::grow(),
                            scroll: Some(ScrollDecl {
                                offset_x: model.terminal.tab_scroll,
                                offset_y: 0.0,
                            }),
                            ..Default::default()
                        },
                        |tree| {
                            for session in &model.terminal.sessions {
                                tree.leaf(ElementDecl {
                                    key: Some(UiKey::TerminalAction(TabAction::Select(session.id))),
                                    sizing: SizingAxes::new(
                                        Sizing::Fixed(model.char_width * 22.0),
                                        Sizing::GROW,
                                    ),
                                    padding: Padding::xy(model.metrics.padding_medium as f32, 0.0),
                                    ..Default::default()
                                });
                            }
                        },
                    );
                },
            );
            button(tree, TabAction::New);
            button(tree, TabAction::Close);
        },
    );
}

/// Reveal the active tab using solved bounds, including after dock/font resize.
pub(crate) fn reveal_active_tab(model: &mut AppModel) {
    let Some(session) = model.terminal.active_session() else {
        return;
    };
    let chrome = crate::layout::chrome::chrome(model);
    let (Some(view), Some(tab)) = (
        chrome.rect(UiKey::TerminalTabViewport),
        chrome.rect(UiKey::TerminalAction(TabAction::Select(session.id))),
    ) else {
        return;
    };
    let delta = if tab.x < view.x || tab.x + tab.width > view.x + view.width {
        tab.x - view.x
    } else {
        0.0
    };
    model.terminal.tab_scroll = (model.terminal.tab_scroll + delta).max(0.0);
}

pub(crate) fn render_tabs(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    chrome: &LayoutSnapshot,
) {
    use crate::view::button::{render_button, ButtonState};
    let Some(bar) = chrome.rect(UiKey::TerminalTabs) else {
        return;
    };
    let theme = &model.theme.sidebar;
    frame.push_clip(bar);
    frame.fill_rect(bar, theme.background.to_argb_u32());
    for action in [
        TabAction::Previous,
        TabAction::Next,
        TabAction::New,
        TabAction::Close,
    ] {
        let Some(rect) = chrome.rect(UiKey::TerminalAction(action)) else {
            continue;
        };
        let label = match action {
            TabAction::Previous => "<",
            TabAction::Next => ">",
            TabAction::New if model.terminal.has_pending_spawn() => "...",
            TabAction::New => "+",
            _ => "x",
        };
        let state = if model.terminal.hovered_tab == Some(action) {
            ButtonState::Hovered
        } else {
            ButtonState::Normal
        };
        render_button(
            frame,
            painter,
            &model.theme,
            rect,
            label,
            crate::view::button::ButtonStyle {
                state,
                ..Default::default()
            },
        );
    }
    if let Some(view) = chrome.rect(UiKey::TerminalTabViewport) {
        frame.push_clip(view);
        for (index, session) in model.terminal.sessions.iter().enumerate() {
            let action = TabAction::Select(session.id);
            let Some(node) = chrome.node(UiKey::TerminalAction(action)) else {
                continue;
            };
            if node.rect.x + node.rect.width <= view.x || node.rect.x >= view.x + view.width {
                continue;
            }
            let active = index == model.terminal.active;
            if active || model.terminal.hovered_tab == Some(action) {
                frame.fill_rect_blended(node.rect, theme.selection_background.to_argb_u32());
            }
            frame.push_clip(node.content_rect);
            let title: String = session
                .title
                .chars()
                .filter(|c| !c.is_control())
                .take(20)
                .collect();
            let title = format!(
                "{}: {}{}",
                index + 1,
                if session.exited { "[exited] " } else { "" },
                title
            );
            let x = node.content_rect.x.max(0.0) as usize;
            let y = (node.rect.y + (node.rect.height - painter.line_height() as f32).max(0.0) / 2.0)
                as usize;
            painter.draw(
                frame,
                x,
                y,
                &title,
                if active {
                    theme.selection_foreground
                } else {
                    theme.foreground
                }
                .to_argb_u32(),
            );
            frame.pop_clip();
        }
        frame.pop_clip();
    }
    frame.pop_clip();
}

const ANSI_COLORS: [u32; 16] = [
    0xFF00_0000, // black
    0xFFCD_3131, // red
    0xFF0D_BC79, // green
    0xFFE5_E510, // yellow
    0xFF24_72C8, // blue
    0xFFBC_3FBC, // magenta
    0xFF11_A8CD, // cyan
    0xFFE5_E5E5, // white
    0xFF66_6666, // bright black
    0xFFF1_4C4C, // bright red
    0xFF23_D18B, // bright green
    0xFFF5_F543, // bright yellow
    0xFF3B_8EEA, // bright blue
    0xFFD6_70D6, // bright magenta
    0xFF29_B8DB, // bright cyan
    0xFFFF_FFFF, // bright white
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalGridSize {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Copy)]
pub enum TerminalColorRole {
    Foreground,
    Background,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalPalette {
    pub default_fg: u32,
    pub default_bg: u32,
    pub cursor: u32,
    pub ansi: [u32; 16],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TerminalCellDecorations {
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
}

/// Shared terminal cell geometry for painting, hit testing and selection.
pub struct TerminalViewport {
    pub rect: Rect,
    char_width: f32,
    line_height: usize,
    rows: usize,
    cols: usize,
    scroll_offset: usize,
}

impl TerminalViewport {
    fn new(
        session: &crate::terminal::TerminalSession,
        rect: Rect,
        char_width: f32,
        line_height: usize,
    ) -> Self {
        let size = grid_size_for_rect(rect, char_width, line_height);
        Self {
            rect,
            char_width: char_width.max(1.0),
            line_height: line_height.max(1),
            rows: usize::from(size.rows).min(session.term().grid().screen_lines()),
            cols: usize::from(size.cols).min(session.term().grid().columns()),
            scroll_offset: session.scroll_offset.min(session.max_scrollback_offset()),
        }
    }

    pub fn for_model(model: &AppModel) -> Option<Self> {
        let session = model.terminal.active_session()?;
        let rect = crate::layout::chrome::chrome(model)
            .rect(UiKey::PanelContent(crate::panel::PanelId::Terminal))?;
        (rect.width > 0.0 && rect.height > 0.0)
            .then(|| Self::new(session, rect, model.char_width, model.line_height))
    }

    /// Clamp a pointer to the visible cells, retaining the left/right half
    /// of the cell so a plain click starts an empty character selection.
    pub fn point_at(&self, x: f64, y: f64) -> (Point, Side) {
        let column = (x as f32 - self.rect.x) / self.char_width;
        let col = (column.floor().max(0.0) as usize).min(self.cols.saturating_sub(1));
        let row = (((y as f32 - self.rect.y) / self.line_height as f32)
            .floor()
            .max(0.0) as usize)
            .min(self.rows.saturating_sub(1));
        let side = if column < col as f32 + 0.5 {
            Side::Left
        } else {
            Side::Right
        };
        (Point::new(Line(self.grid_line(row)), Column(col)), side)
    }

    /// Unlike a captured selection, links must not hit padding outside cells.
    pub fn point_inside(&self, x: f64, y: f64) -> Option<Point> {
        let inside = x >= self.rect.x as f64
            && y >= self.rect.y as f64
            && x < (self.rect.x + self.cols as f32 * self.char_width) as f64
            && y < (self.rect.y + (self.rows * self.line_height) as f32) as f64;
        inside.then(|| self.point_at(x, y).0)
    }

    fn grid_line(&self, row: usize) -> i32 {
        terminal_view_row_to_grid_line(row, self.scroll_offset)
    }

    fn cell_rect(&self, row: usize, col: usize) -> Rect {
        Rect::new(
            self.rect.x + col as f32 * self.char_width,
            self.rect.y + (row * self.line_height) as f32,
            self.char_width.ceil(),
            self.line_height as f32,
        )
    }
}

struct TerminalRenderContext<'a> {
    viewport: TerminalViewport,
    link: Option<std::ops::RangeInclusive<Point>>,
    selection: Option<SelectionRange>,
    selection_bg: u32,
    selection_fg: u32,
    palette: &'a TerminalPalette,
}

impl TerminalPalette {
    pub fn from_model(model: &AppModel) -> Self {
        let theme = &model.theme.sidebar;

        Self {
            default_fg: theme.foreground.to_argb_u32(),
            default_bg: theme.background.to_argb_u32(),
            cursor: theme.selection_foreground.to_argb_u32(),
            ansi: ANSI_COLORS,
        }
    }
}

pub fn grid_size_for_rect(rect: Rect, char_width: f32, line_height: usize) -> TerminalGridSize {
    let cols = if char_width > 0.0 {
        (rect.width.max(0.0) / char_width).floor() as usize
    } else {
        1
    };
    let rows = if line_height > 0 {
        (rect.height.max(0.0) / line_height as f32).floor() as usize
    } else {
        1
    };

    TerminalGridSize {
        rows: rows.clamp(1, u16::MAX as usize) as u16,
        cols: cols.clamp(1, u16::MAX as usize) as u16,
    }
}

pub fn resolve_terminal_color(
    color: AnsiColor,
    palette: &TerminalPalette,
    role: TerminalColorRole,
) -> u32 {
    match color {
        AnsiColor::Named(NamedColor::Foreground)
        | AnsiColor::Named(NamedColor::BrightForeground)
        | AnsiColor::Named(NamedColor::DimForeground) => palette.default_fg,
        AnsiColor::Named(NamedColor::Background) => palette.default_bg,
        AnsiColor::Named(NamedColor::Cursor) => palette.cursor,
        AnsiColor::Named(named) => named_color_index(named)
            .and_then(|index| palette.ansi.get(index).copied())
            .unwrap_or_else(|| fallback_color(palette, role)),
        AnsiColor::Spec(rgb) => {
            0xFF00_0000 | ((rgb.r as u32) << 16) | ((rgb.g as u32) << 8) | rgb.b as u32
        }
        AnsiColor::Indexed(index) => {
            indexed_color(index, palette).unwrap_or_else(|| fallback_color(palette, role))
        }
    }
}

pub fn render_terminal_panel(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    rect: Rect,
) {
    let palette = TerminalPalette::from_model(model);
    frame.fill_rect(rect, palette.default_bg);

    let Some(session) = model.terminal.active_session() else {
        return;
    };

    frame.push_clip(rect);

    let term = session.term();
    let grid = term.grid();
    let line_height = painter.line_height();
    let char_width = painter.char_width();
    let viewport = TerminalViewport::new(session, rect, char_width, line_height);
    let rows = viewport.rows;
    let cols = viewport.cols;

    // Clamp scrollback offset to the available history so it can never
    // scroll past the top of the buffer.
    let max_offset = grid.total_lines().saturating_sub(grid.screen_lines());
    let scroll_offset = viewport.scroll_offset;

    let ctx = TerminalRenderContext {
        viewport,
        link: model
            .terminal
            .hovered_link
            .as_ref()
            .filter(|(id, _)| *id == session.id)
            .map(|(_, link)| link.range.clone()),
        selection: term.selection.as_ref().and_then(|s| s.to_range(term)),
        selection_bg: model.theme.sidebar.selection_background.to_argb_u32(),
        selection_fg: model.theme.sidebar.selection_foreground.to_argb_u32(),
        palette: &palette,
    };

    // Oldest addressable line: history lines sit *above* the screen and are
    // addressed with negative `Line` indices in alacritty.
    let topmost_line = grid.screen_lines() as i32 - grid.total_lines() as i32;

    for row in 0..rows {
        let grid_line = ctx.viewport.grid_line(row);
        if grid_line < topmost_line {
            continue;
        }
        for col in 0..cols {
            let cell = &grid[Line(grid_line)][Column(col)];
            render_terminal_cell(frame, painter, &ctx, row, col, cell);
        }
    }

    render_terminal_cursor(frame, painter, &ctx, rows, cols, grid, scroll_offset);
    render_scrollback_indicator(frame, painter, &ctx, scroll_offset, max_offset);

    frame.pop_clip();
}

fn render_terminal_cell(
    frame: &mut Frame,
    painter: &mut TextPainter,
    ctx: &TerminalRenderContext<'_>,
    row: usize,
    col: usize,
    cell: &alacritty_terminal::term::cell::Cell,
) {
    let skip_glyph = cell
        .flags
        .intersects(Flags::WIDE_CHAR_SPACER | Flags::HIDDEN);
    let (mut fg, bg) = cell_colors(cell, ctx.palette);
    let cell_rect = ctx.viewport.cell_rect(row, col);
    if bg != ctx.palette.default_bg {
        frame.fill_rect(cell_rect, bg);
    }

    let point = Point::new(Line(ctx.viewport.grid_line(row)), Column(col));
    let selected = ctx.selection.is_some_and(|selection| {
        selection.contains(point)
            || (cell.flags.contains(Flags::WIDE_CHAR)
                && selection.contains(Point::new(point.line, point.column + 1)))
            || (cell.flags.contains(Flags::WIDE_CHAR_SPACER)
                && col > 0
                && selection.contains(Point::new(point.line, point.column - 1)))
    });
    if selected {
        frame.fill_rect_blended(cell_rect, ctx.selection_bg);
        fg = ctx.selection_fg;
    }

    if !cell.flags.contains(Flags::HIDDEN)
        && ctx
            .link
            .as_ref()
            .is_some_and(|range| range.contains(&point))
    {
        render_cell_decorations(
            frame,
            &cell_rect,
            fg,
            TerminalCellDecorations {
                underline: true,
                ..Default::default()
            },
        );
    }
    if skip_glyph || cell.c == ' ' {
        return;
    }

    let mut buf = [0; 4];
    let text = cell.c.encode_utf8(&mut buf);
    let decorations = cell_decorations(cell);
    painter.draw(frame, cell_rect.x as usize, cell_rect.y as usize, text, fg);
    if decorations.bold {
        painter.draw(
            frame,
            cell_rect.x as usize + 1,
            cell_rect.y as usize,
            text,
            fg,
        );
    }
    render_cell_decorations(frame, &cell_rect, fg, decorations);
}

fn render_terminal_cursor(
    frame: &mut Frame,
    painter: &mut TextPainter,
    ctx: &TerminalRenderContext<'_>,
    rows: usize,
    cols: usize,
    grid: &alacritty_terminal::grid::Grid<alacritty_terminal::term::cell::Cell>,
    scroll_offset: usize,
) {
    let cursor = grid.cursor.point;
    // The cursor lives on the live screen (line >= 0). Scrolling up by
    // `scroll_offset` pushes it *down* the viewport by that many rows.
    let cursor_line = cursor.line.0;
    let view_row = cursor_line + scroll_offset as i32;

    // Hide the cursor when it has scrolled off the visible viewport.
    if view_row < 0 || view_row as usize >= rows {
        return;
    }

    let row = view_row as usize;
    let col = cursor.column.0;
    if col >= cols {
        return;
    }

    let cell_rect = ctx.viewport.cell_rect(row, col);
    frame.fill_rect(cell_rect, ctx.palette.cursor);

    let cell = &grid[Line(cursor_line)][Column(col)];
    if cell.c == ' ' || cell.flags.contains(Flags::HIDDEN | Flags::WIDE_CHAR_SPACER) {
        return;
    }

    let mut buf = [0; 4];
    let text = cell.c.encode_utf8(&mut buf);
    painter.draw(
        frame,
        cell_rect.x as usize,
        cell_rect.y as usize,
        text,
        ctx.palette.default_bg,
    );
}

/// Map a viewport row (0 = top of the visible area) to an alacritty grid
/// `Line`. Scrolling up by `scroll_offset` reveals history, which alacritty
/// addresses with negative line indices, so the offset is *subtracted*.
fn terminal_view_row_to_grid_line(row: usize, scroll_offset: usize) -> i32 {
    row as i32 - scroll_offset as i32
}

fn cell_colors(
    cell: &alacritty_terminal::term::cell::Cell,
    palette: &TerminalPalette,
) -> (u32, u32) {
    let mut fg = resolve_terminal_color(cell.fg, palette, TerminalColorRole::Foreground);
    let bg = resolve_terminal_color(cell.bg, palette, TerminalColorRole::Background);
    if cell.flags.contains(Flags::DIM) {
        fg = dim_color(fg);
    }

    if cell.flags.contains(Flags::INVERSE) {
        (bg, fg)
    } else {
        (fg, bg)
    }
}

fn indexed_color(index: u8, palette: &TerminalPalette) -> Option<u32> {
    match index {
        0..=15 => palette.ansi.get(index as usize).copied(),
        16..=231 => {
            let index = index - 16;
            let levels: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let r = levels[(index / 36) as usize];
            let g = levels[((index % 36) / 6) as usize];
            let b = levels[(index % 6) as usize];
            Some(rgb_to_argb(r, g, b))
        }
        232..=255 => {
            let level = 8 + (index - 232) * 10;
            Some(rgb_to_argb(level, level, level))
        }
    }
}

fn rgb_to_argb(r: u8, g: u8, b: u8) -> u32 {
    0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn dim_color(color: u32) -> u32 {
    let alpha = color & 0xFF00_0000;
    let r = (((color >> 16) & 0xFF) * 2 / 3) << 16;
    let g = (((color >> 8) & 0xFF) * 2 / 3) << 8;
    let b = (color & 0xFF) * 2 / 3;
    alpha | r | g | b
}

fn cell_decorations(cell: &alacritty_terminal::term::cell::Cell) -> TerminalCellDecorations {
    TerminalCellDecorations {
        bold: cell.flags.contains(Flags::BOLD),
        italic: cell.flags.contains(Flags::ITALIC),
        underline: cell.flags.intersects(Flags::ALL_UNDERLINES),
        strikeout: cell.flags.contains(Flags::STRIKEOUT),
    }
}

fn render_cell_decorations(
    frame: &mut Frame,
    cell_rect: &Rect,
    color: u32,
    decorations: TerminalCellDecorations,
) {
    if decorations.underline {
        let thickness = (cell_rect.height / 12.0).round().max(1.0);
        frame.fill_rect(
            Rect::new(
                cell_rect.x,
                cell_rect.y + cell_rect.height - thickness,
                cell_rect.width,
                thickness,
            ),
            color,
        );
    }

    if decorations.strikeout {
        let thickness = (cell_rect.height / 14.0).round().max(1.0);
        frame.fill_rect(
            Rect::new(
                cell_rect.x,
                cell_rect.y + cell_rect.height * 0.52,
                cell_rect.width,
                thickness,
            ),
            color,
        );
    }
}

fn render_scrollback_indicator(
    frame: &mut Frame,
    painter: &mut TextPainter,
    ctx: &TerminalRenderContext<'_>,
    scroll_offset: usize,
    max_offset: usize,
) {
    let Some(text) = scrollback_indicator_text(scroll_offset, max_offset) else {
        return;
    };

    let viewport = &ctx.viewport;
    let padding = viewport.char_width.round();
    let width = text.chars().count() as f32 * viewport.char_width + padding;
    let x = (viewport.rect.x + viewport.rect.width - width).max(viewport.rect.x);
    let rect = Rect::new(x, viewport.rect.y, width, viewport.line_height as f32);
    frame.fill_rect(rect, ctx.palette.default_bg);
    painter.draw(
        frame,
        (x + padding / 2.0) as usize,
        viewport.rect.y as usize,
        &text,
        ctx.palette.default_fg,
    );
}

fn scrollback_indicator_text(scroll_offset: usize, max_offset: usize) -> Option<String> {
    (scroll_offset > 0).then(|| format!("{scroll_offset}/{max_offset}"))
}

fn fallback_color(palette: &TerminalPalette, role: TerminalColorRole) -> u32 {
    match role {
        TerminalColorRole::Foreground => palette.default_fg,
        TerminalColorRole::Background => palette.default_bg,
    }
}

fn named_color_index(color: NamedColor) -> Option<usize> {
    match color {
        NamedColor::Black | NamedColor::DimBlack => Some(0),
        NamedColor::Red | NamedColor::DimRed => Some(1),
        NamedColor::Green | NamedColor::DimGreen => Some(2),
        NamedColor::Yellow | NamedColor::DimYellow => Some(3),
        NamedColor::Blue | NamedColor::DimBlue => Some(4),
        NamedColor::Magenta | NamedColor::DimMagenta => Some(5),
        NamedColor::Cyan | NamedColor::DimCyan => Some(6),
        NamedColor::White | NamedColor::DimWhite => Some(7),
        NamedColor::BrightBlack => Some(8),
        NamedColor::BrightRed => Some(9),
        NamedColor::BrightGreen => Some(10),
        NamedColor::BrightYellow => Some(11),
        NamedColor::BrightBlue => Some(12),
        NamedColor::BrightMagenta => Some(13),
        NamedColor::BrightCyan => Some(14),
        NamedColor::BrightWhite => Some(15),
        NamedColor::Foreground
        | NamedColor::Background
        | NamedColor::Cursor
        | NamedColor::BrightForeground
        | NamedColor::DimForeground => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::vte::ansi::Rgb;
    use std::sync::mpsc;

    use crate::terminal::{PtyHandle, TerminalSession};

    fn test_palette() -> TerminalPalette {
        let mut ansi = [0; 16];
        for (index, color) in ansi.iter_mut().enumerate() {
            *color = 0xFF00_0000 | index as u32;
        }

        TerminalPalette {
            default_fg: 0xFFAA_BBCC,
            default_bg: 0xFF11_2233,
            cursor: 0xFFFF_FFFF,
            ansi,
        }
    }

    #[test]
    fn rendering_preserves_the_callers_enclosing_clip() {
        let mut model = AppModel::new(100, 100, 1.0);
        let (pty, _pty_rx) = PtyHandle::new_for_test();
        let (msg_tx, _msg_rx) = mpsc::channel();
        model
            .terminal
            .sessions
            .push(TerminalSession::new(7, 4, 10, pty, msg_tx));

        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = crate::view::GlyphCache::default();
        let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);
        let mut buffer = vec![0u32; 100 * 100];
        let mut frame = Frame::new(&mut buffer, 100, 100);

        frame.push_clip(Rect::new(10.0, 10.0, 80.0, 80.0));
        render_terminal_panel(
            &mut frame,
            &mut painter,
            &model,
            Rect::new(20.0, 20.0, 60.0, 60.0),
        );

        // The terminal's clip must be gone while the dock's enclosing clip
        // remains active. Popping it mirrors `DockPaneScene::render` and was
        // the point that previously panicked.
        frame.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), 0xFF00_FF00);
        assert_eq!(frame.get_pixel(15, 15), 0xFF00_FF00);
        assert_eq!(frame.get_pixel(5, 5), 0);
        frame.pop_clip();
    }

    #[test]
    fn grid_size_floors_content_rect_to_terminal_cells() {
        let rect = Rect::new(0.0, 0.0, 241.9, 41.0);

        assert_eq!(
            grid_size_for_rect(rect, 8.0, 20),
            TerminalGridSize { rows: 2, cols: 30 }
        );
    }

    #[test]
    fn grid_size_never_returns_zero_cells() {
        let rect = Rect::new(0.0, 0.0, 0.0, 0.0);

        assert_eq!(
            grid_size_for_rect(rect, 8.0, 20),
            TerminalGridSize { rows: 1, cols: 1 }
        );
    }

    #[test]
    fn terminal_view_row_to_grid_line_accounts_for_scroll_offset() {
        // Row 0 with the view scrolled up 2 lines shows 2 lines of history,
        // which alacritty addresses with negative `Line` indices.
        assert_eq!(terminal_view_row_to_grid_line(0, 2), -2);
        assert_eq!(terminal_view_row_to_grid_line(3, 2), 1);
    }

    fn scrollback_term() -> alacritty_terminal::Term<alacritty_terminal::event::VoidListener> {
        use alacritty_terminal::term::{test::TermSize, Config};
        use alacritty_terminal::vte::ansi::Processor;

        let size = TermSize::new(20, 4);
        let mut term = alacritty_terminal::Term::new(
            Config::default(),
            &size,
            alacritty_terminal::event::VoidListener,
        );
        let mut parser: Processor = Processor::new();
        // 10 lines into a 4-row screen -> 6 lines of scrollback history.
        for line in 0..10 {
            parser.advance(&mut term, format!("line{line}\r\n").as_bytes());
        }
        term
    }

    #[test]
    fn rendering_scrolled_back_history_indexes_valid_grid_lines() {
        // Regression: the renderer used positive `row + scroll_offset` line
        // indices, which addressed the live screen (and ran past it into an
        // out-of-range assertion) instead of reading negative history lines.
        let term = scrollback_term();
        let grid = term.grid();
        let rows = grid.screen_lines();
        let max_offset = grid.total_lines().saturating_sub(rows);
        let topmost = rows as i32 - grid.total_lines() as i32;

        for scroll_offset in 0..=max_offset {
            for row in 0..rows {
                let grid_line = terminal_view_row_to_grid_line(row, scroll_offset);
                assert!(
                    (topmost..rows as i32).contains(&grid_line),
                    "row {row} offset {scroll_offset} produced out-of-range line {grid_line}"
                );
                // Must not panic on the alacritty bounds assertion.
                let _ = grid[Line(grid_line)][Column(0)].c;
            }
        }
    }

    #[test]
    fn color_resolution_maps_defaults_and_cursor_color() {
        let palette = test_palette();

        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Named(NamedColor::Foreground),
                &palette,
                TerminalColorRole::Foreground,
            ),
            palette.default_fg
        );
        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Named(NamedColor::Background),
                &palette,
                TerminalColorRole::Background,
            ),
            palette.default_bg
        );
        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Named(NamedColor::Cursor),
                &palette,
                TerminalColorRole::Foreground,
            ),
            palette.cursor
        );
    }

    #[test]
    fn color_resolution_maps_rgb_and_16_color_palette_entries() {
        let palette = test_palette();

        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Spec(Rgb {
                    r: 0x12,
                    g: 0x34,
                    b: 0x56,
                }),
                &palette,
                TerminalColorRole::Foreground,
            ),
            0xFF12_3456
        );
        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Named(NamedColor::BrightBlue),
                &palette,
                TerminalColorRole::Foreground,
            ),
            palette.ansi[12]
        );
        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Indexed(3),
                &palette,
                TerminalColorRole::Foreground,
            ),
            palette.ansi[3]
        );
    }

    #[test]
    fn color_resolution_maps_256_color_palette_entries() {
        let palette = test_palette();

        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Indexed(196),
                &palette,
                TerminalColorRole::Foreground,
            ),
            0xFFFF_0000
        );
        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Indexed(232),
                &palette,
                TerminalColorRole::Foreground,
            ),
            0xFF08_0808
        );
        assert_eq!(
            resolve_terminal_color(
                AnsiColor::Indexed(255),
                &palette,
                TerminalColorRole::Foreground,
            ),
            0xFFEE_EEEE
        );
    }

    #[test]
    fn dim_cells_reduce_foreground_intensity() {
        let palette = test_palette();
        let mut cell = alacritty_terminal::term::cell::Cell {
            fg: AnsiColor::Spec(Rgb {
                r: 0x60,
                g: 0x30,
                b: 0x18,
            }),
            ..Default::default()
        };
        cell.flags.insert(Flags::DIM);

        let (fg, bg) = cell_colors(&cell, &palette);

        assert_eq!(fg, 0xFF40_2010);
        assert_eq!(bg, palette.default_bg);
    }

    #[test]
    fn cell_decorations_track_terminal_text_attributes() {
        let mut cell = alacritty_terminal::term::cell::Cell::default();
        cell.flags
            .insert(Flags::BOLD | Flags::ITALIC | Flags::UNDERLINE | Flags::STRIKEOUT);

        let decorations = cell_decorations(&cell);

        assert!(decorations.bold);
        assert!(decorations.italic);
        assert!(decorations.underline);
        assert!(decorations.strikeout);
    }

    #[test]
    fn scrollback_indicator_is_only_shown_when_scrolled_up() {
        assert_eq!(scrollback_indicator_text(0, 10), None);
        assert_eq!(scrollback_indicator_text(3, 10), Some("3/10".to_string()));
    }
    #[test]
    fn terminal_pointer_uses_painted_cells_and_scrollback_at_both_scales() {
        for scale in [1.0, 2.0] {
            let (pty, _) = PtyHandle::new_for_test();
            let (tx, _) = mpsc::channel();
            let mut session = TerminalSession::new(0, 2, 4, pty, tx);
            session.apply_bytes(b"one\r\ntwo\r\nthree\r\nfour");
            session.scroll_offset = 1;
            let viewport = TerminalViewport::new(
                &session,
                Rect::new(20.0 * scale, 40.0 * scale, 40.0 * scale, 40.0 * scale),
                10.0 * scale,
                (20.0 * scale) as usize,
            );
            assert_eq!(
                viewport.point_at((31.0 * scale) as f64, (45.0 * scale) as f64),
                (Point::new(Line(-1), Column(1)), Side::Left)
            );
            assert_eq!(
                viewport.point_at((39.0 * scale) as f64, (65.0 * scale) as f64),
                (Point::new(Line(0), Column(1)), Side::Right)
            );
            assert_eq!(
                viewport.point_at(-100.0, -100.0),
                (Point::new(Line(-1), Column(0)), Side::Left)
            );
            assert_eq!(
                viewport.point_at(10000.0, 10000.0),
                (Point::new(Line(0), Column(3)), Side::Right)
            );
        }
    }
}
