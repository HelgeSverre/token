//! Immediate-mode UI layout engine for Token's chrome.
//!
//! A pure-Rust adaptation of the layout model from Clay
//! (<https://github.com/nicbarker/clay>): a declarative element tree solved
//! by a multi-pass algorithm — fit sizing bottom-up, grow/shrink
//! distribution top-down (Clay's equalize-smallest-first), text wrapping via
//! a measure callback, then positioning with clip chains and floating
//! (anchored) elements. The output is a [`snapshot::LayoutSnapshot`]: solved
//! geometry that rendering, hit-testing, and update-layer queries all read,
//! so independently derived geometry — the bug class AGENTS.md warns about —
//! cannot exist for surfaces on the engine.
//!
//! Deliberate divergences from Clay:
//! - No arena, macros, global context, or hashed string IDs. The tree is a
//!   `Vec`-backed builder ([`tree::UiTree`]) and element identity is the
//!   typed [`keys::UiKey`] enum, giving exhaustive `HitTarget` mapping.
//! - Scroll state lives in the app model (Elm purity); the engine only reads
//!   offsets and offers one clamp formula ([`snapshot::RowListView`]).
//! - The primary output is a queryable snapshot, not a render-command array:
//!   Token's imperative painters query rects and draw as before.
//! - Token extension: [`tree::Content::RowList`] virtualizes uniform-row
//!   lists (Problems/Outline panels) as a single node, so thousand-row
//!   diagnostic lists never materialize per-row elements.
//! - Token extension: [`anchor::FloatAnchor::Caret`] carries the editor's
//!   proven popup placement semantics (flip above when space below is
//!   short, edge clamping, `WidthRule` width resolution) moved here from
//!   `view::overlay_surface`.

pub mod algorithm;
pub mod anchor;
pub mod chrome;
pub mod keys;
pub mod sizing;
pub mod snapshot;
pub mod text;
pub mod tree;

pub use anchor::{AttachPoint, FloatAnchor, FloatDecl, WidthRule};
pub use keys::UiKey;
pub use sizing::{AlignX, AlignY, Dir, Padding, Sizing, SizingAxes};
pub use snapshot::{LayoutSnapshot, RowListView, SolvedContent, SolvedNode};
pub use text::{CellMeasure, PainterMeasure, TextMeasure, TextStyle};
pub use tree::{Content, ElementDecl, RowListDecl, ScrollDecl, TextDecl, UiTree, Wrap};
