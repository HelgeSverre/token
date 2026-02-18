# Makefile for token

.PHONY: build release dist debugging run dev csv damage-debug test-syntax trace test clean fmt format lint help samples-files ci screenshots \
        build-prof flamegraph profile-samply profile-memory \
        bench bench-rope bench-render bench-glyph \
        coverage coverage-html coverage-ci \
        watch watch-lint test-fast test-retry \
        setup setup-tools install uninstall \
        compile-all compile-macos-x86 compile-macos-arm compile-linux compile-windows \
        generate-icon icons bundle-macos bundle-linux bundle-windows bundle bundle-all

# Default target
all: help

# Build in debug mode
build:
	cargo build

# Build optimized release binary (fast compile, for local testing)
release:
	cargo build --release

# Build distribution binary (maximum optimization, slower compile)
dist:
	cargo build --profile dist

# Build with full debug info (for detailed debugging)
debugging:
	cargo build --profile debugging

# Install to ~/.local/bin (add to PATH if needed)
INSTALL_DIR := $(HOME)/.local/bin

install: release
	@mkdir -p $(INSTALL_DIR)
	@cp target/release/token $(INSTALL_DIR)/token
	@chmod +x $(INSTALL_DIR)/token
	@echo "Installed token to $(INSTALL_DIR)/token"
	@if ! echo "$$PATH" | grep -q "$(INSTALL_DIR)"; then \
		echo ""; \
		echo "NOTE: Add $(INSTALL_DIR) to your PATH:"; \
		echo "  echo 'export PATH=\"$(INSTALL_DIR):\$$PATH\"' >> ~/.zshrc"; \
		echo "  source ~/.zshrc"; \
	fi

# Uninstall from ~/.local/bin
uninstall:
	@rm -f $(INSTALL_DIR)/token
	@echo "Removed token from $(INSTALL_DIR)"

# Run release build with default samples file
run: release
	./target/release/token samples/sample_code.rs README.md

# Run debug build (faster compile, slower runtime)
dev: build
	./target/debug/token samples/sample_code.rs README.md keymap.yaml samples/sample.html

# Test syntax highlighting with all 20 supported language samples
test-syntax: release
	./target/release/token \
		samples/syntax/sample.rs \
		samples/syntax/sample.js \
		samples/syntax/sample.ts \
		samples/syntax/sample.tsx \
		samples/syntax/sample.html \
		samples/syntax/sample.css \
		samples/syntax/sample.json \
		samples/syntax/sample.yaml \
		samples/syntax/sample.toml \
		samples/syntax/sample.md \
		samples/syntax/sample.py \
		samples/syntax/sample.go \
		samples/syntax/sample.php \
		samples/syntax/sample.c \
		samples/syntax/sample.cpp \
		samples/syntax/sample.java \
		samples/syntax/sample.sh \
		samples/syntax/sample.scm \
		samples/syntax/sample.ini \
		samples/syntax/sample.xml

# Run with CSV sample file for testing CSV viewer
csv: build samples/large_data.csv
	./target/debug/token samples/large_data.csv

# Run with codebase as workspace
workspace: build
	./target/debug/token ./

# Run with damage tracking debug visualization
damage-debug:
	cargo run --release --bin token --features damage-debug -- /Users/helge/conductor/workspaces/token-editor/abu-dhabi/src

# Run with full debug tracing enabled
trace: build
	RUST_LOG=debug ./target/debug/token samples/sample_code.rs

# Run all tests
test:
	cargo test


# Run tests with output
test-verbose:
	cargo test -- --nocapture

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/bundle
	rm -rf Token.app
	rm -f assets/icon.icns assets/icon.ico

# Format all code and markdown files
fmt format:
	cargo fmt
	npx prettier --write "*.md"

# Run clippy lints (mirrors CI)
lint:
	cargo clippy --all-targets --all-features -- -D warnings


samples-files: samples/large.txt samples/binary.bin

samples/large.txt:
	@mkdir -p samples
	@echo "Generating large test file (10000 lines)..."
	@for i in $$(seq 1 10000); do \
		echo "Line $$i: The quick brown fox jumps over the lazy dog. Lorem ipsum dolor sit amet."; \
	done > samples/large.txt

samples/binary.bin:
	@mkdir -p samples
	@echo "Generating binary test file..."
	@head -c 1024 /dev/urandom > samples/binary.bin

samples/large_data.csv:
	@mkdir -p samples
	@echo "Generating large CSV file (10000 rows)..."
	@echo "id,first_name,last_name,email,company,department,job_title,salary,hire_date,country,city,phone,status,age,performance_score" > samples/large_data.csv
	@for i in $$(seq 1 10000); do \
		fn=$$(echo "James Maria Wei Sarah Mohammed Emma Raj Sophie Hans Yuki Carlos Anna John Fatima Lucas Olivia Kim Elena Ahmed Lisa" | tr ' ' '\n' | shuf -n1); \
		ln=$$(echo "Smith Garcia Chen Johnson Hassan Williams Patel Dubois Mueller Tanaka Rodriguez Kowalski Brown Khan Silva Taylor Park Volkov Ibrahim Anderson" | tr ' ' '\n' | shuf -n1); \
		dept=$$(echo "Engineering Marketing Finance Product HR Design Research Sales Support Operations Legal" | tr ' ' '\n' | shuf -n1); \
		country=$$(echo "USA UK Germany France Japan China India Brazil Canada Australia Spain Italy" | tr ' ' '\n' | shuf -n1); \
		city=$$(echo "NYC London Berlin Paris Tokyo Shanghai Mumbai SP Toronto Sydney Madrid Rome" | tr ' ' '\n' | shuf -n1); \
		status=$$(echo "active active active active inactive" | tr ' ' '\n' | shuf -n1); \
		salary=$$((50000 + RANDOM % 150000)); \
		age=$$((22 + RANDOM % 40)); \
		score=$$(echo "3.5 3.7 3.9 4.0 4.2 4.4 4.5 4.7 4.9" | tr ' ' '\n' | shuf -n1); \
		year=$$((2015 + RANDOM % 10)); \
		month=$$(printf "%02d" $$((1 + RANDOM % 12))); \
		day=$$(printf "%02d" $$((1 + RANDOM % 28))); \
		echo "$$i,$$fn,$$ln,$$fn.$$ln@company.com,Company$$i,$$dept,$$dept Manager,$$salary,$$year-$$month-$$day,$$country,$$city,+1-555-$$((1000 + i)),$$status,$$age,$$score"; \
	done >> samples/large_data.csv

# Generate screenshots from scenario YAML files
screenshots:
	cargo run --release --bin screenshot -- --all --out-dir website/v4/public

# Help
help:
	@echo "token Makefile"
	@echo ""
	@echo "Build targets:"
	@echo "  make build        - Build debug binary (fast compile)"
	@echo "  make debugging    - Build with full debug info (for debuggers)"
	@echo "  make release      - Build release binary (fast compile, local testing)"
	@echo "  make dist         - Build distribution binary (max optimization)"
	@echo "  make install      - Install to ~/.local/bin"
	@echo "  make uninstall    - Remove from ~/.local/bin"
	@echo "  make build-prof   - Build with debug symbols for profiling"
	@echo "  make clean        - Remove build artifacts"
	@echo "  make fmt          - Format Rust code and markdown files"
	@echo "  make lint         - Run clippy lints (mirrors CI)"
	@echo ""
	@echo "Run targets:"
	@echo "  make run          - Run with default samples file (indentation.txt)"
	@echo "  make dev          - Run debug build (faster compile)"
	@echo "  make csv          - Run with large CSV file (tests CSV viewer)"
	@echo "  make workspace    - Open codebase folder as workspace (tests sidebar)"
	@echo "  make damage-debug - Run with damage tracking visualization (colored outlines)"
	@echo "  make test-syntax  - Open all 20 syntax sample files for manual testing"
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
	@echo "  make bench-loop   - Main loop benchmarks"
	@echo "  make bench-search - Search operation benchmarks"
	@echo "  make bench-layout - Text layout benchmarks"
	@echo "  make bench-syntax - Syntax highlighting benchmarks"
	@echo "  make bench-multicursor - Multi-cursor benchmarks"
	@echo "  make bench-large  - Large file (500k+) benchmarks"
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
	@echo "  make samples-files - Generate large/binary test files"
	@echo ""
	@echo "CI targets:"
	@echo "  make ci           - Test GitHub Actions locally with act"
	@echo ""
	@echo "Cross-compilation (uses dist profile):"
	@echo "  make compile-all      - Build for all platforms"
	@echo "  make compile-macos-x86 - macOS Intel"
	@echo "  make compile-macos-arm - macOS Apple Silicon"
	@echo "  make compile-linux    - Linux x86_64 (requires cross + Docker)"
	@echo "  make compile-windows  - Windows x86_64 (requires cargo-xwin)"
	@echo ""
	@echo "App bundling:"
	@echo "  make generate-icon    - Generate placeholder icon PNG"
	@echo "  make icons            - Generate .icns and .ico from PNG"
	@echo "  make bundle           - Create app bundle for current platform"
	@echo "  make bundle-macos     - Create macOS .dmg package"
	@echo "  make bundle-linux     - Create Linux .deb package"
	@echo "  make bundle-windows   - Create Windows .zip distribution"
	@echo "  make bundle-all       - Create bundles for all platforms"

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
	cargo install cargo-bundle --locked
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
	cargo flamegraph --profile profiling -- ./target/profiling/token samples/sample_code.rs

# Interactive profiling with samply (requires: cargo install samply)
profile-samply: build-prof
	samply record ./target/profiling/token samples/large.txt

# Memory profiling with dhat (generates dhat-heap.json, auto-opens viewer)
profile-memory:
	cargo run --features dhat-heap --release -- samples/large.txt
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

# Run rope operation benchmarks (includes large file scaling)
bench-rope:
	cargo bench --bench rope_operations

# Run rendering benchmarks
bench-render:
	cargo bench --bench rendering

# Run glyph cache benchmarks (actual fontdue rasterization)
bench-glyph:
	cargo bench --bench glyph_cache

# Run main loop benchmarks (update cycle, multi-cursor)
bench-loop:
	cargo bench --bench main_loop

# Run search benchmarks (find/replace operations)
bench-search:
	cargo bench --bench search

# Run text layout benchmarks (line measurement, viewport)
bench-layout:
	cargo bench --bench layout

# Run syntax highlighting benchmarks
bench-syntax:
	cargo bench --bench syntax

# Run multi-cursor specific benchmarks
bench-multicursor:
	cargo bench --bench main_loop -- multi_cursor

# Run large file benchmarks (500k+ lines)
bench-large:
	cargo bench -- large_file

# === Coverage targets (requires: cargo install cargo-llvm-cov) ===

# Files to exclude from coverage (runtime/rendering code, data-only modules)
COVERAGE_IGNORE := --ignore-filename-regex '(runtime/|view/|debug_dump|debug_overlay|messages\.rs)'

# Generate HTML coverage report
coverage coverage-html:
	cargo llvm-cov --html $(COVERAGE_IGNORE)
	@if [ -f target/llvm-cov/html/index.html ]; then \
		open target/llvm-cov/html/index.html; \
	else \
		echo "\033[0;31mError: Coverage report not found at target/llvm-cov/html/index.html\033[0m"; \
		exit 1; \
	fi

# Generate coverage for CI (codecov format)
coverage-ci:
	cargo llvm-cov --codecov --output-path codecov.json $(COVERAGE_IGNORE)

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

# === Cross-compilation targets (uses dist profile for max optimization) ===

# Build for all platforms
compile-all: compile-macos-x86 compile-macos-arm compile-linux compile-windows
	@echo "All cross-compilation builds complete!"
	@ls -la target/*/dist/token* 2>/dev/null || true

# macOS x86_64 (Intel)
compile-macos-x86:
	cargo build --profile dist --target x86_64-apple-darwin

# macOS aarch64 (Apple Silicon)
compile-macos-arm:
	cargo build --profile dist --target aarch64-apple-darwin

# Linux x86_64 (requires: cargo install cross, Docker running)
compile-linux:
	cross build --profile dist --target x86_64-unknown-linux-gnu

# Windows x86_64 (requires: cargo install cargo-xwin)
compile-windows:
	cargo xwin build --profile dist --target x86_64-pc-windows-msvc

# === App bundling targets ===

# Generate placeholder icon PNG (requires Python with PIL)
generate-icon:
	@python3 -c "\
from PIL import Image, ImageDraw, ImageFont; \
img = Image.new('RGBA', (512, 512), (30, 30, 30, 255)); \
draw = ImageDraw.Draw(img); \
font = ImageFont.truetype('assets/JetBrainsMono.ttf', 380); \
bbox = draw.textbbox((0, 0), 'T', font=font); \
x = (512 - (bbox[2] - bbox[0])) // 2 - bbox[0]; \
y = (512 - (bbox[3] - bbox[1])) // 2 - bbox[1]; \
draw.text((x, y), 'T', font=font, fill=(100, 180, 255, 255)); \
img.save('assets/icon.png'); \
print('Created assets/icon.png')"

# Generate platform-specific icons from PNG
icons: assets/icon.png
	@echo "Generating macOS .icns..."
	@mkdir -p assets/icon.iconset
	@sips -z 16 16     assets/icon.png --out assets/icon.iconset/icon_16x16.png >/dev/null
	@sips -z 32 32     assets/icon.png --out assets/icon.iconset/icon_16x16@2x.png >/dev/null
	@sips -z 32 32     assets/icon.png --out assets/icon.iconset/icon_32x32.png >/dev/null
	@sips -z 64 64     assets/icon.png --out assets/icon.iconset/icon_32x32@2x.png >/dev/null
	@sips -z 128 128   assets/icon.png --out assets/icon.iconset/icon_128x128.png >/dev/null
	@sips -z 256 256   assets/icon.png --out assets/icon.iconset/icon_128x128@2x.png >/dev/null
	@sips -z 256 256   assets/icon.png --out assets/icon.iconset/icon_256x256.png >/dev/null
	@sips -z 512 512   assets/icon.png --out assets/icon.iconset/icon_256x256@2x.png >/dev/null
	@sips -z 512 512   assets/icon.png --out assets/icon.iconset/icon_512x512.png >/dev/null
	@sips -z 1024 1024 assets/icon.png --out assets/icon.iconset/icon_512x512@2x.png >/dev/null
	@iconutil -c icns assets/icon.iconset -o assets/icon.icns
	@rm -rf assets/icon.iconset
	@echo "Created assets/icon.icns"
	@echo "Generating Windows .ico..."
	@if command -v magick >/dev/null 2>&1; then \
		magick assets/icon.png -define icon:auto-resize=256,128,64,48,32,16 assets/icon.ico; \
	elif command -v convert >/dev/null 2>&1; then \
		convert assets/icon.png -define icon:auto-resize=256,128,64,48,32,16 assets/icon.ico; \
	else \
		echo "Warning: ImageMagick not found, skipping .ico generation"; \
		echo "Install with: brew install imagemagick"; \
	fi
	@test -f assets/icon.ico && echo "Created assets/icon.ico" || true

# Bundle for current platform (requires: cargo install cargo-bundle)
bundle: dist icons
	cargo bundle --release --bin token

# Bundle for all platforms (requires cross-compilation toolchains)
bundle-all: bundle-macos bundle-linux bundle-windows
	@echo "All platform bundles created!"
	@ls -la target/bundle/*/

# Create macOS .app bundle / .dmg (macOS only)
bundle-macos: dist icons
	cargo bundle --release --bin token --format osx

# Create Linux .deb package (Linux only)
bundle-linux: dist icons
	cargo bundle --release --bin token --format deb

# Create Windows .zip with executable
bundle-windows: dist icons
	@echo "Creating Windows distribution zip..."
	@mkdir -p target/bundle/windows
	@cp target/dist/token target/bundle/windows/token.exe 2>/dev/null || cp target/release/token target/bundle/windows/token.exe 2>/dev/null || echo "Note: Build for Windows target to get .exe"
	@cp assets/icon.ico target/bundle/windows/ 2>/dev/null || true
	@cp README.md target/bundle/windows/ 2>/dev/null || true
	@cp LICENSE.md target/bundle/windows/ 2>/dev/null || true
	@cd target/bundle && zip -r Token-windows.zip windows/
	@echo "Created target/bundle/Token-windows.zip"
