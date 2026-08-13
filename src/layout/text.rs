//! Text measurement for the layout engine (Clay's measure-text callback).
//!
//! The measurer is passed into [`crate::layout::tree::UiTree::solve`] as a
//! `&mut dyn TextMeasure` for the duration of the solve — no closures stored
//! in the tree, no interior mutability. Two implementations:
//!
//! - [`CellMeasure`] — monospace-cell approximation, painter-free, so chrome
//!   layout is callable from the update layer. Render, hit-test, and update
//!   all use it for chrome, so all three agree by construction (matching
//!   today's `chars().count() * char_width` behavior).
//! - [`PainterMeasure`] — real glyph advances via `TextPainter`, used for
//!   overlays where sub-14px text is measured, not grid-multiplied.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::view::TextPainter;

/// Style inputs that affect measurement: font size and letter tracking, both
/// in physical px (mapping to `TextPainter::measure_sized` inputs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub tracking: f32,
}

impl TextStyle {
    pub fn sized(size: f32) -> Self {
        Self {
            size,
            tracking: 0.0,
        }
    }
}

/// Width/line-height oracle for text layout.
///
/// Implementations must be *additive over characters*: the width of a string
/// equals the sum of its per-character widths. Both implementations here
/// hold that (fontdue advances carry no kerning, and `measure_sized` rounds
/// each advance independently), and the wrapper relies on it to wrap in a
/// single left-to-right pass.
pub trait TextMeasure {
    fn width(&mut self, text: &str, style: TextStyle) -> f32;
    fn line_height(&mut self, style: TextStyle) -> f32;
}

/// Monospace-cell approximation: every char is `char_width` wide and every
/// line is `line_height` tall, regardless of style.
#[derive(Clone, Copy, Debug)]
pub struct CellMeasure {
    pub char_width: f32,
    pub line_height: f32,
}

impl TextMeasure for CellMeasure {
    fn width(&mut self, text: &str, _style: TextStyle) -> f32 {
        text.chars().count() as f32 * self.char_width
    }

    fn line_height(&mut self, _style: TextStyle) -> f32 {
        self.line_height
    }
}

/// Real measurement through the glyph cache. Borrows the painter only for
/// the duration of a solve; the memo keeps repeated solves of identical
/// strings (layout in render, then again in hit-test) cheap.
pub struct PainterMeasure<'p, 'a> {
    painter: &'p mut TextPainter<'a>,
    memo: HashMap<(u64, u32, u32), f32>,
}

impl<'p, 'a> PainterMeasure<'p, 'a> {
    pub fn new(painter: &'p mut TextPainter<'a>) -> Self {
        Self {
            painter,
            memo: HashMap::new(),
        }
    }
}

fn text_hash(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

impl TextMeasure for PainterMeasure<'_, '_> {
    fn width(&mut self, text: &str, style: TextStyle) -> f32 {
        let key = (
            text_hash(text),
            style.size.to_bits(),
            style.tracking.to_bits(),
        );
        if let Some(&w) = self.memo.get(&key) {
            return w;
        }
        let w = self.painter.measure_sized(text, style.size, style.tracking);
        self.memo.insert(key, w);
        w
    }

    fn line_height(&mut self, style: TextStyle) -> f32 {
        self.painter.line_height_for_size(style.size) as f32
    }
}
