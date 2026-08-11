# token-editor task runner — `just --list` for grouped commands.
# External tools: cargo-nextest, cargo-llvm-cov, bacon, flamegraph, samply,
# cargo-bundle, cross, cargo-xwin, ImageMagick, and (for formatting) npx.

set shell := ["sh", "-cu"]

coverage_ignore := "--ignore-filename-regex '(runtime/|view/|debug_dump|debug_overlay|messages\\.rs)'"
syntax_samples := "samples/syntax/sample.rs samples/syntax/sample.js samples/syntax/sample.ts samples/syntax/sample.tsx samples/syntax/sample.html samples/syntax/sample.css samples/syntax/sample.json samples/syntax/sample.yaml samples/syntax/sample.toml samples/syntax/sample.md samples/syntax/sample.py samples/syntax/sample.go samples/syntax/sample.php samples/syntax/sample.c samples/syntax/sample.cpp samples/syntax/sample.java samples/syntax/sample.sh samples/syntax/sample.scm samples/syntax/sample.ini samples/syntax/sample.xml samples/syntax/sample.svelte"
install_dir := env_var('HOME') / ".local/bin"

# List all recipes.
default:
    @just --list

help:
    @just --list

[group('build')]
build:
    cargo build

[group('build')]
release:
    cargo build --release

[group('build')]
dist:
    cargo build --profile dist

[group('build')]
debugging:
    cargo build --profile debugging

[group('build')]
build-prof:
    cargo build --profile profiling

[group('build')]
clean:
    cargo clean
    rm -rf target/bundle Token.app
    rm -f assets/icon.icns assets/icon.ico

[group('run')]
run: release
    ./target/release/token samples/sample_code.rs README.md

[group('run')]
dev: build
    ./target/debug/token samples/sample_code.rs README.md keymap.yaml samples/sample.html

[group('run')]
workspace: build
    ./target/debug/token ./

[group('run')]
trace: build
    RUST_LOG=debug ./target/debug/token samples/sample_code.rs

[group('run')]
damage-debug:
    cargo run --release --bin token --features damage-debug -- ./src

# Open every supported syntax sample as a separate tab.
[group('run')]
test-syntax: release
    ./target/release/token {{ syntax_samples }}

[group('run')]
csv: build sample-csv
    ./target/debug/token samples/large_data.csv

[group('run')]
screenshots:
    cargo run --release --bin screenshot -- --all --out-dir website/public

# Run nextest and doctests; optionally filter nextest by expression/name.
[group('check')]
test filter="":
    cargo nextest run {{ filter }}
    cargo test --doc

[group('check')]
test-one name:
    cargo nextest run {{ name }}

[group('check')]
test-verbose filter="":
    cargo test {{ filter }} -- --nocapture

[group('check')]
test-retry:
    cargo nextest run --retries 2

[group('check')]
fmt:
    cargo fmt --all
    npx prettier --write "*.md"

[group('check')]
format: fmt

[group('check')]
fmt-check:
    cargo fmt --all -- --check
    npx prettier --check "*.md"

[group('check')]
lint:
    cargo clippy --all-targets --all-features -- -D warnings

[group('check')]
check: fmt-check lint test build release

[group('dev')]
watch:
    bacon

[group('dev')]
watch-lint:
    bacon clippy

[group('dev')]
setup:
    rustup component add llvm-tools-preview
    cargo install cargo-nextest --locked
    cargo install cargo-llvm-cov --locked
    cargo install bacon --locked
    cargo install flamegraph --locked
    cargo install samply --locked
    cargo install cargo-bundle --locked

[group('dev')]
setup-tools: setup

[group('dev')]
ci:
    act push --job build --matrix os:ubuntu-latest --matrix target:x86_64-unknown-linux-gnu --container-architecture linux/amd64

[group('profile')]
flamegraph: build-prof
    cargo flamegraph --profile profiling --bin token -- samples/sample_code.rs

[group('profile')]
profile-samply: build-prof sample-large
    samply record ./target/profiling/token samples/large.txt

[group('profile')]
profile-chrome:
    cargo build --release --features profile-chrome --bin token
    ./target/release/token ./
    @echo "Trace written to token-trace.json"
    @if [ "$(uname -s)" = Darwin ]; then open "https://ui.perfetto.dev"; else echo "Open https://ui.perfetto.dev"; fi

[group('profile')]
profile-memory: sample-large
    cargo run --features dhat-heap --release -- samples/large.txt
    @echo "Generated: dhat-heap.json"
    @echo "Open https://nnethercote.github.io/dh_view/dh_view.html to inspect it"

[group('bench')]
bench:
    cargo bench

[group('bench')]
bench-rope:
    cargo bench --bench rope_operations

[group('bench')]
bench-render:
    cargo bench --bench rendering

[group('bench')]
bench-glyph:
    cargo bench --bench glyph_cache

[group('bench')]
bench-loop:
    cargo bench --bench main_loop

[group('bench')]
bench-search:
    cargo bench --bench search

[group('bench')]
bench-layout:
    cargo bench --bench layout

[group('bench')]
bench-syntax:
    cargo bench --bench syntax

[group('bench')]
bench-multicursor:
    cargo bench --bench main_loop -- multi_cursor

[group('bench')]
bench-large:
    cargo bench -- large_file

[group('coverage')]
coverage:
    cargo llvm-cov --html {{ coverage_ignore }}
    @test -f target/llvm-cov/html/index.html || { echo "Coverage report was not generated" >&2; exit 1; }
    @if [ "$(uname -s)" = Darwin ]; then open target/llvm-cov/html/index.html; else echo "Report: target/llvm-cov/html/index.html"; fi

[group('coverage')]
coverage-html: coverage

[group('coverage')]
coverage-ci:
    cargo llvm-cov --codecov --output-path codecov.json {{ coverage_ignore }}

[group('samples')]
samples-files: sample-large sample-binary

[group('samples')]
sample-large:
    @mkdir -p samples
    @if [ ! -f samples/large.txt ]; then for i in $(seq 1 10000); do echo "Line $i: The quick brown fox jumps over the lazy dog. Lorem ipsum dolor sit amet."; done > samples/large.txt; fi

[group('samples')]
sample-binary:
    @mkdir -p samples
    @if [ ! -f samples/binary.bin ]; then head -c 1024 /dev/urandom > samples/binary.bin; fi

[group('samples')]
sample-csv:
    @mkdir -p samples
    @if [ ! -f samples/large_data.csv ]; then echo "id,first_name,last_name,email,company,department,job_title,salary,hire_date,country,city,phone,status,age,performance_score" > samples/large_data.csv; for i in $(seq 1 10000); do echo "$i,James,Smith,james.smith@example.com,Company$i,Engineering,Engineer,75000,2025-01-01,Norway,Oslo,+47-555-$i,active,35,4.5"; done >> samples/large_data.csv; fi

[group('install')]
install dest=install_dir: release
    mkdir -p {{ dest }}
    install -m 0755 target/release/token {{ dest }}/token
    @echo "Installed token to {{ dest }}/token"

[group('install')]
uninstall dest=install_dir:
    rm -f {{ dest }}/token
    @echo "Removed {{ dest }}/token"

[group('cross')]
compile-all: compile-macos-x86 compile-macos-arm compile-linux compile-windows
    @find target -path '*/dist/token*' -maxdepth 4 -print

[group('cross')]
compile-macos-x86:
    cargo build --profile dist --target x86_64-apple-darwin

[group('cross')]
compile-macos-arm:
    cargo build --profile dist --target aarch64-apple-darwin

[group('cross')]
compile-linux:
    cross build --profile dist --target x86_64-unknown-linux-gnu

[group('cross')]
compile-windows:
    cargo xwin build --profile dist --target x86_64-pc-windows-msvc

[group('package')]
[script('python3')]
generate-icon:
    from PIL import Image, ImageDraw, ImageFont
    image = Image.new("RGBA", (512, 512), (30, 30, 30, 255))
    draw = ImageDraw.Draw(image)
    font = ImageFont.truetype("assets/JetBrainsMono.ttf", 380)
    bounds = draw.textbbox((0, 0), "T", font=font)
    x = (512 - (bounds[2] - bounds[0])) // 2 - bounds[0]
    y = (512 - (bounds[3] - bounds[1])) // 2 - bounds[1]
    draw.text((x, y), "T", font=font, fill=(100, 180, 255, 255))
    image.save("assets/icon.png")

[private]
_ensure-icon:
    @test -f assets/icon.png || just generate-icon

[group('package')]
icons: _ensure-icon
    rm -rf assets/icon.iconset
    mkdir -p assets/icon.iconset
    sips -z 16 16 assets/icon.png --out assets/icon.iconset/icon_16x16.png >/dev/null
    sips -z 32 32 assets/icon.png --out assets/icon.iconset/icon_16x16@2x.png >/dev/null
    sips -z 32 32 assets/icon.png --out assets/icon.iconset/icon_32x32.png >/dev/null
    sips -z 64 64 assets/icon.png --out assets/icon.iconset/icon_32x32@2x.png >/dev/null
    sips -z 128 128 assets/icon.png --out assets/icon.iconset/icon_128x128.png >/dev/null
    sips -z 256 256 assets/icon.png --out assets/icon.iconset/icon_128x128@2x.png >/dev/null
    sips -z 256 256 assets/icon.png --out assets/icon.iconset/icon_256x256.png >/dev/null
    sips -z 512 512 assets/icon.png --out assets/icon.iconset/icon_256x256@2x.png >/dev/null
    sips -z 512 512 assets/icon.png --out assets/icon.iconset/icon_512x512.png >/dev/null
    sips -z 1024 1024 assets/icon.png --out assets/icon.iconset/icon_512x512@2x.png >/dev/null
    iconutil -c icns assets/icon.iconset -o assets/icon.icns
    rm -rf assets/icon.iconset
    @if command -v magick >/dev/null 2>&1; then magick assets/icon.png -define icon:auto-resize=256,128,64,48,32,16 assets/icon.ico; elif command -v convert >/dev/null 2>&1; then convert assets/icon.png -define icon:auto-resize=256,128,64,48,32,16 assets/icon.ico; else echo "ImageMagick unavailable; skipping Windows icon"; fi

[group('package')]
bundle: dist icons
    cargo bundle --release --bin token

[group('package')]
bundle-all: bundle-macos bundle-linux bundle-windows

[group('package')]
bundle-macos: dist icons
    cargo bundle --release --bin token --format osx

[group('package')]
bundle-macos-target target:
    sh scripts/package-macos-app.sh {{ target }}

[group('run')]
app:
    #!/bin/sh
    set -eu
    target="$(rustc -vV | sed -n 's/^host: //p')"
    case "$target" in
        aarch64-apple-darwin|x86_64-apple-darwin) ;;
        *) echo "just app is only available on macOS" >&2; exit 2 ;;
    esac
    sh scripts/package-macos-app.sh "$target"
    open "target/$target/release/bundle/osx/Token.app"

[group('package')]
bundle-linux: dist icons
    cargo bundle --release --bin token --format deb

[group('package')]
bundle-windows: dist icons
    mkdir -p target/bundle/windows
    cp target/dist/token target/bundle/windows/token.exe 2>/dev/null || cp target/release/token target/bundle/windows/token.exe
    cp assets/icon.ico target/bundle/windows/ 2>/dev/null || true
    cp README.md LICENSE.md target/bundle/windows/
    cd target/bundle && zip -r Token-windows.zip windows/
