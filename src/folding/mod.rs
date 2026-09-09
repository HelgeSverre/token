//! Whole-line fold candidates and the pane's indexed visual-row projection.
//! None of this state changes document text or participates in text undo.

use std::{ops::Range, sync::Arc};

use crate::{syntax::LanguageId, util::text::TabStops, wrap::WrapCache};
pub mod persistence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldStamp {
    pub revision: u64,
    pub language: LanguageId,
    pub policy_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoldAction {
    Toggle,
    Collapse,
    Expand,
    CollapseAll,
    ExpandAll,
}

/// A visible header followed by at least one hidden whole logical line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldRegion {
    pub header: usize,
    pub end: usize,
    pub kind: String,
    /// Character boundaries, retained to map surviving regions through edits.
    pub offsets: Range<usize>,
    pub fingerprint: u64,
    pub context: u64,
}

impl FoldRegion {
    pub fn hides(&self, line: usize) -> bool {
        self.header < line && line < self.end
    }

    pub fn contains(&self, line: usize) -> bool {
        self.header <= line && line < self.end
    }
}

#[derive(Debug, Clone)]
pub struct FoldCandidates {
    pub stamp: FoldStamp,
    pub regions: Vec<FoldRegion>,
    pub content_fingerprint: u64,
}

/// Version-one, non-cryptographic metadata digest. Never used for file identity
/// or overwrite authorization. Ambiguous fold matches always remain expanded.
pub(crate) fn digest(bytes: impl IntoIterator<Item = u8>) -> u64 {
    bytes.into_iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

pub(crate) fn region(
    buffer: &ropey::Rope,
    header: usize,
    end: usize,
    kind: &str,
) -> Option<FoldRegion> {
    if header.saturating_add(1) >= end || end > buffer.len_lines() {
        return None;
    }
    let header_text = buffer.line(header).to_string();
    let boundary = buffer.line(end - 1).to_string();
    Some(FoldRegion {
        header,
        end,
        kind: kind.into(),
        offsets: buffer.line_to_char(header)..buffer.line_to_char(end),
        fingerprint: digest(
            kind.bytes()
                .chain([0])
                .chain(header_text.trim().bytes())
                .chain([0])
                .chain(boundary.trim().bytes()),
        ),
        context: 0,
    })
}

/// Prefer the widest candidate on each header, then reject crossing intervals.
/// Parent context is metadata only; hidden intervals remain nested or disjoint.
pub fn normalize(mut regions: Vec<FoldRegion>, line_count: usize) -> Vec<FoldRegion> {
    regions.retain(|r| r.header.saturating_add(1) < r.end && r.end <= line_count);
    regions.sort_by(|a, b| {
        a.header
            .cmp(&b.header)
            .then(b.end.cmp(&a.end))
            .then(a.kind.cmp(&b.kind))
    });
    let mut result: Vec<FoldRegion> = Vec::with_capacity(regions.len());
    let mut parents: Vec<usize> = Vec::new();
    for mut candidate in regions {
        if result.last().is_some_and(|r| r.header == candidate.header) {
            continue;
        }
        while parents
            .last()
            .is_some_and(|&i| result[i].end <= candidate.header)
        {
            parents.pop();
        }
        if let Some(&parent) = parents.last() {
            if candidate.end > result[parent].end {
                continue;
            }
            candidate.context = result[parent].fingerprint;
        }
        parents.push(result.len());
        result.push(candidate);
    }
    result
}

/// Blank lines neither open nor close a block; trailing blanks stay visible.
pub fn indentation(buffer: &ropey::Rope, tabs: TabStops) -> Vec<FoldRegion> {
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut previous: Option<(usize, usize)> = None;
    let mut result = Vec::new();
    for (line, slice) in buffer.lines().enumerate() {
        let text: std::borrow::Cow<'_, str> = slice.into();
        if text.trim().is_empty() {
            continue;
        }
        let indent = tabs.visual_width(text.chars().take_while(|c| matches!(c, ' ' | '\t')));
        if let Some((previous_line, previous_indent)) = previous {
            while stack.last().is_some_and(|&(_, level)| level >= indent) {
                if let Some((header, _)) = stack.pop() {
                    result.extend(region(buffer, header, previous_line + 1, "indentation"));
                }
            }
            if indent > previous_indent {
                stack.push((previous_line, previous_indent));
            }
        }
        previous = Some((line, indent));
    }
    if let Some((last, _)) = previous {
        for (header, _) in stack {
            result.extend(region(buffer, header, last + 1, "indentation"));
        }
    }
    normalize(result, buffer.len_lines())
}

#[derive(Debug, Clone)]
struct HiddenInterval {
    lines: Range<usize>,
    rows: Range<usize>,
    header_row: usize,
    removed_before: usize,
    visible_after: usize,
}

/// Converts cached wrap rows to visible rows in logarithmic time, skipping
/// entire hidden subtrees. The wrap cache may retain segments for hidden lines.
#[derive(Debug, Clone, Default)]
pub struct FoldProjection {
    intervals: Vec<HiddenInterval>,
    removed: usize,
}

impl FoldProjection {
    pub fn new(collapsed: &[FoldRegion], wrap: Option<&WrapCache>, line_count: usize) -> Self {
        let base_row = |line| {
            wrap.map_or(line, |cache| {
                if line >= line_count {
                    cache.total_visual_lines()
                } else {
                    cache.logical_line_to_visual(line)
                }
            })
        };
        let mut projection = Self::default();
        for fold in collapsed {
            if fold.end > line_count
                || fold.header + 1 >= fold.end
                || projection
                    .intervals
                    .last()
                    .is_some_and(|i| fold.header < i.lines.end)
            {
                continue;
            }
            let rows = base_row(fold.header + 1)..base_row(fold.end);
            let count = rows.len();
            projection.intervals.push(HiddenInterval {
                lines: fold.header + 1..fold.end,
                header_row: base_row(fold.header),
                removed_before: projection.removed,
                visible_after: rows.start - projection.removed,
                rows,
            });
            projection.removed += count;
        }
        projection
    }

    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }

    pub fn row_count(&self, base_count: usize) -> usize {
        base_count.saturating_sub(self.removed)
    }

    pub fn hidden_header(&self, line: usize) -> Option<usize> {
        let index = self.intervals.partition_point(|i| i.lines.start <= line);
        let interval = index.checked_sub(1).map(|i| &self.intervals[i])?;
        (line < interval.lines.end).then_some(interval.lines.start - 1)
    }

    /// Hidden positions project to the containing header, never a fake row.
    pub fn project(&self, row: usize) -> usize {
        let index = self.intervals.partition_point(|i| i.rows.start <= row);
        let Some(interval) = index.checked_sub(1).map(|i| &self.intervals[i]) else {
            return row;
        };
        if row < interval.rows.end {
            interval.header_row - interval.removed_before
        } else {
            row - interval.removed_before - interval.rows.len()
        }
    }

    pub fn unproject(&self, row: usize) -> usize {
        let index = self.intervals.partition_point(|i| i.visible_after <= row);
        index.checked_sub(1).map_or(row, |i| {
            let interval = &self.intervals[i];
            row + interval.removed_before + interval.rows.len()
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct FoldState {
    pub(crate) saved: Option<persistence::SavedFolds>,
    pub(crate) saved_identity: Option<(Arc<()>, std::sync::Weak<FoldCandidates>)>,
    pub(crate) pending: Option<persistence::PendingFolds>,
    pub(crate) collapsed: Vec<FoldRegion>,
    pub(crate) projection: FoldProjection,
    pub(crate) identity: Arc<()>,
    generation: u64,
    projection_key: Option<(u64, usize, Option<Arc<()>>)>,
}

impl FoldState {
    pub fn collapsed(&self) -> &[FoldRegion] {
        &self.collapsed
    }

    pub fn is_collapsed(&self, header: usize) -> bool {
        self.collapsed
            .binary_search_by_key(&header, |r| r.header)
            .is_ok()
    }

    pub(crate) fn replace(&mut self, collapsed: Vec<FoldRegion>) -> bool {
        if self.collapsed == collapsed {
            return false;
        }
        self.collapsed = collapsed;
        self.changed();
        true
    }

    pub(crate) fn changed(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.identity = Arc::new(());
    }

    pub(crate) fn refresh(&mut self, wrap: Option<&WrapCache>, line_count: usize) -> bool {
        let wrap_identity = wrap.and_then(WrapCache::layout_identity);
        if self
            .projection_key
            .as_ref()
            .is_some_and(|(generation, count, identity)| {
                *generation == self.generation
                    && *count == line_count
                    && match (identity, &wrap_identity) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        (None, None) => true,
                        _ => false,
                    }
            })
        {
            return false;
        }
        self.projection = FoldProjection::new(&self.collapsed, wrap, line_count);
        self.projection_key = Some((self.generation, line_count, wrap_identity));
        true
    }
}
