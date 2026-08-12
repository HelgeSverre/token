# LSP Server Integration

Spawn language servers per workspace, keep documents synchronized over JSON-RPC, and unlock diagnostics, go-to-definition, hover documentation, and completion — without breaking the single-threaded Elm loop or replacing any tree-sitter feature that already works.

> **Status:** 📋 Planned
> **Priority:** P2 (Important)
> **Effort:** XL (2+ weeks, phased — each phase ships independently)
> **Created:** 2026-08-11
> **Updated:** 2026-08-11 (revised after 3-reviewer pass: protocol lifecycle, codebase fidelity, phasing)
> **Milestone:** 4 - Hard Problems

---

## Overview

### Why

Tree-sitter gives token syntactic understanding: highlighting, outline, and syntax-aware expand selection all work from the parse tree of a single file. What it cannot give is *semantic* understanding — what a name resolves to, whether code is wrong, what a symbol's documentation says, or where something is defined in another file. That is exactly what the Language Server Protocol provides, and every server (rust-analyzer, typescript-language-server, pyright, gopls, intelephense, …) speaks the same wire protocol, so one client implementation covers every language.

### Current State

No LSP support exists. Relevant existing infrastructure:

- **Syntax worker** (`src/runtime/app.rs::syntax_worker_loop`): the canonical async pattern — `std::thread` + `mpsc`, results returned as `Msg` on `App.msg_tx`, event loop woken via `EventLoopProxy`, revision-guarded application in `update/syntax.rs`. No tokio in the editor loop; `serde_json` already a dependency.
- **Process spawning** (`src/terminal/pty.rs`): `spawn_pty` shows the child-process pattern. Note the real constraints: anything stored in `AppModel` must be `Debug` (the model is not `Clone`), and spawn results are installed on the main thread via a side-channel `Receiver` (`terminal_spawn_rx`) because spawning blocks — `PtyHandle` itself does live in the model (`TerminalSession.pty`). For LSP we keep handles in the runtime anyway (see Process Model) — but for state-mirroring reasons, not a `Clone` constraint.
- **Automation socket** (`src/automation.rs`): a working request/response JSON line protocol bridged into the Elm loop — closer in shape to LSP than the fire-and-forget syntax worker.
- **Workspace** (`src/model/workspace.rs`): `AppModel.workspace` is `Option<Workspace>` — zero or one root. `fs_watcher.rs` (notify, 500 ms debounce) can feed `workspace/didChangeWatchedFiles`.
- **Cross-file jumps**: `LayoutMsg::OpenFileInNewTab` (`update/layout.rs::open_file_in_new_tab`) loads **synchronously** via `Document::from_file`, reuses an already-open tab via `find_open_file`, and focuses the new tab — cursor placement can happen in the same update. The cursor-jump body in `update/outline.rs::JumpToSymbol` (clamp, set cursor, `ensure_cursor_visible_centered`, focus editor) is the model, but it is same-document-only and its clamp helper reads the focused document; Phase 3 needs a document-parameterized variant.
- **Document lifecycle**: `Document.revision: u64` bumps per content mutation (edits *and* file reloads) — maps onto LSP `textDocument` versions. Documents are refcounted across editors/groups; `release_document_if_unreferenced` (`update/layout.rs`) is where `Cmd::ClearSyntaxState` fires today and is the correct `didClose` hook.
- **Language registry** (`src/syntax/registry.rs`): one static `LanguageDefinition` table per language. It is `pub(crate)`, built by positional macros (`language_registry!` et al.) with ~85 invocations — adding a field means a defaulted macro arm, not touching every call site.
- **Overlay surfaces** ([overlay-surface.md](overlay-surface.md), Milestone 1): owns the cursor-anchored popup shells (`Anchor::Cursor`), `draw_wavy_underline`, and the severity glyph/color conventions. LSP Phases 4–5 consume those surfaces; they do not build them.

### What LSP Unlocks (and in what order)

| Capability | LSP method | UI needed | Phase |
| --- | --- | --- | --- |
| Diagnostics (errors/warnings inline) | `textDocument/publishDiagnostics` | Gutter marks, underlines, status bar | 2 |
| Go to definition | `textDocument/definition` | None new — file-open + cursor jump; adds a back stack | 3 |
| Hover documentation | `textDocument/hover` | Cursor-anchored card from overlay-surface.md | 4 |
| Code completion | `textDocument/completion` | Cursor-anchored dropdown from overlay-surface.md | 5 |
| Find references, rename, code actions, formatting, workspace symbols, semantic tokens | various | Problems panel, pickers | Future |

Diagnostics come first deliberately: they exercise the entire pipeline (spawn, handshake, document sync, server-initiated traffic, rendering) while needing zero interactive UI. If phase 2 works, the protocol layer is proven.

### Goals

- One LSP client implementation, servers registered per language, spawned lazily per root.
- Document synchronization that is provably lossless: every edit reaches the server with a monotonically increasing version derived from `Document.revision`, and no request is ever answered against text the server hasn't seen (flush-before-request invariant).
- Every feature degrades to current behavior when no server is available, installed, or ready. LSP is additive, never load-bearing.
- The UI thread never blocks on a server; stale responses are discarded by revision guards (diagnostics excepted — see below).
- Automation/MCP can drive LSP actions by name and inspect results, like every other editor action.

### Non-Goals

- **Replacing tree-sitter features.** Syntax highlighting, the outline panel, and syntax-aware expand selection remain tree-sitter based. LSP equivalents (`semanticTokens`, `documentSymbol`, `selectionRange`) are strictly optional future *augmentations*.
- Auto-installing or downloading language servers. The user installs binaries; token finds them on `PATH` or via config.
- Multiple simultaneous servers per document (e.g. tailwind + typescript on one file).
- Snippet support. We advertise `snippetSupport: false`, so conforming servers send plain-text inserts and no flattening code is needed. Revisit when the snippets feature (`docs/feature/snippets.md`) exists.
- Pull diagnostics (`textDocument/diagnostic`, 3.17). We deliberately don't advertise it; all three initial servers fall back to push. Pull is the future upgrade path (per-document, on-demand — it would replace the workspace-wide publish flood) — deliberate choice, not an oversight.
- Semantic tokens, inlay hints, code lens, call hierarchy, `willSaveWaitUntil` (format-on-save) — future phases.
- Windows named-pipe or socket transports; stdio only.

---

## Architecture

### Where It Lives

```
src/
├── lsp/                     # New module
│   ├── mod.rs               # Public exports, LspServerDef, ServerState
│   ├── transport.rs         # Content-Length framing over child stdio (~80 lines)
│   ├── client.rs            # Per-server worker thread: lifecycle, request ids, dispatch
│   ├── uri.rs               # Canonical path ↔ file:// URI (symlinks, percent-encoding)
│   └── position.rs          # Editor (line, char-col) ↔ LSP (line, UTF-16 col) conversion
├── update/lsp.rs            # LspMsg handlers, revision guards, response routing
├── messages.rs              # + Msg::Lsp(LspMsg)
├── commands.rs              # + Cmd::Lsp* variants, damage arms
└── runtime/app.rs           # LspManager: server handles, spawn, routing, deadlines
```

**Dependencies:** `lsp-types` (serde-only protocol types, no runtime machinery) — taken outright, not hedged: hand-rolling ~15 types and migrating later is the expensive branch, and the crate pre-pays every future method. Framing stays hand-written (~80 lines over `serde_json`). Do **not** take `tower-lsp` / `async-lsp` — they drag in an async runtime the editor loop doesn't have.

### Process Model

One server process per `(server definition, root)`, spawned lazily on the first `didOpen` of a matching document, owned by an `LspManager` inside `App`.

**Root resolution** (nothing here assumes a workspace — `token foo.rs` must work): the workspace root if the file is under it; else the nearest ancestor containing a project marker from the server def (`Cargo.toml`, `package.json`, `pyproject.toml`); else the file's parent directory. Detached (non-workspace) roots are capped at 4 per session. `initialize` sends `rootUri`, `rootPath`, *and* `workspaceFolders` — `rootUri` is deprecated in 3.6+ but all three target servers still read it. Multi-root support falls out for free if `Workspace` ever becomes a list.

**Ownership and the model mirror.** The runtime owns handles (`Child`, stdin writer, worker `Sender`) and is authoritative. The model holds a derived, render-only mirror updated exclusively by messages — this is the invariant that keeps process handles out of `AppModel` while letting the status bar and automation (which read only the model) show server state:

```rust
// model — render-only mirror, mutated only by LspMsg handlers
pub struct LspUiState {
    pub servers: HashMap<LspServerId, ServerState>,
}
pub enum ServerState { Starting, Indexing, Ready, Restarting { attempt: u8 }, Failed, Missing }
```

`Indexing` matters: rust-analyzer takes seconds to minutes on a cold workspace and answers requests with empty results meanwhile. The state is driven by `$/progress` work-done notifications (we advertise `window.workDoneProgress`) and rust-analyzer's `experimental/serverStatus` where available. While not `Ready`, empty feature results display "language server still indexing…", never "not found".

Each server gets one worker thread owning the child's stdin/stdout, **plus a stderr-drain thread** (rust-analyzer writes volume to stderr; an undrained 64 KB pipe buffer wedges the child — stderr goes to `tracing`, preserving crash context):

```text
App (main thread)                     lsp worker thread (per server)
────────────────────                  ──────────────────────────────
Cmd::Lsp*  ──► Sender<LspWorkerMsg> ──► write JSON-RPC to child stdin
                                        read loop on child stdout:
Msg::Lsp(..) ◄── msg_tx.send(..) ◄──── responses + server notifications
              EventLoopProxy wake       (stderr thread → tracing, never Msg)
```

**Handshake ordering.** Nothing may be written before the `initialize` *response* arrives, and no notification before `initialized` is sent — but servers spawn lazily on a `didOpen` that therefore always races the handshake. The worker queues all outbound traffic (`VecDeque` + state flag) until the handshake completes, then flushes in order. After any restart, `didOpen` is re-sent for every currently-open matching document.

**Worker inbound rules.** `window/logMessage`, `window/showMessage` (log-level), and telemetry are written to `tracing` from the worker thread and **never** forwarded as `Msg` — a chatty server must not wake the render loop. Successive `publishDiagnostics` for the same URI coalesce before drain (newest wins), the same folding the syntax worker does for parse requests. On Windows, server binaries are resolved through `PATHEXT` (`typescript-language-server` and `pyright-langserver` are `.cmd` shims that bare `Command::new` won't find).

**Server → client requests.** The client is a server too: every incoming request MUST get a reply or real servers hang — rust-analyzer and pyright block their init path on `workspace/configuration`. Phase 1 ships this table:

| Incoming request | Reply |
| --- | --- |
| `workspace/configuration` | `[null, …]` (one entry per requested item → server defaults) |
| `client/registerCapability` / `unregisterCapability` | `null` (registration handling itself deferred; the *reply* is not) |
| `window/workDoneProgress/create` | `null` |
| `workspace/applyEdit` | `{ applied: false }` until rename/code-actions exist |
| `window/showMessageRequest` | `null` |
| anything else | JSON-RPC error `MethodNotFound (-32601)` |

Unknown *notifications* are ignored silently.

**Crash and shutdown.** Child exit → `Msg::Lsp(ServerExited)`; restart with exponential backoff, capped at 3 attempts, then `Failed` (restartable via `RestartLanguageServer` command). An explicit `ShuttingDown` state suppresses the restarter during quit/workspace close. Shutdown sequence: `shutdown` request → **await its response** (or 2 s) → `exit` notification → await process exit (or 2 s) → kill. (Sending `exit` before the `shutdown` response makes rust-analyzer/gopls exit non-zero, indistinguishable from a crash.) There is currently **no quit-time child-teardown path in the runtime at all** (`Cmd::Quit` only sets a flag; even PTY children aren't killed) — Phase 1 builds one (an exit hook running LSP teardown; the terminal can adopt it later). Backstop: `initialize` passes `processId`, and all target servers self-terminate when the parent PID disappears — that alone satisfies "no zombie children" even on a crash of token itself.

### Client Capabilities

Not boilerplate — several advertisements delete planned work. Rule: **never advertise a capability we don't implement.** Phase 1 advertises exactly:

- `general.positionEncodings: ["utf-16"]` — see Position Encoding.
- `textDocument.synchronization` — didOpen/didChange/didSave/didClose, no willSave.
- `textDocument.publishDiagnostics: { relatedInformation: true, versionSupport: true, tagSupport: { valueSet: [Unnecessary, Deprecated] } }` — tags drive the faded/strikethrough decorations editor-decorations.md already defines.
- `textDocument.definition.linkSupport: false` — servers must return `Location[]`; `LocationLink` handling never exists.
- `textDocument.hover.contentFormat: ["plaintext", "markdown"]` (Phase 4).
- `textDocument.completion.completionItem: { snippetSupport: false, resolveSupport: { properties: ["documentation", "detail", "additionalTextEdits"] } }` (Phase 5).
- `window.workDoneProgress: true` (drives `Indexing`).
- `dynamicRegistration: false` everywhere — notably `didChangeWatchedFiles`: advertising it without implementing it makes servers assume we watch and go stale.

**Server capability gating:** `InitializeResult.capabilities` is parsed and stored on the handle; every send and every command's enablement checks it. `textDocumentSync` may be a number, an object, or absent (then: no sync messages at all); `save` may be absent (no `didSave`) or `{ includeText: true }` (send full text); a missing `definitionProvider`/`hoverProvider`/`completionProvider` makes the command report "not supported by this server", not "not found".

### URIs and Paths

One helper module (`lsp/uri.rs`) used on both sides of the boundary: `std::fs::canonicalize` when the file exists (macOS symlinks: `/tmp` → `/private/tmp`, home-dir symlinks — raw `PathBuf` equality silently drops every diagnostic), RFC 3986 percent-encoding, Windows drive-letter case. Raw `PathBuf`s are never compared. `didOpen`'s `languageId` uses the LSP-conventional strings (`typescript`, `typescriptreact`, `javascript`, `javascriptreact`, …), mapped from token's `LanguageId` — typescript-language-server keys JSX handling off it.

### Document Synchronization

The correctness core. Rules:

- `didOpen` when a matching document gains a file path and language; full text, version = `Document.revision`.
- `didChange` on every edit, debounced (deadline-map pattern, ~30 ms class) **with a max-wait cap**: the notification fires at latest N ms after the *first* pending edit, regardless of continued typing — an uncapped trailing debounce under continuous typing would let the server drift unboundedly. Full text (`TextDocumentSyncKind::Full`) below a size threshold (~256 KB, tunable); above it, a longer debounce until incremental sync lands. Version = revision at snapshot time; versions may skip (spec-legal), must only increase.
- **Flush-before-request invariant:** any `textDocument/*` request first flushes the document's pending `didChange` (debounce fired early, written to the channel ahead of the request frame). Without this, a request issued inside the debounce window is answered against stale text *and passes the revision guard*, because the response carries the revision we sent.
- **Shared snapshot:** when the syntax-parse and LSP deadlines coincide (they usually will), one `Arc<str>` rope snapshot serves both `Cmd::RunSyntaxParse` and the LSP send — `update/syntax.rs` already pays one `to_string()` per fire; LSP must not double it.
- `didSave` after successful save (with text iff the server asked via `includeText`). `didClose` when the document is **released** — `release_document_if_unreferenced`, alongside `ClearSyntaxState` — never on tab close: documents are refcounted across splits, and closing one of two tabs must not tell the server the file is gone.
- Save As and untitled→saved are a `didClose(old)` + `didOpen(new)` pair, not a rename. Untitled documents are not synced until saved (v1 rule; costs completion in scratch buffers — revisit with `untitled:` URIs if it hurts).
- Incremental sync is **nearer than it looks**, not a remote optimization: `EditOperation::{Insert, Delete, Replace}` already carries exactly the position+text deltas LSP wants; only undo/redo and external reload need the full-text fallback. Kept out of Phase 1 for simplicity; it is the prerequisite for large-file support, scheduled Phase 6.
- Invariant worth a test, not an assumption: **every** rope mutation bumps `Document.revision` (file reload included — it does today via `AppMsg::FileLoaded`, which conveniently forces a resync).

### Position Encoding

LSP columns are UTF-16 code units; token's are characters. `lsp/position.rs` converts **always** — the UTF-16 path is the only path. (Reviewed decision: of the three initial servers only rust-analyzer supports the 3.17 `positionEncoding: "utf-8"` negotiation; ts-ls and pyright are UTF-16-only, so a utf-8 fast path doubles the test matrix to skip nanoseconds of per-line conversion. Revisit only if a profile ever says otherwise.) The conversion helpers take the document, not globals; testing at expand-selection rigor: CRLF, surrogates/emoji, line-end positions. Note: the existing char↔byte helpers from the expand-selection work are private to `syntax/selection.rs`/`syntax/parser.rs` — `position.rs` is new code following their pattern, not an extension of them.

### Requests, Guards, Timeouts

- One outstanding request per feature per document; a newer request supersedes. Superseding sends `$/cancelRequest` and marks the pending entry *abandoned* — it is **not** dropped: cancellation is advisory and the server still replies (usually `RequestCancelled`/`ContentModified`, possibly a normal result), and that reply must be consumed against its id. Pending entries die only on response or server death.
- Timeouts are UI-level abandonment, same mechanism: ~30 s for definition/hover (cold rust-analyzer legitimately exceeds 10 s), ~2 s for completion, 60 s+ for `initialize`. `LspManager` owns the deadlines, folded into the runtime's existing `next_wake` computation in `about_to_wait`.
- **Revision guards, with the diagnostics exception:** feature *responses* (definition, hover, completion) are discarded unless their tagged revision equals the current `Document.revision`. **Diagnostics are exempt** — push diagnostics are always computed against an older revision (cargo check takes seconds; an equality guard would drop essentially all of them). For diagnostics, the optional `version` field is used only to discard out-of-order publishes for the same URI; positions are clamped at render time.

### Message Flow (go-to-definition example)

```text
F12
  → Command::GotoDefinition → Msg::Lsp(LspMsg::GotoDefinition)
  → update/lsp.rs: capture (document_id, revision, position)
  → Cmd::LspRequest(Definition { uri, position, revision })
  → worker: flush pending didChange, then write request
  → server responds → worker resolves pending entry
  → Msg::Lsp(LspMsg::DefinitionResolved { document_id, revision, locations })
  → update/lsp.rs: revision guard; push jump-history entry;
      same file  → clamp + set cursor + ensure_cursor_visible_centered
      other file → LayoutMsg::OpenFileInNewTab(path) — synchronous — then same cursor logic
  → Cmd::redraw_editor()
```

---

## Feature Behavior

### Diagnostics (Phase 2)

**Storage:** the authoritative store lives in `LspManager` as `HashMap<CanonicalUri, Vec<Diagnostic>>` — full replacement per publish, *including URIs with no open document*: rust-analyzer publishes workspace-wide from cargo check, and per-`Document`-only storage would silently drop errors in unopened files (and forecloses the future Problems panel). `Document.diagnostics` is a projection, refreshed on publish (for open docs) and on open (from the store). Publishes route by canonical URI → `find_open_file`; a publish for a document whose path changed under it (Save As) misses and is retained in the store under the old URI until the server re-publishes. A document open in two groups shows one set for free (state is on `Document` — a virtue of the placement, stated deliberately).

**Rendering** goes through the shared decoration layer ([editor-decorations.md](editor-decorations.md)): severity mark in the gutter marks lane, wavy underline (via `draw_wavy_underline` from overlay-surface.md) under the range, `Unnecessary`/`Deprecated` tags as faded/strikethrough. Positions clamp to the current buffer at render time; a vanished line's diagnostic is skipped, never a panic.

**Readable, not just visible:** the phase is only shippable if the user can *read* the error — the status bar shows the message of the highest-severity diagnostic under the cursor (truncated; ~20 lines against the existing segment machinery), plus a `SegmentId::Diagnostics` count segment (`✗ 2 ⚠ 5`). Full text with `relatedInformation` arrives with the hover card in Phase 4.

**Lifecycle:** cleared on document release, server exit, and language change.

### Go to Definition (Phase 3)

- `Command::GotoDefinition`, default F12 (Cmd+Click deferred — needs modifier-aware mouse hit-testing; keyboard-only is complete).
- Servers return `Location[]` (we advertise `linkSupport: false`); multiple locations → first (picker comes with find-references, Future).
- Same-file: clamp + set cursor + center + focus, the `JumpToSymbol` body with a document-parameterized clamp. Cross-file: `LayoutMsg::OpenFileInNewTab` — synchronous, reuses an already-open tab, then the same cursor logic in the same update. **There is no deferred placement and no async gap.** Edge case: `open_file_in_new_tab` can produce an image or binary-placeholder tab — skip cursor placement then.
- Files outside every root (stdlib, `~/.cargo/registry`): routed to the server that *resolved the location* — never spawn a new server rooted in a toolchain directory. Such buffers get `didOpen` on that server but are marked read-only-intent (no `didChange` expected; actual read-only enforcement is out of scope).
- **Jump history** is a general editor feature that LSP merely pushes to (it also wants outline jumps, goto-line, file-finder — users expect one back stack; an LSP-only stack would be rewritten in a month). Owned by the editor layer: entries `{ group_id, document_id, path, line, col }`, one global `Vec` on `AppModel`, `NavigateBack` pops the **focused group's** most recent entry (group-tagged so a jump in split A never yanks split B). `document_id` addresses untitled/open documents; `path` is the reopen fallback. Forward stack deferred until back proves insufficient.
- No server / not ready / no result → status transient, distinguishing "still indexing…" from "no definition found" from "not supported by this server".

### Hover Documentation (Phase 4)

- `Command::ShowHover`, keyboard-invoked (mouse-dwell deferred). The card is the `Anchor::Cursor` surface **from [overlay-surface.md](overlay-surface.md)** — flip/clamp geometry, shell rendering, and severity colors are owned there; this phase supplies content and timing only.
- Content: hover markdown lightly processed to plain text (monospace everywhere anyway); `contentFormat` advertises plaintext preference. The card also shows the diagnostics under the cursor **including `relatedInformation`** (rust-analyzer's "first borrow occurs here" is half the value of the error).
- Dismissed on any keypress/edit/cursor move; open card is `UiState`, not document state.

### Completion (Phase 5)

The largest lift, last on purpose.

- Triggered explicitly (Ctrl+Space) and on server trigger characters while typing; debounced; flush-before-request applies.
- UI: the cursor-anchored list surface from overlay-surface.md. Ordering: server `sortText` first, `nucleo-matcher` fuzzy score as tiebreak; matching runs against `filterText ?? label` — never bare `label` (rust-analyzer labels embed type signatures; matching them ranks visibly wrong).
- `isIncomplete: true` means the item set is **not** a superset for further typing — re-request on the next keystroke instead of filtering locally (ts-ls sets it routinely).
- **Accept = resolve first when the server advertises `resolveProvider`** (ts-ls returns minimal items whose auto-import `additionalTextEdits` only exist after `completionItem/resolve` — skipping resolve silently drops imports), then apply primary `textEdit` + `additionalTextEdits` atomically as **one undo step**. The `textEdit` range is re-anchored to the current cursor if the user typed between response and accept (the type-then-Enter race is one character wide but common). The highlighted item resolves lazily for its docs panel.
- Inserts are plain text by capability (`snippetSupport: false`) — no flattening code.
- Key routing: modals hard-capture *all* keys via an early return in `runtime/input.rs` — the completion popup needs the **opposite** (consume exactly Up/Down/Enter/Esc/Tab, pass everything else to the editor), so it is a dedicated `handle_completion_key` branch in `handle_key` placed after the modal/CSV captures, returning `None` for unconsumed keys. The `KeyContext` flag (named `overlay_routes_keys`, generalized by overlay-surface.md) exists additionally for binding conditions, and requires the usual field + `Condition` variant + serde name + eval arm.
- Popup closes on Escape, cursor-line change, focus loss; document edits invalidate in-flight responses via the revision guard.

---

## Data Structures (sketch)

```rust
// messages.rs
pub enum LspMsg {
    // user intents
    GotoDefinition,
    ShowHover,
    RequestCompletion { explicit: bool },
    NavigateBack,
    RestartServer { server_id: LspServerId },
    // sync lifecycle (internal)
    SyncReady { document_id: DocumentId, revision: u64 },   // debounce/max-wait fired
    // worker → update
    ServerStateChanged { server_id: LspServerId, state: ServerState }, // drives the model mirror
    DiagnosticsPublished { uri: CanonicalUri, version: Option<i64>, diagnostics: Vec<Diagnostic> },
    DefinitionResolved { document_id: DocumentId, revision: u64, locations: Vec<NavLocation> },
    HoverResolved { document_id: DocumentId, revision: u64, content: Option<String> },
    CompletionResolved { document_id: DocumentId, revision: u64, items: Vec<CompletionItem>, is_incomplete: bool },
}

// model — render-only mirror (see Process Model)
pub struct LspUiState { pub servers: HashMap<LspServerId, ServerState> }

// runtime — authoritative, owns non-Clone handles, never in AppModel
struct LspManager {
    servers: HashMap<(LspServerId, PathBuf), ServerHandle>, // worker Sender, child killer, caps, state
    diagnostics: HashMap<CanonicalUri, Vec<Diagnostic>>,    // authoritative store; Document holds projections
    request_deadlines: BinaryHeap<...>,                     // folded into about_to_wait next_wake
    backoff: HashMap<LspServerId, RestartState>,
}
```

`Diagnostic`, `CompletionItem`, `NavLocation` are small `Clone + Debug + Send` structs (`Msg` crosses thread channels). Everything heavyweight stays in the worker.

### Server Registry & Config

`LspServerDef { id, command, args, project_markers }` as a side table (`lsp::lsp_server_def`, keyed by `LanguageId`), not a field on `LanguageDefinition` — the registry table is positional with ~85 call sites, and a side table gets the same "one place to register a server" ergonomics without touching every call site or growing the macro's arity. Initial: `rust-analyzer` (Rust), `typescript-language-server --stdio` (TS/TSX/JS/JSX — one server instance per root for all four), `pyright-langserver --stdio` (Python), `phpantom_lsp` (PHP), `sema lsp` (Sema).

User config — **YAML**, in the existing `~/.config/token-editor/config.yaml`:

```yaml
lsp:
  enabled: true            # master switch
  servers:
    rust-analyzer:
      command: /custom/path/rust-analyzer
    pyright:
      enabled: false
```

Binary not found on `PATH` (after `PATHEXT` resolution on Windows) → `ServerState::Missing`, one-time transient, no error spam.

---

## Interaction with Existing Features

| Feature | Relationship |
| --- | --- |
| **Overlay surfaces** ([overlay-surface.md](overlay-surface.md)) | Owns the completion/hover/action shells (`Anchor::Cursor`), `draw_wavy_underline`, severity glyphs/colors. LSP owns protocol, data, and when surfaces appear. Prerequisite for Phases 4–5 (Milestone 1 vs 4 — ordering already works). |
| **Editor decorations** ([editor-decorations.md](editor-decorations.md)) | Owns gutter lanes, range decorations, overview marks. Diagnostics (Phase 2) is a consumer — possibly the first, or second after find-enhancements. |
| **Syntax-aware expand selection** | Untouched. `SyntaxTreeSnapshot` stays authoritative — synchronous and revision-exact, which `selectionRange` can never be. |
| **Outline panel / syntax highlighting** | Untouched; `documentSymbol` / `semanticTokens` are Future augmentations. |
| **Syntax worker debounce** | Same deadline-map mechanism, separate entry, shared `Arc<str>` snapshot when deadlines coincide. |
| **fs_watcher** | Feeds `workspace/didChangeWatchedFiles` (Phase 6); until then `dynamicRegistration: false` keeps servers from assuming we watch. |
| **File change detection / auto-save** (planned) | External reload = revision bump = normal `didChange`; save emits `didSave`. No special cases. |
| **Snippets** (planned) | `snippetSupport: false` until it exists; then completion advertises it and becomes a snippet-insert client. |
| **Automation/MCP** | All commands invokable by name; snapshots gain server states and per-document diagnostics (new structure — `EditorSnapshot` is currently focused-document-only, and automation lives in the binary crate). |

---

## Implementation Plan

### Phase 1: Transport, Lifecycle, Document Sync — *no user-visible features*

**Effort:** L

- [x] `lsp/transport.rs`: Content-Length framing over child stdio, unit-tested against canned byte streams (partial reads, multiple messages per read, malformed headers).
- [x] Adopt `lsp-types`; `lsp/uri.rs` canonical path↔URI helper with symlink test.
- [x] `lsp/position.rs`: char-col ↔ UTF-16, document-parameterized; CRLF/surrogate/line-end tests.
- [x] `lsp/client.rs` worker: spawn (with `PATHEXT`), handshake with outbound queueing, request-id correlation with abandoned-entry semantics, stderr drain thread, dispatch loop.
- [x] **Client capabilities block** (as specified above) + parse/store `ServerCapabilities`, with gating primitives (`sync_mode`, `supports_definition`/`hover`/`completion`, …); wiring them into actual send paths lands with `didOpen`/`didChange` in the next unit.
- [x] **Server→client request replies** (the table above) + `MethodNotFound` default + ignore-unknown-notifications.
- [x] `LspManager` in runtime; `LspUiState` mirror in model driven by `ServerStateChanged`; `Indexing` from `$/progress`.
- [x] `LspServerDef` via a side table (`lsp::lsp_server_def`) keyed by `LanguageId`, not a field added to the registry macro — same "~85 call sites untouched" outcome without growing the macro's arity; YAML config overrides + master switch; root resolution (workspace → project marker → parent, detached cap).
- [x] Debounced `didChange` with max-wait cap; shared snapshot with syntax parse; flush-before-request plumbing; `didClose` on `release_document_if_unreferenced`; Save As close/open pair; revision-bump-on-every-mutation test.
- [x] Crash backoff + `RestartLanguageServer`; **build the quit-time teardown hook** (none exists) with the shutdown sequence (await shutdown response → exit → wait → kill) and `ShuttingDown` suppressing restart; `processId` in initialize.
- [x] Status bar transient on state changes; automation snapshot exposes server states.
- [x] **Gate:** logs prove a rust-analyzer session stays in sync across an edit-heavy session, including a request issued mid-debounce (flush test), with zero UI change.

### Phase 2: Diagnostics

**Effort:** L (includes decoration-layer work unless find-enhancements built it first)

- [x] Authoritative diagnostics store in `LspManager` keyed by canonical URI (retains unopened-file publishes); `Document.diagnostics` projection refreshed on publish/open; version used for ordering only — **no equality guard**.
- [x] Decoration layer Phases 2 of [editor-decorations.md](editor-decorations.md) (its Phase 1, dynamic gutter width, ships standalone beforehand — see that doc).
- [x] Severity gutter marks + wavy underlines + tag-driven faded/strikethrough, clamped at render.
- [x] `SegmentId::Diagnostics` count segment **and** message-under-cursor in the status bar (the phase is not shippable as marks-only).
- [x] Clear on release / server exit / language change.
- [x] Automation: per-line gutter marks and diagnostics counts queryable (needed *in this phase* for its own tests).

### Phase 3: Go to Definition + Jump History

**Effort:** M

- [x] Request/supersede/abandon plumbing with `$/cancelRequest`; one outstanding request per document; 30s UI-level abandonment folded into `about_to_wait`'s `next_wake`.
- [x] Same-file jump (document-parameterized clamp + `JumpToSymbol` body); cross-file via `LayoutMsg::OpenFileInNewTab` in the same update; image/binary-tab guard.
- [x] Out-of-root files route to the resolving server.
- [x] General jump history (group-tagged global stack, `document_id` + path fallback) + `NavigateBack`; outline/goto-line/file-finder push to it too.
- [x] Palette entries, F12 default, indexing-aware status transients.
- [x] Automation: invoke by name, assert resulting file/cursor.

### Phase 4: Hover

**Effort:** M — depends on overlay-surface.md Phase 5 (`Anchor::Cursor`)

- [x] `ShowHover` on the overlay-surface cursor card; plaintext-preferred content; dismiss rules.
- [x] Diagnostics with `relatedInformation` in the card.

### Phase 5: Completion

**Effort:** L — depends on overlay-surface.md Phase 5

- [ ] Trigger rules, debounce, flush-before-request, revision-guarded responses, `isIncomplete` re-request.
- [ ] Dropdown on the overlay-surface list; `sortText`-primary ordering, `filterText` matching.
- [ ] Resolve-before-accept; `textEdit` + `additionalTextEdits` as one undo step; re-anchoring.
- [ ] `handle_completion_key` input branch + `overlay_routes_keys` context flag.
- [ ] Automation coverage for open→filter→accept including an auto-import case.

### Phase 6+: Future

- [ ] Incremental `didChange` from `EditOperation` deltas (prerequisite for large files).
- [ ] `workspace/didChangeWatchedFiles` from fs_watcher (then advertise its dynamicRegistration).
- [ ] Pull diagnostics (`textDocument/diagnostic`) replacing push.
- [ ] Find references + multi-location picker; rename; code actions; formatting; `willSaveWaitUntil` (format-on-save).
- [ ] Problems panel (new `PanelId`; reads the manager's authoritative store, which already retains unopened-file diagnostics).
- [ ] Semantic tokens over tree-sitter highlights; `documentSymbol` outline augmentation; forward jump stack; mouse-dwell hover; Cmd+Click; workspace symbols in the fuzzy finder; snippet completion (with snippets feature).

---

## Testing Strategy

### Unit

- Framing edge cases; URI canonicalization incl. symlinks; position conversion round trips (ASCII/Unicode/CRLF).
- Sync bookkeeping: bursts → strictly increasing versions; max-wait cap fires under continuous edits; flush-before-request emits `didChange` first; close-after-pending-change sends nothing to a closed doc; every-mutation-bumps-revision assertion.
- Guards: stale feature responses dropped; diagnostics *not* equality-guarded, out-of-order publishes discarded by version; clamping never panics (fuzz stale ranges against random edits).

### Integration: scriptable fake server

A stub binary speaking JSON-RPC, driven by **per-test scenario scripts, not canned fixtures** — the design leans on failure modes a fixture can't produce. Scenarios: full lifecycle; `workspace/configuration` request mid-init (client must reply or the test hangs — by design); never-responds (abandonment); exit mid-request → backoff → restart → documents re-opened; malformed/partial frames; duplicate and unknown response ids; publish for a never-opened URI (retained in store); publish with stale version (dropped); response to a cancelled request (consumed, discarded); stderr flood (no wedge). One `#[ignore]`d test runs real rust-analyzer locally.

Phase 1 implementation: `src/bin/fake_lsp_server.rs` (a `[[bin]]` target, JSON scenario-step interpreter reusing `lsp::transport`), driven from `tests/lsp_fake_server_scenarios.rs` (lifecycle, `workspace/configuration` mid-init, never-responds, exit-mid-request, malformed frames, duplicate/unknown ids, stderr flood, `initialize` error, `#[ignore]`d real rust-analyzer) via `spawn_server` directly, and from a `runtime::app::tests` gate test (`edit_heavy_session_stays_in_sync_with_fake_lsp_server`) that drives the full `App`/`LspManager` path — didOpen, a debounced burst of didChange, a flush-before-request mid-debounce, didSave, didClose, and quit teardown — asserting on a transcript file the fake server writes in receipt order. Publish/cancel-related scenarios (retained-store, stale-version, cancelled-response) are deferred to Phase 2/3 alongside the features that produce that traffic; exit-mid-request → backoff → restart → re-open is exercised at the unit level (`runtime::app::tests`) rather than through the fake server, since the restart path doesn't yet drive real request traffic in Phase 1.

### Manual checklist

- [ ] rust-analyzer on this repo: cold start shows "indexing…", diagnostics appear after break, clear after fix; error in an *unopened* file appears when that file is opened.
- [ ] F12 across files opens a new tab (never clobbers the current one); back returns exactly, per split group.
- [ ] F12 into std lands in the toolchain source without spawning a second server.
- [ ] Hover shows docs + related diagnostic info; Escape/edit dismisses.
- [ ] Completion in TS: accepting an auto-import item inserts the import (resolve path exercised); one undo removes both edits.
- [ ] No server installed → today's editor exactly; kill server externally → editing unaffected, restart recovers, documents re-opened.
- [ ] Quit with servers running exits promptly; `ps` shows no orphans.
- [ ] Multi-cursor editing stays in sync; completion/hover act on the active cursor.

### Performance

- One shared rope snapshot per coincident deadline fire; verify renderer timings unchanged via existing worker profiling.
- Diagnostics publish emits `DamageArea::EditorArea` (sub-Hz event — measured, not assumed, to be invisible; see editor-decorations.md for why finer damage is deliberately not built).

---

## Acceptance Criteria

- With a supported server installed: diagnostics (readable, not just visible), F12 with per-group back stack, hover, completion with working auto-import, for that language.
- Without any server: byte-for-byte today's editor.
- No UI-thread blocking; a hung server degrades to abandoned requests with honest status messages ("indexing…" vs "no result" vs "not supported").
- Every feature response is revision-guarded; diagnostics are version-ordered and render-clamped; no stale result ever moves a cursor or edits text.
- Cross-file navigation never replaces the content of an existing tab.
- Expand selection, outline, and highlighting are unchanged by this feature's presence or absence.
- All new commands palette-visible, rebindable, automation-invokable.

---

## Design Decisions

| Decision | Options | Chosen | Rationale |
| --- | --- | --- | --- |
| Async model | tokio / std threads + mpsc | std threads | Matches syntax worker & PTY; tokio stays confined to MCP |
| Protocol types | hand-rolled / `lsp-types` / tower-lsp | `lsp-types` | Serde-only; hand-roll-then-migrate is the expensive branch; framing stays hand-written |
| First feature | completion / definition / diagnostics | diagnostics | Proves the whole pipeline incl. server-initiated traffic with zero interactive UI |
| Doc sync | incremental / full text | full text first, capped debounce | Torn-state-proof; `EditOperation` deltas make incremental a scheduled Phase 6, not a rewrite |
| Server handles | model / runtime | runtime + message-driven model mirror | Model renders status; runtime owns non-`Debug`-hostile handles; invariant stated to stay honest |
| Diagnostics store | per-Document / manager map + projection | manager map keyed by canonical URI | rust-analyzer publishes workspace-wide; Problems panel comes free |
| Position encoding | negotiate utf-8 / always utf-16 | always utf-16 | 2 of 3 servers are utf-16-only; dual paths double the test matrix for nothing measurable |
| Snippets in completion | flatten `${1:x}` ourselves / advertise `snippetSupport: false` | advertise false | Conforming servers then send plain text; the flattening code never exists |
| Jump history | LSP-owned / general editor feature | general, group-tagged | Users expect one back stack; outline/goto-line/finder push to it too |
| Tree-sitter overlap | replace / augment | tree-sitter stays authoritative | Synchronous, offline, shipped; LSP variants are Future augmentations |

## Open Questions

1. Detached-root cap (4) and project-marker list per server — right defaults? Cheap to tune; revisit after Phase 1 telemetry.
2. Should out-of-root buffers (stdlib/registry) get actual read-only enforcement, or just no-sync? Currently no-sync only; enforcement is a separate feature.

## References

- [LSP 3.17 specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
- [`lsp-types` crate](https://docs.rs/lsp-types)
- rust-analyzer's [`lsp-server`](https://github.com/rust-lang/rust-analyzer/tree/master/lib/lsp-server) — reference minimal stdio transport
- Internal: [overlay-surface.md](overlay-surface.md) (cursor-anchored surfaces, severity conventions — prerequisite for Phases 4–5), [editor-decorations.md](editor-decorations.md) (gutter/decoration contract for Phase 2), [syntax-aware-expand-selection.md](syntax-aware-expand-selection.md) (revision-guard + snapshot patterns), [snippets.md](snippets.md), [diff-gutter.md](diff-gutter.md), `docs/AUTOMATION.md`
