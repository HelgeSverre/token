mod common;

use common::test_model;
use std::path::Path;
use token::image::ImageState;
use token::model::{Rect, ViewMode};
use token::util::is_supported_image;

#[test]
fn test_image_file_detection() {
    assert!(is_supported_image(Path::new("test.png")));
    assert!(is_supported_image(Path::new("test.jpg")));
    assert!(is_supported_image(Path::new("test.jpeg")));
    assert!(is_supported_image(Path::new("test.gif")));
    assert!(is_supported_image(Path::new("test.bmp")));
    assert!(is_supported_image(Path::new("test.webp")));
    assert!(is_supported_image(Path::new("test.ico")));
    assert!(!is_supported_image(Path::new("test.rs")));
    assert!(!is_supported_image(Path::new("test.txt")));
}

#[test]
fn test_compute_fit_scale_large_image() {
    // Image larger than viewport should be scaled down
    let scale = ImageState::compute_fit_scale(1920, 1080, 800, 600);
    assert!(scale < 1.0);
    // Should fit width: 800/1920 ≈ 0.4167
    assert!((scale - 800.0 / 1920.0).abs() < 0.01);
}

#[test]
fn test_compute_fit_scale_small_image() {
    // Image smaller than viewport should stay at 1.0
    let scale = ImageState::compute_fit_scale(100, 100, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_compute_fit_scale_zero_viewport() {
    let scale = ImageState::compute_fit_scale(100, 100, 0, 0);
    assert!((scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_compute_fit_scale_zero_dimension_image() {
    let scale = ImageState::compute_fit_scale(0, 100, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);

    let scale = ImageState::compute_fit_scale(100, 0, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);

    let scale = ImageState::compute_fit_scale(0, 0, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_compute_fit_scale_tall_image() {
    // Tall image should fit by height
    let scale = ImageState::compute_fit_scale(100, 2000, 800, 600);
    assert!((scale - 600.0 / 2000.0).abs() < 0.01);
}

#[test]
fn test_file_size_display() {
    let state = ImageState::new(vec![0; 400], 10, 10, 2_500_000, "PNG".into(), 800, 600);
    assert_eq!(state.file_size_display(), "2.4 MB");

    let state = ImageState::new(vec![0; 400], 10, 10, 150_000, "PNG".into(), 800, 600);
    assert_eq!(state.file_size_display(), "146 KB");

    let state = ImageState::new(vec![0; 400], 10, 10, 500, "PNG".into(), 800, 600);
    assert_eq!(state.file_size_display(), "500 B");
}

#[test]
fn test_zoom_percent() {
    let mut state = ImageState::new(vec![0; 400], 10, 10, 100, "PNG".into(), 800, 600);
    state.scale = 1.0;
    assert_eq!(state.zoom_percent(), 100);
    state.scale = 0.5;
    assert_eq!(state.zoom_percent(), 50);
    state.scale = 2.0;
    assert_eq!(state.zoom_percent(), 200);
}

#[test]
fn test_image_state_auto_fit() {
    // Large image should auto-fit below 1.0
    let state = ImageState::new(vec![0; 16000], 2000, 1000, 16000, "PNG".into(), 800, 600);
    assert!(state.scale < 1.0);
    assert!(!state.user_zoomed);
    assert_eq!(state.offset_x, 0.0);
    assert_eq!(state.offset_y, 0.0);
}

#[test]
fn test_image_state_no_upscale() {
    // Small image should not be scaled up
    let state = ImageState::new(vec![0; 400], 10, 10, 400, "PNG".into(), 800, 600);
    assert!((state.scale - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_sync_all_viewports_auto_fits_image_to_group_content_rect() {
    let mut model = test_model("", 0, 0);
    model.editor_area.focused_group_mut().unwrap().rect = Rect::new(0.0, 0.0, 400.0, 300.0);
    model.editor_mut().view_mode = ViewMode::Image(Box::new(ImageState::new(
        vec![0; 1000 * 500 * 4],
        1000,
        500,
        0,
        "PNG".into(),
        1000,
        500,
    )));

    model
        .editor_area
        .sync_all_viewports(model.line_height, model.char_width, &model.metrics);

    let image = model.editor().view_mode.as_image().unwrap();
    assert!((image.scale - 0.4).abs() < 1e-9);
}

#[test]
fn test_sync_all_viewports_does_not_override_user_zoomed_image_scale() {
    let mut model = test_model("", 0, 0);
    model.editor_area.focused_group_mut().unwrap().rect = Rect::new(0.0, 0.0, 400.0, 300.0);

    let mut image = ImageState::new(
        vec![0; 1000 * 500 * 4],
        1000,
        500,
        0,
        "PNG".into(),
        1000,
        500,
    );
    image.scale = 2.0;
    image.user_zoomed = true;
    model.editor_mut().view_mode = ViewMode::Image(Box::new(image));

    model
        .editor_area
        .sync_all_viewports(model.line_height, model.char_width, &model.metrics);

    let image = model.editor().view_mode.as_image().unwrap();
    assert!((image.scale - 2.0).abs() < 1e-9);
}
