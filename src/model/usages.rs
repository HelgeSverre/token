//! Persistent reference results, independent of the cursor-anchored popup.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use crate::update::navigation::LocationItem;

/// Shared preparation/display bound; reaching it is explicitly shown in the panel.
pub const MAX_REFERENCE_LOCATIONS: usize = 200;

/// Captured destination of a references request, never inferred at reply time.
#[derive(Debug, Clone)]
pub enum ReferencesTarget {
    Popup,
    Panel(Arc<()>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsagesRow {
    Summary,
    File {
        first: usize,
        count: usize,
        collapsed: bool,
    },
    Location(usize),
}

#[derive(Debug, Clone)]
pub struct UsagesPanelState {
    pub items: Vec<LocationItem>,
    pub selected_index: Option<usize>,
    pub scroll_offset: usize,
    pub collapsed: HashSet<PathBuf>,
    pub source: String,
    pub status: String,
    pub(crate) query: Option<UsagesQuery>,
}

#[derive(Debug, Clone)]
pub(crate) struct UsagesQuery {
    pub token: Arc<()>,
    pub document_id: super::DocumentId,
    pub revision: u64,
}

impl Default for UsagesPanelState {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected_index: None,
            scroll_offset: 0,
            collapsed: HashSet::new(),
            source: String::new(),
            status: "Run Find Usages at a symbol to search".into(),
            query: None,
        }
    }
}

impl UsagesPanelState {
    /// The single row-order projection used by layout, painting and interaction.
    pub fn rows(&self) -> Vec<UsagesRow> {
        let mut rows = vec![UsagesRow::Summary];
        let mut first = 0;
        for group in self.items.chunk_by(|a, b| a.path == b.path) {
            let collapsed = self.collapsed.contains(&group[0].path);
            rows.push(UsagesRow::File {
                first,
                count: group.len(),
                collapsed,
            });
            if !collapsed {
                rows.extend((first..first + group.len()).map(UsagesRow::Location));
            }
            first += group.len();
        }
        rows
    }

    pub fn is_loading(&self) -> bool {
        self.query.is_some()
    }
}
