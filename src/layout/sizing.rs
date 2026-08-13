//! Sizing, direction, padding, and alignment primitives.
//!
//! All values are physical pixels as `f32` (the engine consumes
//! already-scaled `ScaledMetrics` tokens); [`crate::layout::snapshot`]
//! exposes the one snap-to-pixel helper so integer rounding happens in a
//! single place at paint time.

/// How an element sizes itself along one axis (Clay's sizing modes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    /// Size to content (children or text), clamped to `[min, max]`.
    Fit { min: f32, max: f32 },
    /// Take a share of the parent's leftover space along the parent's
    /// layout axis, clamped to `[min, max]`. On the cross axis, grow to the
    /// parent's content size.
    Grow { min: f32, max: f32 },
    /// Fraction (`0..=1`) of the parent's content box on this axis.
    Percent(f32),
    /// Exact pixel size.
    Fixed(f32),
}

impl Sizing {
    pub const FIT: Sizing = Sizing::Fit {
        min: 0.0,
        max: f32::INFINITY,
    };
    pub const GROW: Sizing = Sizing::Grow {
        min: 0.0,
        max: f32::INFINITY,
    };

    pub fn fit_min(min: f32) -> Sizing {
        Sizing::Fit {
            min,
            max: f32::INFINITY,
        }
    }

    pub fn grow_clamped(min: f32, max: f32) -> Sizing {
        Sizing::Grow { min, max }
    }
}

impl Default for Sizing {
    fn default() -> Self {
        Sizing::FIT
    }
}

/// Per-axis sizing for one element.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SizingAxes {
    pub w: Sizing,
    pub h: Sizing,
}

impl SizingAxes {
    pub fn new(w: Sizing, h: Sizing) -> Self {
        Self { w, h }
    }

    pub fn fixed(w: f32, h: f32) -> Self {
        Self {
            w: Sizing::Fixed(w),
            h: Sizing::Fixed(h),
        }
    }

    pub fn grow() -> Self {
        Self {
            w: Sizing::GROW,
            h: Sizing::GROW,
        }
    }
}

/// Layout direction: the axis children are stacked along.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dir {
    /// Children left-to-right (Clay `LEFT_TO_RIGHT`).
    Row,
    /// Children top-to-bottom (Clay `TOP_TO_BOTTOM`).
    #[default]
    Column,
}

/// Inset space between an element's border box and its children.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Padding {
    pub l: f32,
    pub r: f32,
    pub t: f32,
    pub b: f32,
}

impl Padding {
    pub fn all(v: f32) -> Self {
        Self {
            l: v,
            r: v,
            t: v,
            b: v,
        }
    }

    pub fn xy(x: f32, y: f32) -> Self {
        Self {
            l: x,
            r: x,
            t: y,
            b: y,
        }
    }

    pub fn x(&self) -> f32 {
        self.l + self.r
    }

    pub fn y(&self) -> f32 {
        self.t + self.b
    }
}

/// Horizontal child alignment within leftover parent space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignX {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical child alignment within leftover parent space.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignY {
    #[default]
    Top,
    Center,
    Bottom,
}
