# WASM / Web Target for Token

> Feasibility analysis for compiling Token to WebAssembly and running it in a browser.
> Investigated: Feb 2026

## Summary

**Feasible** — Token's Elm architecture and CPU-based pixel renderer are well-suited for a WASM port. The core editor logic (model/update/view) is highly portable. The main work is abstracting platform services and replacing the pixel buffer presentation layer.

**Effort:** High (~2–6 weeks) for a minimal demo; Extreme (~2–4 months) for feature parity.

---

## Dependency Compatibility

### ✅ Works in WASM as-is
| Dependency | Notes |
|---|---|
| ropey | Pure Rust, no platform deps |
| fontdue | Pure Rust CPU rasterizer |
| serde / serde_json / serde_yaml | OK (yaml adds some binary bloat) |
| csv | Pure Rust |
| nucleo-matcher | Pure Rust |
| anyhow | OK |
| pulldown-cmark | Pure Rust |
| image (PNG) | Decoding is fine; loading files needs platform abstraction |

### 🟡 Works with caveats
| Dependency | Caveat |
|---|---|
| **winit 0.30** | Supports wasm web backend, but: browser steals some shortcuts (Cmd+T, Cmd+L), IME/text input differs, event loop is non-blocking |
| **tree-sitter 0.25** | Compiles to wasm (includes C code), but significantly increases binary size. Feature-gate languages for web builds |
| **tracing** | Core works; subscriber/appender stack is stdout/file oriented, needs web-specific subscriber |

### 🔴 Must replace or `cfg`-gate
| Dependency | Issue | Web Alternative |
|---|---|---|
| **softbuffer 0.4** | No browser target (biggest blocker) | Canvas2D `putImageData` or WebGL texture upload |
| **arboard 3.4** | Native clipboard only | Web Clipboard API (`navigator.clipboard`) |
| **rfd 0.15** | Native file dialogs | `<input type="file">` / File System Access API |
| **notify 6.1** | OS filesystem watching | Not possible in browser; manual refresh or polling |
| **wry 0.50** | Webview embedding | Already in a browser; use DOM-based preview pane |
| **dirs 5** | OS config directories | Not meaningful in browser |
| **clap 4** | CLI argument parsing | No CLI in browser; `cfg`-gate out |
| **open 5** | Open URL in default app | `window.open()` via JS interop |

---

## Rendering Strategy

Token already does CPU rendering to a `Vec<u32>` pixel buffer via fontdue, then presents via softbuffer. The renderer itself is portable — only the "present" step needs replacement.

### Option A: Canvas2D (simplest, recommended first)
- Create a `<canvas>` element with a 2D context
- Each frame: write pixel buffer to `ImageData`, call `putImageData`
- Can optimize with damage rectangles
- **Pros:** Simplest to implement, minimal JS glue
- **Cons:** Full-frame pixel copies can be slow at retina resolution

### Option B: WebGL texture upload (better performance)
- Treat the CPU pixel buffer as a texture
- Use `texSubImage2D` for damaged regions only
- Draw a fullscreen quad
- **Pros:** Better perf at high DPI, damage rects map well
- **Cons:** More setup code

### Option C: wgpu/WebGPU (long-term)
- Replace softbuffer with wgpu on both native and web
- Unified backend for all platforms
- **Pros:** Best long-term direction, consistent native + web
- **Cons:** Largest refactor, overkill for initial port

**Recommendation:** Start with Canvas2D, upgrade to WebGL if performance is insufficient.

---

## Platform Abstractions Needed

### File I/O
- **Open:** `<input type="file">` → read via FileReader → pass bytes to wasm
- **Save:** Generate Blob + download link, or File System Access API `showSaveFilePicker`
- **Workspace/directory:** File System Access API can grant directory handles (limited browser support, async, permission-gated)
- **Pragmatic first step:** Support single-file open/save; keep virtual in-memory workspace

### Clipboard
- Use Web Clipboard API (`navigator.clipboard.readText/writeText`)
- Must be triggered by user gesture
- Permissions differ by browser

### Threading (syntax worker)
- WASM threads require SharedArrayBuffer + cross-origin isolation headers (COOP/COEP)
- Many deployments don't have this by default
- **Pragmatic first step:** Run tree-sitter parsing on main thread with throttling (already have scheduled parsing)
- **Later:** Move parsing to a Web Worker

---

## Known Risks & Gotchas

1. **Performance at retina resolution** — Full-frame pixel uploads are expensive. Fix with WebGL + partial texture updates using existing damage tracking.
2. **Input/IME correctness** — Web text input handling differs from native winit. Character insertion may need adjustment. Some key events differ.
3. **Browser-reserved shortcuts** — Cmd+T, Cmd+L, Cmd+W, etc. will never reach the app. Must accept or work around.
4. **Async platform APIs** — File pickers and clipboard are async + permission-gated. The Elm `Cmd` system will need async completion messages.
5. **Binary size** — Tree-sitter grammars add significant weight. Feature-gate aggressively; start with 2-3 languages for web.

---

## Incremental Porting Plan

### Phase 1: Core/platform split
- Extract a `PlatformServices` trait (or `cfg`-gated modules) for clipboard, file I/O, dialogs
- Ensure core model/update/render-to-buffer compiles independently of platform deps

### Phase 2: Minimal WASM demo
- `cfg`-gate out: notify, rfd, arboard, wry, dirs, clap, open
- Implement Canvas2D pixel buffer presentation
- Wire up winit web backend
- Single-file editing with hardcoded sample text
- **Target: basic text editing working in browser**

### Phase 3: File operations + clipboard
- Add JS interop for file open/save via `<input type="file">` and download
- Web Clipboard API integration
- Basic theme loading (embed default theme)

### Phase 4: Syntax highlighting
- Enable tree-sitter with 2-3 languages (e.g., Rust, JavaScript, JSON)
- Run parsing synchronously on main thread
- Feature-gate remaining languages

### Phase 5: Enhanced features (optional)
- WebGL rendering backend for better performance
- Web Worker for syntax parsing
- File System Access API for directory/workspace support
- DOM-based markdown preview pane

---

## References & Prior Art

- **egui/eframe** — Rust GUI framework that compiles to wasm, renders to Canvas2D/WebGL. Good reference for input + rendering loop patterns.
- **wgpu examples** — Official wgpu repo demonstrates WebGPU/WebGL on wasm.
- **Bevy (web)** — Game engine demonstrating real-time rendering/event loops in wasm.
- **Tree-sitter playground** — tree-sitter compiled to wasm for web syntax highlighting.
