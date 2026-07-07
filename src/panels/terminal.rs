//! Terminal dock panel rendering helpers.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::vte::ansi::{Color as AnsiColor, NamedColor};

use crate::model::editor_area::Rect;
use crate::model::AppModel;
use crate::view::frame::{Frame, TextPainter};

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

struct TerminalRenderContext<'a> {
    rect: Rect,
    char_width: f32,
    line_height: usize,
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
        AnsiColor::Indexed(index) => palette
            .ansi
            .get(index as usize)
            .copied()
            .unwrap_or_else(|| fallback_color(palette, role)),
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

    frame.set_clip(rect);

    let term = session.term();
    let grid = term.grid();
    let line_height = painter.line_height();
    let char_width = painter.char_width();
    let visible_size = grid_size_for_rect(rect, char_width, line_height);
    let rows = usize::from(visible_size.rows).min(grid.screen_lines());
    let cols = usize::from(visible_size.cols).min(grid.columns());

    // Clamp scrollback offset to the available history so it can never
    // scroll past the top of the buffer.
    let max_offset = grid.total_lines().saturating_sub(grid.screen_lines());
    let scroll_offset = session.scroll_offset.min(max_offset);

    let ctx = TerminalRenderContext {
        rect,
        char_width,
        line_height,
        palette: &palette,
    };

    for row in 0..rows {
        let grid_line = terminal_view_row_to_grid_line(row, scroll_offset);
        if grid_line >= grid.total_lines() {
            break;
        }
        for col in 0..cols {
            let cell = &grid[Line(grid_line as i32)][Column(col)];
            render_terminal_cell(frame, painter, &ctx, row, col, cell);
        }
    }

    render_terminal_cursor(frame, painter, &ctx, rows, cols, grid, scroll_offset);

    frame.clear_clip();
}

fn render_terminal_cell(
    frame: &mut Frame,
    painter: &mut TextPainter,
    ctx: &TerminalRenderContext<'_>,
    row: usize,
    col: usize,
    cell: &alacritty_terminal::term::cell::Cell,
) {
    let skip_glyph = cell.flags.contains(Flags::WIDE_CHAR_SPACER | Flags::HIDDEN);
    let (fg, bg) = cell_colors(cell, ctx.palette);
    let cell_rect = terminal_cell_rect(ctx, row, col);
    if bg != ctx.palette.default_bg {
        frame.fill_rect(cell_rect, bg);
    }

    if skip_glyph || cell.c == ' ' {
        return;
    }

    let mut buf = [0; 4];
    let text = cell.c.encode_utf8(&mut buf);
    painter.draw(frame, cell_rect.x as usize, cell_rect.y as usize, text, fg);
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
    let cursor_row = cursor.line.0 as usize;

    // Hide the cursor when scrolled away from the live bottom of the buffer.
    if cursor_row < scroll_offset || cursor_row >= scroll_offset + rows {
        return;
    }

    let row = cursor_row - scroll_offset;
    let col = cursor.column.0;
    if col >= cols {
        return;
    }

    let cell_rect = terminal_cell_rect(ctx, row, col);
    frame.fill_rect(cell_rect, ctx.palette.cursor);

    let grid_line = terminal_view_row_to_grid_line(row, scroll_offset);
    let cell = &grid[Line(grid_line as i32)][Column(col)];
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

fn terminal_cell_rect(ctx: &TerminalRenderContext<'_>, row: usize, col: usize) -> Rect {
    Rect::new(
        ctx.rect.x + col as f32 * ctx.char_width,
        ctx.rect.y + (row * ctx.line_height) as f32,
        ctx.char_width.ceil().max(1.0),
        ctx.line_height.max(1) as f32,
    )
}

fn terminal_view_row_to_grid_line(row: usize, scroll_offset: usize) -> usize {
    row.saturating_add(scroll_offset)
}

fn cell_colors(
    cell: &alacritty_terminal::term::cell::Cell,
    palette: &TerminalPalette,
) -> (u32, u32) {
    let fg = resolve_terminal_color(cell.fg, palette, TerminalColorRole::Foreground);
    let bg = resolve_terminal_color(cell.bg, palette, TerminalColorRole::Background);

    if cell.flags.contains(Flags::INVERSE) {
        (bg, fg)
    } else {
        (fg, bg)
    }
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
        assert_eq!(terminal_view_row_to_grid_line(0, 2), 2);
        assert_eq!(terminal_view_row_to_grid_line(3, 2), 5);
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
}
