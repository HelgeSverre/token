//! Completion menu hot paths (lsp-integration.md Phase 5): the synchronous
//! per-keystroke refresh with LSP items carried, and the runtime-side
//! conversion of a server response.
//!
//! Run with: cargo bench --bench completion

use token::completion::lsp::items_to_menu_items;
use token::completion::menu::filter_and_sort;
use token::lsp::LspServerId;
use token::messages::{CompletionMsg, DocumentMsg, LspMsg, Msg};
use token::model::AppModel;
use token::update::update;

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
    let mut model = AppModel::new(1920, 1080, 1.0, vec![]);
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
        true,
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

/// One keystroke while the menu is open: words scan + snippet collect +
/// carried LSP items clone + nucleo refilter.
#[divan::bench(args = [0, 200, 1000])]
fn keystroke_with_menu_open(bencher: divan::Bencher, lsp_items: usize) {
    bencher
        .with_inputs(|| model_with_open_menu(lsp_items))
        .bench_local_refs(|model| {
            update(model, Msg::Document(DocumentMsg::InsertChar('l')));
            // Back out so the next iteration sees the same query.
            update(model, Msg::Document(DocumentMsg::DeleteBackward));
            update(model, Msg::Completion(CompletionMsg::Dismiss));
            update(model, Msg::Completion(CompletionMsg::TriggerMenu));
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
                true,
            )
        });
}

#[divan::bench(args = [200, 1000])]
fn filter_and_sort_lsp_items(bencher: divan::Bencher, n: usize) {
    let items = items_to_menu_items(
        server_items(n),
        &LspServerId::from("rust-analyzer"),
        std::path::Path::new("/tmp/proj"),
        true,
    );
    bencher.bench_local(|| filter_and_sort(&items, "val"));
}
