//! Problems panel update handlers.
//!
//! See `docs/feature/assets/palette-c4.png` for the mockup this follows:
//! collapsible per-file groups over `model.lsp.diagnostics` (the render
//! mirror populated by `update::lsp::update_lsp`'s `DiagnosticsPublished`
//! arm), with severity-glyph rows and a `line:col` accessory.

use crate::commands::Cmd;
use crate::messages::ProblemsMsg;
use crate::model::{diagnostic_mark, AppModel, Mark};
use crate::update::navigation;
use crate::view::geometry::{DockHeaderLayout, OutlinePanelLayout, WindowLayout};
use std::path::PathBuf;

/// One addressable row of the Problems panel's flat list.
#[derive(Debug, Clone, PartialEq)]
pub enum ProblemsRow {
    /// A collapsible per-file group header.
    File {
        path: PathBuf,
        count: usize,
        collapsed: bool,
    },
    /// A single diagnostic. `index` indexes `model.lsp.diagnostics[path]`.
    Diagnostic { path: PathBuf, index: usize },
}

/// THE ordering authority for the Problems panel: `BTreeMap` iteration
/// order (stable, path-sorted), diagnostics in publish order within a
/// file, collapsed groups contribute only their `File` row. The view,
/// keyboard nav, click hit-mapping, and activation all consume this —
/// never re-derive independently (same rule outline's
/// `visible_tree_row_at_index` follows).
pub fn problems_rows(model: &AppModel) -> Vec<ProblemsRow> {
    let mut rows = Vec::new();
    for (path, diagnostics) in &model.lsp.diagnostics {
        if diagnostics.is_empty() {
            continue;
        }
        let collapsed = model.problems_panel.collapsed.contains(path);
        rows.push(ProblemsRow::File {
            path: path.clone(),
            count: diagnostics.len(),
            collapsed,
        });
        if !collapsed {
            rows.extend((0..diagnostics.len()).map(|index| ProblemsRow::Diagnostic {
                path: path.clone(),
                index,
            }));
        }
    }
    rows
}

/// `(errors, warnings)` across every file in the mirror — feeds the panel
/// header and (per the mockup) the status bar. `Mark::Info`/`Hint`
/// diagnostics aren't counted; only the two glyphs the header shows.
pub fn severity_counts(model: &AppModel) -> (usize, usize) {
    let mut errors = 0;
    let mut warnings = 0;
    for diagnostics in model.lsp.diagnostics.values() {
        for diagnostic in diagnostics {
            match diagnostic_mark(diagnostic.severity) {
                Mark::Error => errors += 1,
                Mark::Warning => warnings += 1,
                _ => {}
            }
        }
    }
    (errors, warnings)
}

fn problems_visible_capacity(model: &AppModel) -> usize {
    WindowLayout::compute(model)
        .bottom_dock_rect
        .map(|rect| {
            let dock_layout = DockHeaderLayout::new(
                &model.dock_layout.bottom,
                rect,
                &model.metrics,
                model.char_width,
            );
            OutlinePanelLayout::new(dock_layout.content_rect, &model.metrics).visible_capacity()
        })
        .unwrap_or(0)
}

/// Clamps `problems_panel.selected_index`/`scroll_offset` to the current
/// row count. Must run after anything that can shrink `model.lsp.diagnostics`
/// out from under a stored selection — a fresh publish or a clear — or a
/// stale index renders as no visible selection and `OpenSelected` silently
/// no-ops (same defense `ToggleGroup`'s inline clamp gives its own resize).
pub fn clamp_problems_selection(model: &mut AppModel) {
    let total = problems_rows(model).len();
    if total == 0 {
        model.problems_panel.selected_index = None;
        model.problems_panel.scroll_offset = 0;
        return;
    }
    if let Some(idx) = model.problems_panel.selected_index {
        model.problems_panel.selected_index = Some(idx.min(total - 1));
    }
    let visible_capacity = problems_visible_capacity(model);
    model.problems_panel.scroll_offset = if visible_capacity == 0 {
        0
    } else {
        model
            .problems_panel
            .scroll_offset
            .min(total.saturating_sub(visible_capacity))
    };
}

fn reveal_problems_selection(model: &mut AppModel) {
    let Some(selected_index) = model.problems_panel.selected_index else {
        return;
    };
    let visible_capacity = problems_visible_capacity(model);
    if visible_capacity == 0 {
        return;
    }

    let scroll_offset = model.problems_panel.scroll_offset;
    model.problems_panel.scroll_offset = if selected_index < scroll_offset {
        selected_index
    } else if selected_index >= scroll_offset.saturating_add(visible_capacity) {
        selected_index.saturating_add(1) - visible_capacity
    } else {
        scroll_offset
    };
}

/// Jump to a diagnostic's location: char coords via `lsp_to_position` when
/// the file is open (converts LSP UTF-16 correctly), raw LSP values
/// otherwise — `jump_to_location`/`place_cursor_char` clamp either way.
fn open_diagnostic(model: &mut AppModel, path: &std::path::Path, index: usize) -> Option<Cmd> {
    let diagnostic = model.lsp.diagnostics.get(path)?.get(index)?.clone();
    let start = diagnostic.range.start;
    let (line, col) = model
        .editor_area
        .find_open_file(path)
        .and_then(|(doc_id, _, _)| model.editor_area.documents.get(&doc_id))
        .map(|doc| {
            let position = crate::lsp::lsp_to_position(doc, start);
            (position.line, position.column)
        })
        .unwrap_or((start.line as usize, start.character as usize));
    navigation::jump_to_location(model, None, path, line, col)
}

pub fn update_problems(model: &mut AppModel, msg: ProblemsMsg) -> Option<Cmd> {
    match msg {
        ProblemsMsg::SelectPrevious => {
            if let Some(idx) = model.problems_panel.selected_index {
                if idx > 0 {
                    model.problems_panel.selected_index = Some(idx - 1);
                }
            } else {
                model.problems_panel.selected_index = Some(0);
            }
            reveal_problems_selection(model);
            Some(Cmd::Redraw)
        }

        ProblemsMsg::SelectNext => {
            let total = problems_rows(model).len();
            if let Some(idx) = model.problems_panel.selected_index {
                if idx + 1 < total {
                    model.problems_panel.selected_index = Some(idx + 1);
                }
            } else if total > 0 {
                model.problems_panel.selected_index = Some(0);
            }
            reveal_problems_selection(model);
            Some(Cmd::Redraw)
        }

        ProblemsMsg::ToggleGroup => {
            if let Some(idx) = model.problems_panel.selected_index {
                if let Some(ProblemsRow::File { path, .. }) = problems_rows(model).get(idx) {
                    let path = path.clone();
                    if !model.problems_panel.collapsed.remove(&path) {
                        model.problems_panel.collapsed.insert(path);
                    }
                    let total = problems_rows(model).len();
                    model.problems_panel.selected_index = Some(idx.min(total.saturating_sub(1)));
                }
            }
            reveal_problems_selection(model);
            Some(Cmd::Redraw)
        }

        ProblemsMsg::OpenSelected => {
            let selected = model
                .problems_panel
                .selected_index
                .and_then(|idx| problems_rows(model).get(idx).cloned());
            match selected {
                Some(ProblemsRow::Diagnostic { path, index }) => {
                    open_diagnostic(model, &path, index)
                }
                _ => Some(Cmd::Redraw),
            }
        }

        ProblemsMsg::Scroll { lines } => {
            let offset = model.problems_panel.scroll_offset;
            model.problems_panel.scroll_offset = if lines < 0 {
                offset.saturating_sub((-lines) as usize)
            } else {
                offset.saturating_add(lines as usize)
            };

            let total = problems_rows(model).len();
            let visible_capacity = problems_visible_capacity(model);
            model.problems_panel.scroll_offset = if visible_capacity == 0 {
                0
            } else {
                model
                    .problems_panel
                    .scroll_offset
                    .min(total.saturating_sub(visible_capacity))
            };
            Some(Cmd::Redraw)
        }

        ProblemsMsg::ClickRow {
            index,
            click_count,
            on_chevron,
        } => {
            let row = problems_rows(model).get(index).cloned();
            model.problems_panel.selected_index = Some(index);
            match row {
                Some(ProblemsRow::File { path, .. }) if on_chevron => {
                    if !model.problems_panel.collapsed.remove(&path) {
                        model.problems_panel.collapsed.insert(path);
                    }
                }
                Some(ProblemsRow::Diagnostic { path, index }) if click_count >= 2 => {
                    return open_diagnostic(model, &path, index);
                }
                _ => {}
            }
            Some(Cmd::Redraw)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::panel::PanelId;
    use std::path::PathBuf;

    fn model_with_open_problems_panel() -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.dock_layout.bottom.activate(PanelId::PROBLEMS);
        model
    }

    fn diagnostic(
        line: u32,
        severity: lsp_types::DiagnosticSeverity,
        message: &str,
    ) -> lsp_types::Diagnostic {
        lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position { line, character: 0 },
                end: lsp_types::Position { line, character: 3 },
            },
            severity: Some(severity),
            message: message.to_owned(),
            ..Default::default()
        }
    }

    fn populate_two_files(model: &mut AppModel) {
        model.lsp.diagnostics.insert(
            PathBuf::from("/proj/a.rs"),
            vec![
                diagnostic(0, lsp_types::DiagnosticSeverity::ERROR, "a-err"),
                diagnostic(1, lsp_types::DiagnosticSeverity::WARNING, "a-warn"),
            ],
        );
        model.lsp.diagnostics.insert(
            PathBuf::from("/proj/b.rs"),
            vec![diagnostic(0, lsp_types::DiagnosticSeverity::ERROR, "b-err")],
        );
    }

    #[test]
    fn rows_are_path_sorted_with_diagnostics_nested_under_their_file() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);

        let rows = problems_rows(&model);
        assert_eq!(
            rows,
            vec![
                ProblemsRow::File {
                    path: PathBuf::from("/proj/a.rs"),
                    count: 2,
                    collapsed: false,
                },
                ProblemsRow::Diagnostic {
                    path: PathBuf::from("/proj/a.rs"),
                    index: 0,
                },
                ProblemsRow::Diagnostic {
                    path: PathBuf::from("/proj/a.rs"),
                    index: 1,
                },
                ProblemsRow::File {
                    path: PathBuf::from("/proj/b.rs"),
                    count: 1,
                    collapsed: false,
                },
                ProblemsRow::Diagnostic {
                    path: PathBuf::from("/proj/b.rs"),
                    index: 0,
                },
            ]
        );
    }

    #[test]
    fn a_collapsed_file_group_contributes_only_its_file_row() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);
        model
            .problems_panel
            .collapsed
            .insert(PathBuf::from("/proj/a.rs"));

        let rows = problems_rows(&model);
        assert_eq!(rows.len(), 3); // a.rs File row + b.rs File row + b.rs diagnostic
        assert!(matches!(
            &rows[0],
            ProblemsRow::File {
                collapsed: true,
                ..
            }
        ));
    }

    #[test]
    fn severity_counts_tallies_errors_and_warnings_separately() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);
        assert_eq!(severity_counts(&model), (2, 1));
    }

    #[test]
    fn empty_diagnostics_mirror_produces_no_rows() {
        let model = model_with_open_problems_panel();
        assert!(problems_rows(&model).is_empty());
        assert_eq!(severity_counts(&model), (0, 0));
    }

    #[test]
    fn toggle_group_collapses_and_expands_the_selected_file() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);
        model.problems_panel.selected_index = Some(0); // a.rs File row

        update_problems(&mut model, ProblemsMsg::ToggleGroup);
        assert!(model
            .problems_panel
            .collapsed
            .contains(&PathBuf::from("/proj/a.rs")));
        assert_eq!(problems_rows(&model).len(), 3);

        update_problems(&mut model, ProblemsMsg::ToggleGroup);
        assert!(!model
            .problems_panel
            .collapsed
            .contains(&PathBuf::from("/proj/a.rs")));
        assert_eq!(problems_rows(&model).len(), 5);
    }

    #[test]
    fn open_selected_on_a_file_row_is_a_no_op() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);
        model.problems_panel.selected_index = Some(0); // a.rs File row

        let before = model.document().id;
        update_problems(&mut model, ProblemsMsg::OpenSelected);
        assert_eq!(model.document().id, before);
    }

    #[test]
    fn open_selected_on_an_unopened_file_uses_raw_lsp_coords_and_opens_it() {
        let mut model = model_with_open_problems_panel();
        let dir = std::env::temp_dir().join("problems-panel-open-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("target.rs");
        std::fs::write(&file_path, "line0\nline1\nline2\n").unwrap();
        model.lsp.diagnostics.insert(
            file_path.clone(),
            vec![diagnostic(1, lsp_types::DiagnosticSeverity::ERROR, "boom")],
        );
        model.problems_panel.selected_index = Some(1); // the one diagnostic row

        update_problems(&mut model, ProblemsMsg::OpenSelected);

        let doc = model.document();
        assert_eq!(doc.file_path.as_deref(), Some(file_path.as_path()));
        assert_eq!(model.editor().active_cursor().line, 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn select_next_and_previous_clamp_at_the_ends() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);

        for _ in 0..10 {
            update_problems(&mut model, ProblemsMsg::SelectNext);
        }
        assert_eq!(model.problems_panel.selected_index, Some(4));

        for _ in 0..10 {
            update_problems(&mut model, ProblemsMsg::SelectPrevious);
        }
        assert_eq!(model.problems_panel.selected_index, Some(0));
    }

    /// Mirrors outline's `OutlineMsg::ClickRow`: a click anywhere on a File
    /// row other than its chevron must select without collapsing, so a
    /// user can select a file group without toggling it every click.
    #[test]
    fn click_row_off_chevron_selects_a_file_row_without_toggling_it() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);

        update_problems(
            &mut model,
            ProblemsMsg::ClickRow {
                index: 0,
                click_count: 1,
                on_chevron: false,
            },
        );

        assert_eq!(model.problems_panel.selected_index, Some(0));
        assert!(!model
            .problems_panel
            .collapsed
            .contains(&PathBuf::from("/proj/a.rs")));
    }

    #[test]
    fn click_row_on_chevron_toggles_the_file_row_collapse() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);

        update_problems(
            &mut model,
            ProblemsMsg::ClickRow {
                index: 0,
                click_count: 1,
                on_chevron: true,
            },
        );

        assert!(model
            .problems_panel
            .collapsed
            .contains(&PathBuf::from("/proj/a.rs")));
    }

    /// The whole point of clamping: a stale selection/scroll from before a
    /// shrinking publish must never point past the end of the new row
    /// list, or it renders with no visible selection and `OpenSelected`
    /// silently no-ops.
    #[test]
    fn clamp_problems_selection_pulls_a_stale_index_back_onto_the_new_row_list() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);
        model.problems_panel.selected_index = Some(4); // last row, b.rs diagnostic
        model.problems_panel.scroll_offset = 4;

        // Server re-publishes b.rs with zero diagnostics -- it drops out
        // of the mirror entirely, shrinking the row list to 3.
        model.lsp.diagnostics.remove(&PathBuf::from("/proj/b.rs"));

        clamp_problems_selection(&mut model);

        assert_eq!(model.problems_panel.selected_index, Some(2));
        assert_eq!(model.problems_panel.scroll_offset, 0);
    }

    #[test]
    fn clamp_problems_selection_clears_selection_when_the_mirror_goes_empty() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);
        model.problems_panel.selected_index = Some(3);

        model.lsp.diagnostics.clear();
        clamp_problems_selection(&mut model);

        assert_eq!(model.problems_panel.selected_index, None);
        assert_eq!(model.problems_panel.scroll_offset, 0);
    }
}
