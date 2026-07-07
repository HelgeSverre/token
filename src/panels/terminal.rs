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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TerminalCellDecorations {
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
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

    // Oldest addressable line: history lines sit *above* the screen and are
    // addressed with negative `Line` indices in alacritty.
    let topmost_line = grid.screen_lines() as i32 - grid.total_lines() as i32;

    for row in 0..rows {
        let grid_line = terminal_view_row_to_grid_line(row, scroll_offset);
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

    let cell_rect = terminal_cell_rect(ctx, row, col);
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

fn terminal_cell_rect(ctx: &TerminalRenderContext<'_>, row: usize, col: usize) -> Rect {
    Rect::new(
        ctx.rect.x + col as f32 * ctx.char_width,
        ctx.rect.y + (row * ctx.line_height) as f32,
        ctx.char_width.ceil().max(1.0),
        ctx.line_height.max(1) as f32,
    )
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

    let padding = ctx.char_width.max(1.0).round();
    let width = text.chars().count() as f32 * ctx.char_width + padding;
    let x = (ctx.rect.x + ctx.rect.width - width).max(ctx.rect.x);
    let rect = Rect::new(x, ctx.rect.y, width, ctx.line_height.max(1) as f32);
    frame.fill_rect(rect, ctx.palette.default_bg);
    painter.draw(
        frame,
        (x + padding / 2.0) as usize,
        ctx.rect.y as usize,
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
}
