# Screenshot Pipeline & Homepage Improvements

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a headless screenshot binary that renders Token in configurable scenarios to PNG, then use real screenshots to improve the homepage.

**Architecture:** Expose the real `Renderer` static methods (`render_editor_area`, `render_sidebar`, `render_status_bar`) as `pub` so a headless binary can call the exact same rendering code path as the GUI — no divergence. The binary loads YAML scenario files defining files, splits, theme, cursor positions, and window size, renders into a `Vec<u32>` buffer via `Frame` + `TextPainter`, and writes PNG via the `image` crate. A `make screenshots` target generates all website assets.

**Tech Stack:** Rust, fontdue, image crate (already in Cargo.toml), serde_yaml, clap, Frame/TextPainter from `src/view/frame.rs`

---

## Phase 1: Headless Rendering API

### Task 1: Make Renderer static methods public

The key rendering functions on `Renderer` are currently private. They are all static methods that take `Frame` + `TextPainter` — they don't use `self` — so they can be called without a window/surface.

**Files:**
- Modify: `src/view/mod.rs`

**Step 1: Change visibility of render methods**

Make these methods `pub` (they are currently `fn`):

```rust
// Line ~506 - change `fn render_editor_area` to:
pub fn render_editor_area(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    splitters: &[SplitterBar],
)

// Line ~861 - change `fn render_sidebar` to:
pub fn render_sidebar(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    sidebar_width: usize,
    sidebar_height: usize,
)

// Line ~1790 - change `fn render_status_bar` to:
pub fn render_status_bar(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    width: usize,
    height: usize,
)

// Line ~733 - change `fn render_editor_group` to:
pub fn render_editor_group(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    group_id: GroupId,
    rect: Rect,
    is_focused: bool,
)

// Line ~520 (approx) - change `fn render_splitters` to pub:
pub fn render_splitters(frame: &mut Frame, splitters: &[SplitterBar], model: &AppModel)
```

**Step 2: Run build to verify no breakage**

Run: `make build`
Expected: Compiles without errors. Making private methods pub is always safe.

**Step 3: Commit**

```bash
git add src/view/mod.rs
git commit -m "refactor: make Renderer static render methods public for headless use"
```

---

### Task 2: Create the screenshot binary

**Files:**
- Create: `src/bin/screenshot.rs`

**Step 1: Create the binary with CLI args**

```rust
//! Headless screenshot generator for Token editor
//!
//! Renders the editor in configurable scenarios and outputs PNG files.
//! Uses the exact same rendering code path as the GUI.
//!
//! Usage:
//!   cargo run --bin screenshot -- --scenario screenshots/scenarios/hero.yaml
//!   cargo run --bin screenshot -- --all --out-dir website/v4/public

use std::path::PathBuf;
use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "screenshot", about = "Generate screenshots of Token editor")]
struct Args {
    /// Path to a single scenario YAML file
    #[arg(long)]
    scenario: Option<PathBuf>,

    /// Run all scenarios in the scenarios directory
    #[arg(long)]
    all: bool,

    /// Directory containing scenario YAML files
    #[arg(long, default_value = "screenshots/scenarios")]
    scenarios_dir: PathBuf,

    /// Output directory for PNG files
    #[arg(long, default_value = "screenshots/output")]
    out_dir: PathBuf,

    /// Override theme (path to theme YAML)
    #[arg(long)]
    theme: Option<PathBuf>,

    /// Override width
    #[arg(long)]
    width: Option<u32>,

    /// Override height
    #[arg(long)]
    height: Option<u32>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    // ... implementation in later steps
}
```

**Step 2: Run build to verify it compiles**

Run: `cargo build --bin screenshot`
Expected: Compiles (empty main is fine)

**Step 3: Commit**

```bash
git add src/bin/screenshot.rs
git commit -m "feat: add screenshot binary skeleton with CLI args"
```

---

### Task 3: Define the scenario YAML schema

**Files:**
- Create: `src/bin/screenshot/scenario.rs` (or inline in screenshot.rs — keep it simple, inline first)

**Step 1: Add scenario struct definitions to screenshot.rs**

```rust
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct Scenario {
    name: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default)]
    theme: Option<String>,      // theme id like "fleet-dark" or path
    files: Vec<ScenarioFile>,
    #[serde(default)]
    split_direction: SplitDir,
}

#[derive(Deserialize, Debug)]
struct ScenarioFile {
    path: PathBuf,
    #[serde(default)]
    scroll_to: Option<usize>,       // top_line
    #[serde(default)]
    cursor_line: Option<usize>,
    #[serde(default)]
    cursor_column: Option<usize>,
    #[serde(default)]
    extra_cursors: Vec<CursorPos>,
}

#[derive(Deserialize, Debug)]
struct CursorPos {
    line: usize,
    column: usize,
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
enum SplitDir {
    #[default]
    Horizontal,
    Vertical,
}

fn default_width() -> u32 { 2880 }
fn default_height() -> u32 { 1800 }
fn default_scale() -> f64 { 2.0 }
```

**Step 2: Add scenario loading function**

```rust
fn load_scenario(path: &Path) -> Result<Scenario> {
    let content = std::fs::read_to_string(path)?;
    let scenario: Scenario = serde_yaml::from_str(&content)?;
    Ok(scenario)
}
```

**Step 3: Run build**

Run: `cargo build --bin screenshot`
Expected: Compiles

**Step 4: Commit**

```bash
git add src/bin/screenshot.rs
git commit -m "feat: add scenario YAML schema for screenshot generation"
```

---

### Task 4: Implement the headless render + PNG output

This is the core. Build AppModel from scenario, render using the real Renderer static methods, write PNG.

**Files:**
- Modify: `src/bin/screenshot.rs`

**Step 1: Implement model creation from scenario**

Use the same pattern as `profile_render.rs:create_model()`:

```rust
fn create_model_from_scenario(scenario: &Scenario) -> Result<token::model::AppModel> {
    use token::config::EditorConfig;
    use token::messages::{LayoutMsg, Msg};
    use token::model::document::Document;
    use token::model::editor::EditorState;
    use token::model::editor_area::EditorArea;
    use token::model::ui::UiState;
    use token::model::AppModel;
    use token::theme::Theme;
    use token::update::update;

    let scale = scenario.scale as f32;
    let font = fontdue::Font::from_bytes(
        include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
        fontdue::FontSettings::default(),
    ).map_err(|e| anyhow::anyhow!("Font error: {}", e))?;

    let font_size = 14.0 * scale;
    let line_metrics = font.horizontal_line_metrics(font_size)
        .ok_or_else(|| anyhow::anyhow!("Font missing line metrics"))?;
    let line_height = line_metrics.new_line_size.ceil() as usize;
    let (m, _) = font.rasterize('M', font_size);
    let char_width = m.advance_width;

    let status_bar_height = line_height;
    let visible_lines = (scenario.height as usize).saturating_sub(status_bar_height) / line_height;
    let visible_columns = ((scenario.width as f32 - 60.0) / char_width).floor() as usize;

    // Load first file
    let first_file = &scenario.files[0];
    let content = std::fs::read_to_string(&first_file.path)?;
    let document = Document::with_text(&content);
    let mut editor = EditorState::with_viewport(visible_lines, visible_columns);

    // Apply cursor/scroll for first file
    if let Some(line) = first_file.scroll_to {
        editor.viewport.top_line = line;
    }
    if let Some(line) = first_file.cursor_line {
        if let Some(cursor) = editor.cursors.first_mut() {
            cursor.line = line;
            cursor.column = first_file.cursor_column.unwrap_or(0);
        }
    }

    let editor_area = EditorArea::single_document(document, editor);

    // Load theme
    let theme = if let Some(ref theme_id) = scenario.theme {
        // Try as file path first, then as builtin id
        let theme_path = std::path::Path::new(theme_id);
        if theme_path.exists() {
            token::theme::from_file(theme_path)
                .map_err(|e| anyhow::anyhow!("Theme error: {}", e))?
        } else {
            token::theme::load_theme(theme_id)
                .map_err(|e| anyhow::anyhow!("Theme error: {}", e))?
        }
    } else {
        Theme::default()
    };

    let mut model = AppModel {
        editor_area,
        ui: UiState::new(),
        theme,
        config: EditorConfig::default(),
        window_size: (scenario.width, scenario.height),
        line_height,
        char_width,
        metrics: token::model::ScaledMetrics::default(),
        workspace: None,
        dock_layout: token::panel::DockLayout::default(),
        #[cfg(debug_assertions)]
        debug_overlay: None,
    };

    // Add additional splits for remaining files
    for file_spec in scenario.files.iter().skip(1) {
        let direction = match scenario.split_direction {
            SplitDir::Horizontal => token::model::editor_area::SplitDirection::Horizontal,
            SplitDir::Vertical => token::model::editor_area::SplitDirection::Vertical,
        };
        update(&mut model, Msg::Layout(LayoutMsg::SplitFocused(direction)));

        let file_content = std::fs::read_to_string(&file_spec.path)?;
        if let Some(doc) = model.editor_area.focused_document_mut() {
            doc.buffer = ropey::Rope::from_str(&file_content);
        }

        // Apply cursor/scroll
        if let Some(editor) = model.editor_area.focused_editor_mut() {
            if let Some(line) = file_spec.scroll_to {
                editor.viewport.top_line = line;
            }
            if let Some(line) = file_spec.cursor_line {
                if let Some(cursor) = editor.cursors.first_mut() {
                    cursor.line = line;
                    cursor.column = file_spec.cursor_column.unwrap_or(0);
                }
            }
            // Add extra cursors
            for pos in &file_spec.extra_cursors {
                editor.cursors.push(token::model::cursor::Cursor::new(pos.line, pos.column));
            }
        }
    }

    Ok(model)
}
```

**Step 2: Implement the render-to-buffer function**

```rust
fn render_to_buffer(model: &mut AppModel, width: usize, height: usize) -> Vec<u32> {
    use token::view::{Frame, TextPainter, Renderer};
    use token::model::editor_area::Rect;

    let font = fontdue::Font::from_bytes(
        include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
        fontdue::FontSettings::default(),
    ).expect("Font load failed");

    let scale = 2.0f32;
    let font_size = 14.0 * scale;
    let line_metrics = font.horizontal_line_metrics(font_size).unwrap();
    let line_height = line_metrics.new_line_size.ceil() as usize;
    let ascent = line_metrics.ascent;
    let (m, _) = font.rasterize('M', font_size);
    let char_width = m.advance_width;

    let status_bar_height = line_height;
    let sidebar_width = 0.0f32; // No sidebar for now

    let mut buffer = vec![0u32; width * height];
    let mut glyph_cache = std::collections::HashMap::new();

    // Clear with background
    let bg_color = model.theme.editor.background.to_argb_u32();
    buffer.fill(bg_color);

    // Compute layout
    let available_rect = Rect::new(
        sidebar_width,
        0.0,
        (width as f32) - sidebar_width,
        (height as usize).saturating_sub(status_bar_height) as f32,
    );
    let splitters = model.editor_area
        .compute_layout_scaled(available_rect, model.metrics.splitter_width);

    // Render editor area
    {
        let mut frame = Frame::new(&mut buffer, width, height);
        let mut painter = TextPainter::new(
            &font, &mut glyph_cache, font_size, ascent, char_width, line_height,
        );
        Renderer::render_editor_area(&mut frame, &mut painter, model, &splitters);
    }

    // Render status bar
    {
        let mut frame = Frame::new(&mut buffer, width, height);
        let mut painter = TextPainter::new(
            &font, &mut glyph_cache, font_size, ascent, char_width, line_height,
        );
        Renderer::render_status_bar(&mut frame, &mut painter, model, width, height);
    }

    buffer
}
```

**Step 3: Implement ARGB-to-RGBA PNG writer**

```rust
fn save_png(buffer: &[u32], width: u32, height: u32, path: &Path) -> Result<()> {
    let mut rgba_buf: Vec<u8> = Vec::with_capacity((width * height * 4) as usize);
    for &pixel in buffer {
        // Buffer is ARGB (0xAARRGGBB) → need RGBA
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        let a = ((pixel >> 24) & 0xFF) as u8;
        rgba_buf.extend_from_slice(&[r, g, b, a]);
    }

    let img: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> =
        image::ImageBuffer::from_raw(width, height, rgba_buf)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
    img.save(path)?;

    Ok(())
}
```

**Step 4: Wire up main()**

```rust
fn main() -> Result<()> {
    let args = Args::parse();

    std::fs::create_dir_all(&args.out_dir)?;

    let scenarios = if let Some(ref path) = args.scenario {
        vec![load_scenario(path)?]
    } else if args.all {
        let mut scenarios = Vec::new();
        for entry in std::fs::read_dir(&args.scenarios_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yaml" || e == "yml") {
                match load_scenario(&path) {
                    Ok(s) => scenarios.push(s),
                    Err(e) => eprintln!("Warning: skipping {}: {}", path.display(), e),
                }
            }
        }
        scenarios.sort_by(|a, b| a.name.cmp(&b.name));
        scenarios
    } else {
        anyhow::bail!("Specify --scenario <path> or --all");
    };

    for scenario in &scenarios {
        let width = args.width.unwrap_or(scenario.width);
        let height = args.height.unwrap_or(scenario.height);

        eprintln!("Rendering: {} ({}x{})", scenario.name, width, height);

        let mut model = create_model_from_scenario(scenario)?;
        model.window_size = (width, height);

        let buffer = render_to_buffer(&mut model, width as usize, height as usize);

        let out_path = args.out_dir.join(format!("screenshot-{}.png", scenario.name));
        save_png(&buffer, width, height, &out_path)?;
        eprintln!("  → {}", out_path.display());
    }

    eprintln!("Done! {} screenshot(s) generated.", scenarios.len());
    Ok(())
}
```

**Step 5: Run build**

Run: `cargo build --bin screenshot`
Expected: Compiles. May need to adjust import paths based on actual module visibility.

**Step 6: Commit**

```bash
git add src/bin/screenshot.rs
git commit -m "feat: implement headless screenshot rendering with real Renderer code path"
```

---

### Task 5: Create initial scenario files

**Files:**
- Create: `screenshots/scenarios/hero.yaml`
- Create: `screenshots/scenarios/multi-cursor.yaml`
- Create: `screenshots/scenarios/splits.yaml`
- Create: `screenshots/scenarios/csv.yaml`
- Create: `screenshots/scenarios/minimal.yaml`

**Step 1: Create scenarios directory**

```bash
mkdir -p screenshots/scenarios
```

**Step 2: Create hero scenario**

`screenshots/scenarios/hero.yaml`:
```yaml
name: hero
width: 2880
height: 1800
scale: 2.0
theme: fleet-dark
split_direction: horizontal
files:
  - path: samples/syntax/sample.rs
    scroll_to: 0
    cursor_line: 12
    cursor_column: 20
  - path: samples/syntax/sample.yaml
    scroll_to: 0
```

**Step 3: Create multi-cursor scenario**

`screenshots/scenarios/multi-cursor.yaml`:
```yaml
name: multi-cursor
width: 2880
height: 1800
scale: 2.0
theme: fleet-dark
files:
  - path: samples/syntax/sample.rs
    scroll_to: 0
    cursor_line: 5
    cursor_column: 8
    extra_cursors:
      - line: 6
        column: 8
      - line: 7
        column: 8
      - line: 8
        column: 8
```

**Step 4: Create splits scenario**

`screenshots/scenarios/splits.yaml`:
```yaml
name: splits
width: 2880
height: 1800
scale: 2.0
theme: fleet-dark
split_direction: horizontal
files:
  - path: samples/syntax/sample.rs
    scroll_to: 0
  - path: samples/syntax/sample.ts
    scroll_to: 0
  - path: samples/syntax/sample.py
    scroll_to: 0
```

**Step 5: Create CSV scenario**

`screenshots/scenarios/csv.yaml`:
```yaml
name: csv
width: 2880
height: 1800
scale: 2.0
theme: fleet-dark
files:
  - path: samples/large_data.csv
    scroll_to: 0
```

Note: CSV mode rendering requires the editor to be in CSV view mode. If the screenshot binary doesn't auto-detect CSV, this may need a `view_mode: csv` field in the scenario spec — add it when implementing if needed.

**Step 6: Create minimal scenario**

`screenshots/scenarios/minimal.yaml`:
```yaml
name: minimal
width: 2880
height: 1800
scale: 2.0
theme: fleet-dark
files:
  - path: samples/syntax/sample.rs
    scroll_to: 10
    cursor_line: 18
    cursor_column: 0
```

**Step 7: Commit**

```bash
git add screenshots/
git commit -m "feat: add screenshot scenario YAML files"
```

---

### Task 6: Add Makefile target and test the pipeline

**Files:**
- Modify: `Makefile`

**Step 1: Add `screenshots` target to Makefile**

```makefile
.PHONY: screenshots
screenshots:
	cargo run --release --bin screenshot -- --all --out-dir website/v4/public
```

**Step 2: Run it end-to-end**

Run: `make screenshots`
Expected: Generates PNG files in `website/v4/public/`:
- `screenshot-hero.png`
- `screenshot-multi-cursor.png`
- `screenshot-splits.png`
- `screenshot-csv.png`
- `screenshot-minimal.png`

**Step 3: Verify output**

Open one of the generated PNGs and visually confirm it looks like the real editor.

Run: `open website/v4/public/screenshot-hero.png`

**Step 4: Commit**

```bash
git add Makefile
git commit -m "feat: add make screenshots target"
```

---

## Phase 2: Homepage Improvements

### Task 7: Replace the hero visual with a real screenshot

**Files:**
- Modify: `website/v4/src/pages/index.astro`

**Step 1: Replace the HTML "fake editor" in the hero section**

Replace the `<div class="hero-editor">...</div>` block (lines 22-49) with:

```html
<div class="hero-editor">
  <div class="editor-frame">
    <div class="editor-window-chrome">
      <span class="dot red"></span>
      <span class="dot yellow"></span>
      <span class="dot green"></span>
    </div>
    <img src="/screenshot-hero.png" alt="Token editor showing Rust code with multi-cursor editing and split views" loading="eager" />
  </div>
</div>
```

**Step 2: Add window chrome CSS**

Add to the `<style>` block:

```css
.editor-window-chrome {
  display: flex;
  gap: 6px;
  padding: 10px 14px;
  background: var(--panel-bg);
  border-bottom: 1px solid var(--border);
}
.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}
.dot.red { background: #FF5F57; }
.dot.yellow { background: #FEBC2E; }
.dot.green { background: #28C840; }
```

**Step 3: Update the showcase section to use a different screenshot**

Replace the hard-coded `screenshot.png` in the showcase section (line ~143) with `screenshot-splits.png`:

```html
<img src="/screenshot-splits.png" alt="Token editor with three split views" />
```

**Step 4: Build and verify**

Run: `cd website/v4 && npx astro build`
Expected: Builds successfully

**Step 5: Commit**

```bash
git add website/v4/src/pages/index.astro
git commit -m "feat: replace hero fake editor with real screenshot"
```

---

### Task 8: Add micro-interaction polish to homepage

**Files:**
- Modify: `website/v4/src/pages/index.astro`

**Step 1: Add hover lift to feature cards**

Find the `.feature-card:hover` rule and update:

```css
.feature-card:hover {
  border-color: var(--border-strong);
  transform: translateY(-2px);
  transition: border-color 0.2s, transform 0.2s var(--ease-out);
}
```

Add base transition to `.feature-card`:

```css
.feature-card {
  /* ... existing styles ... */
  transition: border-color 0.2s, transform 0.2s var(--ease-out);
}
```

**Step 2: Add hero CTA arrow animation**

```css
.btn-primary .arrow {
  display: inline-block;
  transition: transform var(--duration-normal) var(--ease-out);
}
.btn-primary:hover .arrow {
  transform: translateY(3px);
}
```

**Step 3: Add glow on hero screenshot hover**

```css
.hero-editor .editor-frame {
  transition: box-shadow var(--duration-normal) var(--ease-out);
}
.hero-editor .editor-frame:hover {
  box-shadow: 0 25px 60px -12px rgba(0,0,0,0.5),
              0 0 0 1px rgba(255,255,255,0.04),
              var(--shadow-glow);
}
```

**Step 4: Build and verify**

Run: `cd website/v4 && npx astro build`
Expected: Builds

**Step 5: Commit**

```bash
git add website/v4/src/pages/index.astro
git commit -m "feat: add micro-interaction polish to homepage"
```

---

### Task 9: Add trust/proof row to hero section

**Files:**
- Modify: `website/v4/src/pages/index.astro`

**Step 1: Add trust badges after hero-actions div (after line ~20)**

```html
<div class="hero-trust">
  <span class="trust-item">MIT Licensed</span>
  <span class="trust-sep">·</span>
  <span class="trust-item">No Telemetry</span>
  <span class="trust-sep">·</span>
  <span class="trust-item">Open Source</span>
</div>
```

**Step 2: Add CSS**

```css
.hero-trust {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 24px;
  font-family: var(--font-mono);
  font-size: 0.7rem;
  color: var(--fg-muted);
}
.trust-sep {
  opacity: 0.4;
}
```

**Step 3: Build and verify**

Run: `cd website/v4 && npx astro build`

**Step 4: Commit**

```bash
git add website/v4/src/pages/index.astro
git commit -m "feat: add trust badges to hero section"
```

---

## Notes

### Determinism checklist for CI
- Font is embedded via `include_bytes!` — no OS dependency
- Theme loaded from repo — deterministic
- Cursor blink: not rendered in screenshot mode (static render, no animation state)
- No randomness in any rendering path

### Future improvements (not in this plan)
- Add `--diff` mode to compare against golden images
- Sidebar rendering in screenshots (requires workspace setup)
- CSV view mode forcing in scenarios
- Animated GIF generation for multi-cursor demo
- Per-platform window chrome (macOS/Linux/Windows titlebars)

### Key risk: render method visibility
The biggest risk is that `Renderer`'s static methods might reference private types or methods when made `pub`. If so, those dependencies also need visibility changes. The fix is straightforward — just widen visibility of the referenced types. The compiler will tell you exactly what needs changing.

### File paths reference
- Binary: `src/bin/screenshot.rs`
- Scenarios: `screenshots/scenarios/*.yaml`
- Output: `website/v4/public/screenshot-*.png` (via make target)
- Font: `assets/JetBrainsMono.ttf` (already exists, embedded)
- Themes: `themes/*.yaml` (already exist)
- Sample files: `samples/syntax/*.rs`, `samples/large_data.csv`, etc.
