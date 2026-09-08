//! Production-path regression probes. Timings exclude setup and window presentation.
//! Run `just profile-workloads`, `just profile-workloads find`, or attach a
//! native sampler using the `sample` / `sample-find` modes (12 seconds).
//! `find-cold` measures uncached matches and worker computation separately;
//! `sample-find-cold` / `sample-find-worker` keep those stages busy for sampling.

use std::hint::black_box;
use std::time::{Duration, Instant};
use token::messages::{Direction, EditorMsg, Msg};
use token::model::{AppModel, Cursor, EditOperation, Rect};
use token::view::{Frame, GlyphCache, Renderer, TextPainter};

fn model(text: &str, wrap: bool) -> AppModel {
    let mut m = AppModel::new(1920, 1080, 1.0);
    m.document_mut().buffer = text.into();
    m.config.completion.enabled = false;
    m.config.bracket_matching = false;
    m.char_width = 8.4;
    m.line_height = 20;
    m.editor_mut().soft_wrap = wrap;
    m.editor_area
        .compute_layout(Rect::new(0.0, 0.0, 1920.0, 1060.0));
    m.resync_viewports();
    m
}

fn measure(name: &str, n: usize, mut f: impl FnMut()) {
    for _ in 0..10 {
        f();
    }
    let mut times = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        f();
        times.push(t.elapsed());
    }
    report(name, times);
}

fn report(name: &str, mut times: Vec<Duration>) {
    times.sort_unstable();
    let n = times.len();
    println!(
        "{name}: median={:.3}us p95={:.3}us n={n}",
        times[n / 2].as_secs_f64() * 1e6,
        times[n * 95 / 100].as_secs_f64() * 1e6
    );
}

fn cold_find(sample_stage: Option<&str>) {
    use std::sync::Arc;
    use token::messages::UiMsg;
    use token::model::{FindReplaceState, ModalState};

    fn search_request(cmd: token::Cmd) -> Option<Arc<token::model::ui::FindSearchRequest>> {
        match cmd {
            token::Cmd::RunFindSearch(request) => Some(request),
            token::Cmd::Batch(cmds) => cmds.into_iter().find_map(search_request),
            _ => None,
        }
    }

    for lines in [10_000, 100_000] {
        let text = format!(
            "{}final_marker\n",
            "ordinary text on a short line\n".repeat(lines)
        );
        let mut m = model(&text, false);
        for (name, pattern, count) in [
            ("dense", "ordinary", lines),
            ("sparse", "final_marker", 1),
            ("absent", "missing_marker", 0),
        ] {
            if sample_stage.is_some() && (lines != 100_000 || name != "dense") {
                continue;
            }
            let mut state = FindReplaceState::default();
            state.set_query(pattern);
            assert_eq!(state.matches(m.document()).len(), count);
            // Install a genuinely cold state so update emits the production request.
            let mut state = FindReplaceState::default();
            state.set_query(pattern);
            m.ui.open_modal(ModalState::FindReplace(state));
            let request =
                search_request(token::update::update(&mut m, Msg::Ui(UiMsg::BlinkCursor)).unwrap())
                    .expect("large cold document must schedule Find");
            let scan = || {
                let mut state = FindReplaceState::default();
                state.set_query(pattern);
                black_box(state.matches(m.document()));
            };
            let worker = || {
                black_box(request.compute());
            };
            if let Some(stage) = sample_stage {
                println!("sampling Find {stage}: pid={}", std::process::id());
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(12) {
                    if stage == "cold" {
                        scan();
                    } else {
                        worker();
                    }
                }
            } else {
                measure(&format!("find_cold {name} lines={lines}"), 100, scan);
                measure(&format!("find_worker {name} lines={lines}"), 100, worker);
            }
        }
    }
}

fn insertions() {
    use token::messages::{DocumentMsg, LayoutMsg};
    use token::model::{Selection, SplitDirection};
    for panes in [1, 2] {
        for count in [1, 10, 100, 1000] {
            let text = "abc ".repeat(count + 1);
            let expected = format!("{}abc ", "a🙂bc ".repeat(count));
            let mut m = model(&text, false);
            m.config.lsp.enabled = false;
            if panes == 2 {
                token::update::update(
                    &mut m,
                    Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
                );
            }
            let pristine = m.document().buffer.clone();
            let cursors: Vec<_> = (0..count).map(|i| Cursor::at(0, 1 + 4 * i)).collect();
            let selections: Vec<_> = cursors
                .iter()
                .map(|c| Selection::new(c.to_position()))
                .collect();
            let mut times = Vec::with_capacity(500);
            for sample in 0..510 {
                // Restore the fixture outside timing: no history growth, model
                // construction or rope copying is charged to the edit itself.
                m.document_mut().buffer = pristine.clone();
                m.document_mut().undo_stack.clear();
                m.document_mut().redo_stack.clear();
                for editor in m.editor_area.editors.values_mut() {
                    editor.cursors.clone_from(&cursors);
                    editor.selections.clone_from(&selections);
                    editor.active_cursor_index = 0;
                }
                let start = Instant::now();
                black_box(token::update::update(
                    &mut m,
                    Msg::Document(DocumentMsg::InsertChar('🙂')),
                ));
                let elapsed = start.elapsed();
                if sample >= 10 {
                    times.push(elapsed);
                }
                assert_eq!(m.document().buffer.to_string(), expected);
                assert_eq!(m.document().undo_stack.len(), 1);
                for editor in m.editor_area.editors.values() {
                    for (i, cursor) in editor.cursors.iter().enumerate() {
                        assert_eq!(*cursor, Cursor::at(0, 2 + 5 * i));
                    }
                    assert!(editor.selections.iter().all(Selection::is_empty));
                }
            }
            report(
                &format!("unicode_insert cursors={count} panes={panes}"),
                times,
            );
        }
    }
}

fn movement() {
    for history in [0, 100, 1000, 10000] {
        let mut m = model(&"some ordinary text\n".repeat(10000), false);
        m.document_mut().undo_stack = (0..history)
            .map(|i| EditOperation::Insert {
                position: i,
                text: "x".repeat(64),
                cursor_before: Cursor::at(0, 0),
                cursor_after: Cursor::at(0, 1),
            })
            .collect();
        measure(&format!("cursor_update history={history}"), 500, || {
            m.editor_mut().cursors[0] = Cursor::at(0, 0);
            black_box(token::update::update(
                &mut m,
                Msg::Editor(EditorMsg::MoveCursor(Direction::Right)),
            ));
        });
        measure(&format!("document_clone history={history}"), 500, || {
            black_box(m.document().clone());
        });
    }
}

/// Time mutation, Undo and Redo separately, with exact pane-state assertions.
fn edit_history() {
    use token::messages::{DocumentMsg, LayoutMsg};
    use token::model::{Position, Selection, SplitDirection};

    for (name, message, selected, duplicated_lines, result_line) in [
        ("delete", DocumentMsg::DeleteBackward, false, false, "ab\n"),
        (
            "duplicate_selection",
            DocumentMsg::Duplicate,
            true,
            false,
            "a🙂🙂b\n",
        ),
        (
            "duplicate_lines",
            DocumentMsg::Duplicate,
            false,
            true,
            "a🙂b\na🙂b\n",
        ),
    ] {
        for panes in [1, 2] {
            for count in [1, 100, 1000] {
                let text = format!("{}tail\n", "a🙂b\n".repeat(count));
                let expected = format!("{}tail\n", result_line.repeat(count));
                let mut m = model(&text, false);
                m.config.lsp.enabled = false;
                if panes == 2 {
                    token::update::update(
                        &mut m,
                        Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
                    );
                }
                let author = m.editor_area.focused_editor_id().unwrap();
                let pristine = m.document().buffer.clone();
                let cursors: Vec<_> = (0..count)
                    .map(|line| Cursor {
                        line,
                        column: 2,
                        desired_column: Some(5),
                    })
                    .collect();
                let selections: Vec<_> = (0..count)
                    .map(|line| {
                        Selection::from_anchor_head(
                            Position::new(line, if selected { 1 } else { 2 }),
                            Position::new(line, 2),
                        )
                    })
                    .collect();
                let active = count / 2;
                let mut edit_times = Vec::with_capacity(100);
                let mut undo_times = Vec::with_capacity(100);
                let mut redo_times = Vec::with_capacity(100);
                for sample in 0..110 {
                    m.document_mut().buffer = pristine.clone();
                    m.document_mut().undo_stack.clear();
                    m.document_mut().redo_stack.clear();
                    for editor in m.editor_area.editors.values_mut() {
                        editor.cursors.clone_from(&cursors);
                        editor.selections.clone_from(&selections);
                        editor.active_cursor_index = active;
                    }
                    for (stage, action) in [message.clone(), DocumentMsg::Undo, DocumentMsg::Redo]
                        .into_iter()
                        .enumerate()
                    {
                        let start = Instant::now();
                        black_box(token::update::update(&mut m, Msg::Document(action)));
                        let elapsed = start.elapsed();
                        if sample >= 10 {
                            match stage {
                                0 => edit_times.push(elapsed),
                                1 => undo_times.push(elapsed),
                                2 => redo_times.push(elapsed),
                                _ => unreachable!(),
                            }
                        }
                        assert_eq!(
                            m.document().buffer.to_string(),
                            if stage == 1 {
                                text.as_str()
                            } else {
                                expected.as_str()
                            }
                        );
                        assert_eq!(m.document().undo_stack.len(), usize::from(stage != 1));
                        assert_eq!(m.document().redo_stack.len(), usize::from(stage == 1));
                        for (&id, editor) in &m.editor_area.editors {
                            assert_eq!(editor.active_cursor_index, active);
                            assert_eq!(editor.cursors.len(), count);
                            assert_eq!(editor.selections.len(), count);
                            if stage == 1 {
                                assert_eq!(editor.cursors, cursors);
                                assert_eq!(editor.selections, selections);
                            } else {
                                for (index, (cursor, selection)) in
                                    editor.cursors.iter().zip(&editor.selections).enumerate()
                                {
                                    let line = if duplicated_lines {
                                        2 * index + usize::from(id == author)
                                    } else {
                                        index
                                    };
                                    let column = if duplicated_lines {
                                        2
                                    } else if selected {
                                        3
                                    } else {
                                        1
                                    };
                                    assert_eq!(*cursor, Cursor::at(line, column));
                                    let anchor = if selected && id != author { 1 } else { column };
                                    assert_eq!(
                                        *selection,
                                        Selection::from_anchor_head(
                                            Position::new(line, anchor),
                                            Position::new(line, column)
                                        )
                                    );
                                }
                            }
                        }
                    }
                }
                for (stage, times) in [
                    ("edit", edit_times),
                    ("undo", undo_times),
                    ("redo", redo_times),
                ] {
                    report(
                        &format!("{name} {stage} cursors={count} panes={panes}"),
                        times,
                    );
                }
            }
        }
    }
}

fn render(m: &AppModel, font: &fontdue::Font, cache: &mut GlyphCache, pixels: &mut [u32]) {
    let mut frame = Frame::new(pixels, 1920, 1080);
    let mut painter = TextPainter::new(font, cache, 14.0, 14.0, 8.4, 20);
    let group = m.editor_area.focused_group().unwrap();
    Renderer::render_editor_group(
        &mut frame,
        &mut painter,
        m,
        group.id,
        group.rect,
        true,
        &mut Default::default(),
    );
    black_box(frame.buffer_mut());
}

fn rendering(sample_mode: bool) {
    let font = fontdue::Font::from_bytes(
        include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .unwrap();
    let mut pixels = vec![0; 1920 * 1080];
    for (name, text, wrap) in [
        (
            "short_lines_10000",
            "ordinary text on a short line\n".repeat(10000),
            false,
        ),
        (
            "short_lines_100000",
            "ordinary text on a short line\n".repeat(100000),
            false,
        ),
        ("long_1000", "abc defgh ".repeat(100), true),
        ("long_10000", "abc defgh ".repeat(1000), true),
        ("long_100000", "abc defgh ".repeat(10000), true),
        ("long_1000000", "abc defgh ".repeat(100000), true),
        ("long_1000000_nowrap", "abc defgh ".repeat(100000), false),
    ] {
        if sample_mode && name != "long_1000000" {
            continue;
        }
        let m = model(&text, wrap);
        let mut cache = GlyphCache::new();
        if sample_mode {
            println!(
                "sampling pid={} production render {name}",
                std::process::id()
            );
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(12) {
                render(&m, &font, &mut cache, &mut pixels);
            }
        } else {
            measure(&format!("production_group_render {name}"), 120, || {
                render(&m, &font, &mut cache, &mut pixels)
            });
        }
    }
}

fn replacements() {
    use token::messages::{LayoutMsg, ModalMsg, UiMsg};
    use token::model::{FindReplaceState, ModalState, Selection, SplitDirection};
    for panes in [1, 2] {
        for count in [1, 100, 10_000] {
            let text = format!("header\n{}tail", "foo\n".repeat(count));
            let expected = format!("header\n{}tail", "🙂\nx\n".repeat(count));
            let mut m = model(&text, false);
            m.config.lsp.enabled = false;
            if panes == 2 {
                token::update::update(
                    &mut m,
                    Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Vertical)),
                );
            }
            let pristine = m.document().buffer.clone();
            let focused = m.editor_area.focused_editor_id().unwrap();
            let mut times = Vec::with_capacity(200);
            for sample in 0..210 {
                m.document_mut().buffer = pristine.clone();
                m.document_mut().undo_stack.clear();
                m.document_mut().redo_stack.clear();
                for editor in m.editor_area.editors.values_mut() {
                    editor.cursors = vec![Cursor::at(count + 1, 4)];
                    editor.selections = vec![Selection::new(editor.cursors[0].to_position())];
                    editor.active_cursor_index = 0;
                }
                // A fresh query cache makes the timed update include matching,
                // planning and undo capture. Fixture construction is not timed.
                let mut state = FindReplaceState::default();
                state.set_query("foo");
                state.set_replacement("🙂\nx");
                state.case_sensitive = true;
                m.ui.open_modal(ModalState::FindReplace(state));
                let start = Instant::now();
                black_box(token::update::update(
                    &mut m,
                    Msg::Ui(UiMsg::Modal(ModalMsg::ReplaceAll)),
                ));
                let elapsed = start.elapsed();
                if sample >= 10 {
                    times.push(elapsed);
                }
                assert_eq!(m.document().buffer.to_string(), expected);
                assert_eq!(m.document().undo_stack.len(), 1);
                for (&id, editor) in &m.editor_area.editors {
                    let expected_cursor = if id == focused {
                        Cursor::at(2, 1)
                    } else {
                        Cursor::at(2 * count + 1, 4)
                    };
                    assert_eq!(editor.cursors[0], expected_cursor);
                }
            }
            report(
                &format!("replace_all matches={count} panes={panes} cold_query"),
                times,
            );
        }
    }
}

/// The actual modal painter, with warmed glyph/mask caches and no native surface.
fn settings_render(
    m: &AppModel,
    font: &fontdue::Font,
    cache: &mut GlyphCache,
    masks: &mut token::view::RoundedRectMaskCache,
    pixels: &mut [u32],
    editor: bool,
) {
    let (width, height) = m.window_size;
    let mut frame = Frame::new(pixels, width as usize, height as usize);
    frame.clear(m.theme.editor.background.to_argb_u32());
    let size = 14.0 * m.metrics.scale_factor as f32;
    let mut painter = TextPainter::new(font, cache, size, size, m.char_width, m.line_height);
    if editor {
        let group = m.editor_area.focused_group().unwrap();
        Renderer::render_editor_group(
            &mut frame,
            &mut painter,
            m,
            group.id,
            group.rect,
            true,
            &mut Default::default(),
        );
    }
    token::view::modal::render_modals(
        &mut frame,
        &mut painter,
        m,
        width as usize,
        height as usize,
        masks,
    );
    black_box(frame.buffer_mut());
}

fn settings_scrolling() {
    use token::messages::{ModalMsg, UiMsg};
    use token::view::hit_test::{hit_test_modal, HitTarget, Point};
    println!(
        "settings scroll: debug_assertions={} (CPU only; excludes native input/presentation)",
        cfg!(debug_assertions)
    );
    let font = fontdue::Font::from_bytes(
        include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
        fontdue::FontSettings::default(),
    )
    .unwrap();
    for (width, height, scale) in [(1100, 720, 1.0), (400, 750, 1.0), (2200, 1440, 2.0)] {
        let mut m = model(&"ordinary background editor text\n".repeat(1000), false);
        m.metrics = token::model::ScaledMetrics::new(scale);
        m.line_height = (20.0 * scale) as usize;
        m.char_width = 8.4 * scale as f32;
        m.resize(width, height);
        m.ui.open_modal(token::model::ModalState::Settings(Default::default()));
        token::update::update(&mut m, Msg::Ui(UiMsg::Modal(ModalMsg::Scroll(127))));
        let point = Point::new(width as f64 - 22.0 * scale, height as f64 / 2.0);
        let Some(HitTarget::ModalScrollbar { geometry }) = hit_test_modal(&m, point) else {
            panic!("fixture must exercise a scrolling Settings page");
        };
        assert_eq!(geometry.state.position, 127);
        let label = format!("{width}x{height}@{scale}");
        let mut direction = 1;
        measure(&format!("settings_scroll_update {label}"), 120, || {
            black_box(token::update::update(
                &mut m,
                Msg::Ui(UiMsg::Modal(ModalMsg::Scroll(direction))),
            ));
            direction = -direction;
        });
        measure(&format!("settings_hit_layout {label}"), 120, || {
            black_box(hit_test_modal(&m, point));
        });
        let mut pixels = vec![0; width as usize * height as usize];
        let mut cache = GlyphCache::new();
        let mut masks = token::view::RoundedRectMaskCache::new();
        measure(&format!("settings_backdrop_dim {label}"), 80, || {
            let mut frame = Frame::new(&mut pixels, width as usize, height as usize);
            frame.clear(m.theme.editor.background.to_argb_u32());
            frame.dim(130);
            black_box(frame.buffer_mut());
        });
        measure(&format!("settings_modal_paint {label}"), 80, || {
            settings_render(&m, &font, &mut cache, &mut masks, &mut pixels, false);
        });
        measure(&format!("settings_scroll_and_paint {label}"), 80, || {
            black_box(token::update::update(
                &mut m,
                Msg::Ui(UiMsg::Modal(ModalMsg::Scroll(direction))),
            ));
            direction = -direction;
            settings_render(&m, &font, &mut cache, &mut masks, &mut pixels, false);
        });
        measure(
            &format!("settings_editor_and_modal_paint {label}"),
            80,
            || {
                settings_render(&m, &font, &mut cache, &mut masks, &mut pixels, true);
            },
        );
    }
}

fn main() {
    if let Some(mode) = std::env::args().find(|arg| {
        matches!(
            arg.as_str(),
            "find-cold" | "sample-find-cold" | "sample-find-worker"
        )
    }) {
        cold_find(mode.strip_prefix("sample-find-"));
        return;
    }
    if std::env::args().any(|arg| arg == "settings") {
        settings_scrolling();
        return;
    }
    if std::env::args().any(|arg| arg == "edit-history") {
        edit_history();
        return;
    }
    if std::env::args().any(|arg| arg == "replacements") {
        replacements();
        return;
    }
    if std::env::args().any(|arg| arg == "insertions") {
        insertions();
        return;
    }
    if std::env::args().any(|arg| arg == "file-identity") {
        use token::messages::LayoutMsg;
        use token::util::FileIdentity;
        for open_docs in [1, 100, 1000] {
            let mut m = model("open document\n", false);
            m.config.lsp.enabled = false;
            for index in 0..open_docs {
                if index > 0 {
                    token::update::update(&mut m, Msg::Layout(LayoutMsg::NewTab));
                }
                let source = format!("/fixture/link-{index}.rs");
                let canonical = format!("/canonical/target-{index}.rs");
                let identity =
                    FileIdentity::from_resolved(source.into(), std::path::Path::new(&canonical));
                m.lsp.diagnostics.insert(
                    identity.path().into(),
                    vec![lsp_types::Diagnostic::new_simple(
                        lsp_types::Range::default(),
                        "fixture".into(),
                    )],
                );
                let id = m.document().id;
                *m.document_mut() =
                    token::model::Document::from_loaded_text("open document\n", identity);
                m.document_mut().id = id;
            }
            assert_eq!(token::update::problems::problems_row_count(&m), 2);
            for (case, path) in [
                ("canonical_hit", "/canonical/target-0.rs"),
                ("miss", "/canonical/unopened.rs"),
            ] {
                assert_eq!(
                    m.editor_area
                        .find_document_by_path(std::path::Path::new(path))
                        .is_some(),
                    case == "canonical_hit"
                );
                measure(
                    &format!("file_identity {case} open_docs={open_docs}"),
                    500,
                    || {
                        black_box(
                            m.editor_area
                                .find_document_by_path(std::path::Path::new(path)),
                        );
                    },
                );
            }
            measure(
                &format!("file_identity problems_scope open_docs={open_docs}"),
                500,
                || {
                    black_box(token::update::problems::problems_row_count(&m));
                },
            );
        }
        return;
    }
    if std::env::args().any(|arg| arg == "file-open") {
        use token::messages::LayoutMsg;
        for open_docs in [1, 100, 1000] {
            let mut m = model("open document\n", false);
            m.config.lsp.enabled = false;
            for index in 0..open_docs {
                if index > 0 {
                    token::update::update(&mut m, Msg::Layout(LayoutMsg::NewTab));
                }
                m.document_mut().file_path = Some(format!("/fixture/open-{index}.txt").into());
            }
            measure(
                &format!("file_open request_and_failure_reply open_docs={open_docs}"),
                500,
                || {
                    let token::Cmd::PrepareFileOpen(request) = token::update::update(
                        &mut m,
                        Msg::Layout(LayoutMsg::OpenFileInNewTab("/fixture/pending.rs".into())),
                    )
                    .unwrap() else {
                        panic!("request")
                    };
                    black_box(token::update::update(
                        &mut m,
                        Msg::Layout(LayoutMsg::FilePrepared {
                            request,
                            result: Err("profiling: no disk effects executed".into()),
                        }),
                    ));
                },
            );
            let path = m.document().file_path.clone().unwrap();
            measure(
                &format!("file_open exact_path_reuse open_docs={open_docs}"),
                500,
                || {
                    black_box(token::update::update(
                        &mut m,
                        Msg::Layout(LayoutMsg::OpenFileInNewTab(path.clone())),
                    ));
                },
            );
        }
        return;
    }
    if std::env::args().any(|arg| arg == "file-io") {
        use token::messages::{AppMsg, DocumentMsg};
        let text = "ordinary text on a short line\n".repeat(100000);
        let mut m = model(&text, false);
        m.document_mut().file_path = Some("/fixture/saved.txt".into());
        m.config.format_on_save = false;
        m.config.lsp.enabled = false;
        measure(
            "file_io save_snapshot_and_completion lines=100000",
            500,
            || {
                let token::Cmd::SaveFile {
                    target,
                    path,
                    content,
                } = token::update::update(&mut m, Msg::App(AppMsg::SaveFile)).unwrap()
                else {
                    panic!("write command")
                };
                black_box(token::update::update(
                    &mut m,
                    Msg::App(AppMsg::SaveCompleted {
                        identity: None,
                        target,
                        path,
                        content,
                        result: Ok(()),
                    }),
                ));
            },
        );
        token::update::update(&mut m, Msg::Editor(EditorMsg::MoveCursorDocumentEnd));
        token::update::update(
            &mut m,
            Msg::Editor(EditorMsg::MoveCursorWithSelection(Direction::Left)),
        );
        token::update::update(&mut m, Msg::Document(DocumentMsg::InsertChar('X')));
        measure(
            "file_io undo_redo_saved_comparison lines=100000",
            200,
            || {
                black_box(token::update::update(
                    &mut m,
                    Msg::Document(DocumentMsg::Undo),
                ));
                black_box(token::update::update(
                    &mut m,
                    Msg::Document(DocumentMsg::Redo),
                ));
            },
        );
        return;
    }
    if std::env::args().any(|a| a == "shortcuts") {
        let m = model("selected text", false);
        let context = token::keymap::KeyContext::from_model(&m);
        let palette = token::model::CommandPaletteState::default();
        measure("shortcut_hints all_palette_commands", 500, || {
            for command in palette
                .matches
                .iter()
                .filter_map(|row| row.def.id.to_keymap_command())
            {
                black_box(m.ui.keymap.display_for(command, &context));
            }
        });
        return;
    }
    let sample_mode = std::env::args().any(|a| a == "sample");
    let sample_find = std::env::args().any(|a| a == "sample-find");
    if sample_find || std::env::args().any(|a| a == "find") {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut pixels = vec![0; 1920 * 1080];
        for lines in [10000, 100000] {
            if sample_find && lines != 100000 {
                continue;
            }
            let mut m = model(&"ordinary text on a short line\n".repeat(lines), false);
            let mut state = token::model::ui::FindReplaceState::default();
            state.set_query("ordinary");
            if sample_find {
                // Sample warmed rendering, not the empty pending-search display.
                black_box(state.matches(m.document()));
                m.ui.active_modal = Some(token::model::ModalState::FindReplace(state));
                let mut cache = GlyphCache::new();
                let start = Instant::now();
                while start.elapsed() < Duration::from_secs(12) {
                    render(&m, &font, &mut cache, &mut pixels);
                }
                continue;
            }
            measure(&format!("find_matches lines={lines}"), 40, || {
                black_box(state.matches(m.document()));
            });
            measure(&format!("find_status lines={lines}"), 40, || {
                black_box(state.status(m.document(), &m.editor().selections[0]));
            });
            let eof = m.document().buffer.len_chars();
            let mut inserted = false;
            measure(&format!("find_after_edit lines={lines}"), 40, || {
                let doc = m.document_mut();
                if inserted {
                    doc.buffer.remove(eof..eof + 1);
                } else {
                    doc.buffer.insert(eof, "x");
                }
                inserted = !inserted;
                doc.revision += 1;
                black_box(state.matches(m.document()));
            });
            let mut alternate = false;
            measure(
                &format!("find_after_query_change lines={lines}"),
                40,
                || {
                    state.set_query(if alternate { "ordinary" } else { "text" });
                    alternate = !alternate;
                    black_box(state.matches(m.document()));
                },
            );
            state.set_query("ordinary");
            black_box(state.matches(m.document()));
            m.ui.active_modal = Some(token::model::ModalState::FindReplace(state));
            let mut cache = GlyphCache::new();
            measure(
                &format!("production_group_render_find lines={lines}"),
                40,
                || render(&m, &font, &mut cache, &mut pixels),
            );
            let (line, column) = m.document().offset_to_cursor(eof);
            m.editor_mut().cursors[0] = Cursor::at(line, column);
            m.editor_mut().clear_selection();
            measure(
                &format!("typing_and_render_find_pending lines={lines}"),
                40,
                || {
                    let message = if inserted {
                        token::messages::DocumentMsg::DeleteBackward
                    } else {
                        token::messages::DocumentMsg::InsertChar('x')
                    };
                    inserted = !inserted;
                    black_box(token::update::update(&mut m, Msg::Document(message)));
                    render(&m, &font, &mut cache, &mut pixels);
                },
            );
            // Separate total CPU work from the responsive pending frame. Execute
            // the synthetic worker reply here; this is not OS scheduling latency.
            fn finish_search(model: &mut AppModel, cmd: token::Cmd) {
                match cmd {
                    token::Cmd::RunFindSearch(request) => {
                        let results = request.compute();
                        black_box(token::update::update(
                            model,
                            Msg::Ui(token::messages::UiMsg::FindSearchCompleted {
                                request,
                                result: Ok(results),
                            }),
                        ));
                    }
                    token::Cmd::Batch(cmds) => {
                        for cmd in cmds {
                            finish_search(model, cmd);
                        }
                    }
                    _ => {}
                }
            }
            measure(
                &format!("typing_find_synthetic_roundtrip_render lines={lines}"),
                40,
                || {
                    let message = if inserted {
                        token::messages::DocumentMsg::DeleteBackward
                    } else {
                        token::messages::DocumentMsg::InsertChar('x')
                    };
                    inserted = !inserted;
                    if let Some(cmd) = token::update::update(&mut m, Msg::Document(message)) {
                        finish_search(&mut m, cmd);
                    }
                    render(&m, &font, &mut cache, &mut pixels);
                },
            );
        }
        return;
    }
    if !sample_mode {
        movement();
    }
    rendering(sample_mode);
}
