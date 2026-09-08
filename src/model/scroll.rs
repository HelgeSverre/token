//! Pixel geometry for grid-backed viewports. Document indices stay integral;
//! the displacement within the first cell does not have to be.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelAxis {
    /// Displacement within the first visible cell, in physical pixels.
    pub offset: f64,
    pub unit: f64,
    pub extent: f64,
}

impl PixelAxis {
    pub fn new(unit: f64, extent: f64) -> Self {
        Self {
            offset: 0.0,
            unit: unit.max(1.0),
            extent: extent.max(0.0),
        }
    }

    /// Keep the same within-cell position when font/DPI metrics change.
    pub fn resize(&mut self, unit: f64, extent: f64) {
        let unit = unit.max(1.0);
        self.offset *= unit / self.unit;
        self.unit = unit;
        self.extent = extent.max(0.0);
    }

    pub fn position(self, first: usize) -> f64 {
        first as f64 * self.unit + self.offset
    }

    /// Update the integral grid size without discarding the measured edge cell.
    pub fn set_visible_cells(&mut self, count: usize) {
        self.extent = count as f64 * self.unit + self.extent % self.unit;
    }

    pub fn max_position(self, count: usize) -> f64 {
        (count as f64 * self.unit - self.extent).max(0.0)
    }

    /// Clamp once and normalize into a cell index plus sub-cell displacement.
    /// Non-finite input is ignored rather than poisoning subsequent geometry.
    pub fn set_position(&mut self, first: &mut usize, position: f64, count: usize) -> bool {
        if !position.is_finite() {
            return false;
        }
        let position = position.clamp(0.0, self.max_position(count));
        let index = (position / self.unit).floor() as usize;
        let offset = position - index as f64 * self.unit;
        let changed = *first != index || self.offset != offset;
        *first = index;
        self.offset = offset;
        changed
    }

    /// Includes both partially visible edge cells; consumers clip to the viewport.
    pub fn drawn_count(self) -> usize {
        if self.extent <= 0.0 {
            return 0;
        }
        ((self.extent + self.offset.round()) / self.unit).ceil() as usize
    }

    pub fn cell_at_pixel(self, pixel: f64) -> usize {
        ((pixel.max(0.0) + self.offset.round()) / self.unit).floor() as usize
    }

    /// Subtract locally, not two large absolute pixel positions: long documents
    /// retain fine positioning precision. Negative origins are valid when clipped.
    pub fn cell_origin(self, visible_cell: usize) -> f64 {
        visible_cell as f64 * self.unit - self.offset.round()
    }

    /// Reveal a complete cell while preserving a fractional position that already
    /// satisfies the safe region. The caller supplies the content bound for clamp.
    pub fn reveal(
        self,
        first: usize,
        cell: usize,
        padding: usize,
        mode: super::ScrollRevealMode,
    ) -> f64 {
        use super::ScrollRevealMode;
        let current = self.position(first);
        let padding = (padding as f64 * self.unit).min(((self.extent - self.unit) / 2.0).max(0.0));
        let top = cell as f64 * self.unit;
        let bottom = top + self.unit;
        let above = top < current + padding;
        let below = bottom > current + self.extent - padding;
        if !above && !below {
            return current;
        }
        match mode {
            ScrollRevealMode::Minimal if above => top - padding,
            ScrollRevealMode::Minimal | ScrollRevealMode::BottomAligned => {
                bottom - self.extent + padding
            }
            ScrollRevealMode::TopAligned => top - padding,
            ScrollRevealMode::Centered => top - (self.extent / 2.0).floor(),
        }
        .max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelViewport {
    pub x: PixelAxis,
    pub y: PixelAxis,
}

impl PixelViewport {
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            x: PixelAxis::new(1.0, columns as f64),
            y: PixelAxis::new(1.0, rows as f64),
        }
    }
}

/// A short, deterministic ease-out for discrete wheel input. Platform trackpad
/// pixels are applied directly and never run through this second motion layer.
#[derive(Debug, Clone)]
pub struct ScrollAnimation {
    start: (f64, f64),
    pub target: (f64, f64),
    pub last_position: (f64, f64),
    elapsed: f64,
    pub revision: u64,
    pub cursor: (usize, usize),
}

impl ScrollAnimation {
    const DURATION: f64 = 0.140;

    pub fn new(
        start: (f64, f64),
        target: (f64, f64),
        revision: u64,
        cursor: (usize, usize),
    ) -> Self {
        Self {
            start,
            target,
            last_position: start,
            elapsed: 0.0,
            revision,
            cursor,
        }
    }

    /// Same-direction notches accumulate against the target. Reversing direction
    /// starts at the displayed position so an old target never pulls against input.
    pub fn retarget_axis(current: f64, previous: f64, delta: f64) -> f64 {
        if delta == 0.0 || (previous - current).signum() == delta.signum() {
            previous + delta
        } else {
            current + delta
        }
    }

    pub fn advance(&mut self, seconds: f64) -> (f64, f64) {
        if seconds.is_finite() && seconds > 0.0 {
            self.elapsed = (self.elapsed + seconds).min(Self::DURATION);
        }
        let t = self.elapsed / Self::DURATION;
        let weight = 1.0 - (1.0 - t).powi(3);
        self.last_position = if self.finished() {
            self.target
        } else {
            (
                self.start.0 + (self.target.0 - self.start.0) * weight,
                self.start.1 + (self.target.1 - self.start.1) * weight,
            )
        };
        self.last_position
    }

    pub fn finished(&self) -> bool {
        self.elapsed >= Self::DURATION
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_reveal_retains_safe_offsets_and_supports_navigation_modes() {
        use crate::model::ScrollRevealMode::*;
        let axis = PixelAxis {
            offset: 7.0,
            ..PixelAxis::new(20.0, 95.0)
        };
        assert_eq!(axis.reveal(10, 12, 1, Minimal), 207.0);
        assert_eq!(axis.reveal(10, 9, 1, TopAligned), 160.0);
        assert_eq!(axis.reveal(10, 15, 1, BottomAligned), 245.0);
        assert_eq!(axis.reveal(10, 20, 1, Centered), 353.0);
    }

    #[test]
    fn easing_is_elapsed_time_based_retargets_and_settles_exactly() {
        let mut one = ScrollAnimation::new((0.0, 0.0), (30.0, 90.0), 0, (0, 0));
        let mut many = one.clone();
        let halfway = one.advance(0.070);
        for _ in 0..7 {
            many.advance(0.010);
        }
        assert!((halfway.1 - many.last_position.1).abs() < 0.0001);
        assert!(halfway.1 > 45.0 && halfway.1 < 90.0);
        assert_eq!(ScrollAnimation::retarget_axis(halfway.1, 90.0, 30.0), 120.0);
        assert_eq!(
            ScrollAnimation::retarget_axis(halfway.1, 90.0, -30.0),
            halfway.1 - 30.0
        );
        assert_eq!(one.advance(1.0), (30.0, 90.0));
        assert!(one.finished());
    }

    #[test]
    fn pixel_axis_normalizes_clamps_and_matches_partial_cell_hits() {
        let mut axis = PixelAxis::new(20.0, 95.0);
        let mut first = 0;
        assert!(axis.set_position(&mut first, 47.5, 10));
        assert_eq!((first, axis.offset), (2, 7.5));
        assert_eq!(axis.drawn_count(), 6);
        assert_eq!(axis.cell_origin(0), -8.0);
        assert_eq!(axis.cell_at_pixel(11.5), 0);
        assert_eq!(axis.cell_at_pixel(12.0), 1);
        assert!(axis.set_position(&mut first, 1000.0, 10));
        assert_eq!(axis.position(first), 105.0);
        assert_eq!((first, axis.offset), (5, 5.0));
        assert!(!axis.set_position(&mut first, f64::NAN, 10));
        axis.resize(40.0, 190.0);
        assert_eq!(axis.position(first), 210.0);
        assert!(axis.set_position(&mut first, -1.0, 10));
        assert_eq!(axis.position(first), 0.0);
    }
}
