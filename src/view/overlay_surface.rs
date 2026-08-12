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
    /// `Anchor::Cursor` chrome radius (Visual Language > Chrome).
    pub const RADIUS_CURSOR: f32 = 8.0;
    pub const HEADER_PAD_X: f32 = 16.0;
    pub const PAD_Y: f32 = 12.0;
    pub const ROW_HEIGHT: f32 = 30.0;
    pub const ROW_INSET: f32 = 6.0;
    pub const ROW_RADIUS: f32 = 6.0;
    /// Completion row height/pad/radius (Visual Language > Rows: "Completion").
    pub const ROW_HEIGHT_CURSOR: f32 = 24.0;
    pub const ROW_INSET_CURSOR: f32 = 4.0;
    pub const ROW_RADIUS_CURSOR: f32 = 5.0;
    pub const ROW_ICON_W: f32 = 18.0;
    pub const ROW_TEXT_PAD_X: f32 = 8.0;
    pub const FOOTER_HEIGHT: f32 = 30.0;
    pub const SCROLLBAR_WIDTH: f32 = 3.0;
    pub const SCROLLBAR_INSET: f32 = 2.0;
    pub const SCROLLBAR_MIN_LEN: f32 = 20.0;
    pub const Y: f32 = 64.0;
    /// Gap between the anchor line and a cursor-anchored popup (Visual
    /// Language > Chrome: "below the anchor line + 2").
    pub const CURSOR_GAP: f32 = 2.0;
    /// `Anchor::Cursor` minimum panel width (Visual Language > Chrome:
    /// "content-sized, clamped to window edges, 200px floor").
    pub const CURSOR_WIDTH_FLOOR: f32 = 200.0;
    /// Completion kind badge (Visual Language > Rows: "kind badge (16×16, r4)").
    pub const KIND_BADGE_SIZE: f32 = 16.0;
    pub const KIND_BADGE_RADIUS: f32 = 4.0;
    /// Hover-card zone geometry (Zones body).
    pub const ZONE_BANNER_H: f32 = 28.0;
    pub const ZONE_GAP: f32 = 8.0;
    /// Gap between chips within one chord step's keycap accessory.
    pub const CHIP_GAP: f32 = 4.0;
    /// Theme-swatch dots (theme picker): diameter, intra-strip gap, and the
    /// gap between the strip and the ✓ active mark.
    pub const SWATCH_D: f32 = 7.0;
    pub const SWATCH_GAP: f32 = 3.0;
    pub const SWATCH_CHECK_GAP: f32 = 6.0;
    /// Gap between chord steps in a keycap accessory (Visual Language >
    /// Keycaps: "6px gap between steps").
    pub const CHIP_STEP_GAP: f32 = 6.0;
    /// Top/bottom panel padding for `Fields`/`Zones` bodies, which have no
    /// header row to anchor against.
    pub const PANEL_PAD_Y: f32 = 12.0;
    /// Tab bar region height (Search Everywhere only — overlay-surface.md
    /// Regions: "TabBar (optional, 32h)").
    pub const TAB_BAR_HEIGHT: f32 = 32.0;
    pub const TAB_PAD_X: f32 = 12.0;
    pub const TAB_UNDERLINE_H: f32 = 2.0;
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
    /// Anchored to a pixel point (physical px — the text caret rect from
    /// `view::caret::active_text_input_rect`). `(x, y)` is the caret's
    /// top-left corner and `h` its line height, so flipping above can clear
    /// the caret's own line instead of just its bottom edge. Flips above
    /// the anchor line when there isn't `panel_h` of space below; clamps to
    /// the window edges; no backdrop dim (Visual Language > Chrome).
    Cursor {
        x: usize,
        y: usize,
        h: usize,
        prefer_below: bool,
        width: WidthRule,
    },
}

impl Anchor {
    fn width(&self) -> &WidthRule {
        match self {
            Anchor::Centered { width, .. } | Anchor::Cursor { width, .. } => width,
        }
    }
}

/// A Search Everywhere tab's match-count state (overlay-surface.md Search
/// Everywhere tabs table): rendered as `""` | `"142"` | animated `"···"` |
/// `"—"`. `Unavailable` also dims the label, is unclickable, and is
/// skipped by ⇥.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabCount {
    Hidden,
    N(usize),
    Pending,
    Unavailable,
}

impl TabCount {
    pub fn label(self) -> String {
        match self {
            TabCount::Hidden => String::new(),
            TabCount::N(n) => n.to_string(),
            TabCount::Pending => "\u{22ef}".to_string(), // ⋯
            TabCount::Unavailable => "\u{2014}".to_string(), // —
        }
    }

    pub fn is_available(self) -> bool {
        !matches!(self, TabCount::Unavailable)
    }
}

pub struct TabBar<'a> {
    pub tabs: &'a [(&'a str, TabCount)],
    pub active: usize,
}

pub struct Header<'a> {
    pub glyph: Option<char>,
    pub text: &'a str,
    pub placeholder: &'a str,
    /// Char index of the caret; `None` means a display-only header (no
    /// input, e.g. a future title-only context).
    pub caret: Option<usize>,
    /// Selected char range `(start, end)` in the full text's char space,
    /// ordered, end-exclusive; drawn as a wash behind the text.
    pub selection: Option<(usize, usize)>,
    /// Right-aligned dim text, e.g. `"workspace: token"`.
    pub scope: Option<&'a str>,
}

pub enum RowIcon {
    None,
    Glyph {
        ch: char,
        color: u32,
    },
    /// Completion row icon: a 16×16, r4 badge colored by `CompletionKind`
    /// (Visual Language > Rows: "Completion").
    KindBadge(CompletionKind),
}

/// LSP completion-item kind, coarsened to the badge groups
/// overlay-surface.md's `overlay.kind_*` color table describes. Colors are
/// derived from the existing syntax theme (`Theme::syntax`) rather than new
/// persisted `overlay.kind_*` YAML keys — the doc's "per-kind from syntax
/// colors" fallback rule, without adding nine themes' worth of literal
/// tuning for a shell with no live producer yet.
/// ponytail: new `overlay.kind_*` theme keys with per-theme hand-tuning are
/// the fuller version; add them when autocomplete.md's Phase 1 ships a real
/// completion source and the badge colors need bundled-theme polish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Function,
    Variable,
    Type,
    Keyword,
    Field,
    Module,
    Constant,
    Other,
}

impl CompletionKind {
    /// Single-glyph badge label.
    pub fn glyph(self) -> char {
        match self {
            CompletionKind::Function => 'f',
            CompletionKind::Variable => 'v',
            CompletionKind::Type => 't',
            CompletionKind::Keyword => 'k',
            CompletionKind::Field => '.',
            CompletionKind::Module => 'm',
            CompletionKind::Constant => 'c',
            CompletionKind::Other => '?',
        }
    }
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
    /// A strip of small color dots (theme picker palette preview), with the
    /// ✓ active mark appended when `active` — one accessory slot, so the
    /// check rides along instead of competing for it.
    Swatches {
        colors: &'a [u32],
        active: bool,
    },
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

/// Completion popup scroll cap (Visual Language > Rows: "Overflow ... Max-
/// visible caps: ... completion 8").
pub const MAX_VISIBLE_COMPLETION: usize = 8;

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
    /// Drop overlay (Centered) / hover card (Cursor): stacked content
    /// zones, no list/fields.
    Zones(Zones<'a>),
}

/// LSP severity level, shared by the hover banner, gutter marks, and status
/// bar (overlay-surface.md Colors: "All four LSP severity levels").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    pub fn glyph(self) -> char {
        match self {
            Severity::Error => '\u{2717}',   // ✗
            Severity::Warning => '\u{26A0}', // ⚠
            Severity::Info => '\u{2139}',    // ℹ
            Severity::Hint => '\u{25CF}',    // ●
        }
    }
}

/// Content zones, top to bottom, for a `Body::Zones` context — each
/// optional. Used by the drop overlay (`text` only) and the hover card
/// (`banner`/`code`/`text`, per lsp-integration.md).
#[derive(Default)]
pub struct Zones<'a> {
    /// Severity, message, source (e.g. `(Error, "unused import", "rustc")`).
    pub banner: Option<(Severity, &'a str, &'a str)>,
    /// Signature block, rendered on `panel_secondary`.
    pub code: Option<&'a str>,
    pub text: Option<&'a str>,
}

impl<'a> Body<'a> {
    /// Total selectable rows across all sections (headers excluded); 0 for
    /// non-list bodies.
    pub fn total_rows(&self) -> usize {
        match self {
            Body::List { sections, .. } => sections.iter().map(|s| s.rows.len()).sum(),
            Body::Fields { .. } | Body::Zones(_) => 0,
        }
    }
}

pub struct OverlaySpec<'a> {
    pub anchor: Anchor,
    /// Search Everywhere only — `None` for every other context.
    pub tabs: Option<TabBar<'a>>,
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

/// Row-count shape of one section, without the borrowed `Row` data —
/// enough for `resolve_scroll_for_selection` to know how many display
/// slots a section's header consumes, mirroring [`Section`] for callers
/// (`update::ui`'s selection movement) that only need counts.
#[derive(Clone, Copy)]
pub struct SectionShape {
    pub has_title: bool,
    pub len: usize,
}

/// Header-aware equivalent of [`SelectableListViewport::compute_from`]:
/// minimal-reveal scrolling computed in *display-row* space (accounting
/// for section header slots) but expressed, like every `scroll` field, as
/// a `FlatIndex`-space row offset — so `update::ui`'s list-movement
/// helpers and [`resolve_visible_window`] (used by `layout`/`render`)
/// agree on what's visible even when sections add headers to the window.
/// With a single untitled section this reduces to exactly
/// `compute_from`'s result.
pub fn resolve_scroll_for_selection(
    shapes: &[SectionShape],
    selected: usize,
    max_visible: usize,
    previous_scroll: usize,
) -> usize {
    let mut display_len = 0usize;
    let mut flat_to_display = Vec::new();
    for shape in shapes {
        if shape.has_title {
            display_len += 1;
        }
        for _ in 0..shape.len {
            flat_to_display.push(display_len);
            display_len += 1;
        }
    }
    if flat_to_display.is_empty() {
        return 0;
    }
    let last_flat = flat_to_display.len() - 1;
    let visible = display_len.min(max_visible);

    let selected = selected.min(last_flat);
    let selected_display = flat_to_display[selected];

    let prev_display = flat_to_display[previous_scroll.min(last_flat)];
    let start = prev_display.min(display_len.saturating_sub(visible));

    let target_display = if selected_display < start {
        selected_display
    } else if selected_display >= start + visible {
        selected_display + 1 - visible
    } else {
        start
    };

    flat_to_display
        .iter()
        .position(|&d| d >= target_display)
        .unwrap_or(last_flat)
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

/// The list-context header's query text, clipped to `max_width` — full text
/// when it fits, otherwise head-ellipsized so the *tail* (nearest the caret,
/// which sits at/after the end while typing) stays visible instead of
/// running off the panel. Returns `(visible, kept_from)`, `kept_from` being
/// the char index into the original `text` where the kept tail begins (0
/// when untruncated) — the caller uses it to re-express the caret's column
/// against the possibly-shorter visible string.
fn visible_header_text(
    painter: &mut TextPainter,
    size: f32,
    text: &str,
    max_width: f32,
) -> (String, usize) {
    if painter.measure_sized(text, size, 0.0) <= max_width {
        return (text.to_string(), 0);
    }
    let visible = truncate_head(painter, size, text, max_width);
    let total_chars = text.chars().count();
    let kept_chars = visible.chars().count().saturating_sub(1); // minus the ellipsis
    (visible, total_chars.saturating_sub(kept_chars))
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
    /// `None` unless `spec.tabs` is `Some` (Search Everywhere only).
    pub tab_bar: Option<WidgetRect>,
    /// One rect per tab, in `spec.tabs` order. Empty unless `spec.tabs` is
    /// `Some`.
    pub tab_rects: Vec<WidgetRect>,
    /// `None` when `spec.header` is `None` (`Fields`/`Zones` bodies).
    pub header: Option<WidgetRect>,
    pub row_height: usize,
    /// One rect per visible display row (headers included), in list order.
    /// Empty for non-`List` bodies.
    pub rows: Vec<WidgetRect>,
    /// One entry per `Body::Fields` field, in order. Empty otherwise.
    pub fields: Vec<FieldLayout>,
    /// The banner zone of a `Body::Zones` body. `None` unless
    /// `spec.body`'s `Zones::banner` is `Some`.
    pub zones_banner: Option<WidgetRect>,
    /// The code zone of a `Body::Zones` body.
    pub zones_code: Option<WidgetRect>,
    /// The text zone of a `Body::Zones` body.
    pub zones_text: Option<WidgetRect>,
    pub footer: Option<WidgetRect>,
    pub scrollbar: Option<WidgetRect>,
}

/// Clamp a `WidthRule` against the window width (percent-of-window, clamped
/// to a logical-px min/max, then margin-clamped to the window edges); for
/// `Anchor::Cursor` an additional 200px floor applies (Visual Language >
/// Chrome).
fn resolve_panel_width(
    anchor: &Anchor,
    width: &WidthRule,
    window_width: usize,
    scale_factor: f64,
) -> usize {
    let margin = scaled(32.0, scale_factor);
    let min_w = size_px(width.min, scale_factor) as usize;
    let max_w = size_px(width.max, scale_factor) as usize;
    let mut panel_w = ((window_width as f32 * width.pct) as usize)
        .clamp(min_w, max_w)
        .min(window_width.saturating_sub(margin));
    if matches!(anchor, Anchor::Cursor { .. }) {
        panel_w = panel_w.max(scaled(dims::CURSOR_WIDTH_FLOOR, scale_factor));
    }
    // Never exceed the window itself, even when the 200px cursor floor is
    // wider than `window_width - margin` (narrow-window degradation).
    panel_w.min(window_width)
}

/// Position the panel's top-left corner given its final width/height.
/// Centered: centered X, `min(h/4, Y)` per the Chrome table. Cursor: below
/// the anchor line + gap, flipping above when there isn't `panel_h` of
/// space below, then edge-clamped to the window.
fn position_panel(
    anchor: &Anchor,
    window_width: usize,
    window_height: usize,
    panel_w: usize,
    panel_h: usize,
    scale_factor: f64,
) -> (usize, usize) {
    match anchor {
        Anchor::Centered { .. } => {
            let x = window_width.saturating_sub(panel_w) / 2;
            let y = scaled(dims::Y, scale_factor).min(window_height / 4);
            (x, y)
        }
        Anchor::Cursor {
            x,
            y,
            h,
            prefer_below,
            ..
        } => {
            let gap = scaled(dims::CURSOR_GAP, scale_factor);
            let px = (*x).min(window_width.saturating_sub(panel_w));
            let below_y = y.saturating_add(*h).saturating_add(gap);
            let fits_below = below_y.saturating_add(panel_h) <= window_height;
            let fits_above = *y >= panel_h.saturating_add(gap);
            let py = if *prefer_below && fits_below {
                below_y
            } else if fits_above {
                y - gap - panel_h
            } else if fits_below {
                below_y
            } else {
                // Neither direction has room: clamp to the window.
                window_height.saturating_sub(panel_h)
            };
            (px, py)
        }
    }
}

/// Layout an `OverlaySpec` against the (physical-px) window size. This is
/// the one layout function the doc calls for — paint and hit-testing both
/// consume it.
pub fn layout(
    spec: &OverlaySpec,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
) -> OverlayLayout {
    let panel_w = resolve_panel_width(
        &spec.anchor,
        spec.anchor.width(),
        window_width,
        scale_factor,
    );
    let is_cursor = matches!(spec.anchor, Anchor::Cursor { .. });

    let header_h = spec
        .header
        .as_ref()
        .map(|_| scaled(SIZE_INPUT, scale_factor) + 2 * scaled(dims::PAD_Y, scale_factor));
    let row_h = scaled(
        if is_cursor {
            dims::ROW_HEIGHT_CURSOR
        } else {
            dims::ROW_HEIGHT
        },
        scale_factor,
    );
    let footer_h = spec
        .footer
        .as_ref()
        .map(|_| scaled(dims::FOOTER_HEIGHT, scale_factor));
    let tab_bar_h = spec
        .tabs
        .as_ref()
        .map(|_| scaled(dims::TAB_BAR_HEIGHT, scale_factor));

    match &spec.body {
        Body::List {
            sections,
            scroll,
            max_visible,
            ..
        } => {
            let display_rows = flatten_rows(sections);
            let (start, visible) = resolve_visible_window(&display_rows, *scroll, *max_visible);
            // Reserve one row of body height even when there are zero rows,
            // so an empty-state message ("No files match your query") has
            // somewhere to paint instead of landing in the footer band.
            let visible = visible.max(usize::from(display_rows.is_empty()));
            let list_h = visible * row_h;
            let header_h = header_h.unwrap_or(0);
            let tab_bar_h_v = tab_bar_h.unwrap_or(0);

            let panel_h = tab_bar_h_v + header_h + list_h + footer_h.unwrap_or(0);
            let (panel_x, panel_y) = position_panel(
                &spec.anchor,
                window_width,
                window_height,
                panel_w,
                panel_h,
                scale_factor,
            );
            let panel = WidgetRect {
                x: panel_x,
                y: panel_y,
                w: panel_w,
                h: panel_h,
            };
            let tab_bar = tab_bar_h.map(|h| WidgetRect {
                x: panel.x,
                y: panel.y,
                w: panel.w,
                h,
            });
            let tab_rects: Vec<WidgetRect> = spec
                .tabs
                .as_ref()
                .map(|tab_bar_spec| {
                    let n = tab_bar_spec.tabs.len().max(1);
                    let tab_w = panel.w / n;
                    (0..tab_bar_spec.tabs.len())
                        .map(|i| WidgetRect {
                            x: panel.x + i * tab_w,
                            y: panel.y,
                            w: tab_w,
                            h: tab_bar_h_v,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let header = spec.header.as_ref().map(|_| WidgetRect {
                x: panel.x,
                y: panel.y + tab_bar_h_v,
                w: panel.w,
                h: header_h,
            });

            let list_top = panel.y + tab_bar_h_v + header_h;
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
                tab_bar,
                tab_rects,
                header,
                row_height: row_h,
                rows,
                fields: Vec::new(),
                zones_banner: None,
                zones_code: None,
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
            let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
            let field_h = label_h + gap + input_h;

            let panel_h = pad_y * 2
                + fields.len() * field_h
                + fields.len().saturating_sub(1) * spacing
                + footer_h.unwrap_or(0);
            let (panel_x, panel_y) = position_panel(
                &spec.anchor,
                window_width,
                window_height,
                panel_w,
                panel_h,
                scale_factor,
            );
            let panel = WidgetRect {
                x: panel_x,
                y: panel_y,
                w: panel_w,
                h: panel_h,
            };
            let field_x = panel.x + pad_x;
            let field_w = panel.w.saturating_sub(2 * pad_x);

            let mut y = panel.y + pad_y;
            let field_layouts: Vec<FieldLayout> = fields
                .iter()
                .map(|_| {
                    let label = WidgetRect {
                        x: field_x,
                        y,
                        w: field_w,
                        h: label_h,
                    };
                    let input = WidgetRect {
                        x: field_x,
                        y: y + label_h + gap,
                        w: field_w,
                        h: input_h,
                    };
                    y += field_h + spacing;
                    FieldLayout { label, input }
                })
                .collect();

            let footer = footer_h.map(|h| WidgetRect {
                x: panel.x,
                y: panel.y + panel_h - h,
                w: panel.w,
                h,
            });

            OverlayLayout {
                panel,
                tab_bar: None,
                tab_rects: Vec::new(),
                header: None,
                row_height: row_h,
                rows: Vec::new(),
                fields: field_layouts,
                zones_banner: None,
                zones_code: None,
                zones_text: None,
                footer,
                scrollbar: None,
            }
        }
        Body::Zones(zones) => {
            let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
            let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
            let gap = scaled(dims::ZONE_GAP, scale_factor);
            let line_h = scaled(SIZE_ROW, scale_factor);
            let banner_h = scaled(dims::ZONE_BANNER_H, scale_factor);
            let code_h = zones
                .code
                .map(|s| s.lines().count().max(1) * line_h + 2 * (gap / 2));
            let text_h = zones.text.map(|s| s.lines().count().max(1) * line_h);

            let mut panel_h = pad_y * 2;
            let mut y_off = 0;
            if zones.banner.is_some() {
                panel_h += banner_h;
                y_off += banner_h;
            }
            if let Some(h) = code_h {
                if y_off > 0 {
                    panel_h += gap;
                }
                panel_h += h;
            }
            if let Some(h) = text_h {
                if y_off > 0 || code_h.is_some() {
                    panel_h += gap;
                }
                panel_h += h;
            }

            let (panel_x, panel_y) = position_panel(
                &spec.anchor,
                window_width,
                window_height,
                panel_w,
                panel_h,
                scale_factor,
            );
            let panel = WidgetRect {
                x: panel_x,
                y: panel_y,
                w: panel_w,
                h: panel_h,
            };

            let mut cursor_y = panel.y;
            let zones_banner = zones.banner.map(|_| {
                let r = WidgetRect {
                    x: panel.x,
                    y: cursor_y,
                    w: panel.w,
                    h: banner_h,
                };
                cursor_y += banner_h;
                r
            });
            if zones_banner.is_none() {
                cursor_y += pad_y;
            }
            let zones_code = zones.code.zip(code_h).map(|(_, h)| {
                if zones_banner.is_some() {
                    cursor_y += gap;
                }
                let r = WidgetRect {
                    x: panel.x + pad_x,
                    y: cursor_y,
                    w: panel.w.saturating_sub(2 * pad_x),
                    h,
                };
                cursor_y += h;
                r
            });
            let zones_text = zones.text.zip(text_h).map(|(_, h)| {
                if zones_banner.is_some() || zones_code.is_some() {
                    cursor_y += gap;
                }
                WidgetRect {
                    x: panel.x + pad_x,
                    y: cursor_y,
                    w: panel.w.saturating_sub(2 * pad_x),
                    h,
                }
            });

            OverlayLayout {
                panel,
                tab_bar: None,
                tab_rects: Vec::new(),
                header: None,
                row_height: row_h,
                rows: Vec::new(),
                fields: Vec::new(),
                zones_banner,
                zones_code,
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
    /// An available tab (`Unavailable` tabs are unclickable — the click
    /// lands as `Inside` instead, per Visual Language > TabCount states).
    Tab(usize),
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

    if let Some(tab_bar) = &spec.tabs {
        for (i, rect) in layout.tab_rects.iter().enumerate() {
            let hit = x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h;
            if !hit {
                continue;
            }
            let available = tab_bar
                .tabs
                .get(i)
                .map(|(_, c)| c.is_available())
                .unwrap_or(false);
            return if available {
                OverlayHit::Tab(i)
            } else {
                OverlayHit::Inside
            };
        }
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
    panel_secondary: u32,
    severity_error: u32,
    severity_warning: u32,
    severity_info: u32,
    severity_hint: u32,
    severity_error_text: u32,
    severity_warning_text: u32,
    severity_info_text: u32,
    severity_hint_text: u32,
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
            panel_secondary: theme.panel_secondary.to_argb_u32(),
            severity_error: theme.severity_error.to_argb_u32(),
            severity_warning: theme.severity_warning.to_argb_u32(),
            severity_info: theme.severity_info.to_argb_u32(),
            severity_hint: theme.severity_hint.to_argb_u32(),
            severity_error_text: theme.severity_error_text.to_argb_u32(),
            severity_warning_text: theme.severity_warning_text.to_argb_u32(),
            severity_info_text: theme.severity_info_text.to_argb_u32(),
            severity_hint_text: theme.severity_hint_text.to_argb_u32(),
        }
    }

    fn severity_wash(&self, severity: Severity) -> u32 {
        match severity {
            Severity::Error => self.severity_error,
            Severity::Warning => self.severity_warning,
            Severity::Info => self.severity_info,
            Severity::Hint => self.severity_hint,
        }
    }

    fn severity_text(&self, severity: Severity) -> u32 {
        match severity {
            Severity::Error => self.severity_error_text,
            Severity::Warning => self.severity_warning_text,
            Severity::Info => self.severity_info_text,
            Severity::Hint => self.severity_hint_text,
        }
    }
}

/// Alpha-blend opaque `fg` over opaque `bg`, discarding `fg`'s own alpha and
/// producing an opaque result — the "pre-blended to opaque" idiom Visual
/// Language > Rows uses for badges/tags/keycaps so a selected row doesn't
/// stack washes.
fn blend_opaque(fg: u32, bg: u32, alpha_pct: u32) -> u32 {
    let mix = |shift: u32| {
        let f = (fg >> shift) & 0xFF;
        let b = (bg >> shift) & 0xFF;
        ((f * alpha_pct + b * (100 - alpha_pct)) / 100) & 0xFF
    };
    0xFF00_0000 | (mix(16) << 16) | (mix(8) << 8) | mix(0)
}

impl CompletionKind {
    /// Badge background, pre-blended to opaque over the panel (Visual
    /// Language > Colors: "syntax color @ 20% over panel"). Colors are
    /// picked from the existing overlay palette rather than new
    /// `overlay.kind_*` theme keys — see the `CompletionKind` doc comment.
    fn badge_color(self, colors: &Palette) -> u32 {
        let source = match self {
            CompletionKind::Function => colors.accent,
            CompletionKind::Variable => colors.severity_info,
            CompletionKind::Type => colors.accent_bright,
            CompletionKind::Keyword => colors.severity_warning,
            CompletionKind::Field => colors.text_dim,
            CompletionKind::Module => colors.keycap_fg,
            CompletionKind::Constant => colors.severity_error,
            CompletionKind::Other => colors.text_dim,
        };
        blend_opaque(source, colors.panel_bg, 20)
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
    let layout = layout(spec, window_width, window_height, scale_factor);
    let colors = Palette::from_theme(theme);
    let radius = match &spec.anchor {
        Anchor::Centered { .. } => scaled(dims::RADIUS, scale_factor),
        Anchor::Cursor { .. } => scaled(dims::RADIUS_CURSOR, scale_factor),
    };

    if let Anchor::Centered { dim_alpha, .. } = &spec.anchor {
        frame.dim(*dim_alpha);
    }
    frame.draw_shadow_rings(
        layout.panel.x,
        layout.panel.y,
        layout.panel.w,
        layout.panel.h,
        radius,
        scale_factor,
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

    if let Some(tab_bar) = &spec.tabs {
        render_tab_bar(
            frame,
            painter,
            &colors,
            tab_bar,
            &layout,
            scale_factor,
            radius,
            mask_cache,
        );
    }

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
        Body::Zones(zones) => render_zones(frame, painter, &colors, zones, &layout, scale_factor),
    }
    if let (Some(footer_spec), Some(footer_rect)) = (&spec.footer, layout.footer) {
        render_footer(
            frame,
            painter,
            &colors,
            footer_spec,
            footer_rect,
            scale_factor,
            radius,
            mask_cache,
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
    base_x + painter.measure_sized(&before, size, 0.0) as usize
}

#[allow(clippy::too_many_arguments)]
/// Render the Search Everywhere tab bar: recessed wash background, per-tab
/// label + count suffix, active tab lifted with a 2px accent underline,
/// `Unavailable` tabs dimmed (Visual Language > Regions/TabBar).
fn render_tab_bar(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    tab_bar: &TabBar,
    layout: &OverlayLayout,
    scale_factor: f64,
    radius: usize,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let Some(bar) = layout.tab_bar else { return };
    // The tab bar sits flush against the panel's top edge — a plain
    // `fill_rect_px` would square off the panel's antialiased top corners
    // (Visual Language > Chrome radius).
    frame.fill_rect_top_rounded(
        bar.x,
        bar.y,
        bar.w,
        bar.h,
        radius,
        colors.recessed_wash,
        mask_cache,
    );
    frame.fill_rect_px(
        bar.x,
        bar.y + bar.h.saturating_sub(1),
        bar.w,
        1,
        colors.hairline,
    );

    let size = size_px(SIZE_META, scale_factor);
    let underline_h = scaled(dims::TAB_UNDERLINE_H, scale_factor);
    let pad_x = scaled(dims::TAB_PAD_X, scale_factor);

    for (i, rect) in layout.tab_rects.iter().enumerate() {
        let Some(&(label, count)) = tab_bar.tabs.get(i) else {
            continue;
        };
        let is_active = i == tab_bar.active;
        let text = match count.label() {
            suffix if suffix.is_empty() => label.to_string(),
            suffix => format!("{label}  {suffix}"),
        };
        let color = if !count.is_available() {
            // Unavailable tabs dim further than a merely-inactive one
            // (Visual Language > TabCount states: "Unavailable also dims
            // the label") — same alpha-reduction idiom the scrollbar wash
            // below uses.
            let alpha = (colors.text_dim >> 24) & 0xFF;
            (((alpha * 60 / 100) & 0xFF) << 24) | (colors.text_dim & 0x00FF_FFFF)
        } else if is_active {
            colors.text_primary
        } else {
            colors.text_dim
        };
        let text_h = painter.line_height_for_size(size);
        let text_y = rect.y + (rect.h.saturating_sub(text_h)) / 2;
        painter.draw_sized(frame, rect.x + pad_x, text_y, &text, size, 1.0, color);

        if is_active {
            frame.fill_rect_px(
                rect.x,
                rect.y + rect.h.saturating_sub(underline_h),
                rect.w,
                underline_h,
                colors.accent,
            );
        }
    }
}

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

    // How much horizontal room is left for the query text (and its caret)
    // before it would run under the right-aligned `scope` text or off the
    // panel entirely. Everything drawn from here on is clipped to this
    // band too, as a hard backstop — `visible_header_text` should already
    // keep drawing within it, but a long paste/IME composition must never
    // paint outside the rounded panel regardless.
    let scope_w = header
        .scope
        .map(|s| {
            let scope_size = size_px(SIZE_META, scale_factor);
            painter.measure_sized(s, scope_size, 0.0).ceil() as usize + pad_x
        })
        .unwrap_or(0);
    let content_w = (r.x + r.w).saturating_sub(x + scope_w + pad_x / 2);
    frame.set_clip(crate::model::editor_area::Rect {
        x: x as f32,
        y: r.y as f32,
        width: content_w as f32,
        height: r.h as f32,
    });

    let (visible, kept_from) = if header.text.is_empty() {
        painter.draw_sized(
            frame,
            x,
            text_y,
            header.placeholder,
            size,
            0.0,
            colors.text_dim,
        );
        (String::new(), 0)
    } else {
        let (visible, kept_from) =
            visible_header_text(painter, size, header.text, content_w as f32);
        // Selection wash first, so the text paints over it. Columns are in
        // the full text's char space — re-express against the visible
        // string the same way the caret is.
        if let Some((sel_start, sel_end)) = header.selection {
            if sel_end > sel_start {
                let to_visible =
                    |col: usize| col.saturating_sub(kept_from) + usize::from(kept_from > 0);
                let x0 = caret_x_for_column(painter, x, &visible, to_visible(sel_start), size);
                let x1 = caret_x_for_column(painter, x, &visible, to_visible(sel_end), size);
                if x1 > x0 {
                    frame.fill_rect_px(x0, text_y, x1 - x0, text_h, colors.selection_wash);
                }
            }
        }
        painter.draw_sized(frame, x, text_y, &visible, size, 0.0, colors.text_primary);
        (visible, kept_from)
    };

    if let Some(col) = header.caret {
        if cursor_visible {
            // The caret's column is in the *full* text's char space;
            // re-express it against the (possibly head-truncated) visible
            // string so it never lands off-screen.
            let visible_col = col.saturating_sub(kept_from) + usize::from(kept_from > 0);
            let caret_x = caret_x_for_column(painter, x, &visible, visible_col, size);
            let caret_w = scaled(1.5, scale_factor);
            frame.fill_rect_px(caret_x, text_y, caret_w, text_h, colors.accent_bright);
        }
    }

    frame.clear_clip();

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

    let is_cursor = matches!(spec.anchor, Anchor::Cursor { .. });
    let row_size = size_px(SIZE_ROW, scale_factor);
    let meta_size = size_px(SIZE_META, scale_factor);
    let inset = scaled(
        if is_cursor {
            dims::ROW_INSET_CURSOR
        } else {
            dims::ROW_INSET
        },
        scale_factor,
    );
    let row_radius = scaled(
        if is_cursor {
            dims::ROW_RADIUS_CURSOR
        } else {
            dims::ROW_RADIUS
        },
        scale_factor,
    );
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
                match row.icon {
                    RowIcon::Glyph { ch, color } => {
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
                    RowIcon::KindBadge(kind) => {
                        let badge_size = scaled(dims::KIND_BADGE_SIZE, scale_factor);
                        let badge_radius = scaled(dims::KIND_BADGE_RADIUS, scale_factor);
                        let badge_y = rect.y + (rect.h.saturating_sub(badge_size)) / 2;
                        frame.fill_rounded_rect(
                            x,
                            badge_y,
                            badge_size,
                            badge_size,
                            badge_radius,
                            kind.badge_color(colors),
                            mask_cache,
                        );
                        let glyph_size = size_px(SIZE_META, scale_factor);
                        let mut buf = [0u8; 4];
                        let glyph = kind.glyph().encode_utf8(&mut buf);
                        let glyph_w = painter.measure_sized(glyph, glyph_size, 0.0);
                        let glyph_x = x + (badge_size.saturating_sub(glyph_w.ceil() as usize)) / 2;
                        let glyph_y = badge_y
                            + (badge_size.saturating_sub(painter.line_height_for_size(glyph_size)))
                                / 2;
                        painter.draw_sized(
                            frame,
                            glyph_x,
                            glyph_y,
                            glyph,
                            glyph_size,
                            0.0,
                            colors.text_bright,
                        );
                    }
                    RowIcon::None => {}
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
                        Accessory::Swatches {
                            colors: dots,
                            active,
                        } => {
                            let d = scaled(dims::SWATCH_D, scale_factor);
                            let gap = scaled(dims::SWATCH_GAP, scale_factor);
                            let dot_y = rect.y + (rect.h.saturating_sub(d)) / 2;
                            let mut cx = acc_x;
                            for &dot in *dots {
                                // Hairline ring first, swatch inset 1px on
                                // top — keeps a swatch that matches the
                                // panel background visible.
                                frame.fill_rounded_rect(
                                    cx,
                                    dot_y,
                                    d,
                                    d,
                                    d / 2,
                                    colors.hairline,
                                    mask_cache,
                                );
                                if d > 2 {
                                    frame.fill_rounded_rect(
                                        cx + 1,
                                        dot_y + 1,
                                        d - 2,
                                        d - 2,
                                        (d - 2) / 2,
                                        dot,
                                        mask_cache,
                                    );
                                }
                                cx += d + gap;
                            }
                            if *active {
                                let check_x =
                                    cx - gap + scaled(dims::SWATCH_CHECK_GAP, scale_factor);
                                painter.draw_sized(
                                    frame,
                                    check_x,
                                    acc_y,
                                    "\u{2713}",
                                    meta_size,
                                    0.0,
                                    colors.accent_bright,
                                );
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
        Accessory::Swatches { colors, active } => {
            let d = scaled(dims::SWATCH_D, scale_factor);
            let gap = scaled(dims::SWATCH_GAP, scale_factor);
            let mut w = colors.len() * d + colors.len().saturating_sub(1) * gap;
            if *active {
                w += scaled(dims::SWATCH_CHECK_GAP, scale_factor)
                    + painter.measure_sized("\u{2713}", meta_size, 0.0).ceil() as usize;
            }
            w
        }
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

#[allow(clippy::too_many_arguments)]
fn render_footer(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    footer: &Footer,
    rect: WidgetRect,
    scale_factor: f64,
    radius: usize,
    mask_cache: &mut RoundedRectMaskCache,
) {
    frame.fill_rect_px(rect.x, rect.y, rect.w, 1, colors.hairline);
    // The footer sits flush against the panel's bottom edge — see
    // `render_tab_bar`'s matching top-corner note.
    frame.fill_rect_bottom_rounded(
        rect.x,
        rect.y + 1,
        rect.w,
        rect.h.saturating_sub(1),
        radius,
        colors.recessed_wash,
        mask_cache,
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
        painter.draw_sized(frame, r.x, text_y, field.label, size, 0.0, color);
    }
}

/// Draw the single centered text block of a `Body::Zones` context (drop
/// overlay).
/// Render a `Body::Zones` context: the drop overlay's single centered
/// message (`text` only, no `banner`/`code`) and the hover card (banner +
/// code + text) share this one paint path.
fn render_zones(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    zones: &Zones,
    layout: &OverlayLayout,
    scale_factor: f64,
) {
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);

    if let (Some((severity, message, source)), Some(r)) = (zones.banner, layout.zones_banner) {
        frame.fill_rect_px(r.x, r.y, r.w, r.h, colors.severity_wash(severity));
        let size = size_px(SIZE_ROW, scale_factor);
        let text_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(size))) / 2;
        let mut buf = [0u8; 4];
        let text_color = colors.severity_text(severity);
        let glyph_w = painter.draw_sized(
            frame,
            r.x + pad_x,
            text_y,
            severity.glyph().encode_utf8(&mut buf),
            size,
            0.0,
            text_color,
        );
        let msg_x = r.x + pad_x + glyph_w.ceil() as usize + pad_x / 2;
        painter.draw_sized(frame, msg_x, text_y, message, size, 0.0, text_color);

        if !source.is_empty() {
            let meta_size = size_px(SIZE_META, scale_factor);
            let source_w = painter.measure_sized(source, meta_size, 0.0).ceil() as usize;
            let source_x = r.x + r.w.saturating_sub(pad_x + source_w);
            let source_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(meta_size))) / 2;
            painter.draw_sized(
                frame,
                source_x,
                source_y,
                source,
                meta_size,
                0.0,
                colors.text_dim,
            );
        }
    }

    if let (Some(code), Some(r)) = (zones.code, layout.zones_code) {
        frame.fill_rect_px(r.x, r.y, r.w, r.h, colors.panel_secondary);
        draw_text_lines(
            frame,
            painter,
            r,
            code,
            SIZE_ROW,
            colors.text_primary,
            scale_factor,
        );
    }

    if let (Some(text), Some(r)) = (zones.text, layout.zones_text) {
        if zones.banner.is_none() && zones.code.is_none() && text.lines().count() <= 1 {
            // Single-line, zone-only body (drop overlay): centered, matching
            // the pre-migration look.
            let size = size_px(SIZE_INPUT, scale_factor);
            let text_w = painter.measure_sized(text, size, 0.0).ceil() as usize;
            let text_x = r.x + (r.w.saturating_sub(text_w)) / 2;
            let text_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(size))) / 2;
            painter.draw_sized(frame, text_x, text_y, text, size, 0.0, colors.text_primary);
        } else {
            draw_text_lines(
                frame,
                painter,
                r,
                text,
                SIZE_ROW,
                colors.text_primary,
                scale_factor,
            );
        }
    }
}

/// Draw `text` as left-aligned, stacked lines within `rect` (no wrapping —
/// callers pre-wrap or accept clipping; the real hover-card consumer owns
/// wrapping policy per lsp-integration.md).
fn draw_text_lines(
    frame: &mut Frame,
    painter: &mut TextPainter,
    rect: WidgetRect,
    text: &str,
    size: f32,
    color: u32,
    scale_factor: f64,
) {
    let size = size_px(size, scale_factor);
    let line_h = painter.line_height_for_size(size);
    for (i, line) in text.lines().enumerate() {
        painter.draw_sized(frame, rect.x, rect.y + i * line_h, line, size, 0.0, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fontdue::Font;

    fn test_painter<'a>(
        font: &'a Font,
        glyph_cache: &'a mut super::super::GlyphCache,
    ) -> TextPainter<'a> {
        TextPainter::new(font, glyph_cache, 14.0, 11.0, 8.0, 18)
    }

    /// Regression: the tab bar and footer used to `fill_rect_px` a plain
    /// square band flush against the panel's rounded top/bottom edges,
    /// overwriting the antialiased corners `fill_rounded_rect` left
    /// transparent (Visual Language > Chrome radius 10). With tabs+footer
    /// present, the four panel corners must read the same as with neither.
    #[test]
    fn tab_bar_and_footer_do_not_square_the_panel_corners() {
        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = super::super::GlyphCache::default();
        let theme = OverlayTheme::default_dark();

        let mut render_corners = |tabs: bool, footer: bool| -> [u32; 4] {
            let (w, h) = (1200usize, 800usize);
            let mut buffer = vec![0u32; w * h];
            let mut frame = Frame::new(&mut buffer, w, h);
            let mut painter = test_painter(&font, &mut glyph_cache);
            let mut mask_cache = RoundedRectMaskCache::new();

            let tab_list = [("All", TabCount::Hidden)];
            let sections = [Section {
                title: None,
                rows: &[],
            }];
            let spec = OverlaySpec {
                anchor: Anchor::Centered {
                    width: WidthRule {
                        pct: 0.5,
                        min: 480.0,
                        max: 640.0,
                    },
                    dim_alpha: 0x66,
                },
                tabs: tabs.then_some(TabBar {
                    tabs: &tab_list,
                    active: 0,
                }),
                header: Some(Header {
                    glyph: None,
                    text: "",
                    placeholder: "",
                    caret: Some(0),
                    selection: None,
                    scope: None,
                }),
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(0),
                    scroll: 0,
                    max_visible: 8,
                },
                footer: footer.then_some(Footer {
                    leading: "",
                    trailing: "",
                }),
                hover_row: None,
            };

            render(
                &mut frame,
                &mut painter,
                &mut mask_cache,
                &theme,
                &spec,
                w,
                h,
                1.0,
                true,
            );

            let l = layout(&spec, w, h, 1.0);
            [
                frame.get_pixel(l.panel.x, l.panel.y),
                frame.get_pixel(l.panel.x + l.panel.w - 1, l.panel.y),
                frame.get_pixel(l.panel.x, l.panel.y + l.panel.h - 1),
                frame.get_pixel(l.panel.x + l.panel.w - 1, l.panel.y + l.panel.h - 1),
            ]
        };

        let without_chrome = render_corners(false, false);
        let with_chrome = render_corners(true, true);

        assert_eq!(
            with_chrome, without_chrome,
            "tab bar/footer must not paint over the panel's rounded corners"
        );
    }

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
    fn swatches_accessory_width_counts_dots_gaps_and_check() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let dots = [0xFF112233u32, 0xFF445566, 0xFF778899, 0xFFAABBCC];

        let plain = accessory_width(
            &mut painter,
            &Accessory::Swatches {
                colors: &dots,
                active: false,
            },
            SIZE_META,
            1.0,
        );
        // 4 dots of 7px + 3 gaps of 3px at 1x.
        assert_eq!(plain, 4 * 7 + 3 * 3);

        let with_check = accessory_width(
            &mut painter,
            &Accessory::Swatches {
                colors: &dots,
                active: true,
            },
            SIZE_META,
            1.0,
        );
        let check_w = painter.measure_sized("\u{2713}", SIZE_META, 0.0).ceil() as usize;
        assert_eq!(with_check, plain + 6 + check_w);
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
            tabs: None,
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
                selection: None,
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
            tabs: None,
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
                selection: None,
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
            tabs: None,
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
                selection: None,
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
    fn resolve_scroll_for_selection_keeps_last_row_of_sectioned_list_visible() {
        // Regression: two 8-row titled sections (16 rows, 2 headers) with
        // max_visible 10 — the same shape as Recent Files'
        // Pinned/Today/Yesterday/Earlier grouping or the Theme Picker's
        // User/Built-in split. Walking Down through every row must always
        // land the selection inside the display window that
        // `resolve_visible_window` will actually paint.
        let shapes = [
            SectionShape {
                has_title: true,
                len: 8,
            },
            SectionShape {
                has_title: true,
                len: 8,
            },
        ];
        let mut scroll = 0usize;
        for selected in 0..16 {
            scroll = resolve_scroll_for_selection(&shapes, selected, 10, scroll);
            // Rebuild the display-row skeleton the view would render for
            // this scroll/selection and assert the selected FlatIndex's
            // display slot actually falls inside the visible window.
            let mut display_len = 0usize;
            let mut flat_to_display = Vec::new();
            for shape in &shapes {
                if shape.has_title {
                    display_len += 1;
                }
                for _ in 0..shape.len {
                    flat_to_display.push(display_len);
                    display_len += 1;
                }
            }
            let visible = display_len.min(10);
            let scroll_display = flat_to_display[scroll];
            let start = scroll_display.min(display_len.saturating_sub(visible));
            let selected_display = flat_to_display[selected];
            assert!(
                selected_display >= start && selected_display < start + visible,
                "selected {selected} (display {selected_display}) not in window [{start}, {})",
                start + visible
            );
        }
    }

    #[test]
    fn resolve_scroll_for_selection_matches_compute_from_without_sections() {
        // Single untitled section (Command Palette / File Finder shape):
        // must reduce to exactly SelectableListViewport::compute_from.
        let shapes = [SectionShape {
            has_title: false,
            len: 15,
        }];
        let mut scroll_a = 0usize;
        let mut scroll_b = 0usize;
        for selected in [0, 5, 9, 12, 8, 14, 0] {
            scroll_a = resolve_scroll_for_selection(&shapes, selected, 8, scroll_a);
            scroll_b =
                SelectableListViewport::compute_from(15, selected, 8, scroll_b).scroll_offset;
            assert_eq!(scroll_a, scroll_b);
        }
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
        assert_eq!(at_start, 100, "column 0 caret sits flush at the text start");
    }

    #[test]
    fn header_caret_renders_on_empty_input() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        // Regression: caret drawing lived in the `else` branch of
        // `header.text.is_empty()`, so an empty palette input drew no
        // caret at all.
        let caret_x = caret_x_for_column(&mut painter, 100, "", 0, SIZE_INPUT);
        assert_eq!(caret_x, 100);
    }

    #[test]
    fn visible_header_text_keeps_short_text_unchanged() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let (visible, kept_from) = visible_header_text(&mut painter, SIZE_INPUT, "short", 1000.0);
        assert_eq!(visible, "short");
        assert_eq!(kept_from, 0);
    }

    #[test]
    fn visible_header_text_head_truncates_a_long_query_keeping_the_tail() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let text = "a".repeat(200);
        let (visible, kept_from) = visible_header_text(&mut painter, SIZE_INPUT, &text, 80.0);
        assert!(visible.starts_with('\u{2026}'));
        assert!(kept_from > 0);
        assert!(visible.chars().count() < text.chars().count());
    }

    /// Regression: a long/pasted query used to spill `draw_sized` text past
    /// the panel's right edge with no clipping or truncation. Rendering the
    /// full header (list-context input, e.g. the command palette) with a
    /// long query must paint the same dimmed backdrop right of the panel as
    /// an empty query does — nothing extra from the overflowing text.
    #[test]
    fn long_header_query_stays_within_the_panel() {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = super::super::GlyphCache::default();
        let theme = OverlayTheme::default_dark();
        let (w, h) = (1200usize, 800usize);

        let mut render_row_right_of_panel = |query: &str| -> Vec<u32> {
            let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);
            let mut buffer = vec![0u32; w * h];
            let mut frame = Frame::new(&mut buffer, w, h);
            let mut mask_cache = RoundedRectMaskCache::new();

            let sections: [Section; 0] = [];
            let spec = OverlaySpec {
                anchor: Anchor::Centered {
                    width: WidthRule {
                        pct: 0.5,
                        min: 480.0,
                        max: 640.0,
                    },
                    dim_alpha: 0x66,
                },
                tabs: None,
                header: Some(Header {
                    glyph: Some('\u{276F}'),
                    text: query,
                    placeholder: "",
                    caret: Some(query.chars().count()),
                    selection: None,
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

            render(
                &mut frame,
                &mut painter,
                &mut mask_cache,
                &theme,
                &spec,
                w,
                h,
                1.0,
                true,
            );

            let l = layout(&spec, w, h, 1.0);
            let panel_right = l.panel.x + l.panel.w;
            let header = l.header.unwrap();
            (header.y..header.y + header.h)
                .flat_map(|y| (panel_right..w).map(move |x| (x, y)))
                .map(|(x, y)| frame.get_pixel(x, y))
                .collect()
        };

        let baseline = render_row_right_of_panel("");
        let with_long_query = render_row_right_of_panel(&"x".repeat(200));

        assert_eq!(
            with_long_query, baseline,
            "a long query must not paint past the panel's right edge"
        );
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
            tabs: None,
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
                selection: None,
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
            tabs: None,
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
            tabs: None,
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x80,
            },
            header: None,
            body: Body::Zones(Zones {
                banner: None,
                code: None,
                text: Some("Drop to open: file.rs"),
            }),
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

    // =========================================================================
    // Anchor::Cursor (Phase 5)
    // =========================================================================

    fn one_row() -> Row<'static> {
        Row {
            icon: RowIcon::None,
            label: "foo",
            match_indices: &[],
            detail: None,
            accessory: Accessory::None,
        }
    }

    #[test]
    fn cursor_anchor_positions_below_the_anchor_line_when_it_fits() {
        let rows = [one_row(), one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 100,
                y: 200,
                h: 0,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert!(l.panel.y > 200, "panel should sit below the anchor line");
        assert!(l.panel.y < 200 + l.panel.h + 10);
    }

    #[test]
    fn cursor_anchor_flips_above_when_below_space_is_too_small() {
        let rows = [one_row(), one_row(), one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        // Anchor near the bottom of an 800px-tall window: no room below for
        // a multi-row popup, but plenty of room above.
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 100,
                y: 780,
                h: 20, // caret line height: line spans 780..800
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert!(
            l.panel.y + l.panel.h <= 780,
            "panel should flip above the anchor line: panel.y={} h={}",
            l.panel.y,
            l.panel.h
        );
    }

    #[test]
    fn cursor_anchor_clamps_to_window_edges_when_neither_direction_fits() {
        // Anchor line close enough to both the top and bottom that a
        // small popup fits neither strictly above nor strictly below;
        // it must still clamp inside the window rather than picking an
        // out-of-bounds position.
        let rows = [one_row(), one_row(), one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 100,
                y: 10, // near the top: no room above, and "prefer_below" is off
                h: 0,
                prefer_below: false,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        // No room above (y=10 < panel_h), so it must fall through to
        // "fits_below" even though prefer_below is false — never picking a
        // position that would clip off the top of the window.
        assert!(l.panel.y + l.panel.h <= 800);
        assert!(l.panel.y < 800);
    }

    #[test]
    fn cursor_anchor_clamps_when_popup_is_taller_than_the_window() {
        // Pathological case: the popup can't fit anywhere. The best a
        // clamp can do is pin it to the top of the window rather than
        // pushing it further off-screen.
        let rows: Vec<Row> = (0..50).map(|_| one_row()).collect();
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 100,
                y: 400,
                h: 0,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 50,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert_eq!(l.panel.y, 0, "clamps to the top when it can't fit anywhere");
    }

    #[test]
    fn cursor_anchor_width_floor_never_exceeds_a_narrow_window() {
        // The 200px cursor-width floor is wider than a 150px-wide window;
        // the panel must still be clamped to fit inside it rather than
        // overflowing the right edge.
        let rows = [one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 10,
                y: 100,
                h: 0,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 150, 800, 1.0);
        assert!(l.panel.w <= 150, "panel width {} exceeds window", l.panel.w);
        assert!(
            l.panel.x + l.panel.w <= 150,
            "panel overflows the right edge: x={} w={}",
            l.panel.x,
            l.panel.w
        );
    }

    #[test]
    fn cursor_anchor_clamps_x_to_the_right_window_edge() {
        let rows = [one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 990, // near the right edge of a 1000px window
                y: 100,
                h: 0,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert!(l.panel.x + l.panel.w <= 1000);
    }

    #[test]
    fn cursor_anchor_width_floors_at_200_logical_px() {
        let rows = [one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 100,
                y: 100,
                h: 0,
                prefer_below: true,
                // A width rule that would clamp far below the 200px floor.
                width: WidthRule {
                    pct: 0.0,
                    min: 10.0,
                    max: 50.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 8,
            },
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 2.0);
        assert!(
            l.panel.w >= 400,
            "200 logical px at 2x scale = 400 physical"
        );
    }

    #[test]
    fn cursor_anchor_uses_the_completion_row_height() {
        let rows = [one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let list_body = || Body::List {
            sections: &sections,
            selected: FlatIndex(0),
            scroll: 0,
            max_visible: 8,
        };
        let cursor_spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 0,
                y: 0,
                h: 0,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: list_body(),
            footer: None,
            hover_row: None,
        };
        let centered_spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Centered {
                width: WidthRule {
                    pct: 0.5,
                    min: 300.0,
                    max: 500.0,
                },
                dim_alpha: 0x66,
            },
            header: None,
            body: list_body(),
            footer: None,
            hover_row: None,
        };
        let cursor_layout = layout(&cursor_spec, 1000, 800, 1.0);
        let centered_layout = layout(&centered_spec, 1000, 800, 1.0);
        assert_eq!(cursor_layout.row_height, 24);
        assert_eq!(centered_layout.row_height, 30);
    }

    #[test]
    fn zones_body_stacks_banner_code_and_text_in_order() {
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 50,
                y: 50,
                h: 0,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 0.0,
                    max: 300.0,
                },
            },
            header: None,
            body: Body::Zones(Zones {
                banner: Some((Severity::Warning, "unused import", "rustc")),
                code: Some("fn foo(x: i32) -> i32"),
                text: Some("This value is never read."),
            }),
            footer: None,
            hover_row: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        let banner = l.zones_banner.expect("banner zone");
        let code = l.zones_code.expect("code zone");
        let text = l.zones_text.expect("text zone");
        assert!(banner.y < code.y, "banner sits above code");
        assert!(code.y < text.y, "code sits above text");
        assert!(l.panel.h >= banner.h + code.h + text.h);
    }

    #[test]
    fn flat_index_navigation_and_dismiss_key_pass_through() {
        // Regression for the routing contract: Up/Down/Enter/Esc/Tab are the
        // only keys a cursor overlay should ever claim; every other key
        // (including plain character input) must be classified as
        // "pass through to the editor" by the caller. This is exercised at
        // the `runtime::input` integration-test level; here we just pin the
        // FlatIndex math those handlers dispatch through.
        assert_eq!(FlatIndex(0).next(3), FlatIndex(1));
        assert_eq!(FlatIndex(2).next(3), FlatIndex(0));
    }
}
