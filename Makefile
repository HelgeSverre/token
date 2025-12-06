# Makefile for token

.PHONY: build release run dev test clean fmt format lint help sample-files ci \
        build-prof flamegraph profile-samply profile-memory \
        bench bench-rope bench-render bench-glyph \
        coverage coverage-html coverage-ci \
        watch watch-lint test-fast test-retry \
        setup setup-tools

# Default target
all: help

# Build in debug mode
build:
	cargo build

# Build optimized release binary
release:
	cargo build --release

# Run release build with default sample file
run: release
	./target/release/token test_files/indentation.txt

# Run debug build (faster compile, slower runtime)
dev: build
	./target/debug/token test_files/sample_code.rs

# Run all tests
test:
	cargo test

# Run specific test (usage: make test-one TEST=test_smart_home)
test-one:
	cargo test $(TEST)

# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Clean build artifacts
clean:
	cargo clean

# Format all code and markdown files
fmt format:
	cargo fmt
	npx prettier --write "*.md"

# Run clippy lints (mirrors CI)
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# === Sample file runners ===

# Test indentation/whitespace navigation (smart home/end)
run-indent: release
	./target/release/token test_files/indentation.txt

# Test with large file
run-large: release
	./target/release/token test_files/large.txt

# Test with mixed whitespace (tabs and spaces)
run-mixed: release
	./target/release/token test_files/mixed_whitespace.txt

# Test with trailing whitespace
run-trailing: release
	./target/release/token test_files/trailing_whitespace.txt

# Test with long lines (horizontal scrolling)
run-long: release
	./target/release/token test_files/long_lines.txt

# Test with binary file (edge case)
run-binary: release
	./target/release/token test_files/binary.bin

# Test with unicode/emoji content
run-unicode: release
	./target/release/token test_files/unicode.txt

# Test with mixed languages, emojis, accents, and box-drawing art
run-emoji: release
	./target/release/token test_files/emoji_unicode.txt

# Test with progressive Zalgo text corruption
run-zalgo: release
	./target/release/token test_files/zalgo.txt

# Test with empty file
run-empty: release
	./target/release/token test_files/empty.txt

# Test with single line no newline
run-single: release
	./target/release/token test_files/single_line.txt

# Test source code (realistic use case)
run-code: release
	./target/release/token test_files/sample_code.rs

# === Generate sample files ===

sample-files: test_files/large.txt test_files/binary.bin

test_files/large.txt:
	@mkdir -p test_files
	@echo "Generating large test file (10000 lines)..."
	@for i in $$(seq 1 10000); do \
		echo "Line $$i: The quick brown fox jumps over the lazy dog. Lorem ipsum dolor sit amet."; \
	done > test_files/large.txt

test_files/binary.bin:
	@mkdir -p test_files
	@echo "Generating binary test file..."
	@head -c 1024 /dev/urandom > test_files/binary.bin

# Help
help:
	@echo "token Makefile"
	@echo ""
	@echo "Build targets:"
	@echo "  make build        - Build debug binary"
	@echo "  make release      - Build optimized release binary"
	@echo "  make build-prof   - Build with debug symbols for profiling"
	@echo "  make clean        - Remove build artifacts"
	@echo "  make fmt          - Format Rust code and markdown files"
	@echo "  make lint         - Run clippy lints (mirrors CI)"
	@echo ""
	@echo "Run targets:"
	@echo "  make run          - Run with default sample file (indentation.txt)"
	@echo "  make dev          - Run debug build (faster compile)"
	@echo "  make run-indent   - Test smart home/end with indented code"
	@echo "  make run-large    - Test with large file (10k lines)"
	@echo "  make run-mixed    - Test mixed tabs/spaces"
	@echo "  make run-trailing - Test trailing whitespace"
	@echo "  make run-long     - Test long lines (horizontal scroll)"
	@echo "  make run-binary   - Test binary file handling"
	@echo "  make run-unicode  - Test unicode/emoji content"
	@echo "  make run-emoji    - Test mixed languages, emojis, accents, box art"
	@echo "  make run-zalgo    - Test progressive Zalgo text corruption"
	@echo "  make run-empty    - Test empty file"
	@echo "  make run-single   - Test single line file"
	@echo "  make run-code     - Test with Rust source code"
	@echo ""
	@echo "Test targets:"
	@echo "  make test         - Run all tests"
	@echo "  make test-one TEST=name - Run specific test"
	@echo "  make test-verbose - Run tests with output"
	@echo "  make test-fast    - Fast parallel tests (nextest)"
	@echo "  make test-retry   - Tests with retries for flaky tests"
	@echo ""
	@echo "Benchmarking:"
	@echo "  make bench        - Run all benchmarks"
	@echo "  make bench-rope   - Rope operation benchmarks"
	@echo "  make bench-render - Rendering benchmarks"
	@echo "  make bench-glyph  - Glyph cache benchmarks"
	@echo ""
	@echo "Profiling:"
	@echo "  make flamegraph     - Generate CPU flamegraph"
	@echo "  make profile-samply - Interactive profiling (Firefox Profiler)"
	@echo "  make profile-memory - Heap profiling with dhat"
	@echo ""
	@echo "Coverage:"
	@echo "  make coverage     - Generate HTML coverage report"
	@echo "  make coverage-ci  - Generate codecov.json for CI"
	@echo ""
	@echo "Development:"
	@echo "  make watch        - Start bacon watch mode"
	@echo "  make watch-lint   - Watch with clippy"
	@echo ""
	@echo "Setup:"
	@echo "  make setup        - Install all dev tools (flamegraph, bacon, etc.)"
	@echo "  make sample-files - Generate large/binary test files"
	@echo ""
	@echo "CI targets:"
	@echo "  make ci           - Test GitHub Actions locally with act"

# === Setup targets ===

# Install all development tools
setup setup-tools:
	@echo "Installing development tools..."
	@echo ""
	@echo "==> Installing Rust components..."
	rustup component add llvm-tools-preview
	@echo ""
	@echo "==> Installing cargo tools..."
	cargo install cargo-nextest --locked
	cargo install cargo-llvm-cov --locked
	cargo install bacon --locked
	cargo install flamegraph --locked
	cargo install samply --locked
	@echo ""
	@echo "==> Setup complete!"
	@echo ""
	@echo "Available commands:"
	@echo "  make watch        - Start bacon watch mode"
	@echo "  make test-fast    - Run tests with nextest"
	@echo "  make coverage     - Generate coverage report"
	@echo "  make flamegraph   - Generate CPU flamegraph"
	@echo "  make bench        - Run benchmarks"

# === Profiling targets ===

# Build with debug symbols for profiling
build-prof:
	cargo build --profile profiling

# CPU flamegraph (requires: cargo install flamegraph)
flamegraph: build-prof
	cargo flamegraph --profile profiling -- ./target/profiling/token test_files/large.txt

# Interactive profiling with samply (requires: cargo install samply)
profile-samply: build-prof
	samply record ./target/profiling/token test_files/large.txt

# Memory profiling with dhat (generates dhat-heap.json)
profile-memory:
	cargo run --features dhat-heap --release -- test_files/large.txt
	@echo "Open dhat-heap.json with https://nnethercote.github.io/dh_view/dh_view.html"

# === Benchmarking targets ===

# Run all benchmarks
bench:
	cargo bench

# Run rope operation benchmarks
bench-rope:
	cargo bench --bench rope_operations

# Run rendering benchmarks
bench-render:
	cargo bench --bench rendering

# Run glyph cache benchmarks
bench-glyph:
	cargo bench --bench glyph_cache

# === Coverage targets (requires: cargo install cargo-llvm-cov) ===

# Generate HTML coverage report
coverage coverage-html:
	cargo llvm-cov --html
	@echo "Open target/llvm-cov/html/index.html"

# Generate coverage for CI (codecov format)
coverage-ci:
	cargo llvm-cov --codecov --output-path codecov.json

# === Development workflow targets ===

# Start bacon watch mode (requires: cargo install bacon)
watch:
	bacon

# Watch with clippy
watch-lint:
	bacon clippy

# Fast parallel tests (requires: cargo install cargo-nextest)
test-fast:
	cargo nextest run

# Tests with retries for flaky tests
test-retry:
	cargo nextest run --retries 2

# === CI targets ===

# Test GitHub Actions locally with act
ci:
	act push --job build --matrix os:ubuntu-latest --matrix target:x86_64-unknown-linux-gnu --container-architecture linux/amd64
