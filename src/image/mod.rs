//! Image viewer module
//!
//! Provides image viewing with pan and zoom support.
//! Images are decoded into RGBA pixel buffers and rendered
//! with nearest-neighbor scaling.

pub mod render;

/// State for the image viewer mode
#[derive(Debug, Clone)]
pub struct ImageState {
    /// Decoded RGBA pixel data (4 bytes per pixel)
    pub pixels: Vec<u8>,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
    /// File size in bytes (for status bar)
    pub file_size: u64,
    /// Image format name (e.g. "PNG", "JPEG")
    pub format: String,
    /// Current zoom level (1.0 = 100%)
    pub scale: f64,
    /// Pan offset X in image-space pixels
    pub offset_x: f64,
    /// Pan offset Y in image-space pixels
    pub offset_y: f64,
    /// Whether the user has manually zoomed (disables auto-fit on resize)
    pub user_zoomed: bool,
    /// Last known mouse position (screen coords, for zoom-toward-cursor)
    pub last_mouse_x: f64,
    /// Last known mouse position (screen coords, for zoom-toward-cursor)
    pub last_mouse_y: f64,
    /// Active drag state for panning
    pub drag: Option<DragState>,
}

/// Drag state for click-and-drag panning
#[derive(Debug, Clone)]
pub struct DragState {
    /// Mouse position when drag started (screen coords)
    pub start_mouse_x: f64,
    pub start_mouse_y: f64,
    /// Image offset when drag started
    pub start_offset_x: f64,
    pub start_offset_y: f64,
}

impl ImageState {
    /// Create a new ImageState from decoded image data.
    ///
    /// Computes initial scale: fit-to-viewport if image is larger,
    /// actual size (1.0) if image fits.
    pub fn new(
        pixels: Vec<u8>,
        width: u32,
        height: u32,
        file_size: u64,
        format: String,
        viewport_width: u32,
        viewport_height: u32,
    ) -> Self {
        let scale = Self::compute_fit_scale(width, height, viewport_width, viewport_height);
        Self {
            pixels,
            width,
            height,
            file_size,
            format,
            scale,
            offset_x: 0.0,
            offset_y: 0.0,
            user_zoomed: false,
            last_mouse_x: 0.0,
            last_mouse_y: 0.0,
            drag: None,
        }
    }

    /// Compute scale to fit image within viewport.
    /// Returns 1.0 if image already fits, otherwise scales down.
    pub fn compute_fit_scale(
        img_width: u32,
        img_height: u32,
        viewport_width: u32,
        viewport_height: u32,
    ) -> f64 {
        if viewport_width == 0 || viewport_height == 0 || img_width == 0 || img_height == 0 {
            return 1.0;
        }
        let scale_x = viewport_width as f64 / img_width as f64;
        let scale_y = viewport_height as f64 / img_height as f64;
        let fit_scale = scale_x.min(scale_y);
        // Only scale down, never scale up for auto-fit
        fit_scale.min(1.0)
    }

    /// Get the zoom level as a percentage integer (e.g. 100 for 1.0)
    pub fn zoom_percent(&self) -> u32 {
        (self.scale * 100.0).round() as u32
    }

    /// Format file size for display (e.g. "2.4 MB", "128 KB")
    pub fn file_size_display(&self) -> String {
        if self.file_size >= 1_048_576 {
            format!("{:.1} MB", self.file_size as f64 / 1_048_576.0)
        } else if self.file_size >= 1024 {
            format!("{:.0} KB", self.file_size as f64 / 1024.0)
        } else {
            format!("{} B", self.file_size)
        }
    }
}

/// Load and decode an image file into an ImageState.
///
/// Returns None if the file can't be read or decoded.
pub fn load_image(
    path: &std::path::Path,
    viewport_width: u32,
    viewport_height: u32,
) -> Option<ImageState> {
    let file_size = std::fs::metadata(path).ok()?.len();

    let format = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| match e.to_lowercase().as_str() {
            "jpg" | "jpeg" => "JPEG".to_string(),
            "png" => "PNG".to_string(),
            "gif" => "GIF".to_string(),
            "bmp" => "BMP".to_string(),
            "webp" => "WebP".to_string(),
            other => other.to_uppercase(),
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let img = image::open(path).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels = rgba.into_raw();

    Some(ImageState::new(
        pixels,
        width,
        height,
        file_size,
        format,
        viewport_width,
        viewport_height,
    ))
}
