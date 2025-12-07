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
	./target/release/token test_files/sample_code.rs

# Run debug build (faster compile, slower runtime)
dev: build
	./target/debug/token test_files/sample_code.rs

# Run all tests
test:
	cargo test


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
	cargo flamegraph --profile profiling -- ./target/profiling/token test_files/sample_code.rs

# Interactive profiling with samply (requires: cargo install samply)
profile-samply: build-prof
	samply record ./target/profiling/token test_files/large.txt

# Memory profiling with dhat (generates dhat-heap.json, auto-opens viewer)
profile-memory:
	cargo run --features dhat-heap --release -- test_files/large.txt
ifeq ($(shell uname -s),Darwin)
	@echo "Opening DHAT viewer and loading dhat-heap.json..."
	@osascript -e 'open location "https://nnethercote.github.io/dh_view/dh_view.html"' \
		-e 'delay 2' \
		-e 'tell application "System Events"' \
		-e '  keystroke tab' \
		-e '  keystroke return' \
		-e '  delay 1' \
		-e '  keystroke "g" using {command down, shift down}' \
		-e '  delay 0.5' \
		-e '  keystroke "$(PWD)/dhat-heap.json"' \
		-e '  delay 0.3' \
		-e '  keystroke return' \
		-e '  delay 0.3' \
		-e '  keystroke return' \
		-e 'end tell'
else
	@echo "Generated: dhat-heap.json"
	@echo "Open https://nnethercote.github.io/dh_view/dh_view.html"
	@echo "Click 'Load...' and select $(PWD)/dhat-heap.json"
endif

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
	# todo: if no file, show error message in red
	open target/llvm-cov/html/index.html

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
