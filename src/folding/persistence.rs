//! Versioned, bounded fold metadata; never source snippets or parser node IDs.
use super::{FoldCandidates, FoldRegion};
use crate::{
    model::{Document, EditorState, Position},
    util::ByteSize,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFold {
    kind: String,
    line: usize,
    fingerprint: u64,
    context: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFolds {
    version: u8,
    content: u64,
    regions: Vec<SavedFold>,
}

impl SavedFolds {
    fn capture(candidates: &FoldCandidates, collapsed: &[FoldRegion]) -> Self {
        Self {
            version: 1,
            content: candidates.content_fingerprint,
            regions: collapsed
                .iter()
                .take(4096)
                .map(|region| SavedFold {
                    kind: region.kind.clone(),
                    line: region.header,
                    fingerprint: region.fingerprint,
                    context: region.context,
                })
                .collect(),
        }
    }

    pub fn valid(&self) -> bool {
        self.version == 1
            && self.regions.len() <= 4096
            && self.regions.iter().all(|region| region.kind.len() <= 96)
            && self.estimated_bytes() <= ByteSize::kibibytes(512).as_usize()
    }

    fn estimated_bytes(&self) -> usize {
        64 + self
            .regions
            .iter()
            .map(|r| r.kind.len() + 96)
            .sum::<usize>()
    }

    fn restore(&self, candidates: &FoldCandidates) -> Vec<FoldRegion> {
        if !self.valid() {
            return Vec::new();
        }
        let mut matches: HashMap<_, Vec<_>> = HashMap::new();
        for region in &candidates.regions {
            matches
                .entry((region.kind.as_str(), region.fingerprint, region.context))
                .or_default()
                .push(region);
        }
        let mut restored = Vec::new();
        for saved in &self.regions {
            let Some(regions) =
                matches.get(&(saved.kind.as_str(), saved.fingerprint, saved.context))
            else {
                continue;
            };
            let region = if self.content == candidates.content_fingerprint {
                regions
                    .iter()
                    .copied()
                    .find(|region| region.header == saved.line)
            } else if regions.len() == 1 {
                Some(regions[0])
            } else {
                None
            };
            restored.extend(region.cloned());
        }
        restored.sort_by_key(|region| region.header);
        restored.dedup_by_key(|region| region.header);
        restored
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFolds {
    pub path: PathBuf,
    pub folds: SavedFolds,
}

pub fn prune_recent(records: &mut Vec<RecentFolds>) {
    let mut bytes = 0;
    records.truncate(128);
    records.retain(|record| {
        bytes += record.folds.estimated_bytes() + record.path.as_os_str().len();
        record.path.is_absolute()
            && record.folds.valid()
            && bytes <= ByteSize::mebibytes(1).as_usize()
    });
}

#[derive(Debug, Clone)]
pub(crate) struct PendingFolds {
    pub saved: SavedFolds,
    pub top: Option<Position>,
}

impl EditorState {
    /// Dirty buffers retain only their last record made against saved content.
    pub(crate) fn refresh_saved_folds(&mut self, document: &Document) {
        if document.is_modified || !self.is_plain_text_mode() {
            return;
        }
        let Some(candidates) = document.folds.as_ref().filter(|folds| {
            folds.stamp.revision == document.revision
                && folds.stamp.language == document.language
                && folds.stamp.policy_generation == document.text_policy_generation
        }) else {
            return;
        };
        if self
            .folds
            .saved_identity
            .as_ref()
            .is_some_and(|(state, data)| {
                Arc::ptr_eq(state, &self.folds.identity)
                    && std::ptr::eq(data.as_ptr(), Arc::as_ptr(candidates))
            })
        {
            return;
        }
        self.folds.saved = Some(SavedFolds::capture(candidates, self.folds.collapsed()));
        self.folds.saved_identity =
            Some((Arc::clone(&self.folds.identity), Arc::downgrade(candidates)));
    }

    pub(crate) fn restore_folds(&mut self, document: &Document) -> bool {
        let Some(candidates) = document.folds.as_ref().filter(|folds| {
            folds.stamp.revision == document.revision
                && folds.stamp.language == document.language
                && folds.stamp.policy_generation == document.text_policy_generation
        }) else {
            return false;
        };
        let Some(pending) = self.folds.pending.take() else {
            return false;
        };
        self.folds.replace(pending.saved.restore(candidates));
        self.reveal_folded_carets(document);
        self.ensure_wrap_cache(document);
        if let Some(top) = pending.top {
            let row = self.viewport_map(document).visual_line_for_position(
                top.line.min(document.line_count().saturating_sub(1)),
                top.column,
            );
            let (x, _) = self.pixel_scroll_position();
            let y = self.viewport.pixels.y.position(row);
            self.set_pixel_scroll(document, x, y);
        }
        true
    }
}
