//! The unified overlay surface: one component (chrome, header, sectioned
//! list, footer) driving every palette/picker/popup in the editor.
//!
//! Phase 1 ([docs/feature/overlay-surface.md](../../docs/feature/overlay-surface.md))
//! implements `Anchor::Centered`, `Header`, `Body::List`, and `Footer` —
//! enough to render the command palette. Tabs, `ListWithPanel`, `Zones`,
//! and `Fields` land with the phases that consume them.
//!
//! `layout()` is the single source of truth for geometry; it is designed
//! to be shared with hit-testing once that lands (Phase 3), the same way
//! `ModalLayout` is shared by `view::modal` and `view::hit_test` today.

use super::frame::{Frame, RoundedRectMaskCache, TextPainter};
use super::geometry::WidgetRect;
use crate::theme::OverlayTheme;

/// Logical-px chrome constants for `Anchor::Centered`, per the Visual
/// Language spec in overlay-surface.md. All rendered as
/// `round(v * scale_factor)`, with a 1px floor for strokes.
mod dims {
    pub const RADIUS: f32 = 10.0;
    pub const HEADER_PAD_X: f32 = 16.0;
    pub const PAD_Y: f32 = 12.0;
    pub const ROW_HEIGHT: f32 = 30.0;
    pub const ROW_INSET: f32 = 6.0;
    pub const ROW_RADIUS: f32 = 6.0;
    pub const ROW_ICON_W: f32 = 18.0;
    pub const ROW_TEXT_PAD_X: f32 = 8.0;
    pub const FOOTER_HEIGHT: f32 = 30.0;
    pub const SCROLLBAR_WIDTH: f32 = 3.0;
    pub const SCROLLBAR_INSET: f32 = 2.0;
    pub const SCROLLBAR_MIN_LEN: f32 = 20.0;
    pub const Y: f32 = 64.0;
    /// Gap between chips within one chord step's keycap accessory.
    pub const CHIP_GAP: f32 = 4.0;
    /// Gap between chord steps in a keycap accessory (Visual Language >
    /// Keycaps: "6px gap between steps").
    pub const CHIP_STEP_GAP: f32 = 6.0;
}

/// The three-size type scale (input / rows / metadata), in logical px.
pub const SIZE_INPUT: f32 = 14.0;
pub const SIZE_ROW: f32 = 13.0;
pub const SIZE_META: f32 = 11.0;

#[inline]
fn scaled(v: f32, scale_factor: f64) -> usize {
    (v as f64 * scale_factor).round().max(1.0) as usize
}

#[inline]
fn size_px(logical: f32, scale_factor: f64) -> f32 {
    (logical as f64 * scale_factor) as f32
}

/// Horizontal padding inside the header, in physical px — exposed so
/// `view::caret` can position the IME caret rect inside the header without
/// duplicating the layout constant.
pub fn header_pad_x(scale_factor: f64) -> usize {
    scaled(dims::HEADER_PAD_X, scale_factor)
}

/// A modal width rule: percent of window width, clamped to a logical-px
/// min/max, then clamped again to leave a margin against the window edges.
pub struct WidthRule {
    pub pct: f32,
    pub min: f32,
    pub max: f32,
}

pub enum Anchor {
    /// Centered X; Y follows the Chrome table's `min(h/4, Y)` class. Dims
    /// the backdrop at `dim_alpha`.
    Centered { width: WidthRule, dim_alpha: u8 },
}

pub struct Header<'a> {
    pub glyph: Option<char>,
    pub text: &'a str,
    pub placeholder: &'a str,
    /// Char index of the caret; `None` means a display-only header (no
    /// input, e.g. a future title-only context).
    pub caret: Option<usize>,
    /// Right-aligned dim text, e.g. `"workspace: token"`.
    pub scope: Option<&'a str>,
}

pub enum RowIcon {
    None,
    Glyph { ch: char, color: u32 },
}

pub enum Accessory<'a> {
    None,
    DimText(&'a str),
    Check,
    /// Keycap chips for a keybinding: outer = chord steps, inner = the
    /// chips within a step (one per modifier, one for the key). Built by
    /// `binding_chips`; more than 4 chips total should fall back to
    /// `DimText` before reaching here (Visual Language > Keycaps).
    Keycaps(&'a [Vec<Chip>]),
}

/// One keycap chip's label (e.g. `"⌘"`, `"⇧"`, `"T"`, `"F12"`).
#[derive(Debug, Clone)]
pub struct Chip {
    pub label: String,
}

/// Split a platform keybinding display string (e.g. `"⇧⌘N"`, `"⌘K ⌘C"`) into
/// chord steps of chips — one chip per leading modifier glyph, one chip for
/// the trailing key (kept together regardless of how many glyphs it has, so
/// `"F12"` is one chip, not four). Space separates chord steps.
pub fn binding_chips(binding: &str) -> Vec<Vec<Chip>> {
    const MODIFIERS: [char; 4] = ['⌃', '⌥', '⇧', '⌘'];
    binding
        .split(' ')
        .filter(|step| !step.is_empty())
        .map(|step| {
            let mut chips: Vec<Chip> = step
                .chars()
                .take_while(|c| MODIFIERS.contains(c))
                .map(|c| Chip {
                    label: c.to_string(),
                })
                .collect();
            let key: String = step.chars().skip_while(|c| MODIFIERS.contains(c)).collect();
            if !key.is_empty() {
                chips.push(Chip { label: key });
            }
            chips
        })
        .collect()
}

/// Total chip count across all chord steps — callers use this against the
/// >4-chip fallback threshold (Visual Language > Keycaps).
pub fn chip_count(steps: &[Vec<Chip>]) -> usize {
    steps.iter().map(Vec::len).sum()
}

pub struct Row<'a> {
    pub icon: RowIcon,
    pub label: &'a str,
    /// Nucleo char indices into `label`; coalesced into runs at paint time.
    pub match_indices: &'a [u32],
    /// Dim inline text (path, description); truncates before the label.
    pub detail: Option<&'a str>,
    pub accessory: Accessory<'a>,
}

pub struct Section<'a> {
    pub title: Option<&'a str>,
    pub rows: &'a [Row<'a>],
}

/// Index into the flattened concatenation of all section rows — section
/// headers are not addressable, so Up/Down naturally skip them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatIndex(pub usize);

impl FlatIndex {
    /// Next selectable row, wrapping at the end. `total` excludes headers.
    pub fn next(self, total: usize) -> FlatIndex {
        if total == 0 {
            return FlatIndex(0);
        }
        FlatIndex((self.0 + 1) % total)
    }

    /// Previous selectable row, wrapping at the start.
    pub fn prev(self, total: usize) -> FlatIndex {
        if total == 0 {
            return FlatIndex(0);
        }
        FlatIndex((self.0 + total - 1) % total)
    }
}

pub struct Footer<'a> {
    pub leading: &'a str,
    pub trailing: &'a str,
}

pub enum Body<'a> {
    List {
        sections: &'a [Section<'a>],
        selected: FlatIndex,
        scroll: usize,
        max_visible: usize,
    },
}

impl<'a> Body<'a> {
    /// Total selectable rows across all sections (headers excluded).
    pub fn total_rows(&self) -> usize {
        match self {
            Body::List { sections, .. } => sections.iter().map(|s| s.rows.len()).sum(),
        }
    }
}

pub struct OverlaySpec<'a> {
    pub anchor: Anchor,
    pub header: Header<'a>,
    pub body: Body<'a>,
    pub footer: Option<Footer<'a>>,
}

/// One entry in the flattened, on-screen row list: either a section header
/// (not selectable) or a row paired with its `FlatIndex`.
enum DisplayRow<'a> {
    SectionHeader(&'a str),
    Row(&'a Row<'a>, FlatIndex),
}

fn flatten_rows<'a>(sections: &'a [Section<'a>]) -> Vec<DisplayRow<'a>> {
    let mut out = Vec::new();
    let mut flat_i = 0;
    for section in sections {
        if let Some(title) = section.title {
            out.push(DisplayRow::SectionHeader(title));
        }
        for row in section.rows {
            out.push(DisplayRow::Row(row, FlatIndex(flat_i)));
            flat_i += 1;
        }
    }
    out
}

/// Resolve `scroll` (a `FlatIndex`-space row offset, e.g. from
/// `SelectableListViewport`) into a display-row-space window: the first
/// visible display slot and the visible count. Section headers occupy
/// display slots but not `FlatIndex` slots, so scroll must be re-anchored
/// to the display row that actually carries that `FlatIndex` rather than
/// applied as a raw offset into `display_rows` — otherwise every header
/// above the window shifts the selected row out of view by one slot.
fn resolve_visible_window(
    display_rows: &[DisplayRow],
    scroll: usize,
    max_visible: usize,
) -> (usize, usize) {
    let visible = display_rows.len().min(max_visible);
    let scroll_display = display_rows
        .iter()
        .position(|dr| matches!(dr, DisplayRow::Row(_, FlatIndex(i)) if *i == scroll))
        .unwrap_or(scroll);
    let start = scroll_display.min(display_rows.len().saturating_sub(visible));
    (start, visible)
}

/// Coalesce ascending, deduplicated nucleo match char-indices into
/// contiguous `[start, end)` runs, so match highlighting can paint one
/// blend region per run instead of one per matched character.
pub fn coalesce_match_indices(indices: &[u32]) -> Vec<(u32, u32)> {
    let mut runs = Vec::new();
    let mut iter = indices.iter().copied();
    let Some(mut start) = iter.next() else {
        return runs;
    };
    let mut end = start + 1;
    for idx in iter {
        if idx == end {
            end = idx + 1;
        } else {
            runs.push((start, end));
            start = idx;
            end = idx + 1;
        }
    }
    runs.push((start, end));
    runs
}

/// Tail-ellipsize `text` to fit `max_width`, appending `…`. Char-boundary
/// safe for multi-byte text.
pub fn truncate_tail(painter: &mut TextPainter, size: f32, text: &str, max_width: f32) -> String {
    if painter.measure_sized(text, size, 0.0) <= max_width {
        return text.to_string();
    }
    let ellipsis_w = painter.measure_sized("\u{2026}", size, 0.0);
    let budget = (max_width - ellipsis_w).max(0.0);
    let mut out = String::new();
    let mut w = 0.0;
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        let cw = painter.measure_sized(ch.encode_utf8(&mut buf), size, 0.0);
        if w + cw > budget {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push('\u{2026}');
    out
}

/// Head-ellipsize `text` to fit `max_width`, prepending `…` (used for
/// `detail` truncation, e.g. `…/view/geometry.rs`).
pub fn truncate_head(painter: &mut TextPainter, size: f32, text: &str, max_width: f32) -> String {
    if painter.measure_sized(text, size, 0.0) <= max_width {
        return text.to_string();
    }
    let ellipsis_w = painter.measure_sized("\u{2026}", size, 0.0);
    let budget = (max_width - ellipsis_w).max(0.0);
    let mut kept: Vec<char> = Vec::new();
    let mut w = 0.0;
    let mut buf = [0u8; 4];
    for ch in text.chars().rev() {
        let cw = painter.measure_sized(ch.encode_utf8(&mut buf), size, 0.0);
        if w + cw > budget {
            break;
        }
        kept.push(ch);
        w += cw;
    }
    kept.reverse();
    let mut out = String::from('\u{2026}');
    out.extend(kept);
    out
}

/// Computed geometry for an `Anchor::Centered` surface — panel chrome,
/// header, list rows (already scrolled to the visible window), footer, and
/// scrollbar thumb. The single source of truth shared by `render()` today
/// and by hit-testing once Phase 3 lands.
pub struct OverlayLayout {
    pub panel: WidgetRect,
    pub header: WidgetRect,
    pub row_height: usize,
    /// One rect per visible display row (headers included), in list order.
    pub rows: Vec<WidgetRect>,
    pub footer: Option<WidgetRect>,
    pub scrollbar: Option<WidgetRect>,
}

/// Layout an `Anchor::Centered` surface against the (physical-px) window
/// size. This is the one layout function the doc calls for — paint and
/// (later) hit-testing both consume it.
pub fn layout(
    spec: &OverlaySpec,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
) -> OverlayLayout {
    let Anchor::Centered { width, .. } = &spec.anchor;

    let margin = scaled(32.0, scale_factor);
    let min_w = size_px(width.min, scale_factor) as usize;
    let max_w = size_px(width.max, scale_factor) as usize;
    let panel_w = ((window_width as f32 * width.pct) as usize)
        .clamp(min_w, max_w)
        .min(window_width.saturating_sub(margin));

    let header_h = scaled(SIZE_INPUT, scale_factor) + 2 * scaled(dims::PAD_Y, scale_factor);
    let row_h = scaled(dims::ROW_HEIGHT, scale_factor);
    let footer_h = spec
        .footer
        .as_ref()
        .map(|_| scaled(dims::FOOTER_HEIGHT, scale_factor));

    let Body::List {
        sections,
        scroll,
        max_visible,
        ..
    } = &spec.body;
    let display_rows = flatten_rows(sections);
    let (start, visible) = resolve_visible_window(&display_rows, *scroll, *max_visible);
    let list_h = visible * row_h;

    let panel_h = header_h + list_h + footer_h.unwrap_or(0);
    let panel_x = window_width.saturating_sub(panel_w) / 2;
    let panel_y = scaled(dims::Y, scale_factor).min(window_height / 4);

    let panel = WidgetRect {
        x: panel_x,
        y: panel_y,
        w: panel_w,
        h: panel_h,
    };
    let header = WidgetRect {
        x: panel.x,
        y: panel.y,
        w: panel.w,
        h: header_h,
    };

    let list_top = header.y + header.h;
    let rows: Vec<WidgetRect> = (0..visible)
        .map(|i| WidgetRect {
            x: panel.x,
            y: list_top + i * row_h,
            w: panel.w,
            h: row_h,
        })
        .collect();

    let footer = footer_h.map(|h| WidgetRect {
        x: panel.x,
        y: list_top + list_h,
        w: panel.w,
        h,
    });

    let total = display_rows.len();
    let scrollbar = if total > *max_visible {
        let inset = scaled(dims::SCROLLBAR_INSET, scale_factor);
        let sb_w = scaled(dims::SCROLLBAR_WIDTH, scale_factor);
        let min_len = scaled(dims::SCROLLBAR_MIN_LEN, scale_factor);
        let track_h = list_h;
        let thumb_h = ((visible as f32 / total as f32) * track_h as f32).round() as usize;
        let thumb_h = thumb_h.max(min_len).min(track_h);
        let max_thumb_y = track_h.saturating_sub(thumb_h);
        let scroll_range = total.saturating_sub(visible).max(1);
        let thumb_y = (start * max_thumb_y) / scroll_range;
        Some(WidgetRect {
            x: panel.x + panel.w.saturating_sub(sb_w + inset),
            y: list_top + thumb_y,
            w: sb_w,
            h: thumb_h,
        })
    } else {
        None
    };

    OverlayLayout {
        panel,
        header,
        row_height: row_h,
        rows,
        footer,
        scrollbar,
    }
}

/// Resolved colors pulled once from `OverlayTheme` per render call.
struct Palette {
    panel_bg: u32,
    hairline: u32,
    text_primary: u32,
    text_bright: u32,
    text_dim: u32,
    accent_bright: u32,
    match_on_selection: u32,
    selection_wash: u32,
    recessed_wash: u32,
    keycap_bg: u32,
    keycap_border: u32,
    keycap_fg: u32,
}

impl Palette {
    fn from_theme(theme: &OverlayTheme) -> Self {
        Self {
            panel_bg: theme.panel_background.to_argb_u32(),
            hairline: theme.hairline.to_argb_u32(),
            text_primary: theme.text_primary.to_argb_u32(),
            text_bright: theme.text_bright.to_argb_u32(),
            text_dim: theme.text_dim.to_argb_u32(),
            accent_bright: theme.accent_bright.to_argb_u32(),
            match_on_selection: theme.match_on_selection.to_argb_u32(),
            selection_wash: theme.selection_wash.to_argb_u32(),
            recessed_wash: theme.recessed_wash.to_argb_u32(),
            keycap_bg: theme.keycap_bg.to_argb_u32(),
            keycap_border: theme.keycap_border.to_argb_u32(),
            keycap_fg: theme.keycap_fg.to_argb_u32(),
        }
    }
}

/// Render an `Anchor::Centered` `OverlaySpec`: backdrop dim, shadow, panel,
/// header, list rows (sections, match highlighting, accessories,
/// truncation, scrollbar), footer.
#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    painter: &mut TextPainter,
    mask_cache: &mut RoundedRectMaskCache,
    theme: &OverlayTheme,
    spec: &OverlaySpec,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
    cursor_visible: bool,
) {
    let Anchor::Centered { dim_alpha, .. } = &spec.anchor;
    let layout = layout(spec, window_width, window_height, scale_factor);
    let colors = Palette::from_theme(theme);
    let radius = scaled(dims::RADIUS, scale_factor);

    frame.dim(*dim_alpha);
    frame.draw_shadow_rings(
        layout.panel.x,
        layout.panel.y,
        layout.panel.w,
        layout.panel.h,
        radius,
        mask_cache,
    );
    frame.fill_rounded_rect(
        layout.panel.x,
        layout.panel.y,
        layout.panel.w,
        layout.panel.h,
        radius,
        colors.panel_bg,
        mask_cache,
    );
    // 1px light hairline edge — the dark edge the panel reads against comes
    // from the shadow rings above, never from this border (Visual Language:
    // Chrome > Border).
    frame.stroke_rounded_rect(
        layout.panel.x,
        layout.panel.y,
        layout.panel.w,
        layout.panel.h,
        radius,
        colors.hairline,
        mask_cache,
    );

    render_header(
        frame,
        painter,
        &colors,
        &spec.header,
        &layout,
        scale_factor,
        cursor_visible,
    );
    render_list(
        frame,
        painter,
        mask_cache,
        &colors,
        spec,
        &layout,
        scale_factor,
    );
    if let (Some(footer_spec), Some(footer_rect)) = (&spec.footer, layout.footer) {
        render_footer(
            frame,
            painter,
            &colors,
            footer_spec,
            footer_rect,
            scale_factor,
        );
    }
}

/// X position for a caret at char column `col` of `text`, measured from
/// `base_x` — i.e. the width of `text` truncated to `col` chars, not the
/// width of the whole string. `col` beyond `text`'s length clamps to the end
/// (mirrors `str::chars().take(col)` behavior).
fn caret_x_for_column(
    painter: &mut TextPainter,
    base_x: usize,
    text: &str,
    col: usize,
    size: f32,
) -> usize {
    let before: String = text.chars().take(col).collect();
    base_x + painter.measure_sized(&before, size, 0.0).ceil() as usize + 1
}

#[allow(clippy::too_many_arguments)]
fn render_header(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    header: &Header,
    layout: &OverlayLayout,
    scale_factor: f64,
    cursor_visible: bool,
) {
    let size = size_px(SIZE_INPUT, scale_factor);
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
    let r = layout.header;

    // Bottom hairline separating the header from the list.
    frame.fill_rect_px(r.x, r.y + r.h.saturating_sub(1), r.w, 1, colors.hairline);

    let text_h = painter.line_height_for_size(size);
    let text_y = r.y + (r.h.saturating_sub(text_h)) / 2;
    let mut x = r.x + pad_x;

    if let Some(glyph) = header.glyph {
        let w = painter.draw_sized(
            frame,
            x,
            text_y,
            &glyph.to_string(),
            size,
            0.0,
            colors.text_dim,
        );
        x += w.ceil() as usize + pad_x / 2;
    }

    if header.text.is_empty() {
        painter.draw_sized(
            frame,
            x,
            text_y,
            header.placeholder,
            size,
            0.0,
            colors.text_dim,
        );
    } else {
        painter.draw_sized(
            frame,
            x,
            text_y,
            header.text,
            size,
            0.0,
            colors.text_primary,
        );
    }

    if let Some(col) = header.caret {
        if cursor_visible {
            let caret_x = caret_x_for_column(painter, x, header.text, col, size);
            let caret_w = scaled(1.5, scale_factor);
            frame.fill_rect_px(caret_x, text_y, caret_w, text_h, colors.accent_bright);
        }
    }

    if let Some(scope) = header.scope {
        let scope_size = size_px(SIZE_META, scale_factor);
        let w = painter.measure_sized(scope, scope_size, 0.0).ceil() as usize;
        let scope_x = r.x + r.w.saturating_sub(pad_x + w);
        let scope_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(scope_size))) / 2;
        painter.draw_sized(
            frame,
            scope_x,
            scope_y,
            scope,
            scope_size,
            0.0,
            colors.text_dim,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_list(
    frame: &mut Frame,
    painter: &mut TextPainter,
    mask_cache: &mut RoundedRectMaskCache,
    colors: &Palette,
    spec: &OverlaySpec,
    layout: &OverlayLayout,
    scale_factor: f64,
) {
    let Body::List {
        sections,
        selected,
        scroll,
        max_visible,
    } = &spec.body;
    let display_rows = flatten_rows(sections);
    let (start, _visible) = resolve_visible_window(&display_rows, *scroll, *max_visible);

    let row_size = size_px(SIZE_ROW, scale_factor);
    let meta_size = size_px(SIZE_META, scale_factor);
    let inset = scaled(dims::ROW_INSET, scale_factor);
    let row_radius = scaled(dims::ROW_RADIUS, scale_factor);
    let icon_w = scaled(dims::ROW_ICON_W, scale_factor);
    let text_pad = scaled(dims::ROW_TEXT_PAD_X, scale_factor);

    for (slot, rect) in layout.rows.iter().enumerate() {
        let Some(display_row) = display_rows.get(start + slot) else {
            continue;
        };
        match display_row {
            DisplayRow::SectionHeader(title) => {
                let text_y = rect.y
                    + (rect
                        .h
                        .saturating_sub(painter.line_height_for_size(meta_size)))
                        / 2;
                painter.draw_sized(
                    frame,
                    rect.x + text_pad,
                    text_y,
                    &title.to_uppercase(),
                    meta_size,
                    1.0,
                    colors.text_dim,
                );
            }
            DisplayRow::Row(row, flat_index) => {
                let is_selected = *flat_index == *selected;
                if is_selected {
                    frame.fill_rounded_rect(
                        rect.x + inset,
                        rect.y,
                        rect.w.saturating_sub(inset * 2),
                        rect.h,
                        row_radius,
                        colors.selection_wash,
                        mask_cache,
                    );
                }
                let text_color = if is_selected {
                    colors.text_bright
                } else {
                    colors.text_primary
                };
                let match_color = if is_selected {
                    colors.match_on_selection
                } else {
                    colors.accent_bright
                };

                let mut x = rect.x + inset + text_pad;
                if let RowIcon::Glyph { ch, color } = row.icon {
                    let text_y = rect.y
                        + (rect
                            .h
                            .saturating_sub(painter.line_height_for_size(row_size)))
                            / 2;
                    let mut buf = [0u8; 4];
                    painter.draw_sized(
                        frame,
                        x,
                        text_y,
                        ch.encode_utf8(&mut buf),
                        row_size,
                        0.0,
                        color,
                    );
                }
                x += icon_w;

                // Reserve the accessory's measured width so it never truncates.
                let accessory_w = accessory_width(painter, &row.accessory, meta_size, scale_factor);
                let label_right = rect.x + rect.w.saturating_sub(inset + text_pad + accessory_w);
                let available = label_right.saturating_sub(x);
                let text_y = rect.y
                    + (rect
                        .h
                        .saturating_sub(painter.line_height_for_size(row_size)))
                        / 2;

                // Truncation priority: detail truncates first (head-first);
                // only if the label still doesn't fit at full width does it
                // tail-ellipsize. The accessory (above) never truncates.
                let full_label_w = painter.measure_sized(row.label, row_size, 0.0);
                if full_label_w <= available as f32 {
                    draw_label_with_matches(
                        frame,
                        painter,
                        x,
                        text_y,
                        row.label,
                        row_size,
                        row.match_indices,
                        text_color,
                        match_color,
                    );
                    if let Some(detail) = row.detail {
                        let gap = text_pad as f32;
                        let leftover = available as f32 - full_label_w - gap;
                        if leftover > 0.0 {
                            let detail_x = x + full_label_w.round() as usize + text_pad;
                            let detail = truncate_head(painter, meta_size, detail, leftover);
                            painter.draw_sized(
                                frame,
                                detail_x,
                                text_y,
                                &detail,
                                meta_size,
                                0.0,
                                colors.text_dim,
                            );
                        }
                    }
                } else {
                    let label = truncate_tail(painter, row_size, row.label, available as f32);
                    draw_label_with_matches(
                        frame,
                        painter,
                        x,
                        text_y,
                        &label,
                        row_size,
                        row.match_indices,
                        text_color,
                        match_color,
                    );
                }

                if let Accessory::None = row.accessory {
                } else {
                    let acc_x = rect.x + rect.w.saturating_sub(inset + text_pad + accessory_w);
                    let acc_y = rect.y
                        + (rect
                            .h
                            .saturating_sub(painter.line_height_for_size(meta_size)))
                            / 2;
                    match &row.accessory {
                        Accessory::DimText(text) => {
                            painter.draw_sized(
                                frame,
                                acc_x,
                                acc_y,
                                text,
                                meta_size,
                                0.0,
                                colors.text_dim,
                            );
                        }
                        Accessory::Check => {
                            painter.draw_sized(
                                frame,
                                acc_x,
                                acc_y,
                                "\u{2713}",
                                meta_size,
                                0.0,
                                colors.accent_bright,
                            );
                        }
                        Accessory::Keycaps(steps) => {
                            let chip_h = painter.line_height_for_size(meta_size)
                                + 2 * scaled(2.0, scale_factor);
                            let chip_y = rect.y + (rect.h.saturating_sub(chip_h)) / 2;
                            let chip_gap = scaled(dims::CHIP_GAP, scale_factor);
                            let step_gap = scaled(dims::CHIP_STEP_GAP, scale_factor);
                            let mut cx = acc_x;
                            for (i, step) in steps.iter().enumerate() {
                                if i > 0 {
                                    cx += step_gap;
                                }
                                for (j, chip) in step.iter().enumerate() {
                                    if j > 0 {
                                        cx += chip_gap;
                                    }
                                    let w = super::frame::draw_keycap(
                                        frame,
                                        painter,
                                        mask_cache,
                                        cx,
                                        chip_y,
                                        &chip.label,
                                        colors.keycap_bg,
                                        colors.keycap_border,
                                        colors.keycap_fg,
                                        scale_factor,
                                    );
                                    cx += w;
                                }
                            }
                        }
                        Accessory::None => {}
                    }
                }
            }
        }
    }

    if let Some(sb) = layout.scrollbar {
        let alpha = (colors.text_dim >> 24) & 0xFF;
        let sb_color = (((alpha * 40 / 100) & 0xFF) << 24) | (colors.text_dim & 0x00FF_FFFF);
        frame.blend_rect_px(sb.x, sb.y, sb.w, sb.h, sb_color);
    }
}

fn accessory_width(
    painter: &mut TextPainter,
    accessory: &Accessory,
    meta_size: f32,
    scale_factor: f64,
) -> usize {
    match accessory {
        Accessory::None => 0,
        Accessory::DimText(text) => painter.measure_sized(text, meta_size, 0.0).ceil() as usize,
        Accessory::Check => painter.measure_sized("\u{2713}", meta_size, 0.0).ceil() as usize,
        Accessory::Keycaps(steps) => keycaps_width(painter, steps, scale_factor),
    }
}

/// Total width of a row of keycap chips: chip widths plus the intra-step and
/// inter-step gaps (Visual Language > Keycaps).
fn keycaps_width(painter: &mut TextPainter, steps: &[Vec<Chip>], scale_factor: f64) -> usize {
    let chip_gap = scaled(dims::CHIP_GAP, scale_factor);
    let step_gap = scaled(dims::CHIP_STEP_GAP, scale_factor);
    let mut w = 0;
    for (i, step) in steps.iter().enumerate() {
        if i > 0 {
            w += step_gap;
        }
        for (j, chip) in step.iter().enumerate() {
            if j > 0 {
                w += chip_gap;
            }
            w += super::frame::keycap_width(painter, &chip.label, scale_factor);
        }
    }
    w
}

/// Draw `label` with matched-character runs (from `match_indices`,
/// coalesced) painted in `match_color`, everything else in `base_color`.
#[allow(clippy::too_many_arguments)]
fn draw_label_with_matches(
    frame: &mut Frame,
    painter: &mut TextPainter,
    x: usize,
    y: usize,
    label: &str,
    size: f32,
    match_indices: &[u32],
    base_color: u32,
    match_color: u32,
) {
    if match_indices.is_empty() {
        painter.draw_sized(frame, x, y, label, size, 0.0, base_color);
        return;
    }
    let runs = coalesce_match_indices(match_indices);
    let mut current_x = x as f32;
    let mut buf = [0u8; 4];
    for (i, ch) in label.chars().enumerate() {
        let i = i as u32;
        let matched = runs.iter().any(|(s, e)| i >= *s && i < *e);
        let color = if matched { match_color } else { base_color };
        let w = painter.draw_sized(
            frame,
            current_x.round() as usize,
            y,
            ch.encode_utf8(&mut buf),
            size,
            0.0,
            color,
        );
        current_x += w;
    }
}

fn render_footer(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    footer: &Footer,
    rect: WidgetRect,
    scale_factor: f64,
) {
    frame.fill_rect_px(rect.x, rect.y, rect.w, 1, colors.hairline);
    frame.fill_rect_px(
        rect.x,
        rect.y + 1,
        rect.w,
        rect.h.saturating_sub(1),
        colors.recessed_wash,
    );

    let size = size_px(SIZE_META, scale_factor);
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
    let text_y = rect.y + (rect.h.saturating_sub(painter.line_height_for_size(size))) / 2;

    painter.draw_sized(
        frame,
        rect.x + pad_x,
        text_y,
        footer.leading,
        size,
        0.0,
        colors.text_dim,
    );

    let trailing_w = painter.measure_sized(footer.trailing, size, 0.0).ceil() as usize;
    let trailing_x = rect.x + rect.w.saturating_sub(pad_x + trailing_w);
    painter.draw_sized(
        frame,
        trailing_x,
        text_y,
        footer.trailing,
        size,
        0.0,
        colors.text_dim,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_index_wraps_in_both_directions() {
        assert_eq!(FlatIndex(0).prev(3), FlatIndex(2));
        assert_eq!(FlatIndex(2).next(3), FlatIndex(0));
        assert_eq!(FlatIndex(1).next(3), FlatIndex(2));
        assert_eq!(FlatIndex(1).prev(3), FlatIndex(0));
    }

    #[test]
    fn flat_index_navigation_ignores_section_headers() {
        // Headers aren't part of the FlatIndex space at all, so "skip
        // headers" falls out of flattening rather than needing special
        // Up/Down handling.
        let rows_a = [Row {
            icon: RowIcon::None,
            label: "a",
            match_indices: &[],
            detail: None,
            accessory: Accessory::None,
        }];
        let rows_b = [Row {
            icon: RowIcon::None,
            label: "b",
            match_indices: &[],
            detail: None,
            accessory: Accessory::None,
        }];
        let sections = [
            Section {
                title: Some("Group A"),
                rows: &rows_a,
            },
            Section {
                title: Some("Group B"),
                rows: &rows_b,
            },
        ];
        let display = flatten_rows(&sections);
        // 2 headers + 2 rows = 4 display entries, but only 2 selectable.
        assert_eq!(display.len(), 4);
        let total_selectable: usize = sections.iter().map(|s| s.rows.len()).sum();
        assert_eq!(total_selectable, 2);
        assert_eq!(FlatIndex(0).next(total_selectable), FlatIndex(1));
        assert_eq!(FlatIndex(1).next(total_selectable), FlatIndex(0));
    }

    #[test]
    fn coalesce_merges_consecutive_indices_into_runs() {
        assert_eq!(
            coalesce_match_indices(&[0, 1, 2, 5, 6, 9]),
            vec![(0, 3), (5, 7), (9, 10)]
        );
        assert_eq!(coalesce_match_indices(&[]), vec![]);
        assert_eq!(coalesce_match_indices(&[4]), vec![(4, 5)]);
    }

    fn test_painter_and_frame() -> (fontdue::Font, super::super::GlyphCache) {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        (font, super::super::GlyphCache::default())
    }

    #[test]
    fn truncate_tail_keeps_short_text_unchanged() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let out = truncate_tail(&mut painter, 13.0, "short", 1000.0);
        assert_eq!(out, "short");
    }

    #[test]
    fn truncate_tail_ellipsizes_multibyte_text_on_char_boundaries() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let text = "日本語テキストとても長い文字列です";
        let out = truncate_tail(&mut painter, 13.0, text, 40.0);
        assert!(out.ends_with('\u{2026}'));
        assert!(out.chars().count() < text.chars().count());
    }

    #[test]
    fn truncate_head_prepends_ellipsis_and_keeps_tail() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let out = truncate_head(&mut painter, 13.0, "src/view/geometry.rs", 60.0);
        assert!(out.starts_with('\u{2026}'));
        assert!(out.ends_with(".rs"));
    }

    #[test]
    fn layout_panel_width_clamps_between_min_and_max() {
        let sections: [Section; 0] = [];
        let spec = OverlaySpec {
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x66,
            },
            header: Header {
                glyph: None,
                text: "",
                placeholder: "",
                caret: Some(0),
                scope: None,
            },
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
        };
        let l = layout(&spec, 500, 800, 1.0);
        assert_eq!(l.panel.w, 300, "must clamp up to the logical-px minimum");

        let l2 = layout(&spec, 4000, 800, 1.0);
        assert_eq!(l2.panel.w, 500, "must clamp down to the logical-px maximum");
    }

    #[test]
    fn layout_panel_y_scales_with_scale_factor() {
        // dims::Y (64 logical px) must scale to physical px like every other
        // chrome constant — on a tall-enough window this is the value that
        // wins the `.min(window_height / 4)` clamp.
        let sections: [Section; 0] = [];
        let spec = OverlaySpec {
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x66,
            },
            header: Header {
                glyph: None,
                text: "",
                placeholder: "",
                caret: Some(0),
                scope: None,
            },
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
        };
        let l = layout(&spec, 2000, 4000, 2.0);
        assert_eq!(
            l.panel.y, 128,
            "64 logical px * 2.0 scale = 128 physical px"
        );
    }

    #[test]
    fn layout_scrollbar_only_appears_past_max_visible() {
        let rows: Vec<Row> = (0..20)
            .map(|_| Row {
                icon: RowIcon::None,
                label: "row",
                match_indices: &[],
                detail: None,
                accessory: Accessory::None,
            })
            .collect();
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x66,
            },
            header: Header {
                glyph: None,
                text: "",
                placeholder: "",
                caret: Some(0),
                scope: None,
            },
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 10,
            },
            footer: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert!(l.scrollbar.is_some());
        assert_eq!(l.rows.len(), 10);
    }

    #[test]
    fn resolve_visible_window_reanchors_past_section_headers() {
        // `scroll` is in FlatIndex (row-only) space, e.g. from
        // SelectableListViewport. A titled section ahead of the selected
        // row must not shift the window by one slot per header, or the
        // selected row scrolls off-screen (regression for the bug where
        // `layout()`/`render_list()` applied `scroll` directly as a
        // `display_rows` offset).
        let rows: Vec<Row> = (0..5)
            .map(|_| Row {
                icon: RowIcon::None,
                label: "row",
                match_indices: &[],
                detail: None,
                accessory: Accessory::None,
            })
            .collect();
        let sections = [Section {
            title: Some("Group"),
            rows: &rows,
        }];
        let display_rows = flatten_rows(&sections);
        // display_rows = [header, row0, row1, row2, row3, row4] (6 slots).
        assert_eq!(display_rows.len(), 6);

        // scroll = 3 (FlatIndex space) must land on display slot 4 (row3),
        // not display slot 3 (row2), because of the header ahead of it.
        let (start, visible) = resolve_visible_window(&display_rows, 3, 2);
        assert_eq!(visible, 2);
        assert!(matches!(
            display_rows[start],
            DisplayRow::Row(_, FlatIndex(3))
        ));
    }

    #[test]
    fn header_caret_tracks_column_not_text_end() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        // Regression: the caret used to always draw at
        // `x + measure(text)` regardless of the cursor's actual column, so
        // moving the caret left/Home never moved it on screen.
        let text = "abcd";
        let at_start = caret_x_for_column(&mut painter, 100, text, 0, SIZE_INPUT);
        let at_mid = caret_x_for_column(&mut painter, 100, text, 2, SIZE_INPUT);
        let at_end = caret_x_for_column(&mut painter, 100, text, 4, SIZE_INPUT);
        assert!(at_start < at_mid);
        assert!(at_mid < at_end);
        assert_eq!(at_start, 101, "column 0 caret sits right at the text start");
    }

    #[test]
    fn header_caret_renders_on_empty_input() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        // Regression: caret drawing lived in the `else` branch of
        // `header.text.is_empty()`, so an empty palette input drew no
        // caret at all.
        let caret_x = caret_x_for_column(&mut painter, 100, "", 0, SIZE_INPUT);
        assert_eq!(caret_x, 101);
    }

    #[test]
    fn binding_chips_splits_modifiers_and_key_into_separate_chips() {
        let steps = binding_chips("\u{21e7}\u{2318}T"); // ⇧⌘T
        assert_eq!(steps.len(), 1, "single keystroke is one chord step");
        let labels: Vec<&str> = steps[0].iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, vec!["\u{21e7}", "\u{2318}", "T"]);
    }

    #[test]
    fn binding_chips_keeps_multi_glyph_function_keys_as_one_chip() {
        let steps = binding_chips("F12");
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0].len(),
            1,
            "F12 is one chip regardless of glyph count"
        );
        assert_eq!(steps[0][0].label, "F12");
    }

    #[test]
    fn binding_chips_splits_chords_into_separate_steps() {
        let steps = binding_chips("\u{2318}K \u{2318}C"); // ⌘K ⌘C
        assert_eq!(steps.len(), 2, "space-separated chord has two steps");
        assert_eq!(steps[0].len(), 2); // ⌘, K
        assert_eq!(steps[1].len(), 2); // ⌘, C
    }

    #[test]
    fn chip_count_sums_across_chord_steps_for_the_dim_text_fallback() {
        // ⇧⌥⌘H: 3 modifiers + 1 key = 4 chips, at the fallback threshold.
        let steps = binding_chips("\u{21e7}\u{2325}\u{2318}H");
        assert_eq!(chip_count(&steps), 4);

        // A two-step chord where the first step alone has 3 chips crosses
        // the >4-chip fallback threshold (Visual Language > Keycaps: "more
        // than 4 chips total falls back to Accessory::DimText").
        let over_threshold = binding_chips("\u{21e7}\u{2318}K \u{2318}C");
        assert_eq!(chip_count(&over_threshold), 5);
        assert!(chip_count(&over_threshold) > 4);
    }
}
