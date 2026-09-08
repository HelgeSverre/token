//! Completion menu hot paths (lsp-integration.md Phase 5): the synchronous
//! per-keystroke refresh with LSP items carried, and the runtime-side
//! conversion of a server response.
//!
//! Run with: just bench-completion

use token::completion::lsp::items_to_menu_items;
use token::completion::menu::filter_and_sort;
use token::lsp::LspServerId;
use token::messages::{DocumentMsg, LspMsg, Msg};
use token::model::AppModel;
use token::update::update;

// Compile the runtime's private implementation into this benchmark, without
// exporting a profiling API or maintaining a second ring implementation.
#[path = "../src/runtime/inline_context.rs"]
mod inline_context;
mod support;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

/// ts-ls-shaped items: label + kind + sortText + detail + textEdit + data.
fn server_items(n: usize) -> Vec<lsp_types::CompletionItem> {
    (0..n)
        .map(|i| {
            serde_json::from_value(serde_json::json!({
                "label": format!("value_{i}_something"),
                "kind": 6,
                "sortText": format!("{:05}", i),
                "detail": "(property) value: string",
                "textEdit": {
                    "range": { "start": { "line": 10, "character": 4 }, "end": { "line": 10, "character": 6 } },
                    "newText": format!("value_{i}_something"),
                },
                "data": { "file": "/tmp/proj/src/lib.rs", "line": 10, "offset": 6, "entryNames": [format!("value_{i}_something")] },
            }))
            .unwrap()
        })
        .collect()
}

fn model_with_open_menu(lsp_items: usize) -> AppModel {
    // ~5000 lines of identifier-dense text, cursor at the end.
    let mut text = String::new();
    for i in 0..5000 {
        text.push_str(&format!(
            "let value_{i} = other_{i} + compute_{i}(arg_{i});\n"
        ));
    }
    text.push('\n');
    let mut model = AppModel::new(1920, 1080, 1.0);
    model.document_mut().buffer = ropey::Rope::from(text.as_str());
    model.document_mut().language = token::syntax::LanguageId::Rust;
    model.document_mut().file_path = Some("/tmp/proj/src/lib.rs".into());
    let last = model.document().line_count() - 1;
    model.editor_mut().cursors[0] = token::model::Cursor::at(last, 0);
    for ch in "va".chars() {
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
    }
    let state = model.ui.completion_menu.as_ref().expect("menu open");
    let (document_id, revision) = (state.document_id, state.revision);
    let items = items_to_menu_items(
        server_items(lsp_items),
        &LspServerId::from("rust-analyzer"),
        std::path::Path::new("/tmp/proj"),
        Some(&lsp_types::CompletionOptions {
            resolve_provider: Some(true),
            ..Default::default()
        }),
    );
    update(
        &mut model,
        Msg::Lsp(LspMsg::CompletionResolved {
            document_id,
            revision,
            items,
            is_incomplete: false,
        }),
    );
    assert!(model.ui.completion_menu.is_some());
    model
}

/// One keystroke with a pending parse: local sources wait for fresh syntax;
/// carried LSP items are refiltered immediately.
#[divan::bench(args = [0, 200, 1000])]
fn keystroke_with_menu_open(bencher: divan::Bencher, lsp_items: usize) {
    bencher
        .with_inputs(|| model_with_open_menu(lsp_items))
        .bench_local_refs(|model| {
            update(model, Msg::Document(DocumentMsg::InsertChar('l')));
        });
}

#[divan::bench(args = [200, 1000])]
fn convert_server_response(bencher: divan::Bencher, n: usize) {
    bencher
        .with_inputs(|| server_items(n))
        .bench_values(|items| {
            items_to_menu_items(
                items,
                &LspServerId::from("rust-analyzer"),
                std::path::Path::new("/tmp/proj"),
                Some(&lsp_types::CompletionOptions {
                    resolve_provider: Some(true),
                    ..Default::default()
                }),
            )
        });
}

#[divan::bench(args = [200, 1000])]
fn filter_and_sort_lsp_items(bencher: divan::Bencher, n: usize) {
    let items = items_to_menu_items(
        server_items(n),
        &LspServerId::from("rust-analyzer"),
        std::path::Path::new("/tmp/proj"),
        Some(&lsp_types::CompletionOptions {
            resolve_provider: Some(true),
            ..Default::default()
        }),
    );
    bencher.bench_local(|| filter_and_sort(&items, "val"));
}

/// A completed parse allows syntax-filtered fallback words. Keep parsing and
/// fixture construction outside the measured collection/filter path.
#[divan::bench]
fn fresh_code_word_fallback(bencher: divan::Bencher) {
    let mut text = String::from("fn main() {\n");
    for i in 0..2000 {
        text.push_str(&format!("let value_{i} = 1; // value_comment_{i}\n"));
    }
    text.push_str("va\n}\n");
    let mut doc = token::model::Document::with_text(&text);
    doc.language = token::syntax::LanguageId::Rust;
    doc.syntax_highlights = Some(token::syntax::ParserState::new().parse_and_highlight(
        &text,
        doc.language,
        token::model::DocumentId(1),
        doc.revision,
    ));
    let cursor = token::model::Cursor::at(2001, 2);
    let collect = || token::completion::sources::collect_words(&doc, cursor, "va", 3);
    let items = collect();
    assert_eq!(items.len(), 500);
    assert!(!items.iter().any(|item| item.label.contains("comment")));
    bencher.bench_local(|| filter_and_sort(&collect(), "va"));
}

/// Worker-side syntax/indentation filtering, excluding snapshot capture and
/// network time. Reuse the parser as the production worker does.
#[divan::bench(args = [100, 1000])]
fn inline_context_filters(bencher: divan::Bencher, lines: usize) {
    use token::completion::postprocess::{InlineContext, InlinePostprocessor};
    let mut source = String::from("fn main() {\n");
    for i in 0..lines {
        source.push_str(&format!("    let value_{i} = call({i});\n"));
    }
    source.push_str("    \n}\n");
    let mut document = token::model::Document::with_text(&source);
    document.language = token::syntax::LanguageId::Rust;
    let context = InlineContext::capture(&document, (lines + 1, 4));
    let mut processor = InlinePostprocessor::default();
    let texts = vec!["next();\n\tmore();".to_owned()];
    assert_eq!(
        processor.serve_all(&texts, context.as_ref()),
        ["next();\n    more();"]
    );
    bencher.bench_local(|| processor.serve_all(&texts, context.as_ref()));
}

struct RecencyFixture {
    model: AppModel,
    ring: inline_context::InlineContextRing,
    job: token::completion::provider::InlineJob,
    due: std::time::Instant,
}

impl RecencyFixture {
    fn new(chunks: usize, committed: bool, shared_tokens: bool) -> Self {
        use token::completion::recency::{ContextStrategy, CHUNK_BYTES};
        let mut model = support::make_model(1);
        model.config.lsp.enabled = false;
        model.config.completion.inline.enabled = true;
        model.config.completion.inline.provider = "bench".into();
        let provider = token::config::ProviderConfig {
            context: ContextStrategy::RecencyRing {
                max_chunks: chunks,
                chunk_lines: 64,
            },
            ..Default::default()
        };
        model
            .config
            .completion
            .providers
            .insert("bench".into(), provider.clone());
        let mut ring = inline_context::InlineContextRing::default();
        let now = std::time::Instant::now();
        for index in 0..chunks {
            if index != 0 {
                update(&mut model, Msg::Layout(token::messages::LayoutMsg::NewTab));
            }
            let mut text = String::new();
            let mut word = 0;
            while text.len() < CHUNK_BYTES.as_usize() {
                // Shared tokens sort first, forcing near-duplicate comparisons
                // to walk most matches before rejecting the remaining tail.
                let source = if shared_tokens {
                    if word % 100 < 94 {
                        0
                    } else {
                        index + 1
                    }
                } else {
                    index
                };
                text.push_str(&format!("item_{source:02}_{word:04} "));
                word += 1;
            }
            text.truncate(CHUNK_BYTES.as_usize());
            model.document_mut().buffer = text.as_str().into();
            ring.observe(&model, now);
        }
        let due = ring
            .deadline()
            .expect("queued captures have an idle deadline");
        let request = token::completion::inline::build_request(
            model.document(),
            (0, 0),
            1,
            Some("rust".into()),
            false,
        )
        .unwrap();
        let job = token::completion::provider::InlineJob {
            request,
            provider,
            context: None,
        };
        let mut fixture = Self {
            model,
            ring,
            job,
            due,
        };
        if committed {
            fixture.ring.observe(&fixture.model, due);
            fixture.ring.attach(&fixture.model, &mut fixture.job, due);
            assert_eq!(
                fixture.job.request.extra_context.len(),
                chunks,
                "distinct snippets must not collapse during setup"
            );
            assert_eq!(
                fixture
                    .job
                    .request
                    .extra_context
                    .iter()
                    .map(|chunk| chunk.text.len())
                    .sum::<usize>(),
                chunks * CHUNK_BYTES.as_usize()
            );
            // A production build_request starts with an unallocated Vec. Do
            // not let setup's validation preallocate the timed attachment.
            fixture.job.request.extra_context = Vec::new();
        }
        fixture
    }

    fn queue_refresh(&mut self) {
        let mut ids: Vec<_> = self.model.editor_area.documents.keys().copied().collect();
        ids.sort_unstable_by_key(|id| id.0);
        let now = self.due + std::time::Duration::from_secs(1);
        for id in ids {
            self.ring.saved(&self.model, id, now);
        }
        self.due = self.ring.deadline().expect("saves queue idle captures");
    }
}

/// First idle drain of distinct maximum-byte snippets, with an initially empty ring.
#[divan::bench(args = [8, 32], sample_count = 100)]
fn recency_idle_fill(bencher: divan::Bencher, chunks: usize) {
    bencher
        .with_inputs(|| RecencyFixture::new(chunks, false, false))
        .bench_local_refs(|fixture| {
            fixture.ring.observe(&fixture.model, fixture.due);
        });
}

/// Refresh a full ring after all buffers have saved. Setup and queueing are excluded.
#[divan::bench(args = [8, 32], sample_count = 100)]
fn recency_idle_refresh(bencher: divan::Bencher, chunks: usize) {
    bencher
        .with_inputs(|| {
            let mut fixture = RecencyFixture::new(chunks, true, false);
            fixture.queue_refresh();
            fixture
        })
        .bench_local_refs(|fixture| {
            fixture.ring.observe(&fixture.model, fixture.due);
        });
}

#[divan::bench(args = [8, 32], sample_count = 100)]
fn recency_observe_stable(bencher: divan::Bencher, chunks: usize) {
    let mut fixture = RecencyFixture::new(chunks, true, false);
    bencher.bench_local(|| fixture.ring.observe(&fixture.model, fixture.due));
}

/// Approximately 94% shared tokens => Jaccard similarity just below 0.9.
/// Setup asserts all snippets survive, so the full ring is compared each time.
#[divan::bench(args = [8, 32], sample_count = 100)]
fn recency_idle_refresh_near_duplicates(bencher: divan::Bencher, chunks: usize) {
    bencher
        .with_inputs(|| {
            let mut fixture = RecencyFixture::new(chunks, true, true);
            fixture.queue_refresh();
            fixture
        })
        .bench_local_refs(|fixture| {
            fixture.ring.observe(&fixture.model, fixture.due);
        });
}

#[divan::bench(args = [8, 32], sample_count = 100)]
fn recency_attach_request(bencher: divan::Bencher, chunks: usize) {
    bencher
        .with_inputs(|| RecencyFixture::new(chunks, true, false))
        .bench_local_refs(|fixture| {
            fixture
                .ring
                .attach(&fixture.model, &mut fixture.job, fixture.due);
        });
}

/// Production update fixtures, with no config I/O or provider/network work.
/// The backend result is already normalized; only editor-side projection is
/// measured here. Whole-document wrap setup is excluded from arrival/cycling.
struct GhostFixture {
    model: AppModel,
    arrival: Option<Msg>,
}

impl GhostFixture {
    fn new(lines: usize, anchor_chars: usize, visible: bool) -> Self {
        use token::messages::CompletionMsg;
        use token::model::{Cursor, Position, Selection};

        let mut model = support::make_model(lines);
        model.config.lsp.enabled = false;
        model.config.completion.inline.enabled = true;
        model.config.completion.inline.provider = "bench".into();
        model
            .config
            .completion
            .providers
            .insert("bench".into(), Default::default());
        let line = lines / 2;
        let start = model.document().buffer.line_to_char(line);
        let end = start + model.document().line_length(line);
        model.document_mut().buffer.remove(start..end);
        model
            .document_mut()
            .buffer
            .insert(start, &"x".repeat(anchor_chars));
        let column = anchor_chars / 2;
        let editor = model.editor_mut();
        editor.cursors = vec![Cursor::at(line, column)];
        editor.selections = vec![Selection::new(Position::new(line, column))];
        editor.soft_wrap = true;
        editor.viewport.visible_columns = 80;
        model.editor_area.refresh_wrap_caches();
        model.ensure_cursor_visible();
        update(
            &mut model,
            Msg::Completion(CompletionMsg::TriggerInline { explicit: true }),
        );
        let snapshot = model.ui.inline_session.as_ref().unwrap().snapshot.clone();
        let texts = vec![
            "let item = build();\n\tuse_item(item);\n".repeat(4),
            "let other = make();\n\tfinish(other);\n".repeat(4),
        ];
        let mut fixture = Self {
            model,
            arrival: Some(Msg::Completion(CompletionMsg::InlineReady {
                snapshot,
                texts,
            })),
        };
        if visible {
            fixture.arrive();
            assert!(token::update::inline::visible(&fixture.model).is_some());
            assert!(
                fixture
                    .model
                    .editor()
                    .viewport_map(fixture.model.document())
                    .row_count()
                    > lines + 1
            );
        }
        assert_eq!(fixture.model.document().line_count(), lines + 1);
        assert!(fixture.model.document().undo_stack.is_empty());
        fixture
    }

    fn arrive(&mut self) {
        update(
            &mut self.model,
            self.arrival
                .take()
                .expect("one backend arrival per generated input"),
        );
    }

    fn warm_renderer(&mut self, renderer: &mut support::BenchRenderer) {
        renderer.render_frame(&mut self.model); // establish actual font/viewport metrics
        let (document, editor) = self
            .model
            .editor_area
            .focused_document_and_editor_mut()
            .unwrap();
        let row = editor.cursor_visual_line(document);
        editor.set_top_line_clamped(document, row.saturating_sub(3));
        renderer.render_frame(&mut self.model); // warm the visible glyphs
        let map = self.model.editor().viewport_map(self.model.document());
        let cursor = self.model.editor().active_cursor();
        assert_eq!(
            map.visible_row_for_position(cursor.line, cursor.column),
            Some(3)
        );
        assert!(
            map.visible_row_for_doc_line(cursor.line + 1).is_some(),
            "all ghost rows and the following source line must be on screen"
        );
        let extra_lines = token::update::inline::visible(&self.model).map_or(0, |state| {
            state.remaining().chars().filter(|&ch| ch == '\n').count()
        });
        // Every source line fits this warmed pane. UI visibility alone would
        // not detect a lost derived projection after the font/width refresh.
        assert_eq!(
            map.row_count(),
            self.model.document().line_count() + extra_lines
        );
    }
}

#[divan::bench(args = [100, 10_000], sample_count = 100)]
fn ghost_arrival(bencher: divan::Bencher, lines: usize) {
    bencher
        .with_inputs(|| GhostFixture::new(lines, 80, false))
        .bench_local_refs(GhostFixture::arrive);
}

/// Character counts, not byte limits. A long anchor must not silently truncate.
#[divan::bench(args = [80, 4_096, 65_536], sample_count = 100)]
fn ghost_arrival_long_anchor(bencher: divan::Bencher, anchor_chars: usize) {
    bencher
        .with_inputs(|| GhostFixture::new(100, anchor_chars, false))
        .bench_local_refs(GhostFixture::arrive);
}

#[divan::bench(args = [100, 10_000], sample_count = 100)]
fn ghost_cycle(bencher: divan::Bencher, lines: usize) {
    let mut fixture = GhostFixture::new(lines, 80, true);
    bencher.bench_local(|| {
        update(
            &mut fixture.model,
            Msg::Completion(token::messages::CompletionMsg::CycleInline { forward: true }),
        )
    });
}

#[divan::bench(args = [100, 10_000], sample_count = 100)]
fn ghost_blink_update(bencher: divan::Bencher, lines: usize) {
    let mut fixture = GhostFixture::new(lines, 80, true);
    bencher.bench_local(|| {
        update(
            &mut fixture.model,
            Msg::Ui(token::messages::UiMsg::BlinkCursor),
        )
    });
}

/// Includes the ordinary edit transaction, source wrap refresh and reprojected
/// remainder. Runtime effects returned by update are intentionally not executed.
#[divan::bench(args = [80, 4_096, 65_536], sample_count = 100)]
fn ghost_type_through(bencher: divan::Bencher, anchor_chars: usize) {
    bencher
        .with_inputs(|| GhostFixture::new(100, anchor_chars, true))
        .bench_local_refs(|fixture| {
            update(
                &mut fixture.model,
                Msg::Document(DocumentMsg::InsertChar('l')),
            )
        });
}

/// A width change necessarily also rewraps the source document. This measures
/// that complete shared refresh, not just the anchor-line insertion projection.
#[divan::bench(args = [100, 10_000], sample_count = 100)]
fn ghost_width_reflow(bencher: divan::Bencher, lines: usize) {
    bencher
        .with_inputs(|| GhostFixture::new(lines, 80, true))
        .bench_local_refs(|fixture| {
            fixture.model.editor_mut().viewport.visible_columns = 40;
            fixture.model.editor_area.refresh_wrap_caches();
        });
}

#[divan::bench(args = [100, 10_000], sample_count = 100)]
fn ghost_render_visible(bencher: divan::Bencher, lines: usize) {
    let mut fixture = GhostFixture::new(lines, 80, true);
    let mut renderer = support::BenchRenderer::new(1100, 720, 20);
    fixture.warm_renderer(&mut renderer);
    assert!(token::update::inline::visible(&fixture.model).is_some());
    bencher.bench_local(|| renderer.render_frame(&mut fixture.model));
}

#[divan::bench(args = [100, 10_000], sample_count = 100)]
fn ghost_render_plain(bencher: divan::Bencher, lines: usize) {
    let mut fixture = GhostFixture::new(lines, 80, false);
    let mut renderer = support::BenchRenderer::new(1100, 720, 20);
    fixture.warm_renderer(&mut renderer);
    assert!(token::update::inline::visible(&fixture.model).is_none());
    bencher.bench_local(|| renderer.render_frame(&mut fixture.model));
}
