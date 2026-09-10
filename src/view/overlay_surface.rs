//! The unified overlay surface: one component (chrome, header, sectioned
//! list, footer) driving every palette/picker/popup in the editor.
//!
//! Phase 1 ([docs/feature/overlay-surface.md](../../docs/feature/overlay-surface.md))
//! implements `Anchor::Centered`, `Header`, `Body::List`, and `Footer` —
//! enough to render the command palette. Tabs, `ListWithPanel`, `Zones`,
//! and `Fields` land with the phases that consume them.
//!
//! `layout()` is the single source of truth shared by rendering and
//! hit-testing.

#[path = "settings_page.rs"]
mod settings_page;
pub use settings_page::visible_count as settings_visible_count;

pub(crate) fn settings_field_options(
    spec: &OverlaySpec,
    layout: &OverlayLayout,
    row_index: usize,
) -> Option<super::TextFieldOptions> {
    let Body::List { sections, .. } = &spec.body else {
        return None;
    };
    let rows = flatten_rows(sections);
    layout.settings_items.iter().find_map(|(display, rect)| {
        let DisplayRow::Row(row, index) = rows.get(*display)? else {
            return None;
        };
        (index.0 == row_index)
            .then(|| settings_page::field_options(row, *rect, layout.scale_factor))
            .flatten()
    })
}

use super::frame::{FontRole, Frame, RoundedRectMaskCache, TextPainter};
use super::geometry::WidgetRect;
use super::helpers::EllipsisSide;
use super::scrollbar::{
    render_scrollbar, ScrollbarColors, ScrollbarGeometry, ScrollbarState, SCROLLBAR_WIDTH_LOGICAL,
};
use crate::completion::menu::MenuItemKind;
use crate::layout::{
    AttachPoint, Content, Dir, ElementDecl, FloatAnchor, FloatDecl, LayoutSnapshot, Padding,
    RowListDecl, Sizing, SizingAxes, UiKey, UiTree,
};
use crate::model::editor_area::Rect;
use crate::model::ui::DocumentationState;
use crate::model::{runs_in_spans, Span, SpanStyle, StyledText};
#[cfg(test)]
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
    // The centered-Y, cursor-gap, and cursor-width-floor constants moved to
    // `layout::anchor::dims` with the placement functions.
    /// Completion kind badge (Visual Language > Rows: "kind badge (16×16, r4)").
    pub const KIND_BADGE_SIZE: f32 = 16.0;
    pub const KIND_BADGE_RADIUS: f32 = 4.0;
    /// Hover-card zone geometry (Zones body).
    pub const ZONE_BANNER_H: f32 = 28.0;
    pub const ZONE_GAP: f32 = 8.0;
    /// Gap between chips within one chord step's keycap accessory.
    pub const CHIP_GAP: f32 = 4.0;
    /// Zone text metrics (hover card / drop overlay): the stacking line
    /// height (17px clears the 13px glyphs) and the monospace cell width
    /// backing `cell_measure` — the painter-free fallback used by tests
    /// and painterless contexts. The real render/hit-test paths measure
    /// through the glyph cache (`layout::PainterMeasure`) instead; at 1x
    /// the two agree (0.6·13 = 7.8 rounds to 8).
    pub const ZONE_LINE_H: f32 = 17.0;
    pub const ZONE_CELL_W: f32 = 8.0;
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
    /// Completion docs card width.
    pub const DOCS_WIDTH: f32 = 360.0;
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
/// (Moved to `layout::anchor` so the layout engine and this surface share
/// one implementation; re-exported here for existing consumers.)
pub use crate::layout::anchor::WidthRule;

pub enum Anchor {
    /// Spacious preferences page with category navigation.
    Settings {
        width: WidthRule,
        close_hovered: bool,
    },
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
    /// Content-sized context menu, sharing cursor placement and edge clamping.
    Menu {
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
            Anchor::Centered { width, .. }
            | Anchor::Cursor { width, .. }
            | Anchor::Menu { width, .. }
            | Anchor::Settings { width, .. } => width,
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
    /// Completion row icon: a 16×16, r4 badge colored by `MenuItemKind`
    /// (Visual Language > Rows: "Completion").
    KindBadge(MenuItemKind),
}

impl MenuItemKind {
    /// Single-glyph badge label.
    fn badge_glyph(self) -> char {
        match self {
            Self::Function => 'f',
            Self::Method => 'M',
            Self::Variable => 'v',
            Self::Type => 't',
            Self::Keyword => 'k',
            Self::Field => '.',
            Self::Module => 'm',
            Self::File => 'F',
            Self::Folder => '/',
            Self::Constant => 'c',
            Self::Other => '?',
        }
    }
}

pub enum Accessory<'a> {
    SettingInput {
        content: &'a crate::editable::EditableState<crate::editable::StringBuffer>,
        focused: bool,
        browse: bool,
        line_height: usize,
        char_width: f32,
    },
    /// A configuration value that remains visible in compact forms.
    SettingValue {
        text: &'a str,
        action: Option<&'a str>,
    },
    None,
    /// Explicit preset choices. `None` active preserves off-preset config values.
    Choices {
        labels: &'a [&'a str],
        active: Option<usize>,
    },
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
            let mut key: String = step.chars().skip_while(|c| MODIFIERS.contains(c)).collect();
            // Windows/Linux display uses textual modifiers rather than glyphs.
            while let Some(prefix) = ["Ctrl+", "Alt+", "Shift+", "Win+"]
                .into_iter()
                .find(|prefix| key.starts_with(prefix))
            {
                chips.push(Chip {
                    label: prefix.trim_end_matches('+').to_owned(),
                });
                key = key[prefix.len()..].to_owned();
            }
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
    /// `Some(Code)` renders `detail` as a recessed chip (a completion's
    /// type signature); `None` is the dim meta text.
    pub detail_style: Option<SpanStyle>,
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
    /// Right-aligned text on the label row: a match count, a validation
    /// error. Painted dim, or in the error colour when `trailing_is_error`.
    pub trailing: Option<&'a str>,
    pub trailing_is_error: bool,
}

impl<'a> Field<'a> {
    pub const fn labeled(label: &'a str) -> Self {
        Self {
            label,
            trailing: None,
            trailing_is_error: false,
        }
    }
}

pub enum Body<'a> {
    List {
        sections: &'a [Section<'a>],
        selected: FlatIndex,
        /// Selectable-row offset for lists; physical pixels for Settings forms.
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
    /// Center a short notification (the file-drop overlay), never documentation.
    pub center_text: bool,
    /// Interactive hover documentation uses the same row window as completion.
    pub documentation: Option<DocumentationState>,
    /// Severity, message, source (e.g. `(Error, "unused import", "rustc")`).
    pub banner: Option<(Severity, &'a str, &'a str)>,
    /// Style spans (byte ranges) into the banner message.
    pub banner_spans: &'a [Span],
    /// Signature block, rendered on `panel_secondary`.
    pub code: Option<&'a str>,
    /// Style spans into `code` (signature help's active parameter).
    pub code_spans: &'a [Span],
    pub text: Option<&'a str>,
    /// Style spans into `text` (inline code chips, emphasis).
    pub text_spans: &'a [Span],
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

/// A documentation side card uses the same measured plan for paint and input.
pub struct Documentation<'a> {
    pub text: &'a StyledText,
    pub state: DocumentationState,
}

impl<'a> From<&'a StyledText> for Documentation<'a> {
    fn from(text: &'a StyledText) -> Self {
        Self {
            text,
            state: DocumentationState::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentationViewport {
    pub scroll: usize,
    pub visible: usize,
    pub total: usize,
}

impl DocumentationViewport {
    pub fn max_scroll(self) -> usize {
        self.total.saturating_sub(self.visible)
    }

    pub fn scrolled(self, lines: isize) -> usize {
        self.scroll
            .saturating_add_signed(lines)
            .min(self.max_scroll())
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
    /// Styled documentation floated beside the panel, with its own viewport.
    pub docs: Option<Documentation<'a>>,
}

/// One entry in the flattened, on-screen row list: either a section header
/// (not selectable) or a row paired with its `FlatIndex`.
enum DisplayRow<'a> {
    SectionHeader(&'a str),
    /// An untitled section boundary (context-menu separators): occupies a
    /// display slot, rendered as a centered hairline, never selectable.
    Separator,
    Row(&'a Row<'a>, FlatIndex),
}

fn flatten_rows<'a>(sections: &'a [Section<'a>]) -> Vec<DisplayRow<'a>> {
    let mut out = Vec::new();
    let mut flat_i = 0;
    for (i, section) in sections.iter().enumerate() {
        match section.title {
            Some(title) => out.push(DisplayRow::SectionHeader(title)),
            // Untitled non-first sections are visual separators — without
            // a slot they rendered no break at all (context-menu spec:
            // groups get a visible boundary).
            None if i > 0 => out.push(DisplayRow::Separator),
            None => {}
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
    let start = list_window_start(scroll_display, scroll, display_rows.len(), visible);
    (start, visible)
}

/// At the top, include the first section heading when there is also room for a
/// selectable row. A one-slot window must prioritize the selectable row.
fn list_window_start(
    row_display: usize,
    scroll: usize,
    display_len: usize,
    visible: usize,
) -> usize {
    let desired = if scroll == 0 && row_display < visible {
        0
    } else {
        row_display
    };
    desired.min(display_len.saturating_sub(visible))
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

pub(crate) fn section_positions(shapes: &[SectionShape]) -> (Vec<usize>, usize) {
    let mut display_len = 0;
    let mut positions = Vec::new();
    for (index, shape) in shapes.iter().enumerate() {
        if shape.has_title || index > 0 {
            display_len += 1;
        }
        for _ in 0..shape.len {
            positions.push(display_len);
            display_len += 1;
        }
    }
    (positions, display_len)
}

/// Convert a scrollbar's display-row position (including headings) to a list offset.
pub fn resolve_scroll_for_display(
    shapes: &[SectionShape],
    position: usize,
    capacity: usize,
) -> usize {
    let (positions, total) = section_positions(shapes);
    scroll_for_display(&positions, total, total.min(capacity), position)
}

fn scroll_for_display(positions: &[usize], total: usize, visible: usize, position: usize) -> usize {
    if positions.is_empty() {
        return 0;
    }
    let target = position.min(total.saturating_sub(visible));
    if target == 0 {
        return 0;
    }
    let last = positions.len() - 1;
    let candidate = positions.partition_point(|&row| row < target).min(last);
    if list_window_start(positions[candidate], candidate, total, visible) < target {
        (candidate + 1).min(last)
    } else {
        candidate
    }
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
    let (flat_to_display, display_len) = section_positions(shapes);
    if flat_to_display.is_empty() {
        return 0;
    }
    let last_flat = flat_to_display.len() - 1;
    let visible = display_len.min(max_visible);

    let selected = selected.min(last_flat);
    if selected == 0 {
        return 0;
    }
    let selected_display = flat_to_display[selected];

    let prev_display = flat_to_display[previous_scroll.min(last_flat)];
    let start = list_window_start(prev_display, previous_scroll, display_len, visible);

    let target_display = if selected_display < start {
        selected_display
    } else if selected_display >= start + visible {
        selected_display + 1 - visible
    } else {
        start
    };

    scroll_for_display(&flat_to_display, display_len, visible, target_display)
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
    let visible = painter.truncate_sized(text, size, max_width, EllipsisSide::Start);
    if matches!(visible, std::borrow::Cow::Borrowed(value) if value == text) {
        return (visible.into_owned(), 0);
    }
    let total_chars = text.chars().count();
    let kept_chars = visible.chars().count().saturating_sub(1); // minus the ellipsis
    (visible.into_owned(), total_chars.saturating_sub(kept_chars))
}

/// Geometry for one `Field` in a `Body::Fields` layout: the label row above
/// an input box. The caller paints actual field content (text, selection,
/// caret) into `input` via `TextFieldRenderer`.
#[derive(Clone, Copy, Debug)]
pub struct FieldLayout {
    pub label: WidgetRect,
    pub input: WidgetRect,
}

/// Solved Clay geometry for a centered or cursor-anchored surface: panel
/// chrome, header, list rows, fields, zones, footer, and scrollbar thumb.
/// Rendering and hit-testing consume this same snapshot.
pub struct OverlayLayout {
    scale_factor: f64,
    snapshot: LayoutSnapshot,
    /// Solved choice-chip rectangles, shared by painting and hit testing.
    choices: Vec<(FlatIndex, Vec<WidgetRect>)>,
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
    /// Pixel-scrolled form body; painting and hit testing share its clipped range.
    pub settings_viewport: Option<crate::layout::RowListView>,
    /// Visible form rows retain signed origins for partially scrolled text areas.
    pub(crate) settings_items: Vec<(usize, Rect)>,
    pub(crate) settings_positions: Vec<std::ops::Range<usize>>,
    /// One entry per `Body::Fields` field, in order. Empty otherwise.
    pub fields: Vec<FieldLayout>,
    /// The banner zone of a `Body::Zones` body. `None` unless
    /// `spec.body`'s `Zones::banner` is `Some`.
    pub zones_banner: Option<WidgetRect>,
    /// The code zone of a `Body::Zones` body.
    pub zones_code: Option<WidgetRect>,
    /// The text zone of a `Body::Zones` body.
    pub zones_text: Option<WidgetRect>,
    /// Interactive reading surface: the completion side card or hover panel.
    pub docs_panel: Option<WidgetRect>,
    pub docs_text: Option<WidgetRect>,
    pub footer: Option<WidgetRect>,
    pub scrollbar: Option<ScrollbarGeometry>,
    /// The measured zone plan (wrapped lines + heights) for a `Body::Zones`
    /// body, computed once in `layout_measured` and consumed by
    /// `render_zones` — the plan is never re-derived. `None` for other
    /// bodies.
    pub(crate) zone_plan: Option<ZonePlan>,
    /// Wrapped docs-card lines (`lines, truncated, height`), measured once
    /// like `zone_plan`.
    pub(crate) docs_plan: Option<TextZonePlan>,
    /// The docs card's leading code block (lines, height), when the docs
    /// open with a code fence.
    pub(crate) docs_code_plan: Option<(Vec<StyledLine>, usize)>,
    /// Content rect of the docs card's code block.
    pub docs_code: Option<WidgetRect>,
    pub docs_scrollbar: Option<ScrollbarGeometry>,
    pub docs_viewport: Option<DocumentationViewport>,
}

fn float_decl(anchor: &Anchor) -> FloatDecl {
    let width = *anchor.width();
    let float_anchor = match anchor {
        Anchor::Centered { .. } | Anchor::Settings { .. } => FloatAnchor::WindowCentered,
        Anchor::Cursor {
            x,
            y,
            h,
            prefer_below,
            ..
        }
        | Anchor::Menu {
            x,
            y,
            h,
            prefer_below,
            ..
        } => FloatAnchor::Caret {
            x: *x as f32,
            y: *y as f32,
            line_h: *h as f32,
            prefer_below: *prefer_below,
        },
    };
    FloatDecl {
        anchor: float_anchor,
        z: 10,
        width: Some(width),
    }
}

fn widget_rect(rect: Rect) -> WidgetRect {
    let (x, y, w, h) = crate::layout::snapshot::snap(rect);
    WidgetRect { x, y, w, h }
}

fn solved_rect(snapshot: &LayoutSnapshot, key: UiKey) -> Option<WidgetRect> {
    snapshot.rect(key).map(widget_rect)
}

fn solved_content_rect(snapshot: &LayoutSnapshot, key: UiKey) -> Option<WidgetRect> {
    snapshot.content_rect(key).map(widget_rect)
}

fn spacer(t: &mut UiTree, height: usize) {
    t.leaf(ElementDecl {
        sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
        ..Default::default()
    });
}

/// The painter-free fallback measure: monospace cells at the historical
/// zone constants (8px cell, 17px line at 1x). Used by tests and any
/// context without a painter; the real render and hit-test paths measure
/// through the glyph cache (`layout::PainterMeasure`) instead.
pub fn cell_measure(scale_factor: f64) -> crate::layout::CellMeasure {
    crate::layout::CellMeasure {
        char_width: scaled(dims::ZONE_CELL_W, scale_factor) as f32,
        line_height: scaled(dims::ZONE_LINE_H, scale_factor) as f32,
    }
}

/// `layout_measured` with the monospace-cell fallback measure. Prefer
/// `layout_measured` with a `PainterMeasure` wherever a painter is in
/// reach — real advances wrap zone text where it actually fits.
pub fn layout(
    spec: &OverlaySpec,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
) -> OverlayLayout {
    layout_measured(
        spec,
        window_width,
        window_height,
        scale_factor,
        &mut cell_measure(scale_factor),
    )
}

/// Layout an `OverlaySpec` against the (physical-px) window size. This is
/// the one layout function the doc calls for — paint and hit-testing both
/// consume it. `measure` drives zone text wrapping (`Body::Zones`).
pub fn layout_measured(
    spec: &OverlaySpec,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
    measure: &mut dyn crate::layout::TextMeasure,
) -> OverlayLayout {
    let mut width = *spec.anchor.width();
    if matches!(spec.anchor, Anchor::Menu { .. }) {
        width.min = (menu_content_width(spec, scale_factor, measure) as f32 / scale_factor as f32)
            .ceil()
            .clamp(width.min, width.max);
    }
    let is_cursor = matches!(&spec.anchor, Anchor::Cursor { .. } | Anchor::Menu { .. });
    let panel_w =
        crate::layout::anchor::resolve_width(&width, is_cursor, window_width, scale_factor);

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
    let display_rows = match &spec.body {
        Body::List { sections, .. } => flatten_rows(sections),
        Body::Fields { .. } | Body::Zones(_) => Vec::new(),
    };
    let list_info = match &spec.body {
        Body::List {
            scroll,
            max_visible,
            ..
        } => {
            let (start, visible) = resolve_visible_window(&display_rows, *scroll, *max_visible);
            // Empty lists still reserve one row for their empty-state text.
            let visible = visible.max(usize::from(display_rows.is_empty()));
            Some((start, visible, display_rows.len(), *max_visible))
        }
        Body::Fields { .. } | Body::Zones(_) => None,
    };
    // Text wrapping is content measurement; the solved boxes below consume
    // its heights instead of re-deriving their stack manually.
    let zone_plan = match &spec.body {
        Body::Zones(zones) => {
            let mut plan = plan_zones(zones, panel_w, scale_factor, measure);
            if let Some(state) = zones.documentation {
                plan.scroll_documentation(state, window_height, scale_factor);
            }
            Some(plan)
        }
        Body::List { .. } | Body::Fields { .. } => None,
    };
    // Horizontal placement is independent of height. Reuse the solver's
    // anchor functions so wrapping fits the actual larger side of the menu,
    // including when neither side can fit the preferred documentation width.
    let panel_x = match &spec.anchor {
        Anchor::Cursor {
            x,
            y,
            h,
            prefer_below,
            ..
        }
        | Anchor::Menu {
            x,
            y,
            h,
            prefer_below,
            ..
        } => {
            crate::layout::anchor::position_at_caret(
                *x,
                *y,
                *h,
                *prefer_below,
                window_width,
                window_height,
                panel_w,
                0,
                scale_factor,
            )
            .0
        }
        Anchor::Centered { .. } | Anchor::Settings { .. } => {
            crate::layout::anchor::position_centered(
                window_width,
                window_height,
                panel_w,
                scale_factor,
            )
            .0
        }
    };
    let side_space = panel_x.max(window_width.saturating_sub(panel_x + panel_w));
    let docs_w = scaled(dims::DOCS_WIDTH, scale_factor).min(side_space);
    let DocumentationPlan {
        code: docs_code_plan,
        text: docs_plan,
        viewport: docs_viewport,
        gap: docs_gap,
    } = spec
        .docs
        .as_ref()
        .map(|docs| plan_docs(docs, docs_w, window_height, scale_factor, measure))
        .unwrap_or_default();

    let mut tree = UiTree::new();
    tree.node(ElementDecl::default(), |t| {
        t.node(
            ElementDecl {
                key: Some(UiKey::OverlayPanel),
                dir: Dir::Column,
                sizing: SizingAxes::new(Sizing::FIT, Sizing::FIT),
                clip: true,
                float: Some(FloatDecl {
                    width: Some(width),
                    ..float_decl(&spec.anchor)
                }),
                ..Default::default()
            },
            |t| match &spec.body {
                Body::List { .. } => {
                    if let (Some(tabs), Some(height)) = (&spec.tabs, tab_bar_h) {
                        t.node(
                            ElementDecl {
                                key: Some(UiKey::OverlayTabBar),
                                dir: Dir::Row,
                                sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
                                ..Default::default()
                            },
                            |t| {
                                let tab_width = panel_w / tabs.tabs.len().max(1);
                                for index in 0..tabs.tabs.len() {
                                    t.leaf(ElementDecl {
                                        key: Some(UiKey::OverlayTab(index)),
                                        sizing: SizingAxes::new(
                                            Sizing::Fixed(tab_width as f32),
                                            Sizing::GROW,
                                        ),
                                        ..Default::default()
                                    });
                                }
                            },
                        );
                    }
                    if let Some(height) = header_h {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayHeader),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
                            ..Default::default()
                        });
                    }
                    let visible = list_info.map(|(_, visible, _, _)| visible).unwrap_or(0);
                    t.leaf(ElementDecl {
                        key: Some(UiKey::OverlayRows),
                        sizing: SizingAxes::new(
                            Sizing::GROW,
                            Sizing::Fixed((visible * row_h) as f32),
                        ),
                        content: Content::RowList(RowListDecl {
                            row_height: row_h as f32,
                            count: visible,
                            scroll_offset: 0,
                        }),
                        ..Default::default()
                    });
                    if let Some(height) = footer_h {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayFooter),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
                            ..Default::default()
                        });
                    }
                }
                Body::Fields { fields, .. } => {
                    let label_h = scaled(dims::FIELD_LABEL_H, scale_factor);
                    let label_gap = scaled(dims::FIELD_LABEL_GAP, scale_factor);
                    let input_h =
                        scaled(SIZE_INPUT, scale_factor) + 2 * scaled(dims::PAD_Y, scale_factor);
                    let field_spacing = scaled(dims::FIELD_SPACING, scale_factor);
                    let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
                    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);

                    spacer(t, pad_y);
                    t.node(
                        ElementDecl {
                            dir: Dir::Column,
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::FIT),
                            padding: Padding::xy(pad_x as f32, 0.0),
                            gap: field_spacing as f32,
                            ..Default::default()
                        },
                        |t| {
                            for index in 0..fields.len() {
                                t.node(
                                    ElementDecl {
                                        dir: Dir::Column,
                                        sizing: SizingAxes::new(Sizing::GROW, Sizing::FIT),
                                        gap: label_gap as f32,
                                        ..Default::default()
                                    },
                                    |t| {
                                        t.leaf(ElementDecl {
                                            key: Some(UiKey::OverlayFieldLabel(index)),
                                            sizing: SizingAxes::new(
                                                Sizing::GROW,
                                                Sizing::Fixed(label_h as f32),
                                            ),
                                            ..Default::default()
                                        });
                                        t.leaf(ElementDecl {
                                            key: Some(UiKey::OverlayFieldInput(index)),
                                            sizing: SizingAxes::new(
                                                Sizing::GROW,
                                                Sizing::Fixed(input_h as f32),
                                            ),
                                            ..Default::default()
                                        });
                                    },
                                );
                            }
                        },
                    );
                    spacer(t, pad_y);
                    if let Some(height) = footer_h {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayFooter),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
                            ..Default::default()
                        });
                    }
                }
                Body::Zones(zones) => {
                    let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
                    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
                    let gap = scaled(dims::ZONE_GAP, scale_factor);
                    let plan = zone_plan
                        .as_ref()
                        .expect("zones always have a measured plan");
                    let banner_h = plan.banner.as_ref().map(|banner| banner.h);
                    let code_h = plan.code.as_ref().map(|(_, height)| *height);
                    let text_h = plan.text.as_ref().map(|(_, _, height)| *height);

                    if let Some(height) = banner_h {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayZoneBanner),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
                            ..Default::default()
                        });
                    } else if code_h.is_none() {
                        spacer(t, pad_y);
                    }
                    if let Some(height) = code_h {
                        if banner_h.is_some() {
                            spacer(t, gap);
                        }
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayZoneCode),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
                            padding: Padding::xy(pad_x as f32, 0.0),
                            ..Default::default()
                        });
                    }
                    if let Some(height) = text_h {
                        if plan.gap {
                            spacer(
                                t,
                                if plan.viewport.is_some() {
                                    scaled(dims::ZONE_LINE_H, scale_factor)
                                } else {
                                    gap
                                },
                            );
                        } else if banner_h.is_some() && code_h.is_none() {
                            spacer(t, gap);
                        }
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayZoneText),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height as f32)),
                            padding: Padding::xy(pad_x as f32, 0.0),
                            ..Default::default()
                        });
                    }
                    spacer(
                        t,
                        if zones.banner.is_some() {
                            2 * pad_y
                        } else {
                            pad_y
                        },
                    );
                }
            },
        );
        // The docs card: a second float attached to the panel's top-right
        // (declared after it, so the solver sees the panel's solved rect),
        // flipping to its left when the window lacks room on the right.
        if docs_viewport.is_some() {
            let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
            let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
            let line_h = scaled(dims::ZONE_LINE_H, scale_factor);
            t.node(
                ElementDecl {
                    key: Some(UiKey::OverlayDocsPanel),
                    dir: Dir::Column,
                    sizing: SizingAxes::new(Sizing::Fixed(docs_w as f32), Sizing::FIT),
                    clip: true,
                    float: Some(FloatDecl {
                        anchor: FloatAnchor::Element {
                            target: UiKey::OverlayPanel,
                            attach: AttachPoint::RightTop,
                        },
                        z: 10,
                        width: None,
                    }),
                    ..Default::default()
                },
                |t| {
                    if docs_code_plan.is_none() {
                        spacer(t, pad_y);
                    }
                    if let Some((_, code_h)) = &docs_code_plan {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayDocsCode),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(*code_h as f32)),
                            padding: Padding::xy(pad_x as f32, 0.0),
                            ..Default::default()
                        });
                    }
                    if docs_gap {
                        spacer(t, line_h);
                    }
                    if let Some((_, _, text_h)) = &docs_plan {
                        t.leaf(ElementDecl {
                            key: Some(UiKey::OverlayDocsText),
                            sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(*text_h as f32)),
                            padding: Padding::xy(pad_x as f32, 0.0),
                            ..Default::default()
                        });
                    }
                    spacer(t, pad_y);
                },
            );
        }
    });

    let snapshot = tree.solve(
        Rect::new(0.0, 0.0, window_width as f32, window_height as f32),
        scale_factor,
        measure,
    );
    let panel = solved_rect(&snapshot, UiKey::OverlayPanel)
        .expect("overlay tree always declares its panel");
    let tab_bar = solved_rect(&snapshot, UiKey::OverlayTabBar);
    let tab_rects = spec
        .tabs
        .as_ref()
        .map(|tabs| {
            (0..tabs.tabs.len())
                .map(|index| {
                    solved_rect(&snapshot, UiKey::OverlayTab(index))
                        .expect("overlay tree declares every requested tab")
                })
                .collect()
        })
        .unwrap_or_default();
    let header = solved_rect(&snapshot, UiKey::OverlayHeader);
    let rows: Vec<WidgetRect> = snapshot
        .row_list(UiKey::OverlayRows)
        .map(|rows| {
            rows.drawn_range()
                .map(|index| {
                    widget_rect(
                        rows.row_rect(index)
                            .expect("drawn overlay row always has a solved rectangle"),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let fields = match &spec.body {
        Body::Fields { fields, .. } => (0..fields.len())
            .map(|index| FieldLayout {
                label: solved_rect(&snapshot, UiKey::OverlayFieldLabel(index))
                    .expect("overlay tree declares every field label"),
                input: solved_rect(&snapshot, UiKey::OverlayFieldInput(index))
                    .expect("overlay tree declares every field input"),
            })
            .collect(),
        Body::List { .. } | Body::Zones(_) => Vec::new(),
    };
    let zones_banner = solved_rect(&snapshot, UiKey::OverlayZoneBanner);
    let zones_code = solved_content_rect(&snapshot, UiKey::OverlayZoneCode);
    let zones_text = solved_content_rect(&snapshot, UiKey::OverlayZoneText);
    let zone_viewport = zone_plan.as_ref().and_then(|plan| plan.viewport);
    let docs_viewport = docs_viewport.or(zone_viewport);
    let docs_panel =
        solved_rect(&snapshot, UiKey::OverlayDocsPanel).or_else(|| zone_viewport.map(|_| panel));
    let docs_text = solved_content_rect(&snapshot, UiKey::OverlayDocsText);
    let docs_code = solved_content_rect(&snapshot, UiKey::OverlayDocsCode);
    let docs_scrollbar = docs_panel.zip(docs_viewport).and_then(|(panel, viewport)| {
        (viewport.max_scroll() > 0).then(|| {
            // Keep the shared scrollbar inside the card's rounded corners.
            // Horizontal text padding already leaves room for the track.
            let inset = scaled(dims::PANEL_PAD_Y, scale_factor).min(panel.h / 2);
            list_scrollbar(
                WidgetRect {
                    y: panel.y + inset,
                    h: panel.h.saturating_sub(2 * inset),
                    ..panel
                },
                ScrollbarState::new(viewport.total, viewport.visible, viewport.scroll),
                scale_factor,
            )
        })
    });
    let footer = solved_rect(&snapshot, UiKey::OverlayFooter);
    let scrollbar = list_info.and_then(|(start, visible, total, max_visible)| {
        if total <= max_visible {
            return None;
        }
        let track = solved_rect(&snapshot, UiKey::OverlayRows)?;
        Some(list_scrollbar(
            track,
            ScrollbarState::new(total, visible, start),
            scale_factor,
        ))
    });

    let mut choices = Vec::new();
    if let Some((start, _, _, _)) = list_info {
        for (entry, rect) in display_rows.iter().skip(start).zip(&rows) {
            if let DisplayRow::Row(row, index) = entry {
                if let Accessory::Choices { labels, .. } = &row.accessory {
                    choices.push((*index, choice_rects(*rect, labels.len(), scale_factor)));
                }
            }
        }
    }
    let mut result = OverlayLayout {
        scale_factor,
        snapshot,
        choices,
        panel,
        tab_bar,
        tab_rects,
        header,
        row_height: row_h,
        rows,
        settings_viewport: None,
        settings_items: Vec::new(),
        settings_positions: Vec::new(),
        fields,
        zones_banner,
        zones_code,
        zones_text,
        docs_panel,
        docs_text,
        docs_code,
        footer,
        scrollbar,
        zone_plan,
        docs_plan,
        docs_code_plan,
        docs_scrollbar,
        docs_viewport,
    };
    if matches!(spec.anchor, Anchor::Settings { .. }) {
        settings_page::layout(spec, &mut result, window_width, window_height, scale_factor);
    }
    result
}

/// Where a point landed within a rendered `OverlaySpec`/`OverlayLayout` —
/// consumed by `hit_test::hit_test_modal`, which builds the exact same spec
/// the renderer draws and calls `layout()` + this function against it (one
/// layout, two consumers, so a click is always tested against the geometry
/// actually painted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayHit {
    Close,
    Input {
        row: FlatIndex,
        position: crate::editable::Position,
    },
    Scrollbar,
    /// Outside the panel entirely (dismiss on click).
    Outside,
    /// A selectable row (`Body::List` only).
    Row(FlatIndex),
    Choice {
        row: FlatIndex,
        choice: usize,
    },
    /// An available tab (`Unavailable` tabs are unclickable — the click
    /// lands as `Inside` instead, per Visual Language > TabCount states).
    Tab(usize),
    /// Inside the panel but not on a specific row (header, footer, section
    /// header, padding, a `Fields`/`Zones` body) — consumed, no action.
    Inside,
    Documentation {
        viewport: DocumentationViewport,
    },
}

/// Hit-test a point (physical px) against a laid-out `OverlaySpec`.
pub fn hit_test(spec: &OverlaySpec, layout: &OverlayLayout, x: usize, y: usize) -> OverlayHit {
    if layout
        .scrollbar
        .as_ref()
        .is_some_and(|bar| bar.needed && bar.hits_track(x as f32, y as f32))
    {
        return OverlayHit::Scrollbar;
    }
    if matches!(spec.anchor, Anchor::Settings { .. }) {
        return settings_page::hit_test(spec, layout, x, y);
    }
    if let (Some(panel), Some(viewport)) = (layout.docs_panel, layout.docs_viewport) {
        if x >= panel.x && x < panel.x + panel.w && y >= panel.y && y < panel.y + panel.h {
            return OverlayHit::Documentation { viewport };
        }
    }
    for (row, choices) in &layout.choices {
        for (choice, rect) in choices.iter().enumerate() {
            if x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h {
                return OverlayHit::Choice { row: *row, choice };
            }
        }
    }
    match layout.snapshot.hit(x as f32, y as f32) {
        Some(UiKey::OverlayTab(index)) => {
            let available = spec
                .tabs
                .as_ref()
                .and_then(|tabs| tabs.tabs.get(index))
                .map(|(_, count)| count.is_available())
                .unwrap_or(false);
            if available {
                OverlayHit::Tab(index)
            } else {
                OverlayHit::Inside
            }
        }
        Some(UiKey::OverlayRows) => {
            let Body::List {
                sections,
                scroll,
                max_visible,
                ..
            } = &spec.body
            else {
                return OverlayHit::Inside;
            };
            let display_rows = flatten_rows(sections);
            let (start, _) = resolve_visible_window(&display_rows, *scroll, *max_visible);
            let Some(slot) = layout
                .snapshot
                .row_list(UiKey::OverlayRows)
                .and_then(|rows| rows.row_at_y(y as f32))
            else {
                return OverlayHit::Inside;
            };
            match display_rows.get(start + slot) {
                Some(DisplayRow::Row(_, flat_index)) => OverlayHit::Row(*flat_index),
                Some(DisplayRow::SectionHeader(_) | DisplayRow::Separator) | None => {
                    OverlayHit::Inside
                }
            }
        }
        Some(
            UiKey::OverlayPanel
            | UiKey::OverlayTabBar
            | UiKey::OverlayHeader
            | UiKey::OverlayFieldLabel(_)
            | UiKey::OverlayFieldInput(_)
            | UiKey::OverlayZoneBanner
            | UiKey::OverlayZoneCode
            | UiKey::OverlayZoneText
            | UiKey::OverlayDocsPanel
            | UiKey::OverlayDocsText
            | UiKey::OverlayDocsCode
            | UiKey::OverlayFooter,
        ) => OverlayHit::Inside,
        Some(
            UiKey::EditorTabBar(_)
            | UiKey::EditorTab(_, _)
            | UiKey::PreviewPane(_)
            | UiKey::PreviewHeader(_)
            | UiKey::PreviewContent(_)
            | UiKey::Dock(_)
            | UiKey::DockHeader(_)
            | UiKey::DockTab(_, _)
            | UiKey::PanelContent(_)
            | UiKey::PanelRows(_)
            | UiKey::Sidebar
            | UiKey::EditorArea
            | UiKey::StatusBar,
        )
        | Some(UiKey::TerminalTabs | UiKey::TerminalTabViewport | UiKey::TerminalAction(_))
        | None => OverlayHit::Outside,
    }
}

/// Resolved colors pulled once from `OverlayTheme` per render call.
struct Palette {
    scrollbar: ScrollbarColors,
    syntax: crate::theme::SyntaxTheme,
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
    fn from_theme(theme: &crate::theme::Theme) -> Self {
        let scrollbar = ScrollbarColors::from(&theme.scrollbar);
        let syntax = theme.syntax.clone();
        let theme = &theme.overlay;
        Self {
            scrollbar,
            syntax,
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

    /// Banner ground: the severity color at 15% over the panel — the SAME
    /// mix `theme.rs::banner_ground` calibrates `severity_*_text` against.
    /// Filling with the raw severity color made the calibrated text
    /// illegible on a full-strength band.
    fn severity_wash(&self, severity: Severity) -> u32 {
        let raw = match severity {
            Severity::Error => self.severity_error,
            Severity::Warning => self.severity_warning,
            Severity::Info => self.severity_info,
            Severity::Hint => self.severity_hint,
        };
        super::frame::blend_colors(self.panel_bg, raw, 0.15)
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

impl MenuItemKind {
    /// Badge background, pre-blended to opaque over the panel (Visual
    /// Language > Colors: "syntax color @ 20% over panel"). Colors are
    /// picked from the existing overlay palette rather than new
    /// `overlay.kind_*` theme keys. Methods share the function palette.
    fn badge_color(self, colors: &Palette) -> u32 {
        let source = match self {
            Self::Function | Self::Method => colors.accent,
            Self::Variable => colors.severity_info,
            Self::Type => colors.accent_bright,
            Self::Keyword => colors.severity_warning,
            Self::Field => colors.text_dim,
            Self::Module | Self::Folder => colors.keycap_fg,
            Self::File => colors.text_dim,
            Self::Constant => colors.severity_error,
            Self::Other => colors.text_dim,
        };
        blend_opaque(source, colors.panel_bg, 20)
    }
}

/// Dim the backdrop without blending pixels the following opaque panel fill
/// will overwrite. Keep a conservative inset so rounded/antialiased edges
/// still see the dimmed background. Translucent panels need the full backdrop.
/// The caller must paint this panel under the same clip after this call.
fn render_backdrop(
    frame: &mut Frame,
    panel: WidgetRect,
    radius: usize,
    panel_color: u32,
    dim_alpha: u8,
) {
    if dim_alpha == 0 {
        return;
    }
    if panel_color >> 24 != 255 {
        return frame.dim(dim_alpha);
    }

    let (width, height) = (frame.width(), frame.height());
    let x0 = panel.x.saturating_add(radius).min(width);
    let y0 = panel.y.saturating_add(radius).min(height);
    let x1 = panel
        .x
        .saturating_add(panel.w.saturating_sub(radius))
        .min(width);
    let y1 = panel
        .y
        .saturating_add(panel.h.saturating_sub(radius))
        .min(height);
    if x0 >= x1 || y0 >= y1 {
        return frame.dim(dim_alpha);
    }

    // Four disjoint bands leave only the panel's guaranteed opaque interior
    // untouched. The existing rectangle primitive applies the active clip.
    let color = u32::from(dim_alpha) << 24;
    for (x, y, w, h) in [
        (0, 0, width, y0),
        (0, y1, width, height - y1),
        (0, y0, x0, y1 - y0),
        (x1, y0, width - x1, y1 - y0),
    ] {
        frame.blend_rect_px(x, y, w, h, color);
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
    theme: &crate::theme::Theme,
    spec: &OverlaySpec,
    window_width: usize,
    window_height: usize,
    scale_factor: f64,
    cursor_visible: bool,
) {
    // Measure through the glyph cache so zone wrapping uses real advances;
    // the borrow ends before painting begins.
    let layout = {
        let mut measure = crate::layout::PainterMeasure::new(painter);
        layout_measured(
            spec,
            window_width,
            window_height,
            scale_factor,
            &mut measure,
        )
    };
    let mut colors = Palette::from_theme(theme);
    // Reading surfaces must separate documentation from the editor beneath.
    // Keep translucency on other overlays as configured by the theme.
    if matches!(spec.body, Body::Zones(_)) && matches!(spec.anchor, Anchor::Cursor { .. }) {
        colors.panel_bg |= 0xFF00_0000;
    }
    if matches!(spec.anchor, Anchor::Settings { .. }) {
        settings_page::render(
            frame,
            painter,
            mask_cache,
            &colors,
            theme,
            spec,
            &layout,
            scale_factor,
            cursor_visible,
        );
        return;
    }
    let radius = match &spec.anchor {
        Anchor::Centered { .. } | Anchor::Settings { .. } => scaled(dims::RADIUS, scale_factor),
        Anchor::Cursor { .. } | Anchor::Menu { .. } => scaled(dims::RADIUS_CURSOR, scale_factor),
    };

    if let Anchor::Centered { dim_alpha, .. } = &spec.anchor {
        render_backdrop(frame, layout.panel, radius, colors.panel_bg, *dim_alpha);
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
        Body::Zones(zones) => render_zones(
            frame,
            painter,
            &colors,
            zones,
            &layout,
            scale_factor,
            radius,
            mask_cache,
        ),
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
            colors.recessed_wash,
            mask_cache,
        );
    }
    if let Some(panel) = layout.docs_panel.filter(|_| spec.docs.is_some()) {
        frame.draw_shadow_rings(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            radius,
            scale_factor,
            mask_cache,
        );
        frame.fill_rounded_rect(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            radius,
            colors.panel_bg | 0xFF00_0000,
            mask_cache,
        );
        frame.stroke_rounded_rect(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            radius,
            colors.hairline,
            mask_cache,
        );
        frame.set_clip(crate::model::editor_area::Rect {
            x: panel.x as f32,
            y: panel.y as f32,
            width: panel.w as f32,
            height: panel.h as f32,
        });
        if let (Some(code), Some((lines, _))) = (layout.docs_code, layout.docs_code_plan.as_ref()) {
            render_code_band(
                frame,
                painter,
                &colors,
                panel,
                code,
                lines,
                scale_factor,
                radius,
                mask_cache,
            );
        }
        if let (Some(text), Some((lines, truncated, _))) =
            (layout.docs_text, layout.docs_plan.as_ref())
        {
            draw_text_lines(
                frame,
                painter,
                &colors,
                text,
                lines,
                *truncated,
                crate::layout::TextStyle::sized(SIZE_ROW),
                colors.text_primary,
                scale_factor,
            );
        }
        frame.clear_clip();
    }
    if let Some(bar) = &layout.docs_scrollbar {
        render_scrollbar(frame, bar, false, &colors.scrollbar);
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
    // Editable headers (search, Settings, pickers) share the code font with
    // every other text input. Non-editable headings retain UI typography.
    let mut painter = painter.with_font(if header.caret.is_none() {
        FontRole::Ui
    } else {
        FontRole::Code
    });

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
    frame.push_clip(crate::model::editor_area::Rect {
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
            visible_header_text(&mut painter, size, header.text, content_w as f32);
        // Selection wash first, so the text paints over it. Columns are in
        // the full text's char space — re-express against the visible
        // string the same way the caret is.
        if let Some((sel_start, sel_end)) = header.selection {
            if sel_end > sel_start {
                let to_visible =
                    |col: usize| col.saturating_sub(kept_from) + usize::from(kept_from > 0);
                let x0 = caret_x_for_column(&mut painter, x, &visible, to_visible(sel_start), size);
                let x1 = caret_x_for_column(&mut painter, x, &visible, to_visible(sel_end), size);
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
            let caret_x = caret_x_for_column(&mut painter, x, &visible, visible_col, size);
            let caret_w = scaled(1.5, scale_factor);
            frame.fill_rect_px(caret_x, text_y, caret_w, text_h, colors.accent_bright);
        }
    }

    frame.pop_clip();

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

    let is_cursor = matches!(spec.anchor, Anchor::Cursor { .. } | Anchor::Menu { .. });
    let keycap_scale = keycap_scale(spec, scale_factor);
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
    let icon_w = list_icon_width(sections, scale_factor);
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
            DisplayRow::Separator => {
                let text_pad = scaled(dims::ROW_TEXT_PAD_X, scale_factor);
                let y = rect.y + rect.h / 2;
                frame.fill_rect_px(
                    rect.x + text_pad,
                    y,
                    rect.w.saturating_sub(2 * text_pad),
                    1,
                    colors.hairline,
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
                        let glyph = kind.badge_glyph().encode_utf8(&mut buf);
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
                let choice_rects = layout
                    .choices
                    .iter()
                    .find(|(id, _)| id == flat_index)
                    .map(|(_, rects)| rects.as_slice())
                    .unwrap_or_default();
                let accessory_w = choice_rects.first().map_or_else(
                    || {
                        if let Accessory::Keycaps(steps) = &row.accessory {
                            keycaps_width(painter, steps, keycap_scale)
                        } else {
                            accessory_width(painter, &row.accessory, meta_size, scale_factor)
                        }
                    },
                    |first| (rect.x + rect.w).saturating_sub(inset + text_pad + first.x),
                );
                let label_right = (rect.x + rect.w.saturating_sub(inset + text_pad + accessory_w))
                    .saturating_sub(if accessory_w > 0 { text_pad } else { 0 });
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
                            let detail = painter.truncate_sized(
                                detail,
                                meta_size,
                                leftover,
                                EllipsisSide::Start,
                            );
                            let run = StyledLine {
                                runs: row
                                    .detail_style
                                    .map(|style| vec![(0..detail.len(), style)])
                                    .unwrap_or_default(),
                                text: detail.into_owned(),
                            };
                            draw_styled_run(
                                frame,
                                painter,
                                colors,
                                detail_x,
                                text_y,
                                &run,
                                crate::layout::TextStyle::sized(meta_size),
                                colors.text_dim,
                                painter.line_height_for_size(meta_size),
                                scale_factor,
                                false,
                            );
                        }
                    }
                } else {
                    let label = painter.truncate_sized(
                        row.label,
                        row_size,
                        available as f32,
                        EllipsisSide::End,
                    );
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
                        Accessory::Choices { labels, active } => {
                            for (index, (label, chip)) in
                                labels.iter().zip(choice_rects).enumerate()
                            {
                                if chip.w == 0 || chip.h == 0 {
                                    continue;
                                }
                                let selected = *active == Some(index);
                                frame.fill_rounded_rect(
                                    chip.x,
                                    chip.y,
                                    chip.w,
                                    chip.h,
                                    scaled(4.0, scale_factor),
                                    if selected {
                                        colors.accent
                                    } else {
                                        colors.keycap_border
                                    },
                                    mask_cache,
                                );
                                if !selected && chip.w > 2 && chip.h > 2 {
                                    frame.fill_rounded_rect(
                                        chip.x + 1,
                                        chip.y + 1,
                                        chip.w - 2,
                                        chip.h - 2,
                                        scaled(4.0, scale_factor).saturating_sub(1),
                                        colors.keycap_bg,
                                        mask_cache,
                                    );
                                }
                                let pad = scaled(4.0, scale_factor).min(chip.w / 2);
                                let label = painter.truncate_sized(
                                    label,
                                    meta_size,
                                    chip.w.saturating_sub(pad * 2) as f32,
                                    EllipsisSide::End,
                                );
                                if painter.measure_sized(&label, meta_size, 0.0)
                                    <= chip.w.saturating_sub(pad * 2) as f32
                                    && painter.line_height_for_size(meta_size) <= chip.h
                                {
                                    painter.draw_sized(
                                        frame,
                                        chip.x + pad,
                                        chip.y
                                            + (chip.h - painter.line_height_for_size(meta_size))
                                                / 2,
                                        &label,
                                        meta_size,
                                        0.0,
                                        if selected {
                                            colors.text_bright
                                        } else {
                                            colors.keycap_fg
                                        },
                                    );
                                }
                            }
                        }
                        Accessory::SettingInput { .. } => {}
                        Accessory::DimText(text) | Accessory::SettingValue { text, .. } => {
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
                            let chip_h = super::frame::keycap_height(painter, keycap_scale);
                            let chip_y = rect.y + (rect.h.saturating_sub(chip_h)) / 2;
                            let chip_gap = scaled(dims::CHIP_GAP, keycap_scale);
                            let step_gap = scaled(dims::CHIP_STEP_GAP, keycap_scale);
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
                                        keycap_scale,
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

    render_list_scrollbar(frame, layout, colors);
}

/// Overlay surfaces share the editor scrollbar's width, geometry and paint primitive.
fn list_scrollbar(body: WidgetRect, state: ScrollbarState, scale: f64) -> ScrollbarGeometry {
    let width = scaled(SCROLLBAR_WIDTH_LOGICAL as f32, scale).min(body.w);
    ScrollbarGeometry::vertical(
        Rect::new(
            (body.x + body.w - width) as f32,
            body.y as f32,
            width as f32,
            body.h as f32,
        ),
        &state,
    )
}

fn render_list_scrollbar(frame: &mut Frame, layout: &OverlayLayout, colors: &Palette) {
    if let Some(bar) = &layout.scrollbar {
        render_scrollbar(frame, bar, false, &colors.scrollbar);
    }
}

/// Equal-width chips shrink within the accessory budget on narrow windows.
/// Labels may truncate, but every choice keeps its own exact hit target.
fn choice_rects(row: WidgetRect, count: usize, scale: f64) -> Vec<WidgetRect> {
    if count == 0 {
        return Vec::new();
    }
    let pad = scaled(dims::ROW_INSET + dims::ROW_TEXT_PAD_X, scale).min(row.w / 2);
    let budget = row.w.saturating_sub(pad * 2) * 2 / 3;
    let gap = scaled(dims::CHIP_GAP, scale).min(budget / count);
    let width = scaled(72.0, scale).min(budget.saturating_sub(gap * (count - 1)) / count);
    let height = scaled(22.0, scale).min(row.h);
    let total = width * count + gap * (count - 1);
    let start = row.x + row.w.saturating_sub(pad + total);
    (0..count)
        .map(|i| WidgetRect {
            x: start + i * (width + gap),
            y: row.y + (row.h - height) / 2,
            w: width,
            h: height,
        })
        .collect()
}

fn list_icon_width(sections: &[Section<'_>], scale_factor: f64) -> usize {
    if sections
        .iter()
        .flat_map(|section| section.rows)
        .any(|row| !matches!(row.icon, RowIcon::None))
    {
        scaled(dims::ROW_ICON_W, scale_factor)
    } else {
        0
    }
}

fn keycap_scale(spec: &OverlaySpec, scale_factor: f64) -> f64 {
    scale_factor
        * if matches!(spec.anchor, Anchor::Menu { .. }) {
            0.8
        } else {
            1.0
        }
}

/// Measure the same label, accessory and padding that `render_list` paints.
fn menu_content_width(
    spec: &OverlaySpec,
    scale_factor: f64,
    measure: &mut dyn crate::layout::TextMeasure,
) -> usize {
    let Body::List { sections, .. } = &spec.body else {
        return 0;
    };
    let row_style = crate::layout::TextStyle::sized(size_px(SIZE_ROW, scale_factor));
    let meta_style = crate::layout::TextStyle::sized(size_px(SIZE_META, scale_factor));
    let pad = scaled(dims::ROW_TEXT_PAD_X, scale_factor);
    let edges = 2 * (scaled(dims::ROW_INSET_CURSOR, scale_factor) + pad);
    let icon_w = list_icon_width(sections, scale_factor);
    sections
        .iter()
        .flat_map(|section| section.rows)
        .map(|row| {
            let accessory = match &row.accessory {
                Accessory::Keycaps(steps) => {
                    keycaps_width(measure, steps, keycap_scale(spec, scale_factor))
                }
                Accessory::DimText(text) => measure.width(text, meta_style).ceil() as usize,
                _ => 0, // Context menus only carry shortcut hints.
            };
            edges
                + icon_w
                + measure.width(row.label, row_style).ceil() as usize
                + accessory
                + if accessory > 0 { pad } else { 0 }
        })
        .max()
        .unwrap_or(edges)
}

fn accessory_width(
    painter: &mut TextPainter,
    accessory: &Accessory,
    meta_size: f32,
    scale_factor: f64,
) -> usize {
    match accessory {
        // Choice widths are solved with the row, not independently measured.
        Accessory::Choices { .. } => 0,
        Accessory::None => 0,
        Accessory::SettingInput { .. } => 0,
        Accessory::DimText(text) | Accessory::SettingValue { text, .. } => {
            painter.measure_sized(text, meta_size, 0.0).ceil() as usize
        }
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
fn keycaps_width(
    measure: &mut dyn crate::layout::TextMeasure,
    steps: &[Vec<Chip>],
    scale_factor: f64,
) -> usize {
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
            w += super::frame::keycap_width(measure, &chip.label, scale_factor);
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
    background: u32,
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
        background,
        mask_cache,
    );

    let size = size_px(SIZE_META, scale_factor);
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
    let text_y = rect.y + (rect.h.saturating_sub(painter.line_height_for_size(size))) / 2;
    let room = rect.w.saturating_sub(pad_x * 2);
    let leading = fit_with_ellipsis(painter, footer.leading, size, room);
    let leading_w = painter.measure_sized(&leading, size, 0.0).ceil() as usize;
    if leading_w > room {
        return;
    }

    painter.draw_sized(
        frame,
        rect.x + pad_x,
        text_y,
        &leading,
        size,
        0.0,
        colors.text_dim,
    );

    let trailing_w = painter.measure_sized(footer.trailing, size, 0.0).ceil() as usize;
    // Secondary hints yield to navigation hints on narrow windows.
    if leading_w + scaled(8.0, scale_factor) + trailing_w > room {
        return;
    }
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

/// `text`, or its longest prefix plus "…" that measures within `room`.
fn fit_with_ellipsis(painter: &mut TextPainter, text: &str, size: f32, room: usize) -> String {
    if painter.measure_sized(text, size, 0.0).ceil() as usize <= room {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    for keep in (1..chars.len()).rev() {
        let candidate: String = chars[..keep].iter().collect::<String>() + "\u{2026}";
        if painter.measure_sized(&candidate, size, 0.0).ceil() as usize <= room {
            return candidate;
        }
    }
    "\u{2026}".to_owned()
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
        if let Some(trailing) = field.trailing {
            let meta = size_px(SIZE_META, scale_factor);
            let meta_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(meta))) / 2;
            // Never run into the label: shorten with an ellipsis to the
            // room left of it.
            let label_w = painter.measure_sized(field.label, size, 0.0).ceil() as usize;
            let room =
                r.w.saturating_sub(label_w + scaled(dims::HEADER_PAD_X, scale_factor));
            let text = fit_with_ellipsis(painter, trailing, meta, room);
            let w = painter.measure_sized(&text, meta, 0.0).ceil() as usize;
            let color = if field.trailing_is_error {
                colors.severity_error_text
            } else {
                colors.text_dim
            };
            let x = r.x + r.w.saturating_sub(w);
            painter.draw_sized(frame, x, meta_y, &text, meta, 0.0, color);
        }
    }
}

/// Draw the single centered text block of a `Body::Zones` context (drop
/// overlay).
/// Render a `Body::Zones` context: the drop overlay's single centered
/// message (`text` only, no `banner`/`code`) and the hover card (banner +
/// code + text) share this one paint path.
#[allow(clippy::too_many_arguments)]
fn render_zones(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    zones: &Zones,
    layout: &OverlayLayout,
    scale_factor: f64,
    radius: usize,
    mask_cache: &mut RoundedRectMaskCache,
) {
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
    let gap = scaled(dims::ZONE_GAP, scale_factor);
    let line_h = scaled(dims::ZONE_LINE_H, scale_factor);
    // The one measured plan, computed in `layout_measured` and carried on
    // the layout — never re-derived here.
    let Some(plan) = layout.zone_plan.as_ref() else {
        return;
    };

    // Hard backstop: nothing in any zone may paint outside the panel.
    frame.set_clip(crate::model::editor_area::Rect {
        x: layout.panel.x as f32,
        y: layout.panel.y as f32,
        width: layout.panel.w as f32,
        height: layout.panel.h as f32,
    });

    if let (Some((severity, _, source)), Some(bp), Some(r)) =
        (zones.banner, plan.banner.as_ref(), layout.zones_banner)
    {
        let wash = colors.severity_wash(severity);
        if r.y == layout.panel.y {
            // The banner is always the first zone (Zones has no header,
            // `layout()`'s Zones arm hardcodes `header: None`), so it
            // sits flush against the panel's rounded top edge — a plain
            // `fill_rect_px` would square off the antialiased corners
            // `fill_rounded_rect` deliberately left alone (matches
            // `render_tab_bar`'s identical top-corner note).
            frame.fill_rect_top_rounded(r.x, r.y, r.w, r.h, radius, wash, mask_cache);
        } else {
            frame.fill_rect_px(r.x, r.y, r.w, r.h, wash);
        }
        let size = size_px(SIZE_ROW, scale_factor);
        let text_color = colors.severity_text(severity);
        let top = r.y
            + (gap / 2).max(
                r.h.saturating_sub((bp.lines.len() + usize::from(bp.truncated)).max(1) * line_h)
                    / 2,
            );
        let mut buf = [0u8; 4];
        let glyph_w = painter.draw_sized(
            frame,
            r.x + pad_x,
            top,
            severity.glyph().encode_utf8(&mut buf),
            size,
            0.0,
            text_color,
        );
        let msg_x = r.x + pad_x + glyph_w.ceil() as usize + pad_x / 2;
        for (i, line) in bp.lines.iter().enumerate() {
            draw_styled_line(
                frame,
                painter,
                colors,
                msg_x,
                top + i * line_h,
                line,
                crate::layout::TextStyle::sized(size),
                text_color,
                scale_factor,
            );
        }
        if bp.truncated {
            painter.draw_sized(
                frame,
                msg_x,
                top + bp.lines.len() * line_h,
                "\u{2026}",
                size,
                0.0,
                text_color,
            );
        }
        if !source.is_empty() {
            let meta_size = size_px(SIZE_META, scale_factor);
            let source_w = painter.measure_sized(source, meta_size, 0.0).ceil() as usize;
            let source_x = r.x + r.w.saturating_sub(pad_x + source_w);
            painter.draw_sized(
                frame,
                source_x,
                top,
                source,
                meta_size,
                0.0,
                colors.text_dim,
            );
        }
    }

    if let (Some((lines, _)), Some(r)) = (plan.code.as_ref(), layout.zones_code) {
        render_code_band(
            frame,
            painter,
            colors,
            layout.panel,
            r,
            lines,
            scale_factor,
            radius,
            mask_cache,
        );
    }

    if let (Some(_), Some((lines, truncated, _)), Some(r)) =
        (zones.text, plan.text.as_ref(), layout.zones_text)
    {
        if should_center_zone_text(zones, lines, *truncated) {
            // Single-line, zone-only body (drop overlay): centered.
            let size = size_px(SIZE_INPUT, scale_factor);
            let text = &lines[0];
            let text_w = painter.measure_sized(&text.text, size, 0.0).ceil() as usize;
            let text_x = r.x + (r.w.saturating_sub(text_w)) / 2;
            let text_y = r.y + (r.h.saturating_sub(painter.line_height_for_size(size))) / 2;
            draw_styled_line(
                frame,
                painter,
                colors,
                text_x,
                text_y,
                text,
                crate::layout::TextStyle::sized(size),
                colors.text_primary,
                scale_factor,
            );
        } else {
            draw_text_lines(
                frame,
                painter,
                colors,
                r,
                lines,
                *truncated,
                crate::layout::TextStyle::sized(if zones.center_text {
                    SIZE_INPUT
                } else {
                    SIZE_ROW
                }),
                colors.text_primary,
                scale_factor,
            );
        }
    }

    frame.clear_clip();
}

/// The same full-width, padded signature header for hover and completion docs.
/// The solved code box keeps its horizontal text inset; only its wash bleeds.
#[allow(clippy::too_many_arguments)]
fn render_code_band(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    panel: WidgetRect,
    code: WidgetRect,
    lines: &[StyledLine],
    scale_factor: f64,
    radius: usize,
    mask_cache: &mut RoundedRectMaskCache,
) {
    if code.y == panel.y {
        frame.fill_rect_top_rounded(
            panel.x,
            code.y,
            panel.w,
            code.h,
            radius,
            colors.panel_secondary,
            mask_cache,
        );
    } else {
        frame.fill_rect_px(panel.x, code.y, panel.w, code.h, colors.panel_secondary);
    }
    let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
    let content = WidgetRect {
        y: code.y + pad_y,
        h: code.h.saturating_sub(2 * pad_y),
        ..code
    };
    draw_text_lines(
        frame,
        painter,
        colors,
        content,
        lines,
        false,
        crate::layout::TextStyle {
            code: true,
            ..crate::layout::TextStyle::sized(SIZE_ROW)
        },
        colors.text_primary,
        scale_factor,
    );
}

fn should_center_zone_text(zones: &Zones<'_>, lines: &[StyledLine], truncated: bool) -> bool {
    zones.center_text
        && zones.banner.is_none()
        && zones.code.is_none()
        && lines.len() == 1
        && lines[0].runs.is_empty()
        && !truncated
}

/// Cap for non-interactive zones (notifications and signature-help calltips).
/// Interactive documentation uses a viewport instead of truncation.
const MAX_ZONE_TEXT_LINES: usize = 14;

/// Cap on wrapped banner-message lines.
const MAX_ZONE_BANNER_LINES: usize = 4;

/// Default documentation viewport height, before expansion.
const MAX_DOCS_LINES: usize = 12;

/// `(lines, truncated, height)` for one wrapped text zone.
pub(crate) type TextZonePlan = (Vec<StyledLine>, bool, usize);

/// The fully measured plan for a `Body::Zones` body: every wrapped line and
/// zone height, computed ONCE from `(zones, panel_w, scale)` and consumed by
/// both `layout()` and `render_zones` — the previous scheme derived heights
/// and line breaks independently in each, and every divergence was a
/// text-outside-the-panel bug (font-size-as-line-height, unwrapped text,
/// unwrapped banner).
pub(crate) struct ZonePlan {
    pub banner: Option<BannerPlan>,
    pub code: Option<(Vec<StyledLine>, usize)>,
    pub text: Option<(Vec<StyledLine>, bool, usize)>, // lines, truncated, height
    pub viewport: Option<DocumentationViewport>,
    pub gap: bool,
}

impl ZonePlan {
    fn scroll_documentation(
        &mut self,
        state: DocumentationState,
        window_h: usize,
        scale_factor: f64,
    ) {
        let line_h = scaled(dims::ZONE_LINE_H, scale_factor).max(1);
        let gap = scaled(dims::ZONE_GAP, scale_factor);
        let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
        // Reserve fixed banner/padding, code-band padding and window
        // margins. The separator already counts as a row in the shared window.
        // Capacity stays stable when the signature scrolls away.
        let chrome = self
            .banner
            .as_ref()
            .map_or(3 * pad_y, |banner| banner.h + gap + 4 * pad_y)
            + scaled(16.0, scale_factor);
        let available = window_h.saturating_sub(chrome) / line_h;
        let code = self.code.take().map_or_else(Vec::new, |(lines, _)| lines);
        let prose = self
            .text
            .take()
            .map_or_else(Vec::new, |(lines, _, _)| lines);
        let plan = window_documentation(code, prose, state, available, line_h, pad_y);
        self.code = plan.code;
        self.text = plan.text;
        self.viewport = plan.viewport;
        self.gap = plan.gap;
    }
}

pub(crate) struct BannerPlan {
    pub lines: Vec<StyledLine>,
    pub truncated: bool,
    pub h: usize,
}

/// One wrapped zone line: its text plus the styled runs that fall on it
/// (byte ranges relative to `text`, sorted, non-overlapping).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyledLine {
    pub text: String,
    pub runs: Vec<(std::ops::Range<usize>, SpanStyle)>,
}

/// Cuts `text` into `StyledLine`s along `ranges` (byte ranges into `text`),
/// attaching the part of each span that lands on each line.
fn styled_lines(
    text: &str,
    spans: &[Span],
    ranges: impl IntoIterator<Item = std::ops::Range<usize>>,
) -> Vec<StyledLine> {
    ranges
        .into_iter()
        .map(|r| StyledLine {
            text: text[r.clone()].to_owned(),
            runs: runs_in_spans(spans, r),
        })
        .collect()
}

/// One source of truth for documentation font roles in wrapping and painting.
fn documentation_style(
    mut base: crate::layout::TextStyle,
    span: Option<SpanStyle>,
) -> crate::layout::TextStyle {
    if base.code || span.is_some_and(SpanStyle::is_code) {
        base.code = true;
        base.size *= 0.92;
    }
    base
}

fn style_at(spans: &[Span], offset: usize) -> Option<SpanStyle> {
    spans
        .get(spans.partition_point(|span| span.range.end <= offset))
        .filter(|span| span.range.contains(&offset))
        .map(|span| span.style)
}

fn wrap_documentation(
    text: &str,
    spans: &[Span],
    style: crate::layout::TextStyle,
    width: f32,
    measure: &mut dyn crate::layout::TextMeasure,
) -> Vec<StyledLine> {
    styled_lines(
        text,
        spans,
        crate::layout::text::wrap_with_style(text, width, measure, |offset| {
            documentation_style(style, style_at(spans, offset))
        })
        .into_iter()
        .map(|line| line.range),
    )
}

pub(crate) fn plan_zones(
    zones: &Zones,
    panel_w: usize,
    scale_factor: f64,
    measure: &mut dyn crate::layout::TextMeasure,
) -> ZonePlan {
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
    let gap = scaled(dims::ZONE_GAP, scale_factor);
    let line_h = scaled(dims::ZONE_LINE_H, scale_factor);
    let content_w = panel_w.saturating_sub(2 * pad_x) as f32;
    let row_style = crate::layout::TextStyle::sized(size_px(SIZE_ROW, scale_factor));
    let text_style = if zones.center_text {
        crate::layout::TextStyle::sized(size_px(SIZE_INPUT, scale_factor))
    } else {
        row_style
    };
    // Minimum useful wrap width — the historical 8-cell floor.
    let min_wrap_w = size_px(8.0 * dims::ZONE_CELL_W, scale_factor);

    /// The banner shares its first line with the right-aligned source tag.
    fn wrap_ranges_with_first_width(
        text: &str,
        spans: &[Span],
        style: crate::layout::TextStyle,
        first_w: f32,
        later_w: f32,
        min_w: f32,
        measure: &mut dyn crate::layout::TextMeasure,
    ) -> Vec<std::ops::Range<usize>> {
        let first =
            crate::layout::text::wrap_with_style(text, first_w.max(min_w), measure, |offset| {
                documentation_style(style, style_at(spans, offset))
            });
        let Some(first_line) = first.first() else {
            return Vec::new();
        };
        let mut ranges = vec![first_line.range.clone()];
        let rest = &text[first_line.range.end..];
        let skipped = rest.len() - rest.trim_start_matches(char::is_whitespace).len();
        let offset = first_line.range.end + skipped;
        let remainder = &text[offset..];
        if !remainder.is_empty() {
            ranges.extend(
                crate::layout::text::wrap_with_style(
                    remainder,
                    later_w.max(min_w),
                    measure,
                    |byte| documentation_style(style, style_at(spans, offset + byte)),
                )
                .into_iter()
                .map(|line| line.range.start + offset..line.range.end + offset),
            );
        }
        ranges
    }

    let banner = zones.banner.map(|(severity, message, source)| {
        // Budget: the severity glyph plus its half-pad gap always; the
        // right-aligned source tag shares only the first line, so reserve its
        // width there without needlessly narrowing later lines.
        let mut buf = [0u8; 4];
        let glyph_w =
            measure.width(severity.glyph().encode_utf8(&mut buf), row_style) + (pad_x / 2) as f32;
        let source_w = if source.is_empty() {
            0.0
        } else {
            measure.width(source, row_style) + 2.0 * measure.width(" ", row_style)
        };
        let later_w = content_w - glyph_w;
        let first_w = later_w - source_w;
        let mut lines = styled_lines(
            message,
            zones.banner_spans,
            wrap_ranges_with_first_width(
                message,
                zones.banner_spans,
                row_style,
                first_w,
                later_w,
                min_wrap_w,
                measure,
            ),
        );
        let truncated = lines.len() > MAX_ZONE_BANNER_LINES;
        lines.truncate(MAX_ZONE_BANNER_LINES);
        let text_rows = lines.len() + usize::from(truncated);
        let h = (text_rows.max(1) * line_h + 2 * (gap / 2))
            .max(scaled(dims::ZONE_BANNER_H, scale_factor));
        BannerPlan {
            lines,
            truncated,
            h,
        }
    });

    let code = zones.code.map(|s| {
        let lines = wrap_documentation(
            s,
            zones.code_spans,
            crate::layout::TextStyle {
                code: true,
                ..row_style
            },
            content_w.max(min_wrap_w),
            measure,
        );
        let h = lines.len().max(1) * line_h + 2 * scaled(dims::PANEL_PAD_Y, scale_factor);
        (lines, h)
    });

    let text = zones.text.map(|s| {
        plan_text_zone(
            s,
            zones.text_spans,
            text_style,
            content_w,
            min_wrap_w,
            if zones.documentation.is_some() {
                usize::MAX
            } else {
                MAX_ZONE_TEXT_LINES
            },
            line_h,
            measure,
        )
    });

    let gap = code.is_some() && text.is_some();
    ZonePlan {
        banner,
        code,
        text,
        viewport: None,
        gap,
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_text_zone(
    text: &str,
    spans: &[Span],
    style: crate::layout::TextStyle,
    content_w: f32,
    min_wrap_w: f32,
    max_lines: usize,
    line_h: usize,
    measure: &mut dyn crate::layout::TextMeasure,
) -> TextZonePlan {
    let mut lines = wrap_documentation(text, spans, style, content_w.max(min_wrap_w), measure);
    let truncated = lines.len() > max_lines;
    lines.truncate(max_lines);
    let h = (lines.len() + usize::from(truncated)).max(1) * line_h;
    (lines, truncated, h)
}

#[derive(Default)]
struct DocumentationPlan {
    code: Option<(Vec<StyledLine>, usize)>,
    text: Option<TextZonePlan>,
    viewport: Option<DocumentationViewport>,
    gap: bool,
}

/// A single row window over code, separator and prose. Both halves scroll,
/// including a signature longer than the screen; no content is discarded.
fn plan_docs(
    docs: &Documentation<'_>,
    panel_w: usize,
    window_h: usize,
    scale_factor: f64,
    measure: &mut dyn crate::layout::TextMeasure,
) -> DocumentationPlan {
    let pad_x = scaled(dims::HEADER_PAD_X, scale_factor);
    let pad_y = scaled(dims::PANEL_PAD_Y, scale_factor);
    let line_h = scaled(dims::ZONE_LINE_H, scale_factor).max(1);
    let style = crate::layout::TextStyle::sized(size_px(SIZE_ROW, scale_factor));
    let width = panel_w.saturating_sub(2 * pad_x) as f32;
    let (code, prose) = docs.text.split_leading_code();
    let padding = if code.is_some() { 3 * pad_y } else { 2 * pad_y };
    let available = window_h.saturating_sub(padding + scaled(16.0, scale_factor)) / line_h;
    if available == 0 || width <= 0.0 {
        return DocumentationPlan::default();
    }
    let wrap = |text: &StyledText, measure: &mut dyn crate::layout::TextMeasure| {
        if text.text.is_empty() {
            return Vec::new();
        }
        wrap_documentation(&text.text, &text.spans, style, width.max(1.0), measure)
    };
    let code = code
        .as_ref()
        .map(|code| wrap(code, measure))
        .unwrap_or_default();
    let prose = wrap(&prose, measure);
    window_documentation(code, prose, docs.state, available, line_h, pad_y)
}

/// Shared scrolling over the signature, separator and prose. Neither caller
/// truncates content; the measured visible rows also drive mouse/key input.
fn window_documentation(
    code: Vec<StyledLine>,
    prose: Vec<StyledLine>,
    state: DocumentationState,
    available: usize,
    line_h: usize,
    pad_y: usize,
) -> DocumentationPlan {
    let gap = usize::from(!code.is_empty() && !prose.is_empty());
    let total = code.len() + gap + prose.len();
    if total == 0 || available == 0 {
        return DocumentationPlan::default();
    }
    // The caller reserves padding and window-edge breathing room before
    // choosing a row count. The solver still owns final anchoring/flipping.
    let capacity = if state.expanded {
        available
    } else {
        available.min(MAX_DOCS_LINES)
    };
    let visible = total.min(capacity);
    let scroll = state.scroll.min(total.saturating_sub(visible));
    let end = scroll + visible;
    let code_count = code.len();
    let prose_start = code_count + gap;
    let code: Vec<_> = code
        .into_iter()
        .enumerate()
        .filter_map(|(row, line)| (scroll..end).contains(&row).then_some(line))
        .collect();
    let prose: Vec<_> = prose
        .into_iter()
        .enumerate()
        .filter_map(|(row, line)| (scroll..end).contains(&(row + prose_start)).then_some(line))
        .collect();
    DocumentationPlan {
        code: (!code.is_empty()).then(|| {
            let h = code.len() * line_h + 2 * pad_y;
            (code, h)
        }),
        text: (!prose.is_empty()).then(|| {
            let h = prose.len() * line_h;
            (prose, false, h)
        }),
        viewport: Some(DocumentationViewport {
            scroll,
            visible,
            total,
        }),
        gap: gap > 0 && (scroll..end).contains(&code_count),
    }
}

/// Draw pre-wrapped `lines` stacked in `rect`, clipped to it; a zone
/// truncated by `MAX_ZONE_TEXT_LINES` ends with an ellipsis line. Each
/// line's styled runs paint per [`SpanStyle`]: `Code` gets a recessed chip,
/// `Strong`/`Accent` a synthetic bold (the overlay has one face), `Dim` the
/// meta color.
#[allow(clippy::too_many_arguments)]
fn draw_text_lines(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    rect: WidgetRect,
    lines: &[StyledLine],
    truncated: bool,
    mut style: crate::layout::TextStyle,
    color: u32,
    scale_factor: f64,
) {
    style.size = size_px(style.size, scale_factor);
    let line_h = scaled(dims::ZONE_LINE_H, scale_factor);
    for (i, line) in lines.iter().enumerate() {
        draw_styled_line(
            frame,
            painter,
            colors,
            rect.x,
            rect.y + i * line_h,
            line,
            style,
            color,
            scale_factor,
        );
    }
    if truncated {
        painter.draw_sized(
            frame,
            rect.x,
            rect.y + lines.len() * line_h,
            "\u{2026}",
            style.size,
            0.0,
            color,
        );
    }
}

/// One styled line at `(x, y)`: unstyled gaps in `color`, runs per their
/// style. Advances by the painter's own measure so run boundaries land
/// exactly where the glyphs do. Returns the drawn width.
#[allow(clippy::too_many_arguments)]
fn draw_styled_line(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    x: usize,
    y: usize,
    line: &StyledLine,
    style: crate::layout::TextStyle,
    color: u32,
    scale_factor: f64,
) -> f32 {
    let line_h = scaled(dims::ZONE_LINE_H, scale_factor);
    draw_styled_run(
        frame,
        painter,
        colors,
        x,
        y,
        line,
        style,
        color,
        line_h,
        scale_factor,
        true,
    )
}

/// [`draw_styled_line`] with an explicit chip height (list rows are shorter
/// than zone lines).
#[allow(clippy::too_many_arguments)]
fn draw_styled_run(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    x: usize,
    y: usize,
    line: &StyledLine,
    base_style: crate::layout::TextStyle,
    color: u32,
    line_h: usize,
    scale_factor: f64,
    documentation: bool,
) -> f32 {
    let chip_pad = scaled(2.0, scale_factor);
    // A standalone code line already reads as a block. Reserve chips for
    // identifiers embedded in prose, avoiding a second wash on signatures.
    let code_line = base_style.code
        || crate::model::styled_text::code_spans_cover(
            0..line.text.len(),
            line.runs.iter().cloned(),
        );
    let mut cursor = 0usize;
    let mut cx = x as f32;
    let mut segment = |frame: &mut Frame,
                       painter: &mut TextPainter,
                       text: &str,
                       style: Option<SpanStyle>| {
        if text.is_empty() {
            return;
        }
        let text_style = if documentation {
            documentation_style(base_style, style)
        } else {
            base_style
        };
        let role = text_style.font_role(painter.font_role());
        let mut painter = painter.with_font(role);
        let size = text_style.size;
        let text_y = y + line_h.saturating_sub(painter.line_height_for_size(size)) / 2;
        let w = painter.measure_sized(text, size, 0.0);
        let sx = cx.round() as usize;
        match style {
            Some(SpanStyle::Syntax(id)) => {
                painter.draw_sized(
                    frame,
                    sx,
                    text_y,
                    text,
                    size,
                    0.0,
                    colors.syntax.color_for_highlight(id).to_argb_u32(),
                );
            }
            Some(SpanStyle::Code) => {
                let inset = if documentation {
                    scaled(2.0, scale_factor)
                } else {
                    0
                };
                let pad = if documentation { 0 } else { chip_pad };
                if !documentation || !code_line {
                    frame.blend_rect_px(
                        sx.saturating_sub(pad),
                        y + inset,
                        w.ceil() as usize + 2 * pad,
                        line_h.saturating_sub(2 * inset),
                        colors.recessed_wash,
                    );
                }
                painter.draw_sized(frame, sx, text_y, text, size, 0.0, colors.text_bright);
            }
            // Synthetic bold: a second strike one pixel right, and one
            // extra pixel of advance so the widened glyphs never touch
            // the next run.
            Some(SpanStyle::Strong) => {
                painter.draw_sized(frame, sx, text_y, text, size, 0.0, colors.text_bright);
                painter.draw_sized(frame, sx + 1, text_y, text, size, 0.0, colors.text_bright);
                cx += 1.0;
            }
            Some(SpanStyle::Accent) => {
                painter.draw_sized(frame, sx, text_y, text, size, 0.0, colors.accent_bright);
                painter.draw_sized(frame, sx + 1, text_y, text, size, 0.0, colors.accent_bright);
                cx += 1.0;
            }
            Some(SpanStyle::Dim) => {
                painter.draw_sized(frame, sx, text_y, text, size, 0.0, colors.text_dim);
            }
            None => {
                painter.draw_sized(frame, sx, text_y, text, size, 0.0, color);
            }
        }
        cx += w;
    };
    for (range, style) in &line.runs {
        segment(frame, painter, &line.text[cursor..range.start], None);
        segment(frame, painter, &line.text[range.clone()], Some(*style));
        cursor = range.end;
    }
    segment(frame, painter, &line.text[cursor..], None);
    cx - x as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use fontdue::Font;

    #[test]
    fn backdrop_culling_preserves_opaque_and_translucent_panel_compositing() {
        let panel = WidgetRect {
            x: 4,
            y: 3,
            w: 24,
            h: 18,
        };
        for (color, clip) in [
            (0xFF345678, None),
            (0x80345678, None),
            (0xFF345678, Some(Rect::new(2.0, 4.0, 17.0, 14.0))),
        ] {
            let initial: Vec<u32> = (0..32 * 24u32)
                .map(|i| i.wrapping_mul(0x01234567))
                .collect();
            let mut actual = initial.clone();
            let mut expected = initial;
            for (buffer, optimized) in [(&mut actual, true), (&mut expected, false)] {
                let mut frame = Frame::new(buffer, 32, 24);
                let mut masks = RoundedRectMaskCache::new();
                if let Some(clip) = clip {
                    frame.push_clip(Rect::new(1.0, 1.0, 30.0, 22.0));
                    frame.push_clip(clip);
                }
                if optimized {
                    render_backdrop(&mut frame, panel, 3, color, 130);
                } else {
                    frame.dim(130);
                }
                frame.draw_shadow_rings(panel.x, panel.y, panel.w, panel.h, 3, 1.0, &mut masks);
                frame.fill_rounded_rect(panel.x, panel.y, panel.w, panel.h, 3, color, &mut masks);
            }
            assert_eq!(actual, expected, "color={color:08x}, clip={clip:?}");
        }
    }

    #[test]
    fn documentation_viewport_does_not_cover_the_menu_in_a_narrow_window() {
        let docs = StyledText::plain("Long documentation ".repeat(100));
        for width in [320, 400, 600, 800] {
            let mut spec = documentation_spec(&docs, 0, true);
            if let Anchor::Cursor { x, .. } = &mut spec.anchor {
                *x = width / 3;
            }
            let l = layout(&spec, width, 500, 1.0);
            let panel = l.docs_panel.unwrap();
            assert!(
                panel.x + panel.w <= l.panel.x || panel.x >= l.panel.x + l.panel.w,
                "{width}: docs {panel:?}, menu {:?}",
                l.panel
            );
        }
        assert!(layout(&documentation_spec(&docs, 0, true), 1000, 10, 1.0)
            .docs_panel
            .is_none());
    }

    fn documentation_spec(docs: &StyledText, scroll: usize, expanded: bool) -> OverlaySpec<'_> {
        OverlaySpec {
            anchor: Anchor::Cursor {
                x: 60,
                y: 100,
                h: 18,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 240.0,
                    max: 320.0,
                },
            },
            tabs: None,
            header: None,
            body: Body::Zones(Zones::default()),
            footer: None,
            hover_row: None,
            docs: Some(Documentation {
                text: docs,
                state: DocumentationState { scroll, expanded },
            }),
        }
    }

    #[test]
    fn documentation_viewport_reaches_every_row_and_clamps_after_resize() {
        let docs = StyledText::plain(
            (0..40)
                .map(|i| format!("row {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let first = layout(&documentation_spec(&docs, 0, false), 1000, 600, 1.0);
        let viewport = first.docs_viewport.unwrap();
        assert_eq!(viewport.visible, MAX_DOCS_LINES);
        assert_eq!(viewport.total, 40);
        let mut seen = std::collections::HashSet::new();
        for scroll in 0..=viewport.max_scroll() {
            let l = layout(&documentation_spec(&docs, scroll, false), 1000, 600, 1.0);
            let (lines, truncated, _) = l.docs_plan.unwrap();
            assert!(!truncated);
            seen.extend(lines.into_iter().map(|line| line.text));
        }
        assert_eq!(seen.len(), 40);
        let expanded = layout(&documentation_spec(&docs, usize::MAX, true), 1000, 600, 1.0);
        let vp = expanded.docs_viewport.unwrap();
        assert!(vp.visible > viewport.visible);
        assert_eq!(vp.scroll + vp.visible, vp.total);
        assert_eq!(expanded.docs_plan.unwrap().0.last().unwrap().text, "row 39");
        for sf in [1.0, 1.5, 2.0] {
            let l = layout(&documentation_spec(&docs, usize::MAX, true), 1200, 160, sf);
            let panel = l.docs_panel.unwrap();
            assert!(panel.y + panel.h <= 160, "{sf}: {panel:?}");
            assert!(panel.x + panel.w <= 1200);
            let vp = l.docs_viewport.unwrap();
            assert_eq!(vp.scroll, vp.max_scroll());
        }
    }

    #[test]
    fn hover_documentation_viewport_reaches_long_signatures_and_prose() {
        let code = (0..40)
            .map(|i| format!("parameter_{i}: Value,"))
            .collect::<Vec<_>>()
            .join("\n");
        let prose = (0..40)
            .map(|i| format!("Description {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        for scale in [1.0, 1.25, 2.0] {
            for height in [300, 600, 900] {
                let mut spec = OverlaySpec {
                    anchor: Anchor::Cursor {
                        x: 100,
                        y: 150,
                        h: 18,
                        prefer_below: false,
                        width: WidthRule {
                            pct: 0.42,
                            min: 360.0,
                            max: 560.0,
                        },
                    },
                    tabs: None,
                    header: None,
                    footer: None,
                    hover_row: None,
                    docs: None,
                    body: Body::Zones(Zones {
                        documentation: Some(DocumentationState::default()),
                        banner: Some((Severity::Warning, "Deprecated", "fixture")),
                        code: Some(&code),
                        text: Some(&prose),
                        ..Default::default()
                    }),
                };
                let first = layout(&spec, 1000, height, scale);
                let first_view = first.docs_viewport.expect("reading viewport");
                assert!(first_view.total >= 81, "no source rows discarded");
                assert!(
                    first.zone_plan.as_ref().unwrap().code.as_ref().unwrap().0[0]
                        .text
                        .starts_with("parameter_0")
                );
                for expanded in [false, true] {
                    let Body::Zones(zones) = &mut spec.body else {
                        unreachable!()
                    };
                    zones.documentation = Some(DocumentationState {
                        scroll: usize::MAX,
                        expanded,
                    });
                    let bottom = layout(&spec, 1000, height, scale);
                    let view = bottom.docs_viewport.unwrap();
                    assert_eq!(view.scroll, view.max_scroll());
                    assert_eq!(view.total, first_view.total);
                    let (lines, truncated, _) =
                        bottom.zone_plan.as_ref().unwrap().text.as_ref().unwrap();
                    assert!(!truncated);
                    assert_eq!(lines.last().unwrap().text, "Description 39");
                    assert!(
                        bottom.panel.y + bottom.panel.h <= height,
                        "{scale}, {height}: {:?}",
                        bottom.panel
                    );
                    let track = bottom.docs_scrollbar.unwrap().track_rect;
                    assert_eq!(
                        hit_test(&spec, &bottom, track.x as usize + 1, track.y as usize + 1),
                        OverlayHit::Documentation { viewport: view }
                    );
                }
            }
        }
    }

    #[test]
    fn documentation_viewport_scrolls_long_code_into_prose_and_keeps_styles() {
        let code = (0..80)
            .map(|i| format!("fn line_{i}();"))
            .collect::<Vec<_>>()
            .join("\n");
        let docs = crate::lsp::markdown::markdown_to_styled(&format!(
            "```rust\n{code}\n```\nLast **paragraph**."
        ));
        let top = layout(&documentation_spec(&docs, 0, false), 1000, 400, 1.0);
        assert_eq!(top.docs_code_plan.unwrap().0.len(), MAX_DOCS_LINES);
        assert!(top.docs_plan.is_none());
        assert!(top.docs_panel.unwrap().h < 400);
        let bottom = layout(
            &documentation_spec(&docs, usize::MAX, false),
            1000,
            400,
            1.0,
        );
        let vp = bottom.docs_viewport.unwrap();
        assert_eq!(vp.scroll + vp.visible, 82);
        let lines = bottom.docs_plan.unwrap().0;
        assert_eq!(lines.last().unwrap().text, "Last paragraph.");
        assert_eq!(lines.last().unwrap().runs, vec![(5..14, SpanStyle::Strong)]);
    }

    #[test]
    fn documentation_scrollbar_uses_the_reading_viewport_and_stays_clear_of_text() {
        let short = StyledText::plain("one\ntwo\nthree");
        let compact = layout(&documentation_spec(&short, 0, false), 1000, 500, 1.0);
        assert!(
            compact.docs_scrollbar.is_none(),
            "no scrollbar when all text fits"
        );
        assert!(
            compact.docs_viewport.is_some(),
            "wheel input still belongs to the card"
        );
        let docs = StyledText::plain("one\ntwo\nthree\n".repeat(10));
        let spec = documentation_spec(&docs, 0, false);
        let l = layout(&spec, 1000, 500, 1.0);
        let viewport = l.docs_viewport.unwrap();
        let text = l.docs_text.unwrap();
        let bar = l.docs_scrollbar.unwrap();
        assert_eq!(
            bar.state,
            ScrollbarState::new(viewport.total, viewport.visible, viewport.scroll)
        );
        assert!(text.x + text.w <= bar.track_rect.x as usize);
        assert_eq!(
            hit_test(&spec, &l, text.x + 1, text.y + 1),
            OverlayHit::Documentation { viewport }
        );
        assert_eq!(
            hit_test(
                &spec,
                &l,
                bar.track_rect.x as usize + 1,
                bar.track_rect.y as usize + 1
            ),
            OverlayHit::Documentation { viewport }
        );
    }

    #[test]
    fn documentation_wrap_matches_mixed_font_paint_and_restores_ui_measurement() {
        let (font, mut cache) = test_painter_and_frame();
        let ui_font = Font::from_bytes(
            include_bytes!("../../assets/Inter-Regular.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut ui_cache = super::super::GlyphCache::default();
        let mut painter =
            test_painter(&font, &mut cache).with_ui_font(&ui_font, &mut ui_cache, FontRole::Ui);
        let before = painter.measure_sized("proportional width", 13.0, 0.0);
        let docs = crate::lsp::markdown::markdown_to_styled(
            "Replace `current` with `(map/update m key f)` and keep café readable.\n\n```rust\nfn café() { let value = \"猫\"; }\n```",
        );
        let lines = wrap_documentation(
            &docs.text,
            &docs.spans,
            crate::layout::TextStyle::sized(13.0),
            180.0,
            &mut crate::layout::PainterMeasure::new(&mut painter),
        );
        let colors = Palette::from_theme(&crate::theme::Theme::default());
        let mut pixels = vec![0; 300 * 300];
        let mut frame = Frame::new(&mut pixels, 300, 300);
        for (row, line) in lines.iter().enumerate() {
            let width = draw_styled_line(
                &mut frame,
                &mut painter,
                &colors,
                0,
                row * 20,
                line,
                crate::layout::TextStyle::sized(13.0),
                colors.text_primary,
                1.0,
            );
            assert!(width <= 180.0, "{}: {width}", line.text);
        }
        assert_eq!(
            painter.measure_sized("proportional width", 13.0, 0.0),
            before
        );
        assert!(lines
            .iter()
            .any(|line| line.runs.iter().any(|(_, style)| *style == SpanStyle::Code)));

        // A signature is code regardless of whether its parameter is highlighted
        // or the server supplied a Markdown code span.
        let code = crate::layout::TextStyle {
            code: true,
            ..crate::layout::TextStyle::sized(SIZE_ROW)
        };
        for span in [
            None,
            Some(SpanStyle::Accent),
            Some(SpanStyle::Code),
            Some(SpanStyle::Syntax(10)),
        ] {
            let style = documentation_style(code, span);
            assert_eq!(style.font_role(FontRole::Ui), FontRole::Code);
            assert_eq!(style.size, SIZE_ROW * 0.92);
        }
        let plain = Zones {
            text: Some(&docs.text),
            text_spans: &docs.spans,
            ..Default::default()
        };
        let with_signature = Zones {
            code: Some("fn update(map: Map)"),
            ..plain
        };
        let mut measure = crate::layout::PainterMeasure::new(&mut painter);
        let prose = plan_zones(
            &Zones {
                code: None,
                ..with_signature
            },
            220,
            1.0,
            &mut measure,
        )
        .text;
        let signed = plan_zones(&with_signature, 220, 1.0, &mut measure).text;
        assert_eq!(
            prose, signed,
            "a signature must not resize or rewrap the prose"
        );
    }

    #[test]
    fn documentation_viewport_paints_scrolled_text_only_inside_the_card() {
        let docs = StyledText::plain(
            (0..40)
                .map(|i| format!("Documentation row {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let (font, mut cache) = test_painter_and_frame();
        let mut draw = |scroll| {
            let spec = documentation_spec(&docs, scroll, false);
            let mut buffer = vec![0u32; 1000 * 600];
            let mut frame = Frame::new(&mut buffer, 1000, 600);
            let mut painter = test_painter(&font, &mut cache);
            let l = layout_measured(
                &spec,
                1000,
                600,
                1.0,
                &mut crate::layout::PainterMeasure::new(&mut painter),
            );
            render(
                &mut frame,
                &mut painter,
                &mut RoundedRectMaskCache::new(),
                &crate::theme::Theme::default(),
                &spec,
                1000,
                600,
                1.0,
                false,
            );
            (buffer, l.docs_panel.unwrap())
        };
        let (first, panel) = draw(0);
        let (last, last_panel) = draw(usize::MAX);
        assert_eq!(panel, last_panel);
        let changed: Vec<_> = first
            .iter()
            .zip(&last)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert!(changed.len() > 10);
        for index in changed {
            let (x, y) = (index % 1000, index / 1000);
            assert!(x >= panel.x && x < panel.x + panel.w && y >= panel.y && y < panel.y + panel.h);
        }
    }

    #[test]
    fn completion_badges_distinguish_methods_functions_and_modules() {
        for (kind, glyph) in [
            (MenuItemKind::Function, 'f'),
            (MenuItemKind::Method, 'M'),
            (MenuItemKind::Variable, 'v'),
            (MenuItemKind::Type, 't'),
            (MenuItemKind::Keyword, 'k'),
            (MenuItemKind::Field, '.'),
            (MenuItemKind::Module, 'm'),
            (MenuItemKind::File, 'F'),
            (MenuItemKind::Folder, '/'),
            (MenuItemKind::Constant, 'c'),
            (MenuItemKind::Other, '?'),
        ] {
            assert_eq!(kind.badge_glyph(), glyph);
        }
        let colors = Palette::from_theme(&crate::theme::Theme::default());
        assert_eq!(
            MenuItemKind::Method.badge_color(&colors),
            MenuItemKind::Function.badge_color(&colors)
        );
    }

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
                docs: None,
            };

            render(
                &mut frame,
                &mut painter,
                &mut mask_cache,
                &crate::theme::Theme {
                    overlay: theme.clone(),
                    ..Default::default()
                },
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

    /// A `Zones` banner has no header above it (`layout()`'s Zones arm
    /// never sets one) — it sits flush against the panel's rounded top
    /// edge exactly like the tab bar does, and must not square those
    /// corners either. Regression for the diagnostic hover card (banner +
    /// text), reproduced at the finding's exact geometry: 280px min-width
    /// Cursor anchor, `(Error, "unused variable", "rustc")`.
    #[test]
    fn zones_banners_and_code_headers_preserve_corners_and_text_insets() {
        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = super::super::GlyphCache::default();
        let theme = OverlayTheme::default_dark();

        let mut render_corners = |banner: bool, code: bool| -> [u32; 2] {
            let (w, h) = (1200usize, 800usize);
            let mut buffer = vec![0u32; w * h];
            let mut frame = Frame::new(&mut buffer, w, h);
            let mut painter = test_painter(&font, &mut glyph_cache);
            let mut mask_cache = RoundedRectMaskCache::new();

            let spec = OverlaySpec {
                anchor: Anchor::Cursor {
                    x: 400,
                    y: 300,
                    h: 18,
                    prefer_below: false,
                    width: WidthRule {
                        pct: 0.0,
                        min: 280.0,
                        max: 420.0,
                    },
                },
                tabs: None,
                header: None,
                body: Body::Zones(Zones {
                    banner: banner.then_some((Severity::Error, "unused variable", "rustc")),
                    code: code.then_some("fn example()"),
                    text: Some("some explanatory hover text"),
                    ..Default::default()
                }),
                footer: None,
                hover_row: None,
                docs: None,
            };

            render(
                &mut frame,
                &mut painter,
                &mut mask_cache,
                &crate::theme::Theme {
                    overlay: theme.clone(),
                    ..Default::default()
                },
                &spec,
                w,
                h,
                1.0,
                true,
            );

            let l = layout(&spec, w, h, 1.0);
            if let Some(code) = l.zones_code {
                assert_eq!(code.x, l.zones_text.unwrap().x);
                if !banner {
                    assert_eq!(code.y, l.panel.y);
                }
                for x in [l.panel.x + 1, l.panel.x + l.panel.w - 2] {
                    assert_eq!(
                        frame.get_pixel(x, code.y + code.h / 2),
                        theme.panel_secondary.to_argb_u32(),
                        "the code background reaches both panel edges",
                    );
                }
            }
            [
                frame.get_pixel(l.panel.x, l.panel.y),
                frame.get_pixel(l.panel.x + l.panel.w - 1, l.panel.y),
            ]
        };

        let plain = render_corners(false, false);
        for (banner, code) in [(true, false), (false, true), (true, true)] {
            assert_eq!(
                render_corners(banner, code),
                plain,
                "full-width bands must preserve the panel's rounded top corners",
            );
        }
    }

    /// Code has a compact background without invading the preceding prose;
    /// font-size changes may move subsequent text, but not other zones.
    #[test]
    fn styled_runs_paint_a_chip_behind_code_and_bold_for_strong() {
        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = super::super::GlyphCache::default();
        let theme = OverlayTheme::default_dark();
        let text = "call foo() now and more";
        let (w, h) = (1200usize, 800usize);

        let mut render_with = |spans: &[Span]| -> (Vec<u32>, WidgetRect, f32, f32) {
            let mut buffer = vec![0u32; w * h];
            let mut frame = Frame::new(&mut buffer, w, h);
            let mut painter = test_painter(&font, &mut glyph_cache);
            let mut mask_cache = RoundedRectMaskCache::new();
            let spec = OverlaySpec {
                anchor: Anchor::Cursor {
                    x: 400,
                    y: 300,
                    h: 18,
                    prefer_below: false,
                    width: WidthRule {
                        pct: 0.0,
                        min: 280.0,
                        max: 420.0,
                    },
                },
                tabs: None,
                header: None,
                body: Body::Zones(Zones {
                    banner: Some((Severity::Info, "banner", "")),
                    code: None,
                    text: Some(text),
                    text_spans: spans,
                    ..Default::default()
                }),
                footer: None,
                hover_row: None,
                docs: None,
            };
            render(
                &mut frame,
                &mut painter,
                &mut mask_cache,
                &crate::theme::Theme {
                    overlay: theme.clone(),
                    ..Default::default()
                },
                &spec,
                w,
                h,
                1.0,
                true,
            );
            let l = layout(&spec, w, h, 1.0);
            let size = size_px(SIZE_ROW, 1.0);
            let before = painter.measure_sized("call ", size, 0.0);
            let run = painter.measure_sized("foo()", size, 0.0);
            (buffer, l.zones_text.unwrap(), before, run)
        };

        let (plain, rect, before, run) = render_with(&[]);
        let (chipped, ..) = render_with(&[Span {
            range: 5..10,
            style: SpanStyle::Code,
        }]);
        let (bold, ..) = render_with(&[Span {
            range: 5..10,
            style: SpanStyle::Strong,
        }]);
        assert_ne!(plain, chipped, "a code span must change the paint");
        assert_ne!(plain, bold, "a strong span must change the paint");

        let mid_y = rect.y + rect.h.min(scaled(dims::ZONE_LINE_H, 1.0)) / 2;
        let at = |buf: &Vec<u32>, x: usize| buf[mid_y * w + x];
        // The chip stays within the run, without washing over the prose gap.
        let chip_left = rect.x + before.round() as usize - 1;
        assert_eq!(at(&chipped, chip_left), at(&plain, chip_left));
        // Outside the text, the remaining surface is unchanged.
        let outside = rect.x + rect.w - 2;
        assert_eq!(at(&chipped, outside), at(&plain, outside));
        assert_eq!(at(&bold, outside), at(&plain, outside));
        // The whole chip band [run start - pad, run end + pad) differs
        // somewhere on the mid-line row; the rows above the zone do not.
        let band: Vec<usize> = (rect.x + before.floor() as usize - 2
            ..rect.x + (before + run).ceil() as usize + 2)
            .collect();
        assert!(band.iter().any(|&x| at(&chipped, x) != at(&plain, x)));
        let above = (rect.y.saturating_sub(1)) * w;
        assert_eq!(&chipped[above..above + w], &plain[above..above + w]);
    }

    /// Acceptance: docs that open with a code fence get a code block above
    /// the prose in the docs card; prose-only docs get no code block.
    #[test]
    fn docs_card_puts_a_leading_fence_in_a_code_block_above_the_prose() {
        let rows = [one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let fenced = crate::lsp::markdown::markdown_to_styled(
            "```rust\nfn foo() -> u8\n```\nReturns a *byte*.",
        );
        let prose_only = StyledText::plain("Just words.");
        let layout_for = |docs: &StyledText| {
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x: 100,
                    y: 100,
                    h: 18,
                    prefer_below: true,
                    width: WidthRule {
                        pct: 0.0,
                        min: 240.0,
                        max: 320.0,
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
                docs: Some(docs.into()),
            };
            layout(&spec, 1200, 800, 1.0)
        };

        let l = layout_for(&fenced);
        let code = l.docs_code.expect("fenced docs get a code block");
        let text = l.docs_text.expect("and the prose below it");
        assert_eq!(code.y, l.docs_panel.unwrap().y);
        assert_eq!(code.x, text.x);
        assert!(text.y >= code.y + code.h, "prose sits below the code block");
        let (code_lines, _) = l.docs_code_plan.as_ref().unwrap();
        assert_eq!(code_lines[0].text, "fn foo() -> u8");
        assert!(crate::model::styled_text::code_spans_cover(
            0..code_lines[0].text.len(),
            code_lines[0].runs.iter().cloned(),
        ));
        assert!(code_lines[0]
            .runs
            .iter()
            .any(|(_, style)| matches!(style, SpanStyle::Syntax(_))));
        let (prose_lines, _, _) = l.docs_plan.as_ref().unwrap();
        assert_eq!(prose_lines[0].text, "Returns a byte.");
        assert_eq!(prose_lines[0].runs, vec![(10..14, SpanStyle::Strong)]);
        assert!(l.docs_panel.unwrap().h >= code.h + text.h);

        let l = layout_for(&prose_only);
        assert!(l.docs_code.is_none());
        assert!(l.docs_text.is_some());
    }

    /// Acceptance: a `Code`-styled row detail paints a chip; the rest of
    /// the list is untouched.
    #[test]
    fn code_styled_row_detail_paints_a_chip() {
        let font = Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .expect("test font should load");
        let mut glyph_cache = super::super::GlyphCache::default();
        let theme = OverlayTheme::default_dark();
        let (w, h) = (1200usize, 800usize);
        let mut render_with = |style: Option<SpanStyle>| -> (Vec<u32>, Vec<WidgetRect>) {
            let rows = [
                Row {
                    icon: RowIcon::None,
                    label: "len",
                    match_indices: &[],
                    detail: Some("fn(&self) -> usize"),
                    detail_style: style,
                    accessory: Accessory::None,
                },
                one_row(),
            ];
            let sections = [Section {
                title: None,
                rows: &rows,
            }];
            let spec = OverlaySpec {
                tabs: None,
                anchor: Anchor::Cursor {
                    x: 100,
                    y: 100,
                    h: 18,
                    prefer_below: true,
                    width: WidthRule {
                        pct: 0.0,
                        min: 320.0,
                        max: 400.0,
                    },
                },
                header: None,
                body: Body::List {
                    sections: &sections,
                    selected: FlatIndex(1),
                    scroll: 0,
                    max_visible: 8,
                },
                footer: None,
                hover_row: None,
                docs: None,
            };
            let mut buffer = vec![0u32; w * h];
            let mut frame = Frame::new(&mut buffer, w, h);
            let mut painter = test_painter(&font, &mut glyph_cache);
            let mut mask_cache = RoundedRectMaskCache::new();
            render(
                &mut frame,
                &mut painter,
                &mut mask_cache,
                &crate::theme::Theme {
                    overlay: theme.clone(),
                    ..Default::default()
                },
                &spec,
                w,
                h,
                1.0,
                true,
            );
            let l = layout(&spec, w, h, 1.0);
            (buffer, l.rows)
        };
        let (plain, rows) = render_with(None);
        let (chipped, _) = render_with(Some(SpanStyle::Code));
        assert_ne!(plain, chipped);
        // Only the first row's band differs; the second row is identical.
        let band = |buf: &Vec<u32>, r: &WidgetRect| -> Vec<u32> {
            (r.y..r.y + r.h)
                .flat_map(|y| buf[y * w + r.x..y * w + r.x + r.w].iter().copied())
                .collect()
        };
        assert_ne!(band(&plain, &rows[0]), band(&chipped, &rows[0]));
        assert_eq!(band(&plain, &rows[1]), band(&chipped, &rows[1]));
    }

    /// The planner attaches spans to the wrapped lines they land on,
    /// rebased to each line — a span crossing a wrap is split.
    #[test]
    fn plan_zones_splits_spans_across_wrapped_lines() {
        let mut measure = crate::layout::CellMeasure {
            char_width: 8.0,
            line_height: 18.0,
        };
        let text = "alpha beta gamma delta";
        let spans = [Span {
            range: 6..16, // "beta gamma"
            style: SpanStyle::Code,
        }];
        let zones = Zones {
            text: Some(text),
            text_spans: &spans,
            ..Default::default()
        };
        // Narrow panel: ~12 cells of content -> wraps after "alpha beta".
        let pad_x = scaled(dims::HEADER_PAD_X, 1.0);
        let plan = plan_zones(&zones, 12 * 8 + 2 * pad_x, 1.0, &mut measure);
        let (lines, _, _) = plan.text.unwrap();
        assert_eq!(lines[0].text, "alpha beta");
        assert_eq!(lines[0].runs, vec![(6..10, SpanStyle::Code)]);
        assert_eq!(lines[1].text, "gamma delta");
        assert_eq!(lines[1].runs, vec![(0..5, SpanStyle::Code)]);
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
            detail_style: None,
            accessory: Accessory::None,
        }];
        let rows_b = [Row {
            icon: RowIcon::None,
            label: "b",
            match_indices: &[],
            detail: None,
            detail_style: None,
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
    fn untitled_non_first_sections_flatten_to_separator_rows() {
        let row = |label| Row {
            icon: RowIcon::None,
            label,
            match_indices: &[],
            detail: None,
            detail_style: None,
            accessory: Accessory::None,
        };
        let rows_a = [row("a")];
        let rows_b = [row("b")];
        let sections = [
            Section {
                title: None,
                rows: &rows_a,
            },
            Section {
                title: None,
                rows: &rows_b,
            },
        ];
        let display = flatten_rows(&sections);
        // First untitled section emits no boundary; the second emits a
        // Separator slot — context-menu separators must occupy a display
        // row or they render no gap at all.
        assert_eq!(display.len(), 3);
        assert!(matches!(display[0], DisplayRow::Row(_, FlatIndex(0))));
        assert!(matches!(display[1], DisplayRow::Separator));
        assert!(matches!(display[2], DisplayRow::Row(_, FlatIndex(1))));
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
    fn banner_wash_is_a_tint_not_the_raw_severity_color() {
        let theme = crate::theme::Theme::default();
        let palette = Palette::from_theme(&theme);
        for sev in [
            Severity::Error,
            Severity::Warning,
            Severity::Info,
            Severity::Hint,
        ] {
            let wash = palette.severity_wash(sev);
            let raw = match sev {
                Severity::Error => palette.severity_error,
                Severity::Warning => palette.severity_warning,
                Severity::Info => palette.severity_info,
                Severity::Hint => palette.severity_hint,
            };
            assert_ne!(
                wash, raw,
                "{sev:?}: banner must be a wash, not full strength"
            );
            assert_ne!(wash, palette.panel_bg, "{sev:?}: wash must tint the panel");
        }
    }

    #[test]
    fn banner_plan_wraps_long_messages_within_budget() {
        // Regression: the banner drew one unwrapped, unclipped line that
        // ran past the panel and collided with the source tag.
        let zones = Zones {
            banner: Some((
                Severity::Warning,
                "Class 'DateTimeImmutable' not found in the current scope of this file",
                "phpantom",
            )),
            code: None,
            text: None,
            ..Default::default()
        };
        let mut measure = cell_measure(1.0);
        let plan = plan_zones(&zones, 320, 1.0, &mut measure);
        let banner = plan.banner.expect("banner plan");
        assert!(banner.lines.len() > 1, "long message must wrap");
        let content_w = (320 - 2 * scaled(dims::HEADER_PAD_X, 1.0)) as f32;
        let later_budget = content_w - 16.0;
        let first_budget = later_budget - ("phpantom".len() + 2) as f32 * 8.0;
        // Only line 0 shares its row with the source tag (`render_zones`
        // paints it once at `top`) — it alone must respect the narrower
        // pixel budget; lines 1+ get the full glyph-only budget.
        assert!(
            banner.lines[0].text.chars().count() as f32 * 8.0 <= first_budget,
            "line 0 must respect the glyph+source budget: {:?}",
            banner.lines
        );
        assert!(
            banner.lines[1..]
                .iter()
                .all(|l| l.text.chars().count() as f32 * 8.0 <= later_budget),
            "lines after the first need only the glyph budget: {:?}",
            banner.lines
        );
        assert!(
            banner.h >= banner.lines.len() * scaled(dims::ZONE_LINE_H, 1.0),
            "banner height must fit its wrapped lines"
        );
    }

    #[test]
    fn banner_plan_caps_pathological_messages() {
        let long = "word ".repeat(500);
        let zones = Zones {
            banner: Some((Severity::Error, &long, "rust-analyzer")),
            code: None,
            text: None,
            ..Default::default()
        };
        let mut measure = cell_measure(1.0);
        let plan = plan_zones(&zones, 480, 1.0, &mut measure);
        let banner = plan.banner.unwrap();
        assert_eq!(banner.lines.len(), MAX_ZONE_BANNER_LINES);
        assert!(banner.truncated);
    }

    /// Reproduces the reported shredding: a real, longer source tag (a
    /// language-server name, not a two-letter stub) must not shrink lines
    /// 2+ the way it shrinks line 0 — that wasted budget was losing whole
    /// words of the message.
    #[test]
    fn banner_plan_only_narrows_the_first_line_for_the_source_tag() {
        let zones = Zones {
            banner: Some((
                Severity::Error,
                "cannot find value `foo` in this scope and more explanation of the error",
                "typescript-eslint",
            )),
            code: None,
            text: None,
            ..Default::default()
        };
        let mut measure = cell_measure(1.0);
        let plan = plan_zones(&zones, 280, 1.0, &mut measure);
        let banner = plan.banner.expect("banner plan");
        assert!(banner.lines.len() > 1, "message must wrap");

        let content_w = (280 - 2 * scaled(dims::HEADER_PAD_X, 1.0)) as f32;
        let later_budget = content_w - 16.0;
        let first_budget = later_budget - ("typescript-eslint".len() + 2) as f32 * 8.0;

        assert!(banner.lines[0].text.chars().count() as f32 * 8.0 <= first_budget.max(64.0));
        // At least one later line must use more columns than the narrow
        // first-line budget allowed — otherwise the reservation is still
        // silently applied to every line.
        assert!(
            banner.lines[1..]
                .iter()
                .any(|l| l.text.chars().count() as f32 * 8.0 > first_budget.max(64.0)),
            "lines after the first must be free to use the wider, glyph-only budget: {:?}",
            banner.lines
        );
    }

    #[test]
    fn zone_wrapping_follows_the_measure_not_a_fixed_cell() {
        // The point of the measured plan: a proportional measure (narrow
        // 'i's) fits more text per line than the 8px-cell fallback did.
        struct NarrowI;
        impl crate::layout::TextMeasure for NarrowI {
            fn width(&mut self, text: &str, _style: crate::layout::TextStyle) -> f32 {
                text.chars().map(|c| if c == 'i' { 3.0 } else { 8.0 }).sum()
            }
            fn line_height(&mut self, _style: crate::layout::TextStyle) -> f32 {
                17.0
            }
        }

        let text = "iiiiiiii iiiiiiii iiiiiiii iiiiiiii";
        let zones = Zones {
            banner: None,
            code: None,
            text: Some(text),
            ..Default::default()
        };
        let mut cells = cell_measure(1.0);
        let cell_lines = plan_zones(&zones, 200, 1.0, &mut cells).text.unwrap().0;
        let mut narrow = NarrowI;
        let narrow_lines = plan_zones(&zones, 200, 1.0, &mut narrow).text.unwrap().0;
        assert!(
            narrow_lines.len() < cell_lines.len(),
            "narrow glyphs must pack more per line: {narrow_lines:?} vs {cell_lines:?}"
        );
    }

    #[test]
    fn drop_overlay_centers_only_a_single_measured_line() {
        let long = "Drop these files into a narrow target that requires wrapping";
        let zones = Zones {
            center_text: true,
            banner: None,
            code: None,
            text: Some(long),
            ..Default::default()
        };
        let mut measure = cell_measure(1.0);
        let (lines, truncated, _) = plan_zones(&zones, 160, 1.0, &mut measure)
            .text
            .expect("drop text should have a plan");

        assert!(lines.len() > 1, "test message must wrap");
        assert!(!should_center_zone_text(&zones, &lines, truncated));

        let short = Zones {
            center_text: true,
            text: Some("Drop files here"),
            ..Zones::default()
        };
        let mut measure = cell_measure(1.0);
        let (lines, truncated, _) = plan_zones(&short, 320, 1.0, &mut measure)
            .text
            .expect("drop text should have a plan");
        assert!(should_center_zone_text(&short, &lines, truncated));
        let documentation = Zones {
            center_text: false,
            ..short
        };
        assert!(!should_center_zone_text(&documentation, &lines, truncated));
    }

    #[test]
    fn layout_and_render_derive_the_same_plan() {
        // The drift-class regression test: the panel height layout computes
        // must exactly fit the zones the (re-derived) render plan draws.
        let msg = "Class 'DateTimeImmutable' not found and quite a bit more explanation text";
        let doc = "line one of documentation ".repeat(12);
        let zones = Zones {
            banner: Some((Severity::Warning, msg, "phpantom")),
            code: Some("pub const fn black_box<T>(dummy: T) -> T"),
            text: Some(&doc),
            ..Default::default()
        };
        let spec = OverlaySpec {
            anchor: Anchor::Cursor {
                x: 40,
                y: 40,
                h: 19,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 320.0,
                    max: 480.0,
                },
            },
            tabs: None,
            header: None,
            body: Body::Zones(zones),
            footer: None,
            hover_row: None,
            docs: None,
        };
        let layout = layout(&spec, 1200, 900, 1.0);
        let zones2 = match &spec.body {
            Body::Zones(z) => z,
            _ => unreachable!(),
        };
        let mut measure = cell_measure(1.0);
        let plan = plan_zones(zones2, layout.panel.w, 1.0, &mut measure);
        let line_h = scaled(dims::ZONE_LINE_H, 1.0);

        let br = layout.zones_banner.unwrap();
        assert_eq!(br.h, plan.banner.as_ref().unwrap().h);
        let cr = layout.zones_code.unwrap();
        assert_eq!(cr.h, plan.code.as_ref().unwrap().1);
        let tr = layout.zones_text.unwrap();
        let (t_lines, t_trunc, t_h) = plan.text.as_ref().unwrap();
        assert_eq!(tr.h, *t_h);
        assert!(tr.h >= (t_lines.len() + usize::from(*t_trunc)) * line_h);
        // Everything inside the panel.
        for r in [br, cr, tr] {
            assert!(r.y >= layout.panel.y && r.y + r.h <= layout.panel.y + layout.panel.h);
        }
    }

    #[test]
    fn zones_panel_height_fits_wrapped_text() {
        // Regression: hover panels were sized by raw line count at the
        // font size (13px) instead of wrapped count at the line height,
        // so long single-line docs overflowed the panel bottom.
        let long = "word ".repeat(120);
        let zones = Zones {
            banner: None,
            code: None,
            text: Some(&long),
            ..Default::default()
        };
        let spec = OverlaySpec {
            anchor: Anchor::Cursor {
                x: 10,
                y: 10,
                h: 19,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 320.0,
                    max: 480.0,
                },
            },
            tabs: None,
            header: None,
            body: Body::Zones(zones),
            footer: None,
            hover_row: None,
            docs: None,
        };
        let layout = layout(&spec, 800, 600, 1.0);
        let r = layout.zones_text.expect("text zone rect");
        let style = crate::layout::TextStyle::sized(size_px(SIZE_ROW, 1.0));
        let mut measure = cell_measure(1.0);
        let wrapped = crate::layout::text::wrap_to_width(&long, style, r.w as f32, &mut measure)
            .len()
            .min(MAX_ZONE_TEXT_LINES)
            + 1; // +ellipsis
        let line_h = scaled(dims::ZONE_LINE_H, 1.0);
        assert!(
            r.h >= wrapped * line_h,
            "zone rect {}px must fit {} wrapped lines of {}px",
            r.h,
            wrapped,
            line_h
        );
        assert!(
            r.y + r.h <= layout.panel.y + layout.panel.h,
            "text zone must stay inside the panel"
        );
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
        let out = painter.truncate_sized("short", 13.0, 1000.0, EllipsisSide::End);
        assert_eq!(out, "short");
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn truncate_tail_ellipsizes_multibyte_text_on_char_boundaries() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let text = "日本語テキストとても長い文字列です";
        let out = painter.truncate_sized(text, 13.0, 40.0, EllipsisSide::End);
        assert!(out.ends_with('\u{2026}'));
        assert!(out.chars().count() < text.chars().count());
        assert!(painter.measure_sized(&out, 13.0, 0.0) <= 40.0);
        assert_eq!(
            painter.truncate_sized(text, 13.0, 0.0, EllipsisSide::End),
            ""
        );
    }

    #[test]
    fn truncate_head_prepends_ellipsis_and_keeps_tail() {
        let (font, mut cache) = test_painter_and_frame();
        let mut painter = TextPainter::new(&font, &mut cache, 13.0, 10.0, 8.0, 16);
        let out = painter.truncate_sized("src/view/geometry.rs", 13.0, 60.0, EllipsisSide::Start);
        assert!(out.starts_with('\u{2026}'));
        assert!(out.ends_with(".rs"));
        assert!(painter.measure_sized(&out, 13.0, 0.0) <= 60.0);
        for side in [EllipsisSide::Start, EllipsisSide::End] {
            let ellipsis = painter.measure_sized("…", 13.0, 0.0);
            assert_eq!(
                painter.truncate_sized("日本語.rs", 13.0, ellipsis - 1.0, side),
                ""
            );
        }
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
            docs: None,
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
            docs: None,
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
                detail_style: None,
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
            docs: None,
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
                detail_style: None,
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
    fn settings_section_heading_and_selection_share_window_bounds() {
        let rows: Vec<Row> = (0..12)
            .map(|_| Row {
                icon: RowIcon::None,
                label: "Setting",
                match_indices: &[],
                detail: None,
                detail_style: None,
                accessory: Accessory::None,
            })
            .collect();
        let sections = [
            Section {
                title: Some("Appearance"),
                rows: &rows[..5],
            },
            Section {
                title: Some("Editor"),
                rows: &rows[5..],
            },
        ];
        let display = flatten_rows(&sections);
        let shapes = [
            SectionShape {
                has_title: true,
                len: 5,
            },
            SectionShape {
                has_title: true,
                len: 7,
            },
        ];
        assert_eq!(resolve_visible_window(&display, 0, 10).0, 0);
        for max_visible in 1..=14 {
            for previous in 0..12 {
                for selected in 0..12 {
                    let scroll =
                        resolve_scroll_for_selection(&shapes, selected, max_visible, previous);
                    let (start, visible) = resolve_visible_window(&display, scroll, max_visible);
                    assert!(
                        display[start..start + visible].iter().any(
                            |r| matches!(r, DisplayRow::Row(_, FlatIndex(i)) if *i == selected)
                        ),
                        "selected={selected} previous={previous} visible={max_visible}"
                    );
                }
            }
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
                docs: None,
            };

            render(
                &mut frame,
                &mut painter,
                &mut mask_cache,
                &crate::theme::Theme {
                    overlay: theme.clone(),
                    ..Default::default()
                },
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
    fn shortcut_hints_split_textual_platform_modifiers_and_preserve_plus_key() {
        let steps = binding_chips("Ctrl+Shift+K Alt+F12");
        assert_eq!(
            steps
                .iter()
                .map(|step| step
                    .iter()
                    .map(|chip| chip.label.as_str())
                    .collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            vec![vec!["Ctrl", "Shift", "K"], vec!["Alt", "F12"]]
        );
        let plus = binding_chips("Ctrl++");
        assert_eq!(
            plus[0]
                .iter()
                .map(|chip| chip.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Ctrl", "+"]
        );
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
            docs: None,
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
                detail_style: None,
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
    fn settings_choice_geometry_matches_hits_with_headers_scroll_and_scale() {
        let rows: Vec<Row> = (0..16)
            .map(|_| Row {
                icon: RowIcon::None,
                label: "Setting",
                match_indices: &[],
                detail: None,
                detail_style: None,
                accessory: Accessory::Choices {
                    labels: &["Off", "Slow", "Normal", "Fast"],
                    active: None,
                },
            })
            .collect();
        let sections = [Section {
            title: Some("Editor"),
            rows: &rows,
        }];
        for scale in [1.0, 1.5, 2.0] {
            for width in [180, 500, 1200] {
                let mut spec = list_spec(&sections);
                spec.body = Body::List {
                    sections: &sections,
                    selected: FlatIndex(8),
                    scroll: 6,
                    max_visible: 10,
                };
                let layout = layout(&spec, width, 900, scale);
                assert!(!layout.choices.is_empty());
                for (flat, chips) in &layout.choices {
                    assert!(flat.0 >= 6);
                    assert_eq!(chips.len(), 4);
                    for (index, chip) in chips.iter().enumerate() {
                        assert!(
                            chip.x >= layout.panel.x
                                && chip.x + chip.w <= layout.panel.x + layout.panel.w
                        );
                        assert!(chip.w > 0 && chip.h > 0);
                        assert_eq!(
                            hit_test(&spec, &layout, chip.x + chip.w / 2, chip.y + chip.h / 2),
                            OverlayHit::Choice {
                                row: *flat,
                                choice: index
                            }
                        );
                    }
                    for pair in chips.windows(2) {
                        assert!(pair[0].x + pair[0].w <= pair[1].x);
                    }
                }
            }
        }
    }

    #[test]
    fn hit_test_row_returns_its_flat_index() {
        let rows: Vec<Row> = (0..3)
            .map(|_| Row {
                icon: RowIcon::None,
                label: "row",
                match_indices: &[],
                detail: None,
                detail_style: None,
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
                detail_style: None,
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
    fn hit_test_tabs_uses_solved_keys_and_respects_availability() {
        let sections: [Section; 0] = [];
        let tabs = [
            ("All", TabCount::Hidden),
            ("Symbols", TabCount::Unavailable),
        ];
        let mut spec = list_spec(&sections);
        spec.tabs = Some(TabBar {
            tabs: &tabs,
            active: 0,
        });
        let layout = layout(&spec, 1000, 800, 1.0);

        let available = layout.tab_rects[0];
        assert_eq!(
            hit_test(&spec, &layout, available.x + 1, available.y + 1),
            OverlayHit::Tab(0)
        );
        let unavailable = layout.tab_rects[1];
        assert_eq!(
            hit_test(&spec, &layout, unavailable.x + 1, unavailable.y + 1),
            OverlayHit::Inside
        );
    }

    #[test]
    fn layout_fields_stacks_label_then_input_per_field() {
        let fields = [Field::labeled("Find:"), Field::labeled("Replace:")];
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
            docs: None,
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
                ..Default::default()
            }),
            footer: None,
            hover_row: None,
            docs: None,
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
            detail_style: None,
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
            docs: None,
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
            docs: None,
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
    fn cursor_anchor_with_zero_height_flips_above_a_click_near_the_bottom_edge() {
        // A right-click point (context-menu.md's `h: 0` usage — no caret
        // line to flip around, just the click point itself) near the
        // bottom of an 800px window: the popup must still flip above
        // rather than clipping off the bottom edge. Confirms `h: 0`
        // degenerates the flip check to the click point, as the doc
        // claims (`Anchor` "usage note, not a code change").
        let rows = [one_row(), one_row(), one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 100,
                y: 790,
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
            docs: None,
        };
        let l = layout(&spec, 1000, 800, 1.0);
        assert!(
            l.panel.y + l.panel.h <= 790,
            "panel should flip above the click point: panel.y={} h={}",
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
            docs: None,
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
            docs: None,
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
            docs: None,
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
            docs: None,
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
            docs: None,
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
            docs: None,
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
            docs: None,
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
                ..Default::default()
            }),
            footer: None,
            hover_row: None,
            docs: None,
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

    #[test]
    fn docs_card_flips_to_the_panels_left_when_the_right_lacks_room() {
        let rows = [one_row()];
        let sections = [Section {
            title: None,
            rows: &rows,
        }];
        let docs = StyledText::plain("Some documentation for the selected row.");
        let spec = OverlaySpec {
            tabs: None,
            anchor: Anchor::Cursor {
                x: 900,
                y: 100,
                h: 20,
                prefer_below: true,
                width: WidthRule {
                    pct: 0.0,
                    min: 240.0,
                    max: 320.0,
                },
            },
            header: None,
            body: Body::List {
                sections: &sections,
                selected: FlatIndex(0),
                scroll: 0,
                max_visible: 10,
            },
            footer: None,
            hover_row: None,
            docs: Some((&docs).into()),
        };
        let l = layout(&spec, 1000, 800, 1.0);
        let docs = l.docs_panel.expect("docs card");
        assert_eq!(docs.w, 360);
        assert_eq!(
            docs.x + docs.w,
            l.panel.x,
            "no room on the right (panel clamped to the edge) -> card on the left"
        );
        assert_eq!(docs.y, l.panel.y);
        assert!(l.docs_text.unwrap().h > 0);
    }
}
