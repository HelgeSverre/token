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
use std::path::{Path, PathBuf};

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
///
/// File-context specific (JetBrains "Current File"): only the focused
/// document's diagnostics are listed; an untitled focused doc lists
/// nothing. `severity_counts` stays workspace-wide for the status bar.
pub fn problems_rows(model: &AppModel) -> Vec<ProblemsRow> {
    let mut rows = Vec::new();
    for (path, diagnostics) in problem_groups(model) {
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

/// Mirror keys are canonicalized (they decode from server URIs) while a
/// focused document's path may not be byte-identical (macOS `/tmp` vs
/// `/private/tmp`) — byte-compare first, then a file-name-gated
/// `canonicalize` fallback, the same pattern `find_document_by_uri`
/// documents (the gate keeps syscalls to the normally-0-or-1 plausible
/// matches).
fn is_same_file(path: &Path, focused: Option<&Path>) -> bool {
    let Some(focused) = focused else { return false };
    if path == focused {
        return true;
    }
    if path.file_name() != focused.file_name() {
        return false;
    }
    match (std::fs::canonicalize(path), std::fs::canonicalize(focused)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The shared group traversal behind materialized rows and layout row
/// counts. Keeping the current-file predicate here prevents rendering,
/// hit-testing, and update-layer capacity from drifting apart.
fn problem_groups(model: &AppModel) -> impl Iterator<Item = (&PathBuf, &[lsp_types::Diagnostic])> {
    let focused_path = model.document().file_path.as_deref();
    model
        .lsp
        .diagnostics
        .iter()
        .filter_map(move |(path, diagnostics)| {
            (!diagnostics.is_empty() && is_same_file(path, focused_path))
                .then_some((path, diagnostics.as_slice()))
        })
}

/// Row count of `problems_rows` without materializing the rows (no
/// `PathBuf` clones) — feeds the chrome layout's `RowList` declaration.
pub fn problems_row_count(model: &AppModel) -> usize {
    problem_groups(model)
        .map(|(path, diagnostics)| {
            if model.problems_panel.collapsed.contains(path) {
                1
            } else {
                1 + diagnostics.len()
            }
        })
        .sum()
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

/// Row geometry for the Problems panel from the solved chrome — `None`
/// when the panel isn't the active panel of any open dock, wherever that
/// dock is (the old capacity helper hardcoded the bottom dock, so a
/// Problems panel moved to another dock scrolled wrong).
fn problems_rows_view(model: &AppModel) -> Option<crate::layout::RowListView> {
    crate::layout::chrome::chrome(model).row_list(crate::layout::UiKey::PanelRows(
        crate::panel::PanelId::Problems,
    ))
}

fn problems_visible_capacity(model: &AppModel) -> usize {
    problems_rows_view(model)
        .map(|rows| rows.visible_capacity())
        .unwrap_or(0)
}

/// Re-clamp the scroll offset after anything changed the row count or the
/// panel's box — THE one clamp for this panel. An invisible panel (not the
/// active one of any open dock) has no viewport to clamp against, so it
/// resets.
fn clamp_problems_scroll(model: &mut AppModel) {
    model.problems_panel.scroll_offset = match problems_rows_view(model) {
        Some(rows) => rows.clamp_scroll(model.problems_panel.scroll_offset),
        None => 0,
    };
}

/// Clamps `problems_panel.selected_index`/`scroll_offset` to the current
/// row count. Must run after anything that can shrink `model.lsp.diagnostics`
/// out from under a stored selection — a fresh publish or a clear — or a
/// stale index renders as no visible selection and `OpenSelected` silently
/// no-ops (same defense `ToggleGroup`'s inline clamp gives its own resize).
pub fn clamp_problems_selection(model: &mut AppModel) {
    let total = problems_row_count(model);
    if total == 0 {
        model.problems_panel.selected_index = None;
        model.problems_panel.scroll_offset = 0;
        return;
    }
    if let Some(idx) = model.problems_panel.selected_index {
        model.problems_panel.selected_index = Some(idx.min(total - 1));
    }
    clamp_problems_scroll(model);
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
    navigation::jump_to_location(model, None, path, start)
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
            let total = problems_row_count(model);
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
                    let total = problems_row_count(model);
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

            clamp_problems_scroll(model);
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

    /// Two files in the mirror, focused document = a.rs — so the
    /// current-file filter shows a.rs's group (3 rows) and hides b.rs.
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
        model.document_mut().file_path = Some(PathBuf::from("/proj/a.rs"));
    }

    #[test]
    fn rows_show_only_the_focused_files_group() {
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
            ]
        );

        // Switching focus to b.rs swaps the panel to b.rs's group.
        model.document_mut().file_path = Some(PathBuf::from("/proj/b.rs"));
        let rows = problems_rows(&model);
        assert_eq!(rows.len(), 2);
        assert!(matches!(
            &rows[0],
            ProblemsRow::File { path, count: 1, .. } if path == &PathBuf::from("/proj/b.rs")
        ));
    }

    #[test]
    fn an_untitled_focused_document_lists_no_problems() {
        let mut model = model_with_open_problems_panel();
        populate_two_files(&mut model);
        model.document_mut().file_path = None;
        assert!(problems_rows(&model).is_empty());
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
        assert_eq!(rows.len(), 1); // just a.rs's collapsed File row
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
        assert_eq!(problems_rows(&model).len(), 1);

        update_problems(&mut model, ProblemsMsg::ToggleGroup);
        assert!(!model
            .problems_panel
            .collapsed
            .contains(&PathBuf::from("/proj/a.rs")));
        assert_eq!(problems_rows(&model).len(), 3);
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
    fn open_selected_on_a_diagnostic_row_jumps_to_its_line() {
        let mut model = model_with_open_problems_panel();
        let dir = std::env::temp_dir().join("problems-panel-open-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("target.rs");
        std::fs::write(&file_path, "line0\nline1\nline2\n").unwrap();
        // Current-file filter: the file must be the focused document for
        // its diagnostics to be listed at all.
        crate::update::layout::update_layout(
            &mut model,
            crate::messages::LayoutMsg::OpenFileInNewTab(file_path.clone()),
        );
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
        assert_eq!(model.problems_panel.selected_index, Some(2));

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
