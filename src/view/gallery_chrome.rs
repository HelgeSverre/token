//! In-memory chrome specimens. Render real application layouts, then crop at
//! native pixel size into the gallery; never launch shells, LSPs, or open files.
use super::{FontRole, Frame, Renderer, TextPainter};
use crate::model::editor_area::Tab;
use crate::model::{gallery::ChromePreview, AppModel, Document, EditorState, Rect};
use crate::panel::{DockPosition, PanelId};
use crate::theme::Theme;

fn outline_node(
    kind: crate::outline::OutlineKind,
    name: &str,
    line: usize,
    children: Vec<crate::outline::OutlineNode>,
) -> crate::outline::OutlineNode {
    crate::outline::OutlineNode {
        kind,
        name: name.to_owned(),
        range: crate::outline::OutlineRange {
            start_line: line,
            start_col: 0,
            end_line: line + 1,
            end_col: 0,
        },
        children,
    }
}

fn populate_outline(model: &mut AppModel) {
    use crate::outline::OutlineKind;
    let revision = model.document().revision;
    model.document_mut().outline = Some(crate::outline::OutlineData {
        revision,
        roots: vec![
            outline_node(
                OutlineKind::Struct,
                "GalleryRenderer",
                8,
                vec![
                    outline_node(OutlineKind::Field, "theme", 9, Vec::new()),
                    outline_node(
                        OutlineKind::Method,
                        "render_selected_specimen_with_a_long_name",
                        18,
                        Vec::new(),
                    ),
                ],
            ),
            outline_node(OutlineKind::Function, "layout_gallery", 42, Vec::new()),
            outline_node(OutlineKind::Function, "paint_gallery", 77, Vec::new()),
        ],
    });
    model.outline_panel.selected_index = Some(2);
}

fn diagnostic(
    line: u32,
    severity: lsp_types::DiagnosticSeverity,
    message: &str,
) -> lsp_types::Diagnostic {
    let mut diagnostic = lsp_types::Diagnostic::new_simple(
        lsp_types::Range::new(
            lsp_types::Position::new(line, 4),
            lsp_types::Position::new(line, 12),
        ),
        message.to_owned(),
    );
    diagnostic.severity = Some(severity);
    diagnostic.source = Some("rust-analyzer".into());
    diagnostic
}

fn populate_problems(model: &mut AppModel) {
    use lsp_types::DiagnosticSeverity;
    use std::path::PathBuf;

    let focused = PathBuf::from("/workspace/token/src/view/gallery.rs");
    let collapsed = PathBuf::from("/workspace/token/src/runtime/very_long_runtime_module.rs");
    model.document_mut().file_path = Some(focused.clone());
    model.lsp.diagnostics.insert(
        focused,
        vec![
            diagnostic(
                118,
                DiagnosticSeverity::ERROR,
                "borrowed value does not live long enough",
            ),
            diagnostic(
                164,
                DiagnosticSeverity::WARNING,
                "this match arm can be simplified",
            ),
        ],
    );
    model.lsp.diagnostics.insert(
        PathBuf::from("/workspace/token/src/view/panels.rs"),
        vec![
            diagnostic(
                42,
                DiagnosticSeverity::ERROR,
                "mismatched types in panel layout",
            ),
            diagnostic(
                87,
                DiagnosticSeverity::INFORMATION,
                "consider extracting this expression",
            ),
        ],
    );
    model.lsp.diagnostics.insert(
        collapsed.clone(),
        vec![diagnostic(
            12,
            DiagnosticSeverity::WARNING,
            "unused result must be handled",
        )],
    );
    model.problems_panel.current_file_only = false;
    model.problems_panel.selected_index = Some(2);
    model.problems_panel.collapsed.insert(collapsed);
}

fn location(path: &str, line: u32, preview: &str) -> crate::update::navigation::LocationItem {
    crate::update::navigation::LocationItem {
        path: path.into(),
        position: lsp_types::Position::new(line, 8),
        preview: preview.to_owned(),
        route_hint: None,
    }
}

pub(super) fn populate_usages(model: &mut AppModel) {
    let gallery = "/workspace/token/src/view/gallery.rs";
    let panels = "/workspace/token/src/view/panels.rs";
    let collapsed = "/workspace/token/src/runtime/very_long_runtime_module.rs";
    let usages = &mut model.usages_panel;
    usages.items = vec![
        location(
            gallery,
            118,
            "render_dock(&mut frame, &mut painter, &model, position, &chrome);",
        ),
        location(gallery, 164, "let dock = render_dock;"),
        location(panels, 42, "pub fn render_dock("),
        location(
            panels,
            87,
            "render_dock(frame, painter, model, DockPosition::Bottom, chrome)",
        ),
        location(
            collapsed,
            12,
            "super::render_dock(frame, painter, model, position, chrome)",
        ),
    ];
    usages.source = "render_dock".into();
    usages.status = "5 usages".into();
    usages.selected_index = Some(2);
    usages.collapsed.insert(collapsed.into());
}

/// Feed a headless session sized to the live terminal panel with deterministic
/// ANSI output, a text selection and a hovered link.
pub(super) fn populate_terminal_content(model: &mut AppModel) {
    use alacritty_terminal::index::{Column, Line, Point, Side};
    use alacritty_terminal::selection::SelectionType;

    let chrome = crate::layout::chrome::chrome(model);
    let rect = chrome
        .rect(crate::layout::UiKey::PanelContent(PanelId::Terminal))
        .unwrap_or_default();
    let size =
        crate::panels::terminal::grid_size_for_rect(rect, model.char_width, model.line_height);
    let (tx, _rx) = std::sync::mpsc::channel();
    let Some(id) = model.terminal.begin_spawn() else {
        return;
    };
    let (pty, _writes) = crate::terminal::PtyHandle::headless();
    let mut session = crate::terminal::TerminalSession::new(
        id,
        size.rows.max(1).into(),
        size.cols.max(1).into(),
        pty,
        tx,
    );
    session.title = "shell".into();
    session.apply_bytes(
        b"$ cargo test --features ui-gallery\r\n\
          \x1b[32mok\x1b[0m 128 passed; 0 failed\r\n\
          docs: \x1b]8;;https://example.com/docs\x1b\\gallery guide\x1b]8;;\x1b\\ or https://token.dev\r\n\
          \x1b[1mwarning\x1b[0m: unused variable `frame`\r\n\
          \x1b[36m-->\x1b[0m src/view/gallery.rs:42:9\r\n\
          $ \x1b[6;3H",
    );
    session.start_selection(
        Point::new(Line(1), Column(3)),
        Side::Left,
        SelectionType::Simple,
    );
    session.update_selection(Point::new(Line(1), Column(12)), Side::Right);
    let link = session.link_at(Point::new(Line(2), Column(8)));
    model.terminal.sessions.push(session);
    model.terminal.clear_spawn_pending(id);
    model.terminal.active = 0;
    model.terminal.hovered_link = link.map(|link| (id, link));
}

pub(super) fn render(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: Rect,
    scale: f64,
    kind: ChromePreview,
) {
    let mut painter = painter.with_font(FontRole::Code);
    let right = matches!(kind, ChromePreview::RightPanel);
    let width = rect.width.ceil() as usize * if right { 3 } else { 1 };
    let height = if right {
        rect.height as usize + (24.0 * scale) as usize
    } else {
        (600.0 * scale) as usize
    };
    let mut model = AppModel::new(width as u32, height as u32, scale);
    model.theme = theme.clone();
    model.line_height = painter.line_height();
    model.status_bar_height = (24.0 * scale) as usize;
    model.char_width = painter.char_width();
    model.recompute_tab_bar_height_from_line_height();
    let mut pixels = vec![theme.sidebar.background.to_argb_u32(); width * height];
    let source = {
        let mut tile = Frame::new(&mut pixels, width, height);
        match kind {
            ChromePreview::DocumentTabs
            | ChromePreview::DocumentOverflow
            | ChromePreview::DocumentDrag => {
                let area = &mut model.editor_area;
                let group_id = area.focused_group_id;
                area.documents
                    .values_mut()
                    .for_each(|doc| doc.file_path = Some("main.rs".into()));
                for (name, modified, failed) in [("lib.rs", true, false), ("cfg.rs", false, true)] {
                    let doc_id = area.next_document_id();
                    let editor_id = area.next_editor_id();
                    let tab_id = area.next_tab_id();
                    let mut doc = Document::new();
                    doc.id = Some(doc_id);
                    doc.file_path = Some(
                        if modified && matches!(kind, ChromePreview::DocumentOverflow) {
                            "a_very_long_document_name_that_is_clipped.rs"
                        } else {
                            name
                        }
                        .into(),
                    );
                    doc.is_modified = modified;
                    if failed {
                        doc.save_error = Some((0, "Example save failure".into()));
                    }
                    let mut editor = EditorState::with_viewport(1, 1);
                    editor.id = Some(editor_id);
                    editor.document_id = Some(doc_id);
                    area.documents.insert(doc_id, doc);
                    area.editors.insert(editor_id, editor);
                    if let Some(group) = area.groups.get_mut(&group_id) {
                        group.tabs.push(Tab {
                            id: tab_id,
                            editor_id,
                            is_pinned: false,
                            is_preview: false,
                        });
                    }
                }
                if let Some(group) = area.groups.get_mut(&group_id) {
                    group.rect = Rect::new(0.0, 0.0, rect.width, rect.height);
                    group.active_tab_index = 1;
                    if matches!(kind, ChromePreview::DocumentOverflow) {
                        group.tab_scroll = (95.0 * scale) as usize;
                    }
                }
                if let Some(group) = model.editor_area.groups.get(&group_id) {
                    let layout = crate::layout::editor::EditorTabBarLayout::new(
                        group,
                        &model,
                        painter.char_width(),
                    );
                    if matches!(kind, ChromePreview::DocumentDrag) {
                        model.ui.tab_drag = Some(crate::model::ui::TabDragState {
                            tab_id: group.tabs[1].id,
                            press: (0.0, 0.0),
                            current: (rect.width as f64 / 2.0, rect.height as f64 / 2.0),
                            active: true,
                        });
                        Renderer::render_tab_drag_ghost(&mut tile, &mut painter, &model);
                    } else {
                        super::document_tabs::render(
                            &mut tile,
                            &mut painter,
                            &model,
                            group,
                            &layout,
                        );
                    }
                }
                Rect::new(0.0, 0.0, rect.width, rect.height)
            }
            ChromePreview::DockTabs
            | ChromePreview::BottomPanel
            | ChromePreview::ProblemsPopulated
            | ChromePreview::UsagesPopulated
            | ChromePreview::TerminalContent
            | ChromePreview::RightPanel
            | ChromePreview::TerminalTabs
            | ChromePreview::TerminalOverflow
            | ChromePreview::TerminalExited => {
                let position = if right {
                    DockPosition::Right
                } else {
                    DockPosition::Bottom
                };
                let dock = model.dock_layout.dock_mut(position);
                dock.panel_ids = if right {
                    vec![PanelId::Outline]
                } else {
                    vec![PanelId::Terminal, PanelId::Problems, PanelId::Usages]
                };
                dock.active_index = Some(if right { 0 } else { 1 });
                dock.is_open = true;
                dock.set_size(
                    if right {
                        rect.width
                    } else {
                        rect.height.max((140.0 * scale) as f32)
                    },
                    scale,
                );
                if right {
                    populate_outline(&mut model);
                }
                if matches!(kind, ChromePreview::ProblemsPopulated) {
                    populate_problems(&mut model);
                    model.dock_layout.bottom.activate(PanelId::Problems);
                }
                if matches!(kind, ChromePreview::UsagesPopulated) {
                    populate_usages(&mut model);
                    model.dock_layout.bottom.activate(PanelId::Usages);
                }
                if matches!(kind, ChromePreview::TerminalContent) {
                    model.dock_layout.bottom.activate(PanelId::Terminal);
                    populate_terminal_content(&mut model);
                }
                if matches!(
                    kind,
                    ChromePreview::TerminalTabs
                        | ChromePreview::TerminalOverflow
                        | ChromePreview::TerminalExited
                ) {
                    model.dock_layout.bottom.activate(PanelId::Terminal);
                    let (tx, _rx) = std::sync::mpsc::channel();
                    for (i, title) in ["shell", "build", "tests"].into_iter().enumerate() {
                        let Some(id) = model.terminal.begin_spawn() else {
                            break;
                        };
                        let (pty, _writes) = crate::terminal::PtyHandle::headless();
                        let mut session =
                            crate::terminal::TerminalSession::new(id, 10, 40, pty, tx.clone());
                        session.title = title.into();
                        session.exited = i == 2;
                        model.terminal.sessions.push(session);
                        model.terminal.clear_spawn_pending(id);
                    }
                    model.terminal.active = 0;
                    model.terminal.hovered_tab = model
                        .terminal
                        .sessions
                        .get(1)
                        .map(|session| crate::terminal::TabAction::Select(session.id));
                    if matches!(kind, ChromePreview::TerminalExited) {
                        model.terminal.active = 2;
                        model.terminal.hovered_tab = None;
                        crate::panels::terminal::reveal_active_tab(&mut model);
                    }
                    if matches!(kind, ChromePreview::TerminalOverflow) {
                        model.terminal.tab_scroll = (110.0 * scale) as f32;
                    }
                }
                let chrome = crate::layout::chrome::chrome(&model);
                let key = if matches!(
                    kind,
                    ChromePreview::TerminalTabs
                        | ChromePreview::TerminalOverflow
                        | ChromePreview::TerminalExited
                ) {
                    crate::layout::UiKey::TerminalTabs
                } else if matches!(kind, ChromePreview::DockTabs) {
                    crate::layout::UiKey::DockHeader(position)
                } else {
                    crate::layout::UiKey::Dock(position)
                };
                if matches!(
                    kind,
                    ChromePreview::TerminalTabs
                        | ChromePreview::TerminalOverflow
                        | ChromePreview::TerminalExited
                ) {
                    crate::panels::terminal::render_tabs(&mut tile, &mut painter, &model, &chrome);
                } else if matches!(kind, ChromePreview::DockTabs) {
                    super::panels::render_dock_chrome(
                        &mut tile,
                        &mut painter,
                        &model,
                        position,
                        &chrome,
                    );
                } else {
                    super::panels::render_dock(&mut tile, &mut painter, &model, position, &chrome);
                }
                chrome.rect(key).unwrap_or_default()
            }
        }
    };
    // Cropping does not resize text or reconstruct any production geometry.
    let (sx, sy, sw, sh) = crate::layout::snapshot::snap(source);
    for y in 0..sh.min(rect.height as usize).min(height.saturating_sub(sy)) {
        for x in 0..sw.min(rect.width as usize).min(width.saturating_sub(sx)) {
            frame.set_pixel(
                rect.x as usize + x,
                rect.y as usize + y,
                pixels[(sy + y) * width + sx + x],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::usages::UsagesRow;

    fn model() -> AppModel {
        let mut model = AppModel::new(400, 300, 1.0);
        model.line_height = 20;
        model.char_width = 8.0;
        let dock = model.dock_layout.dock_mut(DockPosition::Bottom);
        dock.panel_ids = vec![PanelId::Terminal, PanelId::Problems, PanelId::Usages];
        dock.is_open = true;
        dock.set_size(180.0, 1.0);
        model
    }

    #[test]
    fn usages_fixture_has_file_and_location_rows() {
        let mut model = model();
        populate_usages(&mut model);
        let rows = model.usages_panel.rows();
        assert!(rows.iter().any(|row| matches!(row, UsagesRow::File { .. })));
        assert!(rows.iter().any(|row| matches!(row, UsagesRow::Location(_))));
        assert!(model
            .usages_panel
            .selected_index
            .is_some_and(|i| matches!(rows[i], UsagesRow::Location(_))));
    }

    #[test]
    fn terminal_fixture_has_selection_and_hovered_link() {
        let mut model = model();
        model.dock_layout.bottom.activate(PanelId::Terminal);
        populate_terminal_content(&mut model);
        let session = model.terminal.active_session().unwrap();
        assert!(session.term().selection.is_some());
        assert_eq!(
            model.terminal.hovered_link.as_ref().map(|(id, _)| *id),
            Some(session.id)
        );
    }
}
