//! Soft-wrap full layout, incremental edits and row lookups in release builds.
use token::model::Document;
use token::wrap::WrapCache;

fn main() {
    divan::main();
}

#[divan::bench(args = [10_000, 100_000])]
fn full_layout(bencher: divan::Bencher, lines: usize) {
    let line = format!(
        "{}\n",
        "The quick brown fox jumps over the lazy dog. ".repeat(4)
    );
    let document = Document::with_text(&line.repeat(lines));
    bencher.bench_local(|| {
        let mut cache = WrapCache::new();
        cache.rebuild(&document, 80);
        divan::black_box(cache);
    });
}

#[divan::bench(args = [10_000, 100_000])]
fn middle_line_edit(bencher: divan::Bencher, lines: usize) {
    let mut document =
        Document::with_text(&"The quick brown fox jumps over the lazy dog.\n".repeat(lines));
    let mut cache = WrapCache::new();
    cache.rebuild(&document, 80);
    let offset = document.buffer.line_to_char(lines / 2);
    let mut inserted = false;
    bencher.bench_local(|| {
        if inserted {
            document.buffer.remove(offset..offset + 1);
        } else {
            document.buffer.insert(offset, "x");
        }
        inserted = !inserted;
        document.revision = document.revision.wrapping_add(1);
        cache.refresh(&document, 80);
        divan::black_box(cache.total_visual_lines());
    });
}

#[divan::bench]
fn visual_row_lookup(bencher: divan::Bencher) {
    let document =
        Document::with_text(&"The quick brown fox jumps over the lazy dog.\n".repeat(100_000));
    let mut cache = WrapCache::new();
    cache.rebuild(&document, 20);
    bencher.bench_local(|| divan::black_box(cache.visual_to_logical(divan::black_box(234_567), 8)));
}
