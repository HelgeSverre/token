//! Floating-element anchoring: Token's popup placement semantics.
//!
//! Clay's floating elements attach to a parent or an element ID; Token's
//! popups additionally need the editor's proven placement rules — width
//! resolved by a [`WidthRule`], flip-above when there isn't room below the
//! caret, edge clamping — which moved here from `view::overlay_surface`
//! (`resolve_panel_width` / `position_panel`) so both the engine and the
//! overlay surface share one implementation.

use crate::layout::keys::UiKey;

/// Logical-px anchor constants (Visual Language > Chrome).
mod dims {
    /// Margin kept against the window edges when resolving a width rule.
    pub const EDGE_MARGIN: f32 = 32.0;
    /// Centered overlay Y (clamped to `window_height / 4`).
    pub const CENTERED_Y: f32 = 64.0;
    /// Gap between the anchor line and a caret-anchored popup.
    pub const CURSOR_GAP: f32 = 2.0;
    /// Caret-anchored popup minimum width ("200px floor").
    pub const CURSOR_WIDTH_FLOOR: f32 = 200.0;
}

#[inline]
fn scaled(v: f32, scale_factor: f64) -> usize {
    (v as f64 * scale_factor).round().max(1.0) as usize
}

#[inline]
fn size_px(logical: f32, scale_factor: f64) -> f32 {
    (logical as f64 * scale_factor) as f32
}

/// A panel width rule: percent of window width, clamped to a logical-px
/// min/max, then clamped again to leave a margin against the window edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidthRule {
    pub pct: f32,
    pub min: f32,
    pub max: f32,
}

/// Where a floating element attaches on its target's rect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachPoint {
    /// Float's top-left at the target's bottom-left (menus, dropdowns).
    BelowLeft,
    /// Float's top-right at the target's bottom-right.
    BelowRight,
    /// Float's bottom-left at the target's top-left.
    AboveLeft,
    /// Float's top-left at the target's top-right (submenus).
    RightTop,
}

/// How a floating element is positioned (solved after the normal flow).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloatAnchor {
    /// Centered X in the window; Y = `min(64 logical, window_height / 4)`.
    WindowCentered,
    /// Token's caret anchor (physical px): `(x, y)` is the caret's top-left
    /// and `line_h` its line height, so flipping above clears the caret's
    /// whole line. Places below the line + gap, flips above when below
    /// lacks `panel_h` of space, then edge-clamps to the window.
    Caret {
        x: f32,
        y: f32,
        line_h: f32,
        prefer_below: bool,
    },
    /// Attach to another solved element's rect.
    Element { target: UiKey, attach: AttachPoint },
}

/// Declaration payload making an element float out of the normal flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatDecl {
    pub anchor: FloatAnchor,
    /// Draw-order layer; floats sort after in-flow content by `(z, decl order)`.
    pub z: i16,
    /// Optional width resolution against the window; overrides the
    /// element's declared width sizing when present.
    pub width: Option<WidthRule>,
}

/// Resolve a [`WidthRule`] against the window width: percent-of-window,
/// clamped to the logical-px min/max, then margin-clamped to the window
/// edges. Caret-anchored panels get an additional 200px logical floor, and
/// never exceed the window itself even when that floor is wider than
/// `window_width - margin` (narrow-window degradation).
pub fn resolve_width(
    rule: &WidthRule,
    cursor_anchored: bool,
    window_width: usize,
    scale_factor: f64,
) -> usize {
    let margin = scaled(dims::EDGE_MARGIN, scale_factor);
    let min_w = size_px(rule.min, scale_factor) as usize;
    let max_w = size_px(rule.max, scale_factor) as usize;
    let mut panel_w = ((window_width as f32 * rule.pct) as usize)
        .clamp(min_w, max_w)
        .min(window_width.saturating_sub(margin));
    if cursor_anchored {
        panel_w = panel_w.max(scaled(dims::CURSOR_WIDTH_FLOOR, scale_factor));
    }
    panel_w.min(window_width)
}

/// Position a window-centered panel: centered X, `min(64 logical, h/4)` Y.
pub fn position_centered(
    window_width: usize,
    window_height: usize,
    panel_w: usize,
    scale_factor: f64,
) -> (usize, usize) {
    let x = window_width.saturating_sub(panel_w) / 2;
    let y = scaled(dims::CENTERED_Y, scale_factor).min(window_height / 4);
    (x, y)
}

/// Position a caret-anchored panel: below the anchor line + gap, flipping
/// above when there isn't `panel_h` of space below, then edge-clamped to
/// the window (pinned to the bottom when neither direction fits).
#[allow(clippy::too_many_arguments)]
pub fn position_at_caret(
    x: usize,
    y: usize,
    line_h: usize,
    prefer_below: bool,
    window_width: usize,
    window_height: usize,
    panel_w: usize,
    panel_h: usize,
    scale_factor: f64,
) -> (usize, usize) {
    let gap = scaled(dims::CURSOR_GAP, scale_factor);
    let px = x.min(window_width.saturating_sub(panel_w));
    let below_y = y.saturating_add(line_h).saturating_add(gap);
    let fits_below = below_y.saturating_add(panel_h) <= window_height;
    let fits_above = y >= panel_h.saturating_add(gap);
    let py = if prefer_below && fits_below {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SF: f64 = 1.0;

    fn rule(pct: f32, min: f32, max: f32) -> WidthRule {
        WidthRule { pct, min, max }
    }

    #[test]
    fn resolve_width_percent_clamped_to_min_max() {
        // 50% of 1000 = 500, inside [300, 600].
        assert_eq!(
            resolve_width(&rule(0.5, 300.0, 600.0), false, 1000, SF),
            500
        );
        // 10% of 1000 = 100, floored to min.
        assert_eq!(
            resolve_width(&rule(0.1, 300.0, 600.0), false, 1000, SF),
            300
        );
        // 90% of 1000 = 900, capped to max.
        assert_eq!(
            resolve_width(&rule(0.9, 300.0, 600.0), false, 1000, SF),
            600
        );
    }

    #[test]
    fn resolve_width_respects_edge_margin() {
        // Min 500 in a 520 window: margin (32) clamps to 488.
        assert_eq!(resolve_width(&rule(0.5, 500.0, 900.0), false, 520, SF), 488);
    }

    #[test]
    fn resolve_width_cursor_floor_never_exceeds_window() {
        // 200px floor beats the margin clamp in a 210px window, but never
        // the window itself.
        assert_eq!(resolve_width(&rule(0.0, 0.0, 420.0), true, 210, SF), 200);
        assert_eq!(resolve_width(&rule(0.0, 0.0, 420.0), true, 150, SF), 150);
    }

    #[test]
    fn centered_position() {
        assert_eq!(position_centered(1000, 800, 400, SF), (300, 64));
        // Short window: y = h/4.
        assert_eq!(position_centered(1000, 200, 400, SF), (300, 50));
    }

    #[test]
    fn caret_prefers_below_when_it_fits() {
        // Anchor line at y=100, line 20 tall; popup 100 tall in an 800 window.
        let (x, y) = position_at_caret(50, 100, 20, true, 1000, 800, 300, 100, SF);
        assert_eq!((x, y), (50, 122)); // 100 + 20 + 2
    }

    #[test]
    fn caret_flips_above_when_below_is_short() {
        // Window 250 tall, anchor at 200: below has 28px, popup needs 100.
        let (_, y) = position_at_caret(50, 200, 20, true, 1000, 250, 300, 100, SF);
        assert_eq!(y, 98); // 200 - 2 - 100
    }

    #[test]
    fn caret_prefer_above_falls_back_below() {
        // prefer_below=false but anchor near the top: only below fits.
        let (_, y) = position_at_caret(50, 10, 20, false, 1000, 800, 300, 100, SF);
        assert_eq!(y, 32); // 10 + 20 + 2
    }

    #[test]
    fn caret_pins_to_bottom_when_neither_fits() {
        // Popup taller than the space on either side.
        let (_, y) = position_at_caret(50, 100, 20, true, 1000, 260, 300, 200, SF);
        assert_eq!(y, 60); // window_height - panel_h
    }

    #[test]
    fn caret_clamps_x_to_right_edge() {
        let (x, _) = position_at_caret(950, 100, 20, true, 1000, 800, 300, 100, SF);
        assert_eq!(x, 700);
    }
}
