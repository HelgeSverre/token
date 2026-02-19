//! Benchmarks for syntax highlighting performance
//!
//! Run with: cargo bench --bench syntax

mod support;

use token::model::editor_area::DocumentId;
use token::syntax::{LanguageId, ParserState};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// Sample source code for different languages
// ============================================================================

const RUST_SAMPLE: &str = r#"
use std::collections::HashMap;

/// A simple key-value store
pub struct Store<K, V> {
    data: HashMap<K, V>,
    count: usize,
}

impl<K: std::hash::Hash + Eq, V> Store<K, V> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            count: 0,
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.count += 1;
        self.data.insert(key, value)
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.data.get(key)
    }

    pub fn len(&self) -> usize {
        self.count
    }
}

fn main() {
    let mut store = Store::new();
    store.insert("hello", 42);
    store.insert("world", 100);
    
    if let Some(val) = store.get(&"hello") {
        println!("Found: {}", val);
    }
}
"#;

const JAVASCRIPT_SAMPLE: &str = r#"
import { useState, useEffect } from 'react';

class EventEmitter {
    constructor() {
        this.events = new Map();
    }

    on(event, callback) {
        if (!this.events.has(event)) {
            this.events.set(event, []);
        }
        this.events.get(event).push(callback);
        return () => this.off(event, callback);
    }

    emit(event, ...args) {
        const callbacks = this.events.get(event) || [];
        callbacks.forEach(cb => cb(...args));
    }
}

async function fetchData(url) {
    const response = await fetch(url);
    const data = await response.json();
    return data;
}

const numbers = [1, 2, 3, 4, 5];
const doubled = numbers.map(n => n * 2);
const sum = doubled.reduce((acc, n) => acc + n, 0);

console.log(`Sum of doubled: ${sum}`);
"#;

const HTML_SAMPLE: &str = r##"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sample Page</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <header class="main-header">
        <nav id="navigation">
            <ul>
                <li><a href="#home">Home</a></li>
                <li><a href="#about">About</a></li>
            </ul>
        </nav>
    </header>
    <main>
        <section class="hero">
            <h1>Welcome</h1>
            <p>This is a sample HTML document.</p>
        </section>
    </main>
    <script src="app.js"></script>
</body>
</html>
"##;

const CSS_SAMPLE: &str = r#"
:root {
    --primary-color: #3498db;
    --secondary-color: #2ecc71;
    --font-size: 16px;
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

body {
    font-family: 'Segoe UI', sans-serif;
    font-size: var(--font-size);
    line-height: 1.6;
}

.container {
    max-width: 1200px;
    margin: 0 auto;
    padding: 0 20px;
}

.button {
    display: inline-block;
    padding: 10px 20px;
    background-color: var(--primary-color);
    color: white;
    border-radius: 4px;
    transition: background-color 0.3s ease;
}

.button:hover {
    background-color: darken(var(--primary-color), 10%);
}

@media (max-width: 768px) {
    .container {
        padding: 0 10px;
    }
}
"#;

const YAML_SAMPLE: &str = r#"
name: Build and Test
on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta, nightly]
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          toolchain: ${{ matrix.rust }}
      - name: Build
        run: cargo build --verbose
      - name: Test
        run: cargo test --verbose
"#;

const MARKDOWN_SAMPLE: &str = r##"
# Project Documentation

## Overview

This project implements a **high-performance** text editor with syntax highlighting.

### Features

- Fast rope-based text buffer
- Tree-sitter syntax highlighting
- Multi-cursor editing
- *Italics* and **bold** support

## Installation

```bash
cargo install token
```

## Usage

```rust
let editor = Editor::new();
editor.open("file.rs")?;
```

| Feature | Status |
|---------|--------|
| Parsing | ✓ Done |
| Render  | WIP    |

[Documentation](https://example.com)
"##;

// ============================================================================
// Helper to generate large source files
// ============================================================================

fn generate_large_rust(lines: usize) -> String {
    let mut source = String::with_capacity(lines * 50);
    source.push_str("use std::collections::HashMap;\n\n");

    for i in 0..lines / 10 {
        source.push_str(&format!(
            r#"fn function_{}(x: i32) -> i32 {{
    let result = x * 2;
    println!("Value: {{}}", result);
    result
}}

"#,
            i
        ));
    }
    source
}

fn generate_large_javascript(lines: usize) -> String {
    let mut source = String::with_capacity(lines * 50);
    source.push_str("import { useState } from 'react';\n\n");

    for i in 0..lines / 10 {
        source.push_str(&format!(
            r#"function handler{}(event) {{
    const data = event.target.value;
    console.log('Received:', data);
    return data;
}}

"#,
            i
        ));
    }
    source
}

// ============================================================================
// Full parse benchmarks (current implementation)
// ============================================================================

#[divan::bench(args = ["rust", "javascript", "html", "css", "yaml", "markdown"])]
fn parse_sample(lang: &str) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);

    let (source, language) = match lang {
        "rust" => (RUST_SAMPLE, LanguageId::Rust),
        "javascript" => (JAVASCRIPT_SAMPLE, LanguageId::JavaScript),
        "html" => (HTML_SAMPLE, LanguageId::Html),
        "css" => (CSS_SAMPLE, LanguageId::Css),
        "yaml" => (YAML_SAMPLE, LanguageId::Yaml),
        "markdown" => (MARKDOWN_SAMPLE, LanguageId::Markdown),
        _ => panic!("Unknown language"),
    };

    let highlights = state.parse_and_highlight(source, language, doc_id, 1);
    divan::black_box(highlights);
}

#[divan::bench(args = [100, 500, 1000, 5000])]
fn parse_large_rust(lines: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);

    let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);
    divan::black_box(highlights);
}

#[divan::bench(args = [100, 500, 1000, 5000])]
fn parse_large_javascript(lines: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_javascript(lines);

    let highlights = state.parse_and_highlight(&source, LanguageId::JavaScript, doc_id, 1);
    divan::black_box(highlights);
}

// ============================================================================
// Repeated parsing (simulating editing)
// ============================================================================

#[divan::bench(args = [10, 50, 100])]
fn parse_rust_repeated(iterations: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);

    for rev in 0..iterations {
        let highlights =
            state.parse_and_highlight(RUST_SAMPLE, LanguageId::Rust, doc_id, rev as u64);
        divan::black_box(&highlights);
    }
}

#[divan::bench(args = [10, 50, 100])]
fn parse_javascript_repeated(iterations: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);

    for rev in 0..iterations {
        let highlights = state.parse_and_highlight(
            JAVASCRIPT_SAMPLE,
            LanguageId::JavaScript,
            doc_id,
            rev as u64,
        );
        divan::black_box(&highlights);
    }
}

// ============================================================================
// Incremental edit simulation (for future comparison)
// ============================================================================

#[divan::bench(args = [10, 50, 100])]
fn parse_with_small_edit_rust(iterations: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);

    let mut source = RUST_SAMPLE.to_string();

    for rev in 0..iterations {
        // Simulate small edit: append a character
        source.push('x');
        let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, rev as u64);
        divan::black_box(&highlights);
    }
}

#[divan::bench(args = [10, 50, 100])]
fn parse_with_small_edit_large_rust(iterations: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);

    let mut source = generate_large_rust(1000);

    for rev in 0..iterations {
        // Simulate small edit in the middle
        let mid = source.len() / 2;
        source.insert(mid, 'x');
        let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, rev as u64);
        divan::black_box(&highlights);
    }
}

// ============================================================================
// Highlight extraction only (after parsing)
// ============================================================================

#[divan::bench(args = [100, 500, 1000])]
fn extract_highlights_lookup(lines: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);

    let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);

    // Simulate rendering: look up highlights for visible lines
    for line in 0..50.min(lines) {
        let tokens = highlights.get_line_tokens(line);
        divan::black_box(tokens);
    }
}

// ============================================================================
// Parser initialization
// ============================================================================

#[divan::bench]
fn parser_state_init() {
    let state = ParserState::new();
    divan::black_box(state);
}

// ============================================================================
// Parse-only benchmarks (pre-initialized parser)
// These isolate the actual parse+highlight time from init overhead
// ============================================================================

#[divan::bench(args = ["rust", "javascript", "html", "css", "yaml", "markdown"])]
fn parse_only_sample(bencher: divan::Bencher, lang: &str) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);

    let (source, language) = match lang {
        "rust" => (RUST_SAMPLE, LanguageId::Rust),
        "javascript" => (JAVASCRIPT_SAMPLE, LanguageId::JavaScript),
        "html" => (HTML_SAMPLE, LanguageId::Html),
        "css" => (CSS_SAMPLE, LanguageId::Css),
        "yaml" => (YAML_SAMPLE, LanguageId::Yaml),
        "markdown" => (MARKDOWN_SAMPLE, LanguageId::Markdown),
        _ => panic!("Unknown language"),
    };

    bencher.bench_local(|| {
        let highlights = state.parse_and_highlight(source, language, doc_id, 1);
        divan::black_box(highlights)
    });
}

#[divan::bench(args = [100, 500, 1000, 5000, 10000])]
fn parse_only_large_rust(bencher: divan::Bencher, lines: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);

    bencher.bench_local(|| {
        let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);
        divan::black_box(highlights)
    });
}

// ============================================================================
// Incremental parsing benchmarks (new!)
// These measure the speedup from incremental parsing vs full reparse
// ============================================================================

#[divan::bench(args = [100, 500, 1000, 5000])]
fn incremental_parse_small_edit(bencher: divan::Bencher, lines: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let mut source = generate_large_rust(lines);

    // Initial parse to populate cache
    state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 0);

    bencher.bench_local(|| {
        // Append a character (small edit at end)
        source.push('x');
        let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);
        divan::black_box(highlights)
    });
}

#[divan::bench(args = [100, 500, 1000, 5000])]
fn incremental_parse_middle_edit(bencher: divan::Bencher, lines: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);

    // Initial parse to populate cache
    state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 0);

    bencher.bench_local(|| {
        // Insert in the middle (worst case for incremental)
        let mut modified = source.clone();
        let mid = modified.len() / 2;
        modified.insert(mid, 'x');
        let highlights = state.parse_and_highlight(&modified, LanguageId::Rust, doc_id, 1);
        divan::black_box(highlights)
    });
}

#[divan::bench(args = [100, 500, 1000, 5000])]
fn full_reparse_comparison(bencher: divan::Bencher, lines: usize) {
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);

    bencher.bench_local(|| {
        // Create fresh parser state each time (forces full reparse)
        let mut state = ParserState::new();
        let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);
        divan::black_box(highlights)
    });
}

// ============================================================================
// Query matching (isolated)
// ============================================================================

#[divan::bench(args = ["rust", "javascript"])]
fn query_capture_iteration(lang: &str) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);

    let (source, language) = match lang {
        "rust" => (generate_large_rust(500), LanguageId::Rust),
        "javascript" => (generate_large_javascript(500), LanguageId::JavaScript),
        _ => panic!("Unknown language"),
    };

    // Parse and extract highlights (query matching is inside)
    let highlights = state.parse_and_highlight(&source, language, doc_id, 1);

    let total_tokens: usize = highlights.lines.values().map(|lh| lh.tokens.len()).sum();
    divan::black_box(total_tokens);
}

// ============================================================================
// Memory allocation patterns
// ============================================================================

#[divan::bench]
fn highlights_memory_rust_large() {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(5000);

    let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);

    // Count total allocations in highlights
    let line_count = highlights.lines.len();
    let token_count: usize = highlights.lines.values().map(|lh| lh.tokens.len()).sum();

    divan::black_box((line_count, token_count));
}

// ============================================================================
// Rope to String snapshot cost (bottleneck profiling)
// ============================================================================

#[divan::bench(args = [100, 1000, 5000, 10000])]
fn rope_to_string_snapshot(bencher: divan::Bencher, lines: usize) {
    let source = generate_large_rust(lines);
    let rope = ropey::Rope::from_str(&source);

    bencher.bench_local(|| {
        let snapshot = rope.to_string();
        divan::black_box(snapshot)
    });
}

// ============================================================================
// Highlight shift cost (new: measures shift_for_edit performance)
// ============================================================================

#[divan::bench(args = [100, 500, 1000, 5000])]
fn highlight_shift_for_insert(bencher: divan::Bencher, lines: usize) {
    use token::syntax::SyntaxHighlights;

    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);
    let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);

    bencher.bench_local(|| {
        let mut h = highlights.clone();
        // Simulate inserting a newline in the middle
        h.shift_for_edit(lines / 2, lines, lines + 1);
        divan::black_box(h)
    });
}

#[divan::bench(args = [100, 500, 1000, 5000])]
fn highlight_shift_for_delete(bencher: divan::Bencher, lines: usize) {
    use token::syntax::SyntaxHighlights;

    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);
    let highlights = state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 1);

    bencher.bench_local(|| {
        let mut h = highlights.clone();
        // Simulate deleting a line in the middle
        h.shift_for_edit(lines / 2, lines, lines - 1);
        divan::black_box(h)
    });
}

// ============================================================================
// End-to-end edit latency simulation
// ============================================================================

#[divan::bench(args = [100, 500, 1000, 5000])]
fn edit_to_highlight_latency(bencher: divan::Bencher, lines: usize) {
    let mut state = ParserState::new();
    let doc_id = DocumentId(1);
    let source = generate_large_rust(lines);

    // Initial parse
    state.parse_and_highlight(&source, LanguageId::Rust, doc_id, 0);

    bencher.bench_local(|| {
        // Simulate: edit (insert char in middle) → snapshot → incremental parse
        let mut modified = source.clone();
        let mid = modified.len() / 2;
        modified.insert(mid, 'x');
        let highlights = state.parse_and_highlight(&modified, LanguageId::Rust, doc_id, 1);
        divan::black_box(highlights)
    });
}
