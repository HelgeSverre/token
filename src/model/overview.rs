//! Memoized per-pane overview projection. Geometry and painting live in view.

use std::{cell::RefCell, sync::Arc};

use super::{ui::FindResults, Mark};

/// Derived scrollbar data for one editor pane; callers never need to invalidate it.
#[derive(Debug, Clone, Default)]
pub struct OverviewCache(pub(crate) RefCell<Option<OverviewProjection>>);

#[derive(Debug, Clone)]
pub(crate) struct OverviewProjection {
    pub buffer: ropey::Rope,
    pub revision: u64,
    pub wrap_identity: Option<Arc<()>>,
    pub find: Option<Arc<FindResults>>,
    pub diagnostics: Vec<(lsp_types::Range, Mark)>,
    pub total_rows: usize,
    pub track_height: f32,
    pub rows: Arc<[Option<Mark>]>,
}
