# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build --release    # Build optimized binary
cargo run --release      # Run the editor
cargo test               # Run all 56 unit tests
cargo test test_name     # Run a specific test
```

## Architecture

This is a minimal text editor implementing the **Elm Architecture** in Rust:

```
Message → Update → Command → Render
```

### Core Components (all in `src/main.rs`)

| Component        | Lines    | Purpose                                                              |
| ---------------- | -------- | -------------------------------------------------------------------- |
| **Model**        | 24-172   | Application state: buffer (Rope), cursor, viewport, undo/redo stacks |
| **Msg**          | 207-245  | All possible user events (movement, editing, navigation)             |
| **update()**     | 251-601  | Pure state transformation: `Msg → Model mutation → Option<Cmd>`      |
| **handle_key()** | 603-660  | Maps keyboard input to Msg                                           |
| **Renderer**     | 675-860  | CPU rendering with fontdue + softbuffer                              |
| **App**          | 939-1078 | winit event loop integration                                         |

### Key Data Structures

- **Rope** (ropey): Efficient text buffer with O(log n) edits
- **Cursor**: `{line, column, desired_column}` - desired_column preserves position during vertical movement
- **EditOperation**: Captures insert/delete for undo/redo with cursor positions before/after
- **GlyphCache**: `HashMap<(char, font_size_bits), (Metrics, bitmap)>` avoids re-rasterization

### Character Classification for Word Navigation

IntelliJ-style word boundaries using `CharType`:

- **Whitespace**: Navigable unit (stops at both edges)
- **WordChar**: Alphanumerics
- **Punctuation**: Symbols treated as separate units

### Rendering Pipeline

1. Clear framebuffer (#1E1E1E dark background)
2. Render visible lines with line numbers
3. Draw blinking cursor (500ms interval)
4. Render status bar
5. Present via softbuffer

Font: JetBrains Mono embedded in `assets/JetBrainsMono.ttf`

## Key Bindings

| Action          | Mac                 | Standard        |
| --------------- | ------------------- | --------------- |
| Line start/end  | Cmd+←/→             | Home/End        |
| Word left/right | Option+←/→          | -               |
| Doc start/end   | Ctrl+Home/End       | -               |
| Undo/Redo       | Cmd+Z / Cmd+Shift+Z | Ctrl+Z / Ctrl+Y |
