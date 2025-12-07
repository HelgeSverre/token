use token::overlay::{blend_pixel, OverlayAnchor, OverlayConfig};

#[test]
fn test_overlay_anchor_top_left() {
    let config = OverlayConfig::new(OverlayAnchor::TopLeft, 100, 50).with_margin(10);
    let bounds = config.compute_bounds(800, 600);

    assert_eq!(bounds.x, 10);
    assert_eq!(bounds.y, 10);
}

#[test]
fn test_overlay_anchor_top_right() {
    let config = OverlayConfig::new(OverlayAnchor::TopRight, 100, 50).with_margin(10);
    let bounds = config.compute_bounds(800, 600);

    assert_eq!(bounds.x, 800 - 100 - 10);
    assert_eq!(bounds.y, 10);
}

#[test]
fn test_overlay_anchor_bottom_right() {
    let config = OverlayConfig::new(OverlayAnchor::BottomRight, 100, 50).with_margin(10);
    let bounds = config.compute_bounds(800, 600);

    assert_eq!(bounds.x, 800 - 100 - 10);
    assert_eq!(bounds.y, 600 - 50 - 10);
}

#[test]
fn test_overlay_anchor_center() {
    let config = OverlayConfig::new(OverlayAnchor::Center, 100, 50);
    let bounds = config.compute_bounds(800, 600);

    assert_eq!(bounds.x, (800 - 100) / 2);
    assert_eq!(bounds.y, (600 - 50) / 2);
}

#[test]
fn test_blend_pixel_fully_opaque() {
    let src = 0xFF_FF_00_00; // Opaque red
    let dst = 0xFF_00_FF_00; // Opaque green
    let result = blend_pixel(src, dst);
    assert_eq!(result, 0xFF_FF_00_00); // Red wins
}

#[test]
fn test_blend_pixel_fully_transparent() {
    let src = 0x00_FF_00_00; // Transparent red
    let dst = 0xFF_00_FF_00; // Opaque green
    let result = blend_pixel(src, dst);
    assert_eq!(result, 0xFF_00_FF_00); // Green unchanged
}

#[test]
fn test_blend_pixel_half_alpha() {
    let src = 0x80_FF_00_00; // 50% alpha red
    let dst = 0xFF_00_00_00; // Opaque black
    let result = blend_pixel(src, dst);

    // Should be roughly 50% red
    let r = (result >> 16) & 0xFF;
    assert!(r > 120 && r < 135, "Expected ~128, got {}", r);
}
