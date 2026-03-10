//! Reusable scrollbar rendering and geometry
//!
//! Provides a pure, model-independent scrollbar component that can be used
//! in any pane: editor groups, CSV grid, sidebar, docks, etc.
//!
//! # Usage
//!
//! ```ignore
//! let state = ScrollbarState::new(total_lines, visible_lines, top_line);
//! let geo = ScrollbarGeometry::vertical(track_rect, &state);
//! render_scrollbar(frame, &geo, hovered, &colors);
//! ```

use crate::model::editor_area::Rect;

use super::frame::Frame;

/// Logical scrollbar width in pixels (before DPI scaling)
pub const SCROLLBAR_WIDTH_LOGICAL: f64 = 12.0;

/// Minimum thumb size in physical pixels
const MIN_THUMB_PX: f32 = 20.0;

// ============================================================================
// State
// ============================================================================

/// Describes the content/viewport relationship for one scroll axis
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarState {
    /// Total content size (lines, columns, or items)
    pub total: usize,
    /// Viewport capacity (visible lines, columns, etc.)
    pub visible: usize,
    /// Current scroll offset
    pub position: usize,
}

impl ScrollbarState {
    pub fn new(total: usize, visible: usize, position: usize) -> Self {
        Self {
            total,
            visible,
            position,
        }
    }

    /// Whether scrolling is needed (content exceeds viewport)
    #[inline]
    pub fn needs_scroll(&self) -> bool {
        self.total > self.visible
    }

    /// Maximum valid scroll position
    #[inline]
    pub fn max_position(&self) -> usize {
        self.total.saturating_sub(self.visible)
    }
}

// ============================================================================
// Geometry
// ============================================================================

/// Computed geometry for a scrollbar: track rectangle and thumb rectangle
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarGeometry {
    /// Full scrollbar track area (the background strip)
    pub track_rect: Rect,
    /// Thumb position within the track
    pub thumb_rect: Rect,
    /// Whether scrolling is needed (false = content fits, don't render thumb)
    pub needed: bool,
}

impl ScrollbarGeometry {
    /// Compute geometry for a vertical scrollbar.
    ///
    /// `track` is the full vertical strip the scrollbar occupies (right edge of content).
    pub fn vertical(track: Rect, state: &ScrollbarState) -> Self {
        if !state.needs_scroll() {
            return Self {
                track_rect: track,
                thumb_rect: track,
                needed: false,
            };
        }

        let track_h = track.height;
        let thumb_h = thumb_size(state.visible, state.total, track_h);
        let thumb_y = thumb_offset(state.position, state.max_position(), track_h - thumb_h);

        Self {
            track_rect: track,
            thumb_rect: Rect::new(track.x, track.y + thumb_y, track.width, thumb_h),
            needed: true,
        }
    }

    /// Compute geometry for a horizontal scrollbar.
    ///
    /// `track` is the full horizontal strip (bottom edge of content).
    pub fn horizontal(track: Rect, state: &ScrollbarState) -> Self {
        if !state.needs_scroll() {
            return Self {
                track_rect: track,
                thumb_rect: track,
                needed: false,
            };
        }

        let track_w = track.width;
        let thumb_w = thumb_size(state.visible, state.total, track_w);
        let thumb_x = thumb_offset(state.position, state.max_position(), track_w - thumb_w);

        Self {
            track_rect: track,
            thumb_rect: Rect::new(track.x + thumb_x, track.y, thumb_w, track.height),
            needed: true,
        }
    }

    /// Check if a point hits the thumb rect.
    pub fn hits_thumb(&self, x: f32, y: f32) -> bool {
        let r = self.thumb_rect;
        x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
    }

    /// Check if a point hits the track rect.
    pub fn hits_track(&self, x: f32, y: f32) -> bool {
        let r = self.track_rect;
        x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
    }

    /// Compute new scroll position from a click on the track.
    ///
    /// `coord`: Y (vertical) or X (horizontal) coordinate of the click.
    /// Centers the thumb around the click point.
    pub fn position_from_track_click(&self, coord: f32, state: &ScrollbarState) -> usize {
        let is_vertical = self.track_rect.height >= self.track_rect.width;
        let (track_start, track_len, thumb_sz) = if is_vertical {
            (
                self.track_rect.y,
                self.track_rect.height,
                self.thumb_rect.height,
            )
        } else {
            (
                self.track_rect.x,
                self.track_rect.width,
                self.thumb_rect.width,
            )
        };

        let thumb_travel = (track_len - thumb_sz).max(1.0);
        // Center thumb around click, clamped to valid range
        let thumb_pos = (coord - track_start - thumb_sz / 2.0).clamp(0.0, thumb_travel);
        let ratio = thumb_pos / thumb_travel;
        (ratio * state.max_position() as f32).round() as usize
    }

    /// Compute new scroll position from thumb drag.
    ///
    /// `grab_offset`: where within the thumb the user originally clicked (pixels from thumb start).
    /// `mouse_coord`: current mouse Y (vertical) or X (horizontal) position.
    pub fn position_from_drag(
        &self,
        grab_offset: f32,
        mouse_coord: f32,
        state: &ScrollbarState,
    ) -> usize {
        let is_vertical = self.track_rect.height >= self.track_rect.width;
        let (track_start, track_len, thumb_sz) = if is_vertical {
            (
                self.track_rect.y,
                self.track_rect.height,
                self.thumb_rect.height,
            )
        } else {
            (
                self.track_rect.x,
                self.track_rect.width,
                self.thumb_rect.width,
            )
        };

        let thumb_travel = (track_len - thumb_sz).max(1.0);
        let thumb_pos = (mouse_coord - grab_offset - track_start).clamp(0.0, thumb_travel);
        let ratio = thumb_pos / thumb_travel;
        (ratio * state.max_position() as f32).round() as usize
    }
}

/// Compute scroll position from a click on the scrollbar track.
///
/// Centers the thumb around the click point. Works for both axes —
/// pass the appropriate axis-aligned coordinates.
pub fn position_from_track_click(
    coord: f32,
    track_start: f32,
    track_size: f32,
    thumb_size: f32,
    max_scroll: usize,
) -> usize {
    let thumb_travel = (track_size - thumb_size).max(1.0);
    let thumb_pos = (coord - track_start - thumb_size / 2.0).clamp(0.0, thumb_travel);
    let ratio = thumb_pos / thumb_travel;
    (ratio * max_scroll as f32).round() as usize
}

// ============================================================================
// Colors
// ============================================================================

/// Colors for rendering a scrollbar
#[derive(Debug, Clone, Copy)]
pub struct ScrollbarColors {
    /// Track background color (ARGB u32)
    pub track: u32,
    /// Thumb color when idle (ARGB u32)
    pub thumb: u32,
    /// Thumb color when hovered (ARGB u32)
    pub thumb_hover: u32,
}

// ============================================================================
// Rendering
// ============================================================================

/// Render a scrollbar (track + thumb) into the frame.
///
/// When `geometry.needed` is false (content fits in viewport), only the track
/// background is drawn — no thumb.
pub fn render_scrollbar(
    frame: &mut Frame,
    geometry: &ScrollbarGeometry,
    hovered: bool,
    colors: &ScrollbarColors,
) {
    // Draw track background
    frame.fill_rect(geometry.track_rect, colors.track);

    if geometry.needed {
        // Inset thumb by 2px on each side for visual breathing room
        let thumb = inset_rect(geometry.thumb_rect, 2.0);
        let color = if hovered {
            colors.thumb_hover
        } else {
            colors.thumb
        };
        frame.fill_rect(thumb, color);
    }
}

// ============================================================================
// Internal helpers
// ============================================================================

/// Compute thumb size in pixels given visible/total ratio and track length.
fn thumb_size(visible: usize, total: usize, track_len: f32) -> f32 {
    let ratio = (visible as f32 / total as f32).clamp(0.0, 1.0);
    (ratio * track_len).max(MIN_THUMB_PX)
}

/// Compute thumb offset in pixels given scroll position ratio and available travel distance.
fn thumb_offset(position: usize, max_position: usize, travel: f32) -> f32 {
    if max_position == 0 {
        return 0.0;
    }
    let ratio = position as f32 / max_position as f32;
    (ratio * travel).clamp(0.0, travel)
}

/// Shrink a rect uniformly by `amount` on each side.
fn inset_rect(r: Rect, amount: f32) -> Rect {
    Rect::new(
        r.x + amount,
        r.y + amount,
        (r.width - 2.0 * amount).max(0.0),
        (r.height - 2.0 * amount).max(0.0),
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn test_scrollbar_not_needed_when_content_fits() {
        let track = make_rect(0.0, 0.0, 12.0, 400.0);
        let state = ScrollbarState::new(20, 30, 0);
        let geo = ScrollbarGeometry::vertical(track, &state);
        assert!(!geo.needed);
    }

    #[test]
    fn test_vertical_thumb_at_top() {
        let track = make_rect(0.0, 0.0, 12.0, 400.0);
        let state = ScrollbarState::new(100, 20, 0);
        let geo = ScrollbarGeometry::vertical(track, &state);
        assert!(geo.needed);
        // Thumb should be at track top
        assert!(
            (geo.thumb_rect.y - 0.0).abs() < 1.0,
            "thumb should start at top"
        );
    }

    #[test]
    fn test_vertical_thumb_at_bottom() {
        let track = make_rect(0.0, 0.0, 12.0, 400.0);
        let state = ScrollbarState::new(100, 20, 80); // position = max
        let geo = ScrollbarGeometry::vertical(track, &state);
        assert!(geo.needed);
        // Thumb bottom should be at track bottom
        let thumb_bottom = geo.thumb_rect.y + geo.thumb_rect.height;
        assert!(
            (thumb_bottom - 400.0).abs() < 1.0,
            "thumb bottom should be at track bottom, got {}",
            thumb_bottom
        );
    }

    #[test]
    fn test_thumb_size_proportional() {
        let track = make_rect(0.0, 0.0, 12.0, 400.0);
        // 50% visible → 50% thumb size = 200px
        let state = ScrollbarState::new(100, 50, 0);
        let geo = ScrollbarGeometry::vertical(track, &state);
        assert!((geo.thumb_rect.height - 200.0).abs() < 1.0);
    }

    #[test]
    fn test_thumb_minimum_size() {
        let track = make_rect(0.0, 0.0, 12.0, 400.0);
        // 1% visible → would be 4px but minimum is 20px
        let state = ScrollbarState::new(10000, 1, 0);
        let geo = ScrollbarGeometry::vertical(track, &state);
        assert!(
            geo.thumb_rect.height >= MIN_THUMB_PX,
            "thumb should be at least {} px, got {}",
            MIN_THUMB_PX,
            geo.thumb_rect.height
        );
    }

    #[test]
    fn test_position_from_drag_at_start() {
        let track = make_rect(0.0, 0.0, 12.0, 400.0);
        let state = ScrollbarState::new(100, 20, 0);
        let geo = ScrollbarGeometry::vertical(track, &state);
        // Drag grab offset 0, mouse at track start → position 0
        let pos = geo.position_from_drag(0.0, 0.0, &state);
        assert_eq!(pos, 0);
    }

    #[test]
    fn test_position_from_drag_at_end() {
        let track = make_rect(0.0, 0.0, 12.0, 400.0);
        let state = ScrollbarState::new(100, 20, 0);
        let geo = ScrollbarGeometry::vertical(track, &state);
        // Drag to bottom
        let pos = geo.position_from_drag(0.0, 400.0, &state);
        assert_eq!(pos, state.max_position());
    }

    #[test]
    fn test_horizontal_scrollbar() {
        let track = make_rect(0.0, 388.0, 600.0, 12.0);
        let state = ScrollbarState::new(200, 50, 0);
        let geo = ScrollbarGeometry::horizontal(track, &state);
        assert!(geo.needed);
        // Thumb should be 25% of track width = 150px
        assert!((geo.thumb_rect.width - 150.0).abs() < 1.0);
    }

    #[test]
    fn test_hits_thumb() {
        let track = make_rect(100.0, 0.0, 12.0, 400.0);
        let state = ScrollbarState::new(100, 20, 0);
        let geo = ScrollbarGeometry::vertical(track, &state);
        // Should hit the thumb (top)
        assert!(geo.hits_thumb(105.0, 10.0));
        // Should not hit thumb far down
        assert!(!geo.hits_thumb(105.0, 390.0));
    }
}
