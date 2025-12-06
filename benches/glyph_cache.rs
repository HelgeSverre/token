//! Benchmarks for glyph cache operations
//!
//! Run with: cargo bench glyph_cache

use std::collections::HashMap;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// HashMap lookup patterns
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
// Bitmap allocation patterns
// ============================================================================

#[divan::bench(args = [12, 16, 20, 24])]
fn glyph_bitmap_alloc(font_size: usize) {
    let bitmap_size = font_size * font_size;
    for _ in 0..1000 {
        let bitmap: Vec<u8> = vec![0u8; bitmap_size];
        divan::black_box(bitmap);
    }
}

#[divan::bench]
fn glyph_bitmap_reuse() {
    let mut bitmap: Vec<u8> = Vec::with_capacity(1024);
    for _ in 0..1000 {
        bitmap.clear();
        bitmap.resize(256, 0);
        divan::black_box(&bitmap);
    }
}
