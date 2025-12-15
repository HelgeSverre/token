//! Benchmarks for glyph cache operations
//!
//! Run with: cargo bench glyph_cache

use std::collections::HashMap;

use fontdue::{Font, FontSettings, Metrics};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

fn load_test_font() -> Font {
    Font::from_bytes(
        include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
        FontSettings::default(),
    )
    .expect("Failed to load font")
}

// ============================================================================
// Actual glyph rasterization (using fontdue)
// ============================================================================

#[divan::bench(args = [12.0, 16.0, 20.0, 24.0])]
fn glyph_rasterize(font_size: f32) {
    let font = load_test_font();
    for ch in "The quick brown fox".chars() {
        let (_metrics, bitmap) = font.rasterize(ch, font_size);
        divan::black_box(bitmap);
    }
}

#[divan::bench(args = [12.0, 16.0, 20.0, 24.0])]
fn glyph_rasterize_full_alphabet(font_size: f32) {
    let font = load_test_font();
    for ch in '!'..='~' {
        let (_metrics, bitmap) = font.rasterize(ch, font_size);
        divan::black_box(bitmap);
    }
}

// ============================================================================
// Cache hit vs miss patterns with actual rasterization
// ============================================================================

type GlyphCache = HashMap<(char, u32), (Metrics, Vec<u8>)>;

#[divan::bench]
fn glyph_cache_realistic_paragraph() {
    let font = load_test_font();
    let mut cache: GlyphCache = HashMap::new();
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(10);
    let font_size = 16.0_f32;

    for ch in text.chars() {
        let key = (ch, font_size.to_bits());
        cache
            .entry(key)
            .or_insert_with(|| font.rasterize(ch, font_size));
    }
    divan::black_box(&cache);
}

#[divan::bench]
fn glyph_cache_code_sample() {
    let font = load_test_font();
    let mut cache: GlyphCache = HashMap::new();
    let code = r#"
fn main() {
    let x = 42;
    println!("Hello, world! {}", x);
}
"#
    .repeat(20);
    let font_size = 14.0_f32;

    for ch in code.chars() {
        let key = (ch, font_size.to_bits());
        cache
            .entry(key)
            .or_insert_with(|| font.rasterize(ch, font_size));
    }
    divan::black_box(&cache);
}

#[divan::bench(args = [100, 500, 1000])]
fn glyph_cache_lookup_after_warmup(text_repeats: usize) {
    let font = load_test_font();
    let mut cache: GlyphCache = HashMap::new();
    let font_size = 16.0_f32;

    // Warmup: populate cache with ASCII printable characters
    for ch in ' '..='~' {
        let key = (ch, font_size.to_bits());
        cache.insert(key, font.rasterize(ch, font_size));
    }

    // Benchmark: lookup common text (should be 100% cache hits)
    let text = "The quick brown fox jumps over the lazy dog.\n".repeat(text_repeats);
    for ch in text.chars() {
        let key = (ch, font_size.to_bits());
        divan::black_box(cache.get(&key));
    }
}

// ============================================================================
// HashMap lookup patterns (cache-only, no rasterization)
// ============================================================================

#[divan::bench]
fn cache_lookup_contains_then_get() {
    let mut cache: HashMap<(char, u32), Vec<u8>> = HashMap::new();
    for ch in 'a'..='z' {
        cache.insert((ch, 16), vec![0u8; 256]);
    }

    for _ in 0..1000 {
        let key = ('m', 16);
        if cache.contains_key(&key) {
            divan::black_box(cache.get(&key));
        }
    }
}

#[divan::bench]
fn cache_lookup_get_only() {
    let mut cache: HashMap<(char, u32), Vec<u8>> = HashMap::new();
    for ch in 'a'..='z' {
        cache.insert((ch, 16), vec![0u8; 256]);
    }

    for _ in 0..1000 {
        let key = ('m', 16);
        divan::black_box(cache.get(&key));
    }
}

#[divan::bench]
fn cache_lookup_entry_api() {
    let mut cache: HashMap<(char, u32), Vec<u8>> = HashMap::new();
    for ch in 'a'..='z' {
        cache.insert((ch, 16), vec![0u8; 256]);
    }

    for _ in 0..1000 {
        let key = ('m', 16);
        divan::black_box(cache.entry(key).or_insert_with(|| vec![0u8; 256]));
    }
}

// ============================================================================
// Cache size impact
// ============================================================================

#[divan::bench(args = [100, 500, 1000, 5000])]
fn cache_lookup_varying_size(cache_size: usize) {
    let mut cache: HashMap<(char, u32), Vec<u8>> = HashMap::new();
    for i in 0..cache_size {
        let ch = char::from_u32((i % 65536) as u32).unwrap_or('?');
        cache.insert((ch, 16), vec![0u8; 256]);
    }

    for _ in 0..1000 {
        let key = ('a', 16);
        divan::black_box(cache.get(&key));
    }
}

// ============================================================================
// Key hashing performance
// ============================================================================

#[divan::bench]
fn key_creation_char_u32() {
    for _ in 0..10000 {
        let key: (char, u32) = ('A', 16);
        divan::black_box(key);
    }
}

#[divan::bench]
fn key_creation_with_style() {
    for _ in 0..10000 {
        let key: (char, u32, bool, bool) = ('A', 16, false, false);
        divan::black_box(key);
    }
}

// ============================================================================
// Font metrics extraction (common pattern in line measurement)
// ============================================================================

#[divan::bench]
fn font_metrics_extraction() {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let text = "The quick brown fox jumps over the lazy dog.";

    let mut total_width = 0.0_f32;
    for ch in text.chars() {
        let metrics = font.metrics(ch, font_size);
        total_width += metrics.advance_width;
    }
    divan::black_box(total_width);
}

#[divan::bench(args = [80, 120, 200])]
fn font_metrics_long_line(line_length: usize) {
    let font = load_test_font();
    let font_size = 16.0_f32;
    let text = "x".repeat(line_length);

    let mut total_width = 0.0_f32;
    for ch in text.chars() {
        let metrics = font.metrics(ch, font_size);
        total_width += metrics.advance_width;
    }
    divan::black_box(total_width);
}
