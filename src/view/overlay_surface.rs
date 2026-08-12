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
    /// Top/bottom panel padding for `Fields`/`Zones` bodies, which have no
    /// header row to anchor against.
    pub const PANEL_PAD_Y: f32 = 12.0;
    /// Label row height in a `Fields` body (one line of `SIZE_INPUT` text).
    pub const FIELD_LABEL_H: f32 = 20.0;
    /// Gap between a field's label and its input box.
    pub const FIELD_LABEL_GAP: f32 = 4.0;
    /// Gap between successive fields in a `Fields` body.
    pub const FIELD_SPACING: f32 = 12.0;
}

/// The three-size type scale (input / rows / metadata), in logical px.
pub const SIZE_INPUT: f32 = 14.0;
pub const SIZE_ROW: f32 = 13.0;
pub const SIZE_META: f32 = 11.0;

/// Minimal-reveal scroll-window math for a flat list of selectable rows —
/// shared by every list-body modal context (command palette, file finder,
/// recent files, theme picker). Moved here from the now-deleted
/// `view::selectable_list` (Phase 3: that module's *rendering* helpers had
/// no consumers outside `modal.rs` once every context migrated to
/// `OverlaySurface`, but this scroll math is the one piece every context
/// still needs — it becomes the only path, per overlay-surface.md
/// Behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectableListViewport {
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub visible_count: usize,
    pub items_after: usize,
}

impl SelectableListViewport {
    /// Computes a viewport assuming no prior scroll position (window starts at
    /// the top). Kept for callers that don't track scroll state across
    /// renders; prefer [`Self::compute_from`] when a previous scroll offset is
    /// available, since it implements minimal-reveal scrolling instead of
    /// always pinning to an edge.
    #[cfg(test)]
    pub fn compute(total_items: usize, selected_index: usize, max_visible_items: usize) -> Self {
        Self::compute_from(total_items, selected_index, max_visible_items, 0)
    }

    /// Minimal-reveal scrolling: the visible window is only moved when
    /// `selected_index` falls outside `[previous_scroll_offset,
    /// previous_scroll_offset + max_visible_items)`. When it does, the window
    /// moves by the minimum amount needed to bring the selection back into
    /// view — scrolling up just enough if the selection moved above the
    /// window, or down just enough if it moved below — rather than
    /// unconditionally recomputing from scratch and pinning the selection to
    /// an edge.
    pub fn compute_from(
        total_items: usize,
        selected_index: usize,
        max_visible_items: usize,
        previous_scroll_offset: usize,
    ) -> Self {
        let selected_index = selected_index.min(total_items.saturating_sub(1));
        let visible_count = total_items.min(max_visible_items);

        let max_scroll_offset = total_items.saturating_sub(max_visible_items);
        let mut scroll_offset = previous_scroll_offset.min(max_scroll_offset);

        if selected_index < scroll_offset {
            // Selection moved above the visible window: scroll up just enough.
            scroll_offset = selected_index;
        } else if selected_index >= scroll_offset + max_visible_items {
            // Selection moved below the visible window: scroll down just enough.
            scroll_offset = selected_index + 1 - max_visible_items;
        }

        let items_after = total_items.saturating_sub(scroll_offset + max_visible_items);

        Self {
            selected_index,
            scroll_offset,
            visible_count,
            items_after,
        }
    }
}

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

/// One labeled input field in a `Body::Fields` context (Go to Line,
/// Find/Replace). The caller renders the actual editable text/selection via
/// `TextFieldRenderer` into the `WidgetRect` this layout produces —
/// `Field::text`/`caret` describe the label styling only (focused vs. dim).
pub struct Field<'a> {
    pub label: &'a str,
}

pub enum Body<'a> {
    List {
        sections: &'a [Section<'a>],
        selected: FlatIndex,
        scroll: usize,
        max_visible: usize,
    },
    /// Goto line, Find/Replace: labeled input fields. Field content is
    /// painted by the caller (`TextFieldRenderer`) into the geometry this
    /// layout produces (`OverlayLayout::fields`); this variant only owns the
    /// label text and which field is focused (bright vs. dim label).
    Fields {
        fields: &'a [Field<'a>],
        focused: usize,
    },
    /// Drop overlay: a single block of centered text, no list/fields.
    Zones { text: &'a str },
}

impl<'a> Body<'a> {
    /// Total selectable rows across all sections (headers excluded); 0 for
    /// non-list bodies.
    pub fn total_rows(&self) -> usize {
        match self {
            Body::List { sections, .. } => sections.iter().map(|s| s.rows.len()).sum(),
            Body::Fields { .. } | Body::Zones { .. } => 0,
        }
    }
}

pub struct OverlaySpec<'a> {
    pub anchor: Anchor,
    /// `None` for `Fields`/`Zones` contexts, which have no header row —
    /// input-as-header is a `Body::List` convention (Visual Language >
    /// Header/input).
    pub header: Option<Header<'a>>,
    pub body: Body<'a>,
    pub footer: Option<Footer<'a>>,
    /// The row currently under the mouse in a `Body::List` — hover wash, no
    /// text lift (Visual Language "Pointer"); distinct from `selected`,
    /// which is keyboard-authoritative. Ignored for other body kinds.
    pub hover_row: Option<FlatIndex>,
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

/// Geometry for one `Field` in a `Body::Fields` layout: the label row above
/// an input box. The caller paints actual field content (text, selection,
/// caret) into `input` via `TextFieldRenderer`.
#[derive(Clone, Copy, Debug)]
pub struct FieldLayout {
    pub label: WidgetRect,
    pub input: WidgetRect,
}

/// Computed geometry for an `Anchor::Centered` surface — panel chrome,
/// header, list rows (already scrolled to the visible window), fields,
/// footer, and scrollbar thumb. The single source of truth shared by
/// `render()` and by hit-testing (`hit_test::hit_test_modal`) — one layout,
/// two consumers.
pub struct OverlayLayout {
    pub panel: WidgetRect,
    /// `None` when `spec.header` is `None` (`Fields`/`Zones` bodies).
    pub header: Option<WidgetRect>,
    pub row_height: usize,
    /// One rect per visible display row (headers included), in list order.
    /// Empty for non-`List` bodies.
    pub rows: Vec<WidgetRect>,
    /// One entry per `Body::Fields` field, in order. Empty otherwise.
    pub fields: Vec<FieldLayout>,
    /// The single text block for a `Body::Zones` body. `None` otherwise.
    pub zones_text: Option<WidgetRect>,
    pub footer: Option<WidgetRect>,
    pub scrollbar: Option<WidgetRect>,
}

/// Layout an `Anchor::Centered` surface against the (physical-px) window
/// size. This is the one layout function the doc calls for — paint and
/// hit-testing both consume it.
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

    let header_h = spec
        .header
        .as_ref()
        .map(|_| scaled(SIZE_INPUT, scale_factor) + 2 * scaled(dims::PAD_Y, scale_factor));
    let row_h = scaled(dims::ROW_HEIGHT, scale_factor);
    let footer_h = spec
        .footer
        .as_ref()
        .map(|_| scaled(dims::FOOTER_HEIGHT, scale_factor));

    let panel_x_for = |panel_w: usize| window_width.saturating_sub(panel_w) / 2;
    let panel_y = scaled(dims::Y, scale_factor).min(window_height / 4);

    match &spec.body {
        Body::List {
            sections,
            scroll,
            max_visible,
            ..
        } => {
            let display_rows = flatten_rows(sections);
            let (start, visible) = resolve_visible_window(&display_rows, *scroll, *max_visible);
            let list_h = visible * row_h;
            let header_h = header_h.unwrap_or(0);

            let panel_h = header_h + list_h + footer_h.unwrap_or(0);
            let panel = WidgetRect {
                x: panel_x_for(panel_w),
                y: panel_y,
                w: panel_w,
                h: panel_h,
            };
            let header = spec.header.as_ref().map(|_| WidgetRect {
                x: panel.x,
                y: panel.y,
                w: panel.w,
                h: header_h,
            });

            let list_top = panel.y + header_h;
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
                fields: Vec::new(),
                zones_text: None,
                footer,
                scrollbar,
            }
        }
        Body::Fields { fields, .. } => {
            let label_h = scaled(dims::FIELD_LABEL_H, scale_factor);
            let gap = scaled(dims::FIELD_LABEL_GAP, scale_factor);
            let input_h = scaled(SIZE_INPUT, scale_factor) + 2 * scaled(dims::PAD_Y, scale_factor);
            let spacing = scaled(dims::FIELD_SPACING, scale_factor);
            let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
            let field_h = label_h + gap + input_h;

            let panel_h =
                pad_y * 2 + fields.len() * field_h + fields.len().saturating_sub(1) * spacing;
            let panel = WidgetRect {
                x: panel_x_for(panel_w),
                y: panel_y,
                w: panel_w,
                h: panel_h,
            };

            let mut y = panel.y + pad_y;
            let field_layouts: Vec<FieldLayout> = fields
                .iter()
                .map(|_| {
                    let label = WidgetRect {
                        x: panel.x,
                        y,
                        w: panel.w,
                        h: label_h,
                    };
                    let input = WidgetRect {
                        x: panel.x,
                        y: y + label_h + gap,
                        w: panel.w,
                        h: input_h,
                    };
                    y += field_h + spacing;
                    FieldLayout { label, input }
                })
                .collect();

            OverlayLayout {
                panel,
                header: None,
                row_height: row_h,
                rows: Vec::new(),
                fields: field_layouts,
                zones_text: None,
                footer: None,
                scrollbar: None,
            }
        }
        Body::Zones { .. } => {
            let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
            let text_h = scaled(SIZE_INPUT, scale_factor);
            let panel_h = pad_y * 2 + text_h;
            let panel = WidgetRect {
                x: panel_x_for(panel_w),
                y: panel_y,
                w: panel_w,
                h: panel_h,
            };
            let zones_text = Some(WidgetRect {
                x: panel.x,
                y: panel.y + pad_y,
                w: panel.w,
                h: text_h,
            });

            OverlayLayout {
                panel,
                header: None,
                row_height: row_h,
                rows: Vec::new(),
                fields: Vec::new(),
                zones_text,
                footer: None,
                scrollbar: None,
            }
        }
    }
}

/// Where a point landed within a rendered `OverlaySpec`/`OverlayLayout` —
/// consumed by `hit_test::hit_test_modal`, which builds the exact same spec
/// the renderer draws and calls `layout()` + this function against it (one
/// layout, two consumers, so a click is always tested against the geometry
/// actually painted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayHit {
    /// Outside the panel entirely (dismiss on click).
    Outside,
    /// A selectable row (`Body::List` only).
    Row(FlatIndex),
    /// Inside the panel but not on a specific row (header, footer, section
    /// header, padding, a `Fields`/`Zones` body) — consumed, no action.
    Inside,
}

/// Hit-test a point (physical px) against a laid-out `OverlaySpec`.
pub fn hit_test(spec: &OverlaySpec, layout: &OverlayLayout, x: usize, y: usize) -> OverlayHit {
    let inside_panel = x >= layout.panel.x
        && x < layout.panel.x + layout.panel.w
        && y >= layout.panel.y
        && y < layout.panel.y + layout.panel.h;
    if !inside_panel {
        return OverlayHit::Outside;
    }

    if let Body::List {
        sections,
        scroll,
        max_visible,
        ..
    } = &spec.body
    {
        let display_rows = flatten_rows(sections);
        let (start, _) = resolve_visible_window(&display_rows, *scroll, *max_visible);
        for (slot, rect) in layout.rows.iter().enumerate() {
            let hit = x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h;
            if !hit {
                continue;
            }
            return match display_rows.get(start + slot) {
                Some(DisplayRow::Row(_, flat_index)) => OverlayHit::Row(*flat_index),
                _ => OverlayHit::Inside,
            };
        }
    }

    OverlayHit::Inside
}

/// Resolved colors pulled once from `OverlayTheme` per render call.
struct Palette {
    panel_bg: u32,
    hairline: u32,
    text_primary: u32,
    text_bright: u32,
    text_dim: u32,
    accent: u32,
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
            accent: theme.accent.to_argb_u32(),
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

    if let Some(header) = &spec.header {
        render_header(
            frame,
            painter,
            &colors,
            header,
            &layout,
            scale_factor,
            cursor_visible,
        );
    }
    match &spec.body {
        Body::List { .. } => render_list(
            frame,
            painter,
            mask_cache,
            &colors,
            spec,
            &layout,
            scale_factor,
        ),
        Body::Fields { fields, focused } => render_fields(
            frame,
            painter,
            &colors,
            fields,
            *focused,
            &layout,
            scale_factor,
        ),
        Body::Zones { text } => {
            render_zones_text(frame, painter, &colors, text, &layout, scale_factor)
        }
    }
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
    let Some(r) = layout.header else { return };

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
    } = &spec.body
    else {
        return;
    };
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
                } else if spec.hover_row == Some(*flat_index) {
                    // Hover is not selection — 12% wash, no text lift
                    // (Visual Language > Pointer).
                    let hover_wash = (0x1F << 24) | (colors.accent & 0x00FF_FFFF);
                    frame.blend_rect_px(
                        rect.x + inset,
                        rect.y,
                        rect.w.saturating_sub(inset * 2),
                        rect.h,
                        hover_wash,
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

/// Draw the field labels for a `Body::Fields` context (Go to Line,
/// Find/Replace). Field content (text, selection, caret) is painted by the
/// caller via `TextFieldRenderer` into `layout.fields[i].input` — this only
/// draws the label above it, bright when focused, dim otherwise.
fn render_fields(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    fields: &[Field],
    focused: usize,
    layout: &OverlayLayout,
    scale_factor: f64,
) {
    let size = size_px(SIZE_INPUT, scale_factor);
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
    for (i, field) in fields.iter().enumerate() {
        let Some(field_layout) = layout.fields.get(i) else {
            continue;
        };
        let color = if i == focused {
            colors.text_bright
        } else {
            colors.text_dim
        };
        let r = field_layout.label;
        let text_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(size))) / 2;
        painter.draw_sized(frame, r.x + pad_x, text_y, field.label, size, 0.0, color);
    }
}

/// Draw the single centered text block of a `Body::Zones` context (drop
/// overlay).
fn render_zones_text(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    text: &str,
    layout: &OverlayLayout,
    scale_factor: f64,
) {
    let Some(r) = layout.zones_text else { return };
    let size = size_px(SIZE_INPUT, scale_factor);
    let text_w = painter.measure_sized(text, size, 0.0).ceil() as usize;
    let text_x = r.x + (r.w.saturating_sub(text_w)) / 2;
    let text_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(size))) / 2;
    painter.draw_sized(frame, text_x, text_y, text, size, 0.0, colors.text_primary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_clamps_selection_without_scroll() {
        let viewport = SelectableListViewport::compute(3, 10, 8);
        assert_eq!(viewport.selected_index, 2);
        assert_eq!(viewport.scroll_offset, 0);
        assert_eq!(viewport.visible_count, 3);
        assert_eq!(viewport.items_after, 0);
    }

    #[test]
    fn viewport_scrolls_to_keep_selection_visible() {
        let viewport = SelectableListViewport::compute(15, 12, 8);
        assert_eq!(viewport.selected_index, 12);
        assert_eq!(viewport.scroll_offset, 5);
        assert_eq!(viewport.visible_count, 8);
        assert_eq!(viewport.items_after, 2);
    }

    /// M12 regression: moving the selection down one row at a time should
    /// only nudge the scroll offset by exactly one row at a time, once the
    /// selection actually leaves the visible window — never jump/pin
    /// unconditionally.
    #[test]
    fn compute_from_scrolls_down_minimally_one_row_at_a_time() {
        let total = 20;
        let max_visible = 8;
        let mut offset = 0usize;
        let mut changes = 0;

        for selected in 0..total {
            let viewport =
                SelectableListViewport::compute_from(total, selected, max_visible, offset);
            if viewport.scroll_offset != offset {
                changes += 1;
                assert_eq!(
                    viewport.scroll_offset,
                    offset + 1,
                    "scroll offset should move by exactly one row when the selection \
                     leaves the window from below"
                );
            }
            offset = viewport.scroll_offset;
        }

        // Once selection reaches the last item, the window should be pinned
        // just enough to show it (20 - 8 = 12), and it should only have
        // scrolled once per row past the initial page.
        assert_eq!(offset, total - max_visible);
        assert_eq!(changes, total - max_visible);
    }

    /// M12 regression: after scrolling down to the bottom, moving the
    /// selection back up should also only move the window by the minimum
    /// amount needed — and once it settles back within a stable window,
    /// further moves within that window must not change scroll_offset at all.
    #[test]
    fn compute_from_scrolls_up_minimally_and_holds_steady_within_window() {
        let total = 20;
        let max_visible = 8;

        // Start from the bottom-pinned window (as if the user had scrolled
        // all the way down previously).
        let bottom = SelectableListViewport::compute_from(total, total - 1, max_visible, 0);
        assert_eq!(bottom.scroll_offset, 12);

        // Move the selection up one row at a time and ensure the offset only
        // decreases by exactly one row at a time, right when selection
        // leaves the window from above.
        let mut offset = bottom.scroll_offset;
        let mut changes = 0;
        for selected in (0..total).rev() {
            let viewport =
                SelectableListViewport::compute_from(total, selected, max_visible, offset);
            if viewport.scroll_offset != offset {
                changes += 1;
                assert_eq!(
                    viewport.scroll_offset,
                    offset - 1,
                    "scroll offset should move by exactly one row when the selection \
                     leaves the window from above"
                );
            }
            offset = viewport.scroll_offset;
        }
        assert_eq!(offset, 0);
        assert_eq!(changes, total - max_visible);

        // Within a stable window, moving selection but staying inside the
        // visible range must not touch scroll_offset at all.
        let steady = SelectableListViewport::compute_from(total, 12, max_visible, 10);
        assert_eq!(steady.scroll_offset, 10, "selection stays within window");
    }

    /// M12 regression: this is the exact bug scenario. After the window has
    /// scrolled down to the bottom, jumping the selection directly to a row
    /// far above the window must scroll up by only the minimum amount needed
    /// (so the selection lands at the top edge of the window), not reset the
    /// window all the way back to the start the way the old
    /// "recompute from scratch and pin to an edge" logic did.
    #[test]
    fn compute_from_jump_above_window_scrolls_minimally_not_to_start() {
        let total = 20;
        let max_visible = 8;

        // Window pinned at the bottom: [12, 20).
        let previous_offset = 12;

        // Jump the selection up to row 5, which is above the window but far
        // from the very top of the list.
        let viewport = SelectableListViewport::compute_from(total, 5, max_visible, previous_offset);

        // Minimal reveal: offset should move to exactly the selected row so
        // that it sits at the top edge of the new window, not jump to 0.
        assert_eq!(viewport.scroll_offset, 5);
        assert_ne!(
            viewport.scroll_offset, 0,
            "must not unconditionally pin to the start of the list"
        );
    }

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
            header: Some(Header {
                glyph: None,
                text: "",
                placeholder: "",
                caret: Some(0),
                scope: None,
            }),
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
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
            header: Some(Header {
                glyph: None,
                text: "",
                placeholder: "",
                caret: Some(0),
                scope: None,
            }),
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
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
            header: Some(Header {
                glyph: None,
                text: "",
                placeholder: "",
                caret: Some(0),
                scope: None,
            }),
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 10,
            },
            footer: None,
            hover_row: None,
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

    fn list_spec<'a>(sections: &'a [Section<'a>]) -> OverlaySpec<'a> {
        OverlaySpec {
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x66,
            },
            header: Some(Header {
                glyph: None,
                text: "",
                placeholder: "",
                caret: Some(0),
                scope: None,
            }),
            body: Body::List {
                sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 10,
            },
            footer: None,
            hover_row: None,
        }
    }

    #[test]
    fn hit_test_outside_panel_returns_outside() {
        let rows: Vec<Row> = (0..3)
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
        let spec = list_spec(&sections);
        let l = layout(&spec, 1000, 800, 1.0);
        assert_eq!(hit_test(&spec, &l, 0, 0), OverlayHit::Outside);
    }

    #[test]
    fn hit_test_row_returns_its_flat_index() {
        let rows: Vec<Row> = (0..3)
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
        let spec = list_spec(&sections);
        let l = layout(&spec, 1000, 800, 1.0);
        let second_row = l.rows[1];
        let x = second_row.x + 1;
        let y = second_row.y + 1;
        assert_eq!(hit_test(&spec, &l, x, y), OverlayHit::Row(FlatIndex(1)));
    }

    #[test]
    fn hit_test_section_header_returns_inside_not_a_row() {
        let rows: Vec<Row> = (0..2)
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
        let spec = list_spec(&sections);
        let l = layout(&spec, 1000, 800, 1.0);
        // Slot 0 is the section header, not a selectable row.
        let header_row = l.rows[0];
        let x = header_row.x + 1;
        let y = header_row.y + 1;
        assert_eq!(hit_test(&spec, &l, x, y), OverlayHit::Inside);
    }

    #[test]
    fn layout_fields_stacks_label_then_input_per_field() {
        let fields = [Field { label: "Find:" }, Field { label: "Replace:" }];
        let spec = OverlaySpec {
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x66,
            },
            header: None,
            body: Body::Fields {
                fields: &fields,
                focused: 0,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert!(l.header.is_none());
        assert_eq!(l.fields.len(), 2);
        // Each field's input sits below its own label...
        assert!(l.fields[0].input.y > l.fields[0].label.y);
        // ...and the second field sits fully below the first.
        assert!(l.fields[1].label.y >= l.fields[0].input.y + l.fields[0].input.h);
    }

    #[test]
    fn layout_zones_sizes_panel_around_a_single_text_block() {
        let spec = OverlaySpec {
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x80,
            },
            header: None,
            body: Body::Zones {
                text: "Drop to open: file.rs",
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert!(l.zones_text.is_some());
        assert!(l.fields.is_empty());
        assert!(l.rows.is_empty());
        let text_rect = l.zones_text.unwrap();
        assert!(text_rect.w > 0 && text_rect.h > 0);
        assert!(l.panel.h >= text_rect.h);
    }
}
