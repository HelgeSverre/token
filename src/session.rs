//! Saved-file session metadata. No document text or undo history is serialized.
//! Capture and installation are filesystem-free; runtime owns storage and loading.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::editor_area::{EditorGroup, LayoutNode, Rect, SplitContainer, Tab};
use crate::model::{
    AppModel, Cursor, EditorArea, EditorState, GroupId, Position, Selection, SplitDirection,
    ViewMode,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    version: u32,
    workspace: Option<PathBuf>,
    layout: Option<Node>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recent_folds: Vec<crate::folding::persistence::RecentFolds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Node {
    Group {
        tabs: Vec<SavedTab>,
        active: usize,
        focused: bool,
    },
    Split {
        horizontal: bool,
        children: Vec<Node>,
        ratios: Vec<f32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedTab {
    path: PathBuf,
    selections: Vec<SavedSelection>,
    active_cursor: usize,
    top_position: (usize, usize),
    left_column: usize,
    /// Fractions of a column/row, independent of the previous display's DPI.
    #[serde(default)]
    scroll_fraction: (f64, f64),
    soft_wrap: bool,
    had_unsaved_changes: bool,
    csv: Option<SavedCsv>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    folds: Option<crate::folding::persistence::SavedFolds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedSelection {
    anchor: (usize, usize),
    head: (usize, usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedCsv {
    delimiter: char,
    cell: (usize, usize),
    scroll: (usize, usize),
    header: bool,
}

struct PendingViewport {
    editor_id: crate::model::EditorId,
    top_position: (usize, usize),
    left_column: usize,
    fraction: (f64, f64),
}

impl Session {
    /// Paths are relative to the workspace where possible, absolute otherwise.
    /// `cwd` is captured by the runtime, not looked up during model traversal.
    pub fn capture(model: &AppModel, cwd: &Path) -> Self {
        let workspace = model
            .workspace
            .as_ref()
            .map(|workspace| absolute(&workspace.root, cwd));
        Self {
            version: 1,
            layout: Node::capture(&model.editor_area.layout, model, cwd, workspace.as_deref()),
            workspace,
            recent_folds: model.editor_area.recent_folds.clone(),
        }
    }

    pub fn workspace(&self) -> Option<&Path> {
        self.workspace.as_deref()
    }

    /// Validate before installing any state. Limits bound corrupted/local files,
    /// not ordinary editor buffers; no partial layout is accepted on failure.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.version != 1 {
            return Err("Unsupported session version");
        }
        let mut recent = self.recent_folds.clone();
        crate::folding::persistence::prune_recent(&mut recent);
        if recent.len() != self.recent_folds.len() {
            return Err("Invalid recent fold metadata");
        }
        let mut invalid_path = false;
        self.visit_tabs(&mut |tab| {
            invalid_path |= !tab.path.is_absolute() && self.workspace.is_none()
        });
        if invalid_path {
            return Err("Non-workspace session contains a relative path");
        }
        let mut tabs = 0;
        let mut groups = 0;
        if let Some(layout) = &self.layout {
            layout.validate(0, &mut tabs, &mut groups)?;
        }
        Ok(())
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let mut seen = HashSet::new();
        self.visit_tabs(&mut |tab| {
            let path = self.resolve(&tab.path);
            if seen.insert(path.clone()) {
                paths.push(path);
            }
        });
        paths
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        self.workspace
            .as_ref()
            .map_or_else(|| path.to_path_buf(), |root| root.join(path))
    }

    fn visit_tabs(&self, f: &mut impl FnMut(&SavedTab)) {
        if let Some(layout) = &self.layout {
            layout.visit_tabs(f);
        }
    }

    /// Rebuild panes using documents already installed by the normal file loader.
    /// Missing files are omitted; empty branches collapse. Shared documents keep
    /// independent per-tab editors. Returns the number of restored tabs.
    pub fn install(&self, model: &mut AppModel) -> Result<usize, &'static str> {
        self.validate()?;
        model
            .editor_area
            .recent_folds
            .clone_from(&self.recent_folds);
        let old_editors = std::mem::take(&mut model.editor_area.editors);
        let old_groups = std::mem::take(&mut model.editor_area.groups);
        let mut count = 0;
        let mut focused = None;
        let mut viewports = Vec::new();
        let layout = self.layout.as_ref().and_then(|node| {
            node.install(
                self,
                &mut model.editor_area,
                &old_editors,
                &mut focused,
                &mut count,
                &mut viewports,
            )
        });
        let Some(layout) = layout else {
            model.editor_area.editors = old_editors;
            model.editor_area.groups = old_groups;
            return Ok(0);
        };
        model.editor_area.layout = layout;
        model.editor_area.previews.clear();
        let first = model
            .editor_area
            .layout
            .group_ids()
            .into_iter()
            .next()
            .ok_or("Session has no editor group")?;
        model.editor_area.focused_group_id = focused.unwrap_or(first);
        let used: HashSet<_> = model
            .editor_area
            .editors
            .values()
            .filter_map(|editor| editor.document_id)
            .collect();
        model
            .editor_area
            .documents
            .retain(|id, _| used.contains(id));
        model.resize(model.window_size.0, model.window_size.1);
        for pending in viewports {
            let Some(editor) = model.editor_area.editors.get_mut(&pending.editor_id) else {
                continue;
            };
            if let Some(doc) = editor
                .document_id
                .and_then(|id| model.editor_area.documents.get(&id))
            {
                let line = pending
                    .top_position
                    .0
                    .min(doc.line_count().saturating_sub(1));
                let column = pending.top_position.1.min(doc.line_length(line));
                let top = editor
                    .viewport_map(doc)
                    .visual_line_for_position(line, column);
                editor.set_pixel_scroll(
                    doc,
                    (pending.left_column as f64 + pending.fraction.0)
                        * editor.viewport.pixels.x.unit,
                    (top as f64 + pending.fraction.1) * editor.viewport.pixels.y.unit,
                );
            }
        }
        let mut unsaved = false;
        let mut total = 0;
        self.visit_tabs(&mut |tab| {
            unsaved |= tab.had_unsaved_changes;
            total += 1;
        });
        let mut status = format!("Restored {count} tabs");
        if count < total {
            status.push_str(&format!(
                "; {} missing or unreadable tabs skipped",
                total - count
            ));
        }
        if unsaved {
            status.push_str("; previous unsaved contents were not retained");
        }
        model.ui.set_status(status);
        Ok(count)
    }
}

fn absolute(path: &Path, cwd: &Path) -> PathBuf {
    cwd.join(path)
}

impl Node {
    fn capture(
        node: &LayoutNode,
        model: &AppModel,
        cwd: &Path,
        workspace: Option<&Path>,
    ) -> Option<Self> {
        let area = &model.editor_area;
        match node {
            LayoutNode::Empty | LayoutNode::Preview(_) => None,
            LayoutNode::Group(id) => {
                let group = area.groups.get(id)?;
                let mut tabs = Vec::new();
                let mut active = 0;
                for (index, tab) in group.tabs.iter().enumerate() {
                    let editor = area.editors.get(&tab.editor_id)?;
                    let doc = editor.document_id.and_then(|id| area.documents.get(&id))?;
                    let Some(path) = &doc.file_path else {
                        continue;
                    };
                    let path = absolute(path, cwd);
                    let path = workspace
                        .and_then(|root| path.strip_prefix(root).ok())
                        .unwrap_or(&path)
                        .to_path_buf();
                    if index <= group.active_tab_index {
                        active = tabs.len();
                    }
                    let top = editor.viewport_map(doc).position_at_display_column(
                        doc,
                        editor.viewport.top_line,
                        0,
                    );
                    tabs.push(SavedTab {
                        path,
                        selections: editor
                            .selections
                            .iter()
                            .map(|selection| SavedSelection {
                                anchor: (selection.anchor.line, selection.anchor.column),
                                head: (selection.head.line, selection.head.column),
                            })
                            .collect(),
                        active_cursor: editor.active_cursor_index,
                        top_position: (top.line, top.column),
                        left_column: editor.viewport.left_column,
                        scroll_fraction: (
                            editor.viewport.pixels.x.offset / editor.viewport.pixels.x.unit,
                            editor.viewport.pixels.y.offset / editor.viewport.pixels.y.unit,
                        ),
                        soft_wrap: editor.soft_wrap,
                        folds: editor.folds.saved.clone().or_else(|| {
                            editor
                                .folds
                                .pending
                                .as_ref()
                                .map(|pending| pending.saved.clone())
                        }),
                        had_unsaved_changes: doc.is_modified
                            || editor
                                .view_mode
                                .as_csv()
                                .is_some_and(|csv| csv.is_editing()),
                        csv: editor.view_mode.as_csv().map(|csv| SavedCsv {
                            delimiter: csv.delimiter.char(),
                            cell: (csv.selected_cell.row, csv.selected_cell.col),
                            scroll: (csv.viewport.top_row, csv.viewport.left_col),
                            header: csv.has_header_row,
                        }),
                    });
                }
                (!tabs.is_empty()).then_some(Self::Group {
                    tabs,
                    active,
                    focused: *id == area.focused_group_id,
                })
            }
            LayoutNode::Split(split) => {
                let children: Vec<_> = split
                    .children
                    .iter()
                    .enumerate()
                    .filter_map(|(index, child)| {
                        Self::capture(child, model, cwd, workspace)
                            .map(|child| (child, split.ratios.get(index).copied().unwrap_or(1.0)))
                    })
                    .collect();
                let (children, ratios): (Vec<_>, Vec<_>) = children.into_iter().unzip();
                match children.len() {
                    0 => None,
                    1 => children.into_iter().next(),
                    _ => Some(Self::Split {
                        horizontal: split.direction == SplitDirection::Horizontal,
                        children,
                        ratios: normalized(ratios),
                    }),
                }
            }
        }
    }

    fn visit_tabs(&self, f: &mut impl FnMut(&SavedTab)) {
        match self {
            Self::Group { tabs, .. } => tabs.iter().for_each(f),
            Self::Split { children, .. } => {
                for child in children {
                    child.visit_tabs(f);
                }
            }
        }
    }

    fn validate(
        &self,
        depth: usize,
        tab_count: &mut usize,
        groups: &mut usize,
    ) -> Result<(), &'static str> {
        if depth > 32 {
            return Err("Session layout is too deep");
        }
        match self {
            Self::Group { tabs, .. } => {
                *groups += 1;
                *tab_count += tabs.len();
                if *groups > 128 || *tab_count > 512 {
                    return Err("Session has too many panes or tabs");
                }
                for tab in tabs {
                    if tab.folds.as_ref().is_some_and(|folds| !folds.valid()) {
                        return Err("Invalid saved fold metadata");
                    }
                    if tab.path.as_os_str().is_empty() || tab.selections.len() > 4096 {
                        return Err("Invalid session tab");
                    }
                    if [tab.scroll_fraction.0, tab.scroll_fraction.1]
                        .into_iter()
                        .any(|fraction| !fraction.is_finite() || !(0.0..1.0).contains(&fraction))
                    {
                        return Err("Invalid session scroll fraction");
                    }
                }
            }
            Self::Split {
                children, ratios, ..
            } => {
                if children.is_empty()
                    || children.len() != ratios.len()
                    || ratios
                        .iter()
                        .any(|ratio| !ratio.is_finite() || *ratio <= 0.0)
                {
                    return Err("Invalid session split ratios");
                }
                for child in children {
                    child.validate(depth + 1, tab_count, groups)?;
                }
            }
        }
        Ok(())
    }

    fn install(
        &self,
        session: &Session,
        area: &mut EditorArea,
        bases: &std::collections::HashMap<crate::model::EditorId, EditorState>,
        focused: &mut Option<GroupId>,
        count: &mut usize,
        viewports: &mut Vec<PendingViewport>,
    ) -> Option<LayoutNode> {
        match self {
            Self::Group {
                tabs,
                active,
                focused: is_focused,
            } => {
                let id = area.next_group_id();
                let mut installed = Vec::new();
                let mut active_index = 0;
                for (index, tab) in tabs.iter().enumerate() {
                    let Some(doc_id) = area.find_document_by_path(&session.resolve(&tab.path))
                    else {
                        continue;
                    };
                    let Some(base) = bases
                        .values()
                        .find(|editor| editor.document_id == Some(doc_id))
                    else {
                        continue;
                    };
                    let mut editor = base.clone();
                    let editor_id = area.next_editor_id();
                    editor.id = Some(editor_id);
                    tab.apply(&mut editor, &area.documents[&doc_id]);
                    viewports.push(PendingViewport {
                        editor_id,
                        top_position: tab.top_position,
                        left_column: tab.left_column,
                        fraction: tab.scroll_fraction,
                    });
                    area.editors.insert(editor_id, editor);
                    if index <= *active {
                        active_index = installed.len();
                    }
                    installed.push(Tab {
                        id: area.next_tab_id(),
                        editor_id,
                        is_pinned: false,
                        is_preview: false,
                    });
                    *count += 1;
                }
                if installed.is_empty() {
                    return None;
                }
                area.groups.insert(
                    id,
                    EditorGroup {
                        id,
                        tabs: installed,
                        active_tab_index: active_index,
                        rect: Rect::default(),
                        attached_preview: None,
                        tab_scroll: 0,
                    },
                );
                if *is_focused {
                    *focused = Some(id);
                }
                Some(LayoutNode::Group(id))
            }
            Self::Split {
                horizontal,
                children,
                ratios,
            } => {
                let installed: Vec<_> = children
                    .iter()
                    .zip(ratios)
                    .filter_map(|(child, ratio)| {
                        child
                            .install(session, area, bases, focused, count, viewports)
                            .map(|node| (node, *ratio))
                    })
                    .collect();
                let (children, ratios): (Vec<_>, Vec<_>) = installed.into_iter().unzip();
                match children.len() {
                    0 => None,
                    1 => children.into_iter().next(),
                    len => Some(LayoutNode::Split(SplitContainer {
                        direction: if *horizontal {
                            SplitDirection::Horizontal
                        } else {
                            SplitDirection::Vertical
                        },
                        children,
                        ratios: normalized(ratios),
                        min_sizes: vec![100.0; len],
                    })),
                }
            }
        }
    }
}

fn normalized(ratios: Vec<f32>) -> Vec<f32> {
    let sum: f32 = ratios.iter().sum();
    let len = ratios.len();
    if !sum.is_finite() || sum <= 0.0 {
        return vec![1.0 / len as f32; len];
    }
    ratios.into_iter().map(|ratio| ratio / sum).collect()
}

impl SavedTab {
    fn apply(&self, editor: &mut EditorState, doc: &crate::model::Document) {
        let position = |(line, column): (usize, usize)| {
            let line = line.min(doc.line_count().saturating_sub(1));
            Position::new(line, column.min(doc.line_length(line)))
        };
        editor.selections = self
            .selections
            .iter()
            .map(|s| Selection::from_anchor_head(position(s.anchor), position(s.head)))
            .collect();
        if editor.selections.is_empty() {
            editor.selections.push(Selection::default());
        }
        editor.cursors = editor
            .selections
            .iter()
            .map(|s| Cursor {
                line: s.head.line,
                column: s.head.column,
                desired_column: None,
            })
            .collect();
        editor.active_cursor_index = self.active_cursor.min(editor.cursors.len() - 1);
        editor.soft_wrap = self.soft_wrap;
        editor.folds = Default::default();
        editor.folds.pending =
            self.folds
                .clone()
                .map(|saved| crate::folding::persistence::PendingFolds {
                    saved,
                    top: Some(position(self.top_position)),
                });
        // Resolve the saved logical anchor after split layout establishes each
        // pane's width. Saving a visual row would drift across wrapping/DPI changes.
        editor.viewport.top_line = 0;
        editor.viewport.left_column = 0;
        editor.viewport.pixels.x.offset = 0.0;
        editor.viewport.pixels.y.offset = 0.0;
        editor.viewport.animation = None;
        if let Some(saved) = &self.csv {
            let delimiter = match saved.delimiter {
                ',' => crate::csv::Delimiter::Comma,
                '\t' => crate::csv::Delimiter::Tab,
                '|' => crate::csv::Delimiter::Pipe,
                ';' => crate::csv::Delimiter::Semicolon,
                _ => return,
            };
            if editor.is_plain_text_mode() {
                if let Ok(data) = crate::csv::parse_csv(&doc.buffer.to_string(), delimiter) {
                    let mut csv = crate::csv::CsvState::new(data, delimiter);
                    csv.has_header_row = saved.header;
                    csv.selected_cell = crate::csv::CellPosition::new(saved.cell.0, saved.cell.1);
                    csv.clamp_selection();
                    csv.viewport.top_row =
                        saved.scroll.0.min(csv.data.row_count().saturating_sub(1));
                    csv.viewport.left_col = saved
                        .scroll
                        .1
                        .min(csv.data.column_count().saturating_sub(1));
                    editor.view_mode = ViewMode::Csv(Box::new(csv));
                }
            }
        }
    }
}
