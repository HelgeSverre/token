use std::path::Path;
use token::image::ImageState;
use token::util::is_image_file;

#[test]
fn test_image_file_detection() {
    assert!(is_image_file(Path::new("test.png")));
    assert!(is_image_file(Path::new("test.jpg")));
    assert!(is_image_file(Path::new("test.jpeg")));
    assert!(is_image_file(Path::new("test.gif")));
    assert!(is_image_file(Path::new("test.bmp")));
    assert!(is_image_file(Path::new("test.webp")));
    assert!(!is_image_file(Path::new("test.rs")));
    assert!(!is_image_file(Path::new("test.txt")));
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
fn test_compute_fit_scale_zero_dimension_image() {
    // Zero-width or zero-height image should return 1.0, not infinity
    let scale = ImageState::compute_fit_scale(0, 100, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);

    let scale = ImageState::compute_fit_scale(100, 0, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);

    let scale = ImageState::compute_fit_scale(0, 0, 800, 600);
    assert!((scale - 1.0).abs() < f64::EPSILON);
}
