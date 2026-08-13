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

use crate::layout::snapshot::TextLine;
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

/// Single-line measurement of source lines (no wrapping); `\n` still
/// breaks and a trailing newline yields its empty final line.
pub fn measure_lines(text: &str, style: TextStyle, measure: &mut dyn TextMeasure) -> Vec<TextLine> {
    let mut out = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        out.push(TextLine {
            range: offset..offset + body.len(),
            width: measure.width(body, style),
        });
        offset += line.len();
    }
    if text.is_empty() {
        out.push(TextLine {
            range: 0..0,
            width: 0.0,
        });
    } else if text.ends_with('\n') {
        out.push(TextLine {
            range: text.len()..text.len(),
            width: 0.0,
        });
    }
    out
}

/// Word-wrap to a pixel width: per source line, take the widest window
/// that fits, preferring the last whitespace strictly inside the window as
/// the break point and hard-breaking tokens wider than a full line
/// (the hover card's historical `wrap_zone_text` semantics, width-based
/// instead of column-based).
pub fn wrap_to_width(
    text: &str,
    style: TextStyle,
    max_w: f32,
    measure: &mut dyn TextMeasure,
) -> Vec<TextLine> {
    let mut out = Vec::new();
    let mut line_offset = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        // Per-char byte offsets and cumulative widths: cum[k] = width of
        // chars[0..k]. Additivity of the measure (see `TextMeasure` docs)
        // makes this exact.
        let chars: Vec<(usize, char)> = line.char_indices().collect();
        let mut cum: Vec<f32> = Vec::with_capacity(chars.len() + 1);
        cum.push(0.0);
        {
            let mut buf = [0u8; 4];
            for &(_, ch) in &chars {
                let ch_w = measure.width(ch.encode_utf8(&mut buf), style);
                cum.push(cum.last().unwrap() + ch_w);
            }
        }
        let total = *cum.last().unwrap();
        if total <= max_w || chars.is_empty() {
            out.push(TextLine {
                range: line_offset..line_offset + line.len(),
                width: total,
            });
            line_offset += raw.len();
            continue;
        }

        let byte_at = |k: usize| chars.get(k).map(|&(b, _)| b).unwrap_or(line.len());
        let mut start = 0usize; // char index
        while start < chars.len() {
            // Widest window: the furthest k with cum[k] - cum[start] <= max_w,
            // always at least one char for progress.
            let mut hard_end = start + 1;
            while hard_end < chars.len() && cum[hard_end + 1] - cum[start] <= max_w {
                hard_end += 1;
            }
            let end = if hard_end < chars.len() {
                chars[start..hard_end]
                    .iter()
                    .rposition(|&(_, c)| c.is_whitespace())
                    .map(|p| start + p)
                    .filter(|&p| p > start)
                    .unwrap_or(hard_end)
            } else {
                hard_end
            };
            out.push(TextLine {
                range: line_offset + byte_at(start)..line_offset + byte_at(end),
                width: cum[end] - cum[start],
            });
            start = end;
            while start < chars.len() && chars[start].1.is_whitespace() {
                start += 1;
            }
        }
        line_offset += raw.len();
    }
    if text.is_empty() {
        out.push(TextLine {
            range: 0..0,
            width: 0.0,
        });
    } else if text.ends_with('\n') {
        out.push(TextLine {
            range: text.len()..text.len(),
            width: 0.0,
        });
    }
    out
}
