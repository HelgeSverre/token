//! Centralized geometry helpers for rendering and hit-testing
//!
//! This module provides a single source of truth for layout calculations,
//! coordinate transformations, and hit-testing that is shared between
//! the view (rendering) and runtime (input handling) layers.
//!
//! All functions here are pure (no I/O, no side effects) and can be
//! tested independently of the rendering infrastructure.

use crate::model::editor_area::{EditorGroup, Rect};
use crate::model::{AppModel, Document, EditorState, Position};

use crate::util::text::TABULATOR_WIDTH;

// ============================================================================
// Tab Expansion Helpers
// ============================================================================

use std::borrow::Cow;

/// Expand tab characters to spaces for display.
///
/// Converts each tab character to the appropriate number of spaces based on
/// the current visual column and `TABULATOR_WIDTH`. This is used for rendering
/// text where tabs need to be visually aligned.
///
/// Returns `Cow::Borrowed` if no tabs are present (zero allocation),
/// or `Cow::Owned` with expanded tabs otherwise.
///
/// # Example
/// ```ignore
/// let text = "a\tb";  // Tab at column 1
/// let expanded = expand_tabs_for_display(text);
/// assert_eq!(&*expanded, "a   b");  // Tab becomes 3 spaces (to reach column 4)
/// ```
pub fn expand_tabs_for_display(text: &str) -> Cow<'_, str> {
    // Fast path: if no tabs, return borrowed reference (no allocation)
    if !text.contains('\t') {
        return Cow::Borrowed(text);
    }

    // Slow path: expand tabs
    let mut result = String::with_capacity(text.len() * 2);
    let mut visual_col = 0;

    for ch in text.chars() {
        if ch == '\t' {
            let spaces = TABULATOR_WIDTH - (visual_col % TABULATOR_WIDTH);
            for _ in 0..spaces {
                result.push(' ');
            }
            visual_col += spaces;
        } else {
            result.push(ch);
            visual_col += 1;
        }
    }

    Cow::Owned(result)
}

/// Convert a visual column into a viewport-relative pixel x-coordinate.
///
/// The returned x-position is clamped to the left edge of the text area when
/// the target column is scrolled offscreen.
#[inline]
pub fn column_to_pixel_x(
    visual_col: usize,
    viewport_left: usize,
    text_start_x: usize,
    char_width: f32,
) -> usize {
    let visible_col = visual_col.saturating_sub(viewport_left);
    text_start_x + (visible_col as f32 * char_width).round() as usize
}

// ============================================================================
// Hit-Testing Helpers
// ============================================================================

/// Get the focused group, editor, and document from the model.
///
/// This helper centralizes the lookup of the currently focused editor context,
/// which is needed for hit-testing functions that need to convert global window
/// coordinates to local group coordinates.
fn focused_group_editor_document(
    model: &AppModel,
) -> Option<(&EditorGroup, &EditorState, &Document)> {
    let editor_area = &model.editor_area;

    let group = editor_area.focused_group()?;
    let editor_id = group.active_editor_id()?;
    let editor = editor_area.editors.get(&editor_id)?;
    let doc_id = editor.document_id?;
    let document = editor_area.documents.get(&doc_id)?;

    Some((group, editor, document))
}

/// Convert pixel coordinates to document line and column for the focused editor.
///
/// Takes into account the group's position (including sidebar offset), tab bar,
/// gutter, scroll offset, and horizontal scrolling.
///
/// Uses the same `GroupLayout` as painting, including docked editor controls.
pub fn pixel_to_cursor(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    model: &AppModel,
) -> (usize, usize) {
    if let Some((group, editor, document)) = focused_group_editor_document(model) {
        GroupLayout::new(group, model, char_width).pixel_to_cursor(
            x,
            y,
            char_width,
            line_height,
            editor,
            document,
        )
    } else {
        // No focused group/editor/document - safe fallback
        (0, 0)
    }
}

/// A documentation target must hit real text, not a caret clamped from a gutter,
/// line margin, ghost-text row or space below EOF. Use the same viewport mapping
/// as editing, then check the source glyph's rendered cell before accepting it.
pub fn hover_position(x: f64, y: f64, model: &AppModel) -> Option<Position> {
    let (group, editor, document) = focused_group_editor_document(model)?;
    let layout = GroupLayout::new(group, model, model.char_width);
    if !editor.is_plain_text_mode() || !layout.content_rect.contains(x as f32, y as f32) {
        return None;
    }
    let width = model.char_width as f64;
    let height = model.line_height as f64;
    // Caret hit testing rounds to the nearest boundary; hover wants the cell
    // under the pointer instead, including its right half.
    let (line, column) = layout.pixel_to_cursor(
        x - width / 2.0,
        y,
        model.char_width,
        height,
        editor,
        document,
    );
    let text = document.buffer.get_line(line)?;
    let ch = text.get_char(column)?;
    if ch.is_whitespace() {
        return None;
    }
    let viewport = editor.viewport_map(document);
    let row = viewport.visible_row_at_pixel(y - layout.content_y() as f64, height);
    let visual_column = if let Some(projected) = viewport.ghost_row_for_visible_row(row) {
        // Carets have before-insertion affinity; glyphs follow the renderer's
        // source spans, which place a suffix after the inline suggestion.
        let source = projected
            .sources
            .iter()
            .flatten()
            .find(|source| source.columns.contains(&column))?;
        source.visual_column(&projected.text, column)
    } else {
        if viewport.visible_row_for_position(line, column)? != row {
            return None;
        }
        viewport.display_position(document, line, column).1
    };
    let left =
        layout.text_start_x as f64 + viewport.column_pixel_offset(visual_column, model.char_width);
    let top = layout.content_y() as f64 + viewport.row_pixel_offset(row, height);
    if x < layout.text_start_x as f64
        || x < left
        || x >= left + width
        || y < top
        || y >= top + height
    {
        return None;
    }

    Some(Position::new(line, column))
}

/// Convert pixel coordinates to line and VISUAL column (screen position).
/// Used for rectangle selection where the raw visual column is needed,
/// independent of any specific line's text content.
/// Returns (line, visual_column) where visual_column is the screen column.
///
/// Uses the same `GroupLayout` as painting, including docked editor controls.
pub fn pixel_to_line_and_visual_column(
    x: f64,
    y: f64,
    char_width: f32,
    line_height: f64,
    model: &AppModel,
) -> (usize, usize) {
    if let Some((group, editor, document)) = focused_group_editor_document(model) {
        GroupLayout::new(group, model, char_width).pixel_to_line_and_visual_column(
            x,
            y,
            char_width,
            line_height,
            editor,
            document,
        )
    } else {
        // No focused group/editor/document - safe fallback
        (0, 0)
    }
}

// ============================================================================
// GutterLayout - Gutter lane widths
// ============================================================================

/// Gutter lane widths, in physical pixels.
///
/// `Copy` and allocation-free: it's rebuilt as part of `GroupLayout` on the
/// per-mouse-move hit-test path, so it must stay a plain widths struct with
/// no `Vec`. The line-numbers lane (`numbers_w`, which includes the trailing
/// gutter padding up to the border) is always active; the marks lane
/// activates per-document once it has diagnostics (LSP Phase 2, the first
/// consumer — see editor-decorations.md); fold/diff lanes stay 0 until their
/// consumers ship.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GutterLayout {
    pub marks_w: u16,
    pub numbers_w: u16,
    pub fold_w: u16,
    pub diff_w: u16,
}

impl GutterLayout {
    /// Build the gutter layout for a document with `line_count` lines.
    /// `has_marks` activates the marks lane (~1 char wide, left of line
    /// numbers per editor-decorations.md's lane order).
    pub fn new(
        char_width: f32,
        metrics: &crate::model::ScaledMetrics,
        line_count: usize,
        has_marks: bool,
    ) -> Self {
        // Round the marks lane and the combined gutter-border width from
        // the *same* single sum `gutter_border_x_scaled` uses, then
        // derive `numbers_w` as the remainder — rounding `marks_w` and
        // `numbers_w` independently could disagree by a pixel with
        // `text_start_x_scaled`'s one combined round (fractional
        // `char_width`), drifting the gutter border and text start apart.
        let marks_w = if has_marks {
            char_width.round() as u16
        } else {
            0
        };
        let border_w = crate::model::gutter_border_x_scaled(
            char_width, metrics, line_count, has_marks,
        )
        .round() as u16;
        let numbers_w = border_w.saturating_sub(marks_w);
        Self {
            marks_w,
            numbers_w,
            fold_w: 0,
            diff_w: 0,
        }
    }

    /// Total gutter width in pixels, from the group's left edge to the border.
    pub fn total_width(&self) -> usize {
        self.marks_w as usize
            + self.numbers_w as usize
            + self.fold_w as usize
            + self.diff_w as usize
    }

    /// Which lane, if any, contains `x_in_gutter` (pixels from the group's
    /// left edge). Lanes left to right: marks, line numbers, fold, diff —
    /// matching the visual order in editor-decorations.md. A zero-width
    /// lane (no active consumer) is skipped, never matched.
    pub fn lane_at(&self, x_in_gutter: usize) -> Option<LaneId> {
        let mut cursor = 0usize;
        for (lane, width) in [
            (LaneId::Marks, self.marks_w as usize),
            (LaneId::LineNumbers, self.numbers_w as usize),
            (LaneId::Fold, self.fold_w as usize),
            (LaneId::Diff, self.diff_w as usize),
        ] {
            if width == 0 {
                continue;
            }
            if x_in_gutter < cursor + width {
                return Some(lane);
            }
            cursor += width;
        }
        None
    }
}

/// One of the gutter's vertical lanes (see `GutterLayout`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneId {
    Marks,
    LineNumbers,
    Fold,
    Diff,
}

impl LaneId {
    /// Interactive lanes consume gutter press/drag instead of falling
    /// through to the default focus/drag-select behavior (see
    /// editor-decorations.md's Hit-Testing & Interaction section).
    pub fn is_interactive(&self) -> bool {
        matches!(self, LaneId::Fold | LaneId::Marks)
    }
}

// ============================================================================
// GroupLayout - Unified Layout Computation
// ============================================================================

/// Pre-computed layout for an editor group, with all positions in window coordinates.
///
/// This struct provides a single source of truth for all positioning calculations
/// within an editor group. It uses scaled metrics (DPI-aware) and ensures consistent
/// positioning across all rendering functions.
///
/// # Usage
/// ```ignore
/// let layout = GroupLayout::new(group, model, char_width);
/// // Use layout.content_y(), layout.gutter_right_x, etc.
/// ```
#[derive(Debug, Clone, Copy)]
pub struct GroupLayout {
    /// The group's rect in window coordinates (from compute_layout_scaled)
    pub group_rect: Rect,
    /// Docked Find bar below the tabs (zero height when absent).
    pub find_bar_rect: Rect,
    /// Content area (excludes tabs and docked controls), in window coordinates
    pub content_rect: Rect,
    /// Tab bar height (scaled for DPI)
    pub tab_bar_height: usize,
    /// Gutter lane widths for this group's document
    pub gutter: GutterLayout,
    /// Gutter border X position (absolute window coordinate)
    pub gutter_right_x: usize,
    /// X coordinate where text content starts (absolute window coordinate)
    pub text_start_x: usize,
}

impl GroupLayout {
    /// Create a new GroupLayout from an editor group.
    ///
    /// All positioning values are computed using scaled metrics from the model,
    /// ensuring DPI-correct rendering on all displays. Gutter width is derived
    /// from the group's active document line count.
    pub fn new(group: &EditorGroup, model: &AppModel, char_width: f32) -> Self {
        let find_height = model
            .find_bar_inset()
            .filter(|(id, _)| Some(*id) == group.active_editor_id())
            .map_or(0, |(_, height)| height);
        Self::from_rect(
            group.rect,
            model.editor_area.document_for_group(group),
            &model.metrics,
            char_width,
            find_height,
        )
    }

    /// Shared with viewport synchronization, which needs to lay out inactive
    /// tabs too and mutably borrow editor state independently of the model.
    pub(crate) fn from_rect(
        group_rect: Rect,
        document: Option<&Document>,
        metrics: &crate::model::ScaledMetrics,
        char_width: f32,
        find_height: usize,
    ) -> Self {
        let tab_bar_height = metrics.tab_bar_height;
        let below_tabs = (group_rect.height - tab_bar_height as f32).max(0.0);
        let find_bar_rect = Rect::new(
            group_rect.x,
            group_rect.y + tab_bar_height as f32,
            group_rect.width,
            (find_height as f32).min(below_tabs),
        );
        let content_rect = Rect::new(
            group_rect.x,
            find_bar_rect.y + find_bar_rect.height,
            group_rect.width,
            below_tabs - find_bar_rect.height,
        );

        let line_count = document.map(|doc| doc.line_count()).unwrap_or(1);
        let has_marks = document.is_some_and(|doc| !doc.diagnostics.is_empty());

        let rect_x = group_rect.x.round() as usize;
        let gutter = GutterLayout::new(char_width, metrics, line_count, has_marks);
        let gutter_right_x = rect_x + gutter.total_width();
        let text_start_x = rect_x
            + crate::model::text_start_x_scaled(char_width, metrics, line_count, has_marks).round()
                as usize;

        Self {
            group_rect,
            find_bar_rect,
            content_rect,
            tab_bar_height,
            gutter,
            gutter_right_x,
            text_start_x,
        }
    }

    // =========================================================================
    // Group-level accessors (tab bar area)
    // =========================================================================

    /// Get absolute X position of the group
    #[inline]
    pub fn rect_x(&self) -> usize {
        self.group_rect.x.round() as usize
    }

    /// Get absolute Y position of the group
    #[inline]
    pub fn rect_y(&self) -> usize {
        self.group_rect.y.round() as usize
    }

    /// Get group width in pixels
    #[inline]
    pub fn rect_w(&self) -> usize {
        self.group_rect.width.round() as usize
    }

    // =========================================================================
    // Content-level accessors (below tabs and docked controls)
    // =========================================================================

    /// Get absolute Y position for the content area, below all docked controls.
    #[inline]
    pub fn content_y(&self) -> usize {
        self.content_rect.y.round() as usize
    }

    /// Get content height in pixels
    #[inline]
    pub fn content_h(&self) -> usize {
        self.content_rect.height.round() as usize
    }

    // =========================================================================
    // Gutter accessors
    // =========================================================================

    /// Get gutter width in pixels (from rect_x to gutter_right_x)
    #[inline]
    pub fn gutter_width(&self) -> usize {
        self.gutter_right_x - self.rect_x()
    }

    // =========================================================================
    // Line positioning helpers
    // =========================================================================

    /// Calculate visible line count for this group
    #[inline]
    pub fn visible_lines(&self, line_height: usize) -> usize {
        self.content_h().checked_div(line_height).unwrap_or(0)
    }

    /// Width of the rendered text area, excluding the gutter.
    #[inline]
    pub fn text_width(&self) -> usize {
        (self.rect_x() + self.rect_w()).saturating_sub(self.text_start_x)
    }

    /// Calculate visible text columns, including soft-wrap scrollbar/caret clearance.
    #[inline]
    pub fn visible_columns(
        &self,
        char_width: f32,
        soft_wrap: bool,
        scrollbar_width: usize,
    ) -> usize {
        crate::model::text_viewport_columns(
            self.rect_w() as f32,
            self.text_start_x.saturating_sub(self.rect_x()) as f32,
            char_width,
            soft_wrap,
            scrollbar_width,
        )
    }

    fn text_offset(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x - self.text_start_x as f64,
            (y - self.content_y() as f64).max(0.0),
        )
    }

    /// Convert a window point using this pane's rendered text origin.
    pub fn pixel_to_cursor(
        &self,
        x: f64,
        y: f64,
        char_width: f32,
        line_height: f64,
        editor: &EditorState,
        document: &Document,
    ) -> (usize, usize) {
        let (x, y) = self.text_offset(x, y);
        let position = editor.viewport_map(document).position_for_pixel(
            document,
            x,
            y,
            char_width,
            line_height,
        );
        (position.line, position.column)
    }

    /// Convert a window point to the visual column used by rectangle selection.
    pub fn pixel_to_line_and_visual_column(
        &self,
        x: f64,
        y: f64,
        char_width: f32,
        line_height: f64,
        editor: &EditorState,
        document: &Document,
    ) -> (usize, usize) {
        let (x, y) = self.text_offset(x, y);
        let viewport = editor.viewport_map(document);
        let visible_row = viewport.visible_row_at_pixel(y, line_height);
        let line = viewport
            .top_line()
            .saturating_add(visible_row)
            .min(viewport.last_line());
        (line, viewport.visual_column_for_x_offset(x, char_width))
    }

    // =========================================================================
    // Scrollbar rects (overlay-style: rendered on top of content right/bottom edge)
    // =========================================================================

    /// Get the vertical scrollbar track rect (right edge of content area).
    ///
    /// Returns `None` if scrollbars are disabled (`show_scrollbar` is false).
    /// The scrollbar is rendered as an overlay over the content area's right edge.
    #[inline]
    pub fn v_scrollbar_rect(&self, scrollbar_width: usize) -> Option<Rect> {
        if scrollbar_width == 0 {
            return None;
        }
        let sw = scrollbar_width as f32;
        let cr = self.content_rect;
        Some(Rect::new(cr.x + cr.width - sw, cr.y, sw, cr.height))
    }

    /// Get the horizontal scrollbar track rect (bottom edge of content area).
    ///
    /// Returns `None` if scrollbars are disabled (`show_scrollbar` is false).
    /// Only shown when content is wider than the viewport.
    #[inline]
    pub fn h_scrollbar_rect(&self, scrollbar_width: usize) -> Option<Rect> {
        if scrollbar_width == 0 {
            return None;
        }
        let sw = scrollbar_width as f32;
        let cr = self.content_rect;
        Some(Rect::new(
            cr.x,
            cr.y + cr.height - sw,
            // Leave corner for vertical scrollbar; panes narrower than the
            // scrollbar itself must not produce a negative-width track.
            (cr.width - sw).max(0.0),
            sw,
        ))
    }
}

// ============================================================================
// Modal Geometry
// ============================================================================

/// Standard padding/spacing constants for modal dialogs, in logical px.
///
/// These used to be bare `usize` constants consumed directly by layout code
/// — never multiplied by `scale_factor`, so modals were effectively
/// half-size on a 2x display. They're logical-px bases now; call the scaled
/// accessor for the value actually used in physical-pixel layout math.
pub struct ModalSpacing;

impl ModalSpacing {
    /// Input field internal vertical padding (total top+bottom)
    const BASE_INPUT_PAD_Y: f64 = 8.0;
    /// Input field internal horizontal padding (each side)
    const BASE_INPUT_PAD_X: f64 = 8.0;

    #[inline]
    fn scaled(base: f64, scale_factor: f64) -> usize {
        (base * scale_factor).round() as usize
    }

    pub fn input_pad_y(scale_factor: f64) -> usize {
        Self::scaled(Self::BASE_INPUT_PAD_Y, scale_factor)
    }

    pub fn input_pad_x(scale_factor: f64) -> usize {
        Self::scaled(Self::BASE_INPUT_PAD_X, scale_factor)
    }
}

/// A positioned widget within a modal layout
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetRect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

// ============================================================================
// Binary Placeholder Layout
// ============================================================================

/// Button label used for binary placeholder tabs.
pub const BINARY_PLACEHOLDER_BUTTON_LABEL: &str = "Open with Default Application";

/// Layout positions for the binary file placeholder screen.
///
/// Pre-computes all vertical positions and the button rect so rendering
/// and hit-testing use the exact same geometry.
pub struct BinaryPlaceholderLayout {
    /// Horizontal center of the content area
    pub center_x: usize,
    /// Y position for the filename text
    pub name_y: usize,
    /// Y position for the file size text
    pub size_y: usize,
    /// The button's bounding rect
    pub button_rect: Rect,
}

/// Compute binary placeholder layout from the content area dimensions.
///
/// Used by both the renderer and hit-test code to ensure consistent positioning.
pub fn binary_placeholder_layout(
    content_rect: Rect,
    line_height: usize,
    char_width: f32,
    padding_large: usize,
    padding_medium: usize,
    button_label: &str,
) -> BinaryPlaceholderLayout {
    let center_x = content_rect.x as usize + content_rect.width as usize / 2;
    let center_y = content_rect.y as usize + content_rect.height as usize / 2;

    let name_y = center_y.saturating_sub(line_height * 2);
    let size_y = name_y + line_height + line_height / 2;
    let btn_y = size_y + line_height * 3;

    let padding_h = padding_large * 2;
    let padding_v = padding_medium;
    let button_rect = super::button::button_rect(
        center_x,
        btn_y,
        button_label,
        char_width,
        line_height,
        padding_h,
        padding_v,
    );

    BinaryPlaceholderLayout {
        center_x,
        name_y,
        size_y,
        button_rect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::text::{char_col_to_visual_col, visual_col_to_char_col};

    #[test]
    fn hover_hits_source_cells_after_tabs_and_fractional_scroll() {
        let mut model = AppModel::with_document(800, 600, 1.25, Document::with_text("\talpha\n"));
        model.resize(800, 600);
        model.editor_mut().viewport.pixels.x.offset = 3.0;
        model.editor_mut().viewport.pixels.y.offset = 4.0;
        let layout = GroupLayout::new(
            model.editor_area.focused_group().unwrap(),
            &model,
            model.char_width,
        );
        let x = layout.text_start_x as f64;
        let y = layout.content_y() as f64;
        let width = model.char_width as f64;
        let height = model.line_height as f64;
        // A tab occupies four display columns, but is not a hover target.
        assert_eq!(
            hover_position(x + 2.5 * width - 3.0, y + height / 2.0 - 4.0, &model),
            None
        );
        // The right half of 'a' must still request 'a', not the next caret cell.
        assert_eq!(
            hover_position(x + 4.8 * width - 3.0, y + height / 2.0 - 4.0, &model),
            Some(Position::new(0, 1))
        );
        assert_eq!(
            hover_position(x + 9.5 * width - 3.0, y + height / 2.0 - 4.0, &model),
            None
        );
    }

    #[test]
    fn hover_uses_source_spans_not_caret_affinity_at_inline_suggestions() {
        let mut model = AppModel::with_document(800, 600, 1.0, Document::with_text("before tail"));
        model.resize(800, 600);
        let ghost = crate::model::GhostProjection::new(
            model.document(),
            Position::new(0, 7),
            "inserted\n",
            None,
        )
        .unwrap();
        model.editor_mut().ghost_text.0 = Some(std::sync::Arc::new(ghost));
        let layout = GroupLayout::new(
            model.editor_area.focused_group().unwrap(),
            &model,
            model.char_width,
        );
        let x = layout.text_start_x as f64;
        let y = layout.content_y() as f64;
        let width = model.char_width as f64;
        let height = model.line_height as f64;
        assert_eq!(
            hover_position(x + 7.5 * width, y + 0.5 * height, &model),
            None,
            "the first ghost glyph is not the source at its insertion caret"
        );
        assert_eq!(
            hover_position(x + 0.5 * width, y + 1.5 * height, &model),
            Some(Position::new(0, 7)),
            "the displaced suffix is still source text"
        );
    }

    #[test]
    fn test_expand_tabs() {
        assert_eq!(expand_tabs_for_display("a\tb"), "a   b"); // tab at col 1 -> 3 spaces
        assert_eq!(expand_tabs_for_display("\t"), "    "); // tab at col 0 -> 4 spaces
    }

    #[test]
    fn test_group_layout_visible_columns_respects_text_start() {
        let layout = GroupLayout {
            group_rect: Rect::new(0.0, 0.0, 200.0, 120.0),
            find_bar_rect: Rect::new(0.0, 24.0, 200.0, 0.0),
            content_rect: Rect::new(0.0, 24.0, 200.0, 96.0),
            tab_bar_height: 24,
            gutter: GutterLayout::default(),
            gutter_right_x: 48,
            text_start_x: 60,
        };

        assert_eq!(layout.text_width(), 140);
        assert_eq!(layout.visible_columns(10.0, false, 10), 14);
        // Soft wrap reserves both the scrollbar and one caret column.
        assert_eq!(layout.visible_columns(10.0, true, 10), 12);
    }

    #[test]
    fn test_gutter_layout_lane_at_skips_zero_width_lanes() {
        // Only the line-numbers lane is active (today's shipped state) —
        // any x in the gutter resolves to LineNumbers, never Marks/Fold/Diff.
        let gutter = GutterLayout {
            marks_w: 0,
            numbers_w: 40,
            fold_w: 0,
            diff_w: 0,
        };
        assert_eq!(gutter.lane_at(0), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(39), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(40), None);
    }

    #[test]
    fn test_gutter_layout_lane_at_orders_active_lanes_left_to_right() {
        let gutter = GutterLayout {
            marks_w: 8,
            numbers_w: 40,
            fold_w: 8,
            diff_w: 4,
        };
        assert_eq!(gutter.lane_at(0), Some(LaneId::Marks));
        assert_eq!(gutter.lane_at(7), Some(LaneId::Marks));
        assert_eq!(gutter.lane_at(8), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(47), Some(LaneId::LineNumbers));
        assert_eq!(gutter.lane_at(48), Some(LaneId::Fold));
        assert_eq!(gutter.lane_at(56), Some(LaneId::Diff));
        assert_eq!(gutter.lane_at(60), None);
    }

    #[test]
    fn test_lane_id_interactivity() {
        assert!(LaneId::Marks.is_interactive());
        assert!(LaneId::Fold.is_interactive());
        assert!(!LaneId::LineNumbers.is_interactive());
        assert!(!LaneId::Diff.is_interactive());
    }

    #[test]
    fn test_char_col_to_visual_col() {
        assert_eq!(char_col_to_visual_col("abc", 2), 2);
        // "a\tb": 'a' at char 0 (visual 0), '\t' at char 1 (visual 1-3), 'b' at char 2 (visual 4)
        assert_eq!(char_col_to_visual_col("a\tb", 2), 4);
    }

    #[test]
    fn test_visual_col_to_char_col() {
        assert_eq!(visual_col_to_char_col("abc", 2), 2);
        assert_eq!(visual_col_to_char_col("a\tb", 4), 2); // visual 4 is 'b' which is char 2
    }

    #[test]
    fn test_crlf_cursor_column_math_matches_rendered_text() {
        // Regression test for a CRLF cursor-placement bug: `get_line_cow`
        // (used for rendering) strips a trailing `\r\n`, but before this fix
        // `trim_line_ending` (used for click->column math) and `line_length`
        // (used for column clamping) only stripped the `\n`, leaving the `\r`
        // counted. That mismatch let a click past the end of "hello" land on
        // column 6 (as if the line were "hello\r") instead of clamping to 5.
        let document = crate::model::Document::with_text("hello\r\nworld\r\n");

        let rendered = document.get_line_cow(0).unwrap();
        assert_eq!(&*rendered, "hello");

        let line_text = document.get_line(0).unwrap();
        let trimmed = crate::view::helpers::trim_line_ending(&line_text);
        assert_eq!(trimmed, rendered.as_ref());

        // Simulate clicking far past the end of the line (visual column 100).
        let column = visual_col_to_char_col(trimmed, 100);
        let line_len = document.line_length(0);
        let clamped_column = column.min(line_len);

        assert_eq!(line_len, 5);
        assert_eq!(
            clamped_column, 5,
            "cursor must clamp to 'hello', not include the \\r"
        );
    }

    #[test]
    fn test_column_to_pixel_x() {
        assert_eq!(column_to_pixel_x(2, 0, 100, 8.0), 116);
        assert_eq!(column_to_pixel_x(6, 4, 100, 8.0), 116);
        assert_eq!(column_to_pixel_x(2, 4, 100, 8.0), 100);
        assert_eq!(column_to_pixel_x(3, 1, 100, 7.5), 115);
    }

    #[test]
    fn test_tree_row_layout_positions() {
        let metrics = crate::model::ScaledMetrics::new(1.0);
        let tl = TreeRowLayout::from_metrics(&metrics);

        // Depth 0: just left_padding
        let pos = tl.node_position(0, 100);
        assert_eq!(pos.icon_x, tl.left_padding);
        assert_eq!(pos.text_x, tl.left_padding + tl.indicator_width);
        assert_eq!(pos.text_y, 100 + tl.text_top_padding);

        // Depth 1: left_padding + indent
        let pos1 = tl.node_position(1, 100);
        assert!(pos1.icon_x > pos.icon_x);
    }
}

// ============================================================================
// Tree Row Layout
// ============================================================================

/// Reusable geometry for one row within a Clay-owned tree-list viewport.
///
/// Clay/`RowListView` owns the viewport and vertical row boxes; this helper
/// owns only the indentation and label/accessory positions inside a row.
#[derive(Debug, Clone, Copy)]
pub struct TreeRowLayout {
    /// Left padding from container edge to first-level icons
    pub left_padding: usize,
    /// Width reserved for the expand/collapse indicator
    pub indicator_width: usize,
    /// Vertical padding from row top to text baseline
    pub text_top_padding: usize,
    /// Horizontal indent per nesting level
    pub indent: f32,
}

/// Computed positions for a single tree node at a given depth and y.
#[derive(Debug, Clone, Copy)]
pub struct TreeNodePosition {
    /// X coordinate for the expand/collapse icon
    pub icon_x: usize,
    /// X coordinate for the text label
    pub text_x: usize,
    /// Y coordinate for the text (row y + top padding)
    pub text_y: usize,
}

impl TreeRowLayout {
    /// Create a tree list layout from scaled metrics.
    pub fn from_metrics(metrics: &crate::model::ScaledMetrics) -> Self {
        Self {
            left_padding: metrics.padding_large,
            indicator_width: metrics.padding_large + metrics.padding_large / 2,
            text_top_padding: metrics.padding_small,
            indent: metrics.file_tree_indent,
        }
    }

    /// Create a tree list layout for the outline panel (slightly smaller indicator).
    pub fn outline_from_metrics(metrics: &crate::model::ScaledMetrics) -> Self {
        Self {
            left_padding: metrics.padding_large,
            indicator_width: metrics.padding_large + metrics.padding_medium,
            text_top_padding: metrics.padding_small,
            indent: metrics.file_tree_indent,
        }
    }

    /// Compute the x-offset for a node at the given depth.
    #[inline]
    pub fn x_offset(&self, depth: usize) -> usize {
        (depth as f32 * self.indent) as usize + self.left_padding
    }

    /// Compute icon_x, text_x, and text_y for a node at the given depth and row y.
    #[inline]
    pub fn node_position(&self, depth: usize, row_y: usize) -> TreeNodePosition {
        let x_offset = self.x_offset(depth);
        TreeNodePosition {
            icon_x: x_offset,
            text_x: x_offset + self.indicator_width,
            text_y: row_y + self.text_top_padding,
        }
    }

    /// Compute the available width for text given container width and text_x.
    #[inline]
    pub fn available_text_width(&self, container_width: usize, text_x: usize) -> usize {
        container_width.saturating_sub(text_x + self.left_padding)
    }

    /// Whether an x-coordinate lands on the collapse/expand indicator for a
    /// row at `depth`, in a container whose content starts at `base_x`.
    #[inline]
    pub fn is_on_chevron(&self, base_x: f32, depth: usize, x: f32) -> bool {
        let start = base_x + self.x_offset(depth) as f32;
        let end = start + self.indicator_width as f32;
        x >= start && x < end
    }
}
