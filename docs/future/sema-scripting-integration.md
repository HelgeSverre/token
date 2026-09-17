# Sema Scripting Integration

Embed the Sema Lisp language into Token Editor to enable user-defined commands, hooks, and automation.

> **Status:** Planning
> **Priority:** P3 (Nice-to-have)
> **Effort:** XL (2+ weeks)
> **Created:** 2026-03-10
> **Updated:** 2026-09-09

## Verified implementation status

This is a proposal, not an implemented feature. Token has no embedded Sema
interpreter, script hooks, or `init.sema` loader. Its Sema support uses a vendored
tree-sitter grammar and the external `sema lsp` process. Editor configuration
remains YAML.

The baseline for this plan is Sema 1.36.0 at commit `3b1e81be`. The public
`sema::Interpreter` wrapper provides synchronous `eval_str`, `load_file`, and
`register_fn`. The lower-level `sema_eval::Interpreter` also provides
`submit_str`, `submit_value`, `drive_turn`, and root cancellation. These APIs
allow a host to advance scripts without waiting for timers or external I/O.
The public wrapper does not expose the host-driving APIs yet; do not implement
the editor loop around blocking `eval_str` calls.

Before Phase 1, choose either a wrapper API extension upstream or an explicit
dependency on the evaluator/runtime crates. Keep interpreter values on their
owning thread. Convert results to editor messages at the host boundary. A
bounded runtime turn is not a wall-clock bound on arbitrary native callbacks
or macro compilation; custom callbacks must not perform blocking work.

`default-features = false` does not remove Sema's LLM, MCP, notebook, LSP, or DAP
dependencies. The current package declares these dependencies unconditionally.
`with_llm(false)` and `with_mcp(false)` disable builtin registration at runtime,
not compilation. Measure the dependency cost or add upstream feature gates
before describing this as a lightweight embedding.

The initial mutation contract is a transaction: a script reads and edits a
working copy of text, cursors, and selections. It must see its own edits. On
success, validate every source document revision and apply one undo group;
on cancellation, failure, or revision mismatch, discard the transaction.
File opening, saving, prompts, and other host effects cross an explicit
asynchronous message boundary. Never hold a model borrow while running Sema.

---

## Table of Contents

1. [Overview](#overview)
2. [Why Sema](#why-sema)
3. [Architecture](#architecture)
4. [Editor API Surface](#editor-api-surface)
5. [Integration Model](#integration-model)
6. [Hook System](#hook-system)
7. [Configuration & Discovery](#configuration--discovery)
8. [Data Structures](#data-structures)
9. [Implementation Plan](#implementation-plan)
10. [Prior Art](#prior-art)
11. [Open Questions](#open-questions)

---

## Overview

### Current State

Token Editor has no scripting or plugin system. Customization is limited to:
- **Keymaps**: YAML key-to-command binding (`~/.config/token-editor/keymap.yaml`)
- **Themes**: YAML color definitions
- **Config**: Simple settings (font size, etc.)
- **Macros** (planned): Record/replay of command sequences -- explicitly not a scripting language

The macros design doc (`docs/feature/macros.md`) lists "complex scripting language (Lua, etc.)" as a non-goal for that feature. This document picks up where macros leave off.

### Goals

- Enable users to write custom commands in Sema that compose editor primitives
- Provide a hook system so scripts can react to editor events (save, open, mode change)
- Allow script-defined commands to appear in the command palette and be bound to keys
- Maintain the Elm Architecture invariant: scripts interact through messages, never direct model mutation
- Sandbox untrusted scripts with Sema's capability system

### Non-Goals

- Replace the keymap system (scripts complement keymaps, not replace them)
- Full IDE plugin API (LSP integration, custom UI panels, syntax definitions)
- Remote/networked plugin protocol (unlike Neovim's msgpack-RPC)
- Multi-language plugin support (Sema only, not Lua/Python/WASM)
- Breaking the single-threaded rendering model

---

## Why Sema

Sema is a Scheme-like Lisp created by the same author as Token Editor. Key advantages:

| Property | Benefit |
|----------|---------|
| Written in Rust | Zero FFI overhead, shares Cargo dependency graph |
| `sema-lang` crate on crates.io | Simple `Cargo.toml` dependency |
| `InterpreterBuilder` API | Fine-grained control over what's enabled |
| `register_fn()` | Trivial to expose Rust functions to scripts |
| Capability-based sandbox | `Caps::FS_WRITE`, `Caps::SHELL`, `Caps::NETWORK` -- deny dangerous operations |
| NaN-boxed `Value` type | Efficient, no allocation for small integers/bools/nil |
| Module system | Scripts can import shared libraries |
| Documented standard library | String manipulation, regex, JSON, file I/O out of the box |
| No async/threading requirement | Single-threaded `Rc`-based -- matches Token's render thread model |

### Embedding Example

```rust
use sema::{InterpreterBuilder, Value, Sandbox, Caps};

let interp = InterpreterBuilder::new()
    .with_llm(false)              // No LLM features in editor context
    .with_sandbox(
        Sandbox::deny(Caps::SHELL)  // No shell commands
            .with_more_denied(Caps::NETWORK.union(Caps::FS_WRITE).union(Caps::PROCESS))
    )
    .with_mcp(false)
    .build();

// Expose an editor primitive
interp.register_fn("editor/insert-text", |args: &[Value]| {
    if args.len() != 1 {
        return Err(sema::SemaError::arity("editor/insert-text", "1", args.len()));
    }
    let text = args[0].as_str()
        .ok_or_else(|| sema::SemaError::type_error("string", args[0].type_name()))?;
    let _ = text; // The implementation edits the script transaction here.
    // Queue a Msg::Document(DocumentMsg::InsertString(text.into()))
    Ok(Value::nil())
});

// Run user script
interp.eval_str(r#"
  (define (surround-with open close)
    (let ((sel (editor/get-selection)))
      (editor/replace-selection
        (string-append open sel close))))
"#)?;
```

This example demonstrates registration and definition only. User command
execution must use the bounded host-driving API described above.

### Comparison with Alternatives

| | Sema | Lua (mlua) | Steel (Helix) | WASM |
|---|---|---|---|---|
| Language match | Perfect (same author) | Good | Good (Scheme in Rust) | Language-agnostic |
| Integration effort | Low (Rust native) | Medium (C FFI) | Medium (separate crate) | High (runtime + ABI) |
| Sandbox | Built-in capabilities | Manual | Limited | Memory-safe by design |
| Performance | Bytecode VM available | LuaJIT is faster | Comparable | Near-native |
| Ecosystem | Small (new language) | Massive | Small | Growing |
| User learning curve | Lisp syntax barrier | Low (familiar) | Lisp syntax barrier | Varies |

The ecosystem/learning curve trade-off is real. Sema's Lisp syntax will limit adoption compared to Lua. However, the integration quality and dogfooding value outweigh this for an editor that is itself a personal project.

---

## Architecture

### Integration Points

```
                    ┌─────────────────────┐
                    │   User Input        │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │   Keymap            │──── Command::RunScript("name")
                    └──────────┬──────────┘
                               │
              ┌────────────────▼────────────────┐
              │         update(model, msg)       │
              │  ┌──────────────────────────┐    │
              │  │ Msg::Script(ScriptMsg)   │────┼──► ScriptEngine
              │  └──────────────────────────┘    │      │
              │                                  │      │ reads model state
              │  Pre/post hooks fire here ◄──────┼──────┘ dispatches Msg
              └────────────────┬────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │   Cmd (side effects) │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │   Renderer          │
                    └─────────────────────┘
```

### Module Structure

```
src/
├── scripting/
│   ├── mod.rs           # Public exports, ScriptEngine struct
│   ├── engine.rs        # Sema interpreter lifecycle, script loading
│   ├── api.rs           # Editor API functions registered into Sema
│   ├── hooks.rs         # Hook registry and dispatch
│   ├── command.rs       # Script-to-Command bridge, palette integration
│   └── types.rs         # ScriptId, ScriptMsg, HookPoint, ScriptError
```

### Message Flow

**Script invoked via keybinding or command palette:**

1. User presses bound key or selects command from palette
2. `Command::RunScript(ScriptId)` dispatched
3. `Command::to_msgs()` produces `Msg::Script(ScriptMsg::Execute(ScriptId))`
4. `update_script()` handler runs:
   a. Captures an owned `ScriptTransaction` with document revision, text, cursors, and selections
   b. Submits the script to the cooperative evaluator through a runtime command
   c. Runtime polls bounded turns and wakes again when asynchronous work is ready
   d. Editor API calls read and update transaction-local state
   e. Completion returns a revision-checked commit message; failure or cancellation discards changes
5. A successful transaction commits through `update()` as one undo group; `Cmd::Batch` groups resulting effects
6. Renderer displays results

**Hook-triggered script:**

1. Normal `update()` processes a message (e.g., `AppMsg::FileSaved`)
2. Post-update, a runtime command submits matching hooks with owned event data
3. Hook callbacks use the same cooperative evaluator and transaction protocol
4. Each successful transaction is revision-checked before commit

---

## Editor API Surface

Proposed functions exposed to Sema scripts, organized by namespace. Reads and writes
use transaction-local state with read-your-writes behavior. They never mutate
`AppModel` directly; completion produces a validated commit message.

### Buffer Operations (`editor/`)

| Sema Function | Returns | Effect |
|---------------|---------|--------|
| `(editor/get-text)` | string | Full buffer text (focused document) |
| `(editor/get-line n)` | string | Text of line n (0-indexed) |
| `(editor/get-line-count)` | int | Number of lines |
| `(editor/get-selection)` | string or nil | Selected text (primary cursor) |
| `(editor/get-selections)` | list of strings | All cursor selections |
| `(editor/get-cursor)` | map `{:line n :column m}` | Primary cursor position |
| `(editor/get-cursors)` | list of maps | All cursor positions |
| `(editor/get-file-path)` | string or nil | Current file path |
| `(editor/get-language)` | string or nil | Detected language |

### Multi-Buffer Operations (`buffer/`)

Scripts can operate on any open document via integer `DocumentId` handles (modeled after Neovim's buffer handles). Functions without a `doc-id` argument operate on the focused document.

| Sema Function | Returns | Effect |
|---------------|---------|--------|
| `(buffer/list)` | list of maps | All open buffers: `({:id 1 :path "..." :language "rust" :modified? true} ...)` |
| `(buffer/current)` | int | Focused document's `DocumentId` |
| `(buffer/get-text doc-id)` | string | Full text of any open buffer |
| `(buffer/get-line doc-id n)` | string | Line n of any open buffer |
| `(buffer/get-line-count doc-id)` | int | Line count of any open buffer |
| `(buffer/get-path doc-id)` | string or nil | File path of any open buffer |
| `(buffer/get-language doc-id)` | string or nil | Language of any open buffer |
| `(buffer/find-by-path path)` | int or nil | Find open buffer by file path |

### Editing Actions (`editor/`)

| Sema Function | Effect |
|---------------|--------|
| `(editor/insert-text str)` | Insert at cursor(s) |
| `(editor/replace-selection str)` | Replace selected text |
| `(editor/delete-selection)` | Delete selected text |
| `(editor/set-cursor line col)` | Move primary cursor |
| `(editor/select-range start-line start-col end-line end-col)` | Set selection |
| `(editor/select-all)` | Select entire buffer |
| `(editor/with-undo-group body)` | Execute body; all edits become a single undo step |

### Navigation (`editor/`)

| Sema Function | Effect |
|---------------|--------|
| `(editor/goto-line n)` | Jump to line |
| `(editor/move-cursor direction)` | Move cursor (`:up`, `:down`, `:left`, `:right`) |
| `(editor/scroll-to-line n)` | Scroll viewport |

### Layout Operations (`layout/`)

Maps directly to existing `LayoutMsg` variants. All operations queue messages.

| Sema Function | Effect |
|---------------|--------|
| `(layout/split direction)` | Split focused group (`:horizontal` or `:vertical`) |
| `(layout/split-group group-id direction)` | Split a specific group |
| `(layout/close-group group-id)` | Close a group and all its tabs |
| `(layout/close-focused-group)` | Close the focused group |
| `(layout/focus-group group-id)` | Focus a specific group |
| `(layout/focus-next-group)` | Cycle focus to next group |
| `(layout/focus-prev-group)` | Cycle focus to previous group |
| `(layout/new-tab)` | Create new untitled tab in focused group |
| `(layout/close-tab)` | Close active tab in focused group |
| `(layout/next-tab)` | Switch to next tab |
| `(layout/prev-tab)` | Switch to previous tab |
| `(layout/switch-to-tab n)` | Switch to tab by index (0-indexed) |
| `(layout/open-file-in-tab path)` | Open file in new tab |
| `(layout/move-tab tab-id group-id)` | Move a tab to a different group |

### File Operations (`editor/`)

| Sema Function | Effect |
|---------------|--------|
| `(editor/save)` | Save current file |
| `(editor/open-file path)` | Open file in new tab |
| `(editor/new-file)` | Create empty buffer |

### UI (`editor/`)

| Sema Function | Effect |
|---------------|--------|
| `(editor/show-message str)` | Display in status bar |
| `(editor/prompt label callback)` | Show input prompt, call back with result |

### Configuration (`editor/`)

| Sema Function | Effect |
|---------------|--------|
| `(editor/set-option key value)` | Set an editor config option at runtime |
| `(editor/get-option key)` | Read an editor config option |

### Workspace (`workspace/`)

| Sema Function | Returns |
|---------------|---------|
| `(workspace/root)` | string or nil |
| `(workspace/files)` | list of relative paths |

### Utility (`token/`)

| Sema Function | Returns |
|---------------|---------|
| `(token/version)` | string |
| `(token/config key)` | value or nil |
| `(token/register-command name label fn)` | nil (registers in palette) |
| `(token/log message)` | nil (debug log) |

---

## Integration Model

Three complementary ways scripts interact with the editor:

### 1. Command Scripts

A script defines a named command that can be bound to a key or invoked from the command palette.

**User defines in `~/.config/token-editor/scripts/surround.sema`:**

```scheme
;; metadata
(token/register-command
  "surround-parens"
  "Surround Selection with Parentheses"
  (lambda ()
    (let ((sel (editor/get-selection)))
      (when sel
        (editor/replace-selection
          (string/concat "(" sel ")"))))))

(token/register-command
  "surround-brackets"
  "Surround Selection with Brackets"
  (lambda ()
    (let ((sel (editor/get-selection)))
      (when sel
        (editor/replace-selection
          (string/concat "[" sel "]"))))))
```

**User binds in `keymap.yaml`:**

```yaml
- key: "cmd+shift+9"
  command: Script
  args: "surround-parens"
  when: ["has_selection"]
```

### 2. Hook Scripts

Scripts register callbacks for editor lifecycle events.

**`~/.config/token-editor/scripts/auto-trim.sema`:**

```scheme
(token/on-hook :before-save
  (lambda (event)
    ;; Trim trailing whitespace on save
    (let ((text (editor/get-text)))
      (let ((trimmed (string/join "\n"
              (map string/trim-end
                   (string/split text "\n")))))
        (editor/select-all)
        (editor/replace-selection trimmed)))))
```

### 3. Init Script

A single `init.sema` runs at startup for global configuration.

**`~/.config/token-editor/init.sema`:**

```scheme
;; Load user script modules
(import "~/.config/token-editor/scripts/surround.sema")
(import "~/.config/token-editor/scripts/auto-trim.sema")

;; Set editor preferences programmatically
(token/log "Sema scripting initialized")
```

### Execution Model

```
                    ┌─────────────────────┐
  Startup ─────────► Load init.sema       │
                    │  ├─ imports          │
                    │  ├─ register-command │
                    │  └─ on-hook          │
                    └─────────────────────┘

                    ┌─────────────────────┐
  Key/Palette ─────► Execute command fn   │
                    │  ├─ read state       │
                    │  ├─ edit transaction │
                    │  └─ suspend / finish │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │ Validate and commit │
                    └─────────────────────┘

                    ┌─────────────────────┐
  Editor event ────► Fire hook callbacks  │
                    │  (same as above)    │
                    └─────────────────────┘
```

**Key constraint:** Advance the runtime in bounded turns. Scripts edit an owned transaction, never the live model. Host effects and transaction commits pass through the update loop. A suspended script retains no model borrows.

**Timeout:** The proposed `scripting.timeout_ms` deadline requests root cancellation. Discard the transaction and settle resource cleanup before reporting timeout. The host must separately bound compilation and avoid blocking native callbacks; a timer alone cannot interrupt those operations.

---

## Hook System

### Available Hook Points

Derived from the message categories in `src/messages.rs` and common patterns across editors:

| Hook | Fires When | Event Data Passed |
|------|-----------|-------------------|
| `:after-init` | Editor startup complete | `{:version "..."}` |
| `:before-save` | Before file write | `{:path "..." :language "rust" :document-id 1}` |
| `:after-save` | After file write | `{:path "..." :language "rust" :document-id 1}` |
| `:after-open` | File opened in editor | `{:path "..." :language "rust" :document-id 1}` |
| `:before-close` | Tab about to close | `{:path "..." :modified? true :document-id 1}` |
| `:after-close` | Tab closed | `{:path "..."}` |
| `:on-focus` | Editor group gains focus | `{:group-id 2 :path "..." :document-id 1}` |
| `:on-language-change` | Language detection changes | `{:path "..." :language "rust" :previous-language "text"}` |
| `:on-cursor-idle` | Cursor hasn't moved for 1s | `{:line 42 :column 10 :document-id 1}` |

All hook callbacks receive a single `event` map argument containing the context data. This avoids the problem of callbacks needing to query state themselves (which is racy -- focus may have changed between the event and the callback).

### Hook Registration

```scheme
;; Simple hook -- callback receives event map
(token/on-hook :after-save
  (lambda (event)
    (token/log (string/concat "Saved: " (get event :path)))))

;; Filtered hook -- optional pattern restricts by file glob
(token/on-hook :after-open {:pattern "*.rs"}
  (lambda (event)
    (token/log "Rust file opened")))

;; Multiple patterns
(token/on-hook :before-save {:pattern "*.{rs,toml}"}
  (lambda (event)
    (token/log "Rust project file saving")))

;; Remove a hook
(define my-hook (lambda (event) ...))
(token/on-hook :before-save my-hook)
(token/remove-hook :before-save my-hook)
```

The `{:pattern glob}` filter is matched against the file path from the event data. If the event has no `:path` (e.g., `:after-init`), the filter is ignored and the hook always fires. This follows Neovim's autocommand pattern filtering model.

### Hook Implementation

Hooks are lists of `(callback, filter)` pairs stored in a `HashMap<HookPoint, Vec<HookEntry>>` on the `ScriptEngine`. When fired:

1. Build the event data map from the current `AppModel` state at the call site
2. For each registered callback, check if the optional pattern filter matches the event's `:path`
3. Call matching callbacks in registration order, passing the event map as the sole argument
4. If a `:before-*` hook raises an error, the operation is cancelled and the error displayed in the status bar

---

## Configuration & Discovery

### Directory Layout

```
~/.config/token-editor/
├── config.yaml          # Editor config (existing)
├── keymap.yaml          # Keybindings (existing)
├── init.sema            # Startup script (new)
└── scripts/             # User scripts (new)
    ├── surround.sema
    ├── auto-trim.sema
    └── my-utils.sema
```

### Script Discovery

At startup, Token:
1. Checks for `~/.config/token-editor/init.sema`
2. If found, creates a `ScriptEngine` with sandboxed `Interpreter`
3. Registers all `editor/*`, `workspace/*`, `token/*` API functions
4. Evaluates `init.sema` in the global environment
5. `init.sema` imports scripts, registers commands and hooks
6. Registered commands are added to the command palette

Scripts are NOT auto-discovered from the `scripts/` directory. The user explicitly imports what they want in `init.sema`. This follows Neovim's model: `init.lua` is the entry point, not automatic directory scanning.

### Sandbox Configuration

By default, scripts run with restricted capabilities:

| Capability | Default | Rationale |
|-----------|---------|-----------|
| `FS_READ` | Allowed (scoped to workspace) | Scripts need to read project files |
| `FS_WRITE` | Denied | Prevent accidental file corruption |
| `SHELL` | Denied | Prevent command injection |
| `NETWORK` | Denied | Prevent data exfiltration |
| `ENV_READ` | Allowed | Scripts may need env vars |
| `LLM` | Denied | Not relevant for editing |

Users can relax restrictions in `config.yaml`:

```yaml
scripting:
  enabled: true
  sandbox:
    allow_fs_write: true    # Trust my scripts to write files
    allow_shell: false
    allow_network: false
  timeout_ms: 200           # Max script execution time
```

---

## Data Structures

### ScriptEngine

```rust
pub struct ScriptEngine {
    interpreter: ScriptInterpreter, // Host-driving wrapper selected in Phase 1
    commands: HashMap<String, ScriptCommand>,
    hooks: HashMap<HookPoint, Vec<HookEntry>>,
    transactions: HashMap<DocumentId, ScriptTransaction>,
    pending_effects: Vec<Msg>,
}

pub struct HookEntry {
    pub callback: sema::Value,       // Sema lambda (receives event map)
    pub pattern: Option<String>,     // Optional glob filter on file path
}

pub struct ScriptCommand {
    pub id: String,
    pub label: String,
    pub callback: sema::Value,  // Sema lambda
}
```

### ScriptMsg

```rust
#[derive(Debug, Clone)]
pub enum ScriptMsg {
    /// Execute a named script command
    Execute(String),
    /// Fire hooks for an event
    FireHook(HookPoint),
    /// Script produced an error
    Error(String),
}
```

### HookPoint

```rust
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum HookPoint {
    AfterInit,
    BeforeSave,
    AfterSave,
    AfterOpen,
    BeforeClose,
    AfterClose,
    OnFocus,
    OnLanguageChange,
    OnCursorIdle,
}
```

### ScriptTransaction (owned working state)

```rust
/// No model borrow survives a script turn or suspension.
pub struct ScriptTransaction {
    pub document_id: DocumentId,
    pub source_revision: u64,
    pub buffer_text: ropey::Rope,
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub file_path: Option<PathBuf>,
    pub language: LanguageId,
    pub workspace_root: Option<PathBuf>,
}
```

---

## Implementation Plan

### Phase 1: Core Engine (Foundation)

**Effort:** L (1-2 weeks)

- [ ] Select the public wrapper extension or lower-level runtime embedding; measure build cost (disabling default features does not exclude LLM dependencies)
- [ ] Create `src/scripting/mod.rs` with `ScriptEngine` struct
- [ ] Create `src/scripting/engine.rs`: interpreter init, script loading, eval
- [ ] Create `src/scripting/types.rs`: `ScriptMsg`, `HookPoint`, `HookEntry`, `ScriptCommand`
- [ ] Add `Msg::Script(ScriptMsg)` variant to `messages.rs`
- [ ] Add `update_script()` handler in `src/update/mod.rs`
- [ ] Use the existing `Cmd::Batch` and explicit script host-effect commands
- [ ] Implement a working-copy transaction for text, cursor and selection edits; reads must observe earlier edits
- [ ] Implement undo grouping: mark undo stack position before script execution, collapse all new `EditOperation` entries into a single `EditOperation::Batch` after script completes (uses existing `Batch` variant in `document.rs`)
- [ ] Expose or wrap `submit_str`, bounded `drive_turn`, completion wakeups and cancellation; verify timeouts without blocking the render thread
- [ ] Test commit, rollback, undo grouping, stale document revisions and cancellation of a suspended script
- [ ] Load `init.sema` at startup in `runtime/app.rs`
- [ ] Unit tests for engine lifecycle

### Phase 2: Editor API

**Effort:** M (3-5 days)

- [ ] Create `src/scripting/api.rs` with all `editor/*` functions
- [ ] Implement read-only functions: `get-text`, `get-line`, `get-cursor`, `get-selection`, `get-file-path`
- [ ] Implement action functions: `insert-text`, `replace-selection`, `delete-selection`
- [ ] Implement `editor/with-undo-group` (wraps body in single undo step)
- [ ] Implement navigation: `goto-line`, `set-cursor`
- [ ] Implement file ops: `save`, `open-file`, `new-file`
- [ ] Implement UI: `show-message`
- [ ] Implement `token/register-command` and `token/log`
- [ ] Implement `buffer/*` multi-buffer read functions: `buffer/list`, `buffer/current`, `buffer/get-text`, `buffer/get-line`, `buffer/get-line-count`, `buffer/get-path`, `buffer/get-language`, `buffer/find-by-path` (all read-only, wire to `editor_area.documents` HashMap via `DocumentId` integer handles)
- [ ] Implement `layout/*` functions: `layout/split`, `layout/close-group`, `layout/focus-group`, `layout/focus-next-group`, `layout/focus-prev-group`, `layout/new-tab`, `layout/close-tab`, `layout/next-tab`, `layout/prev-tab`, `layout/switch-to-tab`, `layout/open-file-in-tab`, `layout/move-tab` (all map directly to existing `LayoutMsg` variants -- mechanical wiring)
- [ ] Implement `editor/set-option` and `editor/get-option` for runtime config
- [ ] Tests for each API function

### Phase 3: Keymap & Command Palette Integration

**Effort:** S (1-2 days)

- [ ] Add `Command::RunScript(String)` variant to `src/keymap/command.rs`
- [ ] Add `"Script"` to `Command::from_str` in keymap config parser
- [ ] Make command palette registry dynamic (replace static `COMMANDS` slice)
- [ ] Script-registered commands appear in palette with `[Script]` prefix
- [ ] Add `CommandId::Script(String)` or parallel dynamic registry

### Phase 4: Hook System

**Effort:** M (3-5 days)

- [ ] Create `src/scripting/hooks.rs`
- [ ] Implement `token/on-hook` and `token/remove-hook` API functions
- [ ] Support optional `{:pattern glob}` filter argument for file-path-based filtering
- [ ] Build event data maps at each hook call site from current `AppModel` state
- [ ] Pass event map as sole argument to hook callbacks
- [ ] Add hook dispatch calls in `update/` handlers:
  - `update_app.rs`: `BeforeSave`/`AfterSave` around file save
  - `update_app.rs`: `AfterOpen` when file loaded
  - `update_layout.rs`: `BeforeClose`/`AfterClose` on tab close
  - `update_layout.rs`: `OnFocus` on group focus change
- [ ] Implement `:before-*` cancellation (hook returns error = cancel operation)
- [ ] Timeout enforcement using the configured `scripting.timeout-ms` budget
- [ ] Tests for hook registration, pattern filtering, event data passing, and dispatch

### Phase 5: Polish & Safety

**Effort:** M (3-5 days)

- [ ] Sandbox configuration from `config.yaml`
- [ ] Script error display in status bar
- [ ] Script reload command (`token/reload-scripts`)
- [ ] Graceful handling of missing `init.sema` (no error, just skip)
- [ ] Performance: ensure script overhead is <1ms when no scripts loaded
- [ ] Documentation: user guide for writing scripts
- [ ] Integration tests: end-to-end script execution

### Future Phases (Deferred)

- [ ] `editor/prompt` with async callback (requires modal input integration)
- [ ] Script-defined syntax highlighting rules
- [ ] Script access to Tree-sitter AST
- [ ] Background script execution (long-running tasks on worker thread via `msg_tx`)
- [ ] Script package manager / sharing mechanism
- [ ] Hot-reload scripts on file change (filesystem watcher)
- [ ] REPL / eval-expression command (like Emacs M-:)
- [ ] Script-defined status bar segments

---

## Prior Art

### Emacs (Emacs Lisp)

The gold standard for editor scripting. Key lessons:

- **Everything is a Lisp object**: buffers, windows, keymaps, modes. This makes the language incredibly powerful but tightly coupled.
- **Hooks are just lists of functions**: `(add-hook 'before-save-hook #'my-fn)`. Simple, composable, no framework overhead.
- **Interactive commands**: Functions declare how to obtain arguments when invoked interactively. We achieve similar with `token/register-command` providing a label for the palette.
- **`eval-expression` (M-:)**: Run arbitrary Lisp at any time. Extremely powerful for debugging and experimentation. Worth adding as a future command.
- **The advice system**: Wrap any function with before/after/around advice. We don't need this -- our hook system is sufficient for a focused API.

**What to adopt:** Hook-as-list-of-functions pattern, eval-expression command (future).
**What to skip:** Making everything a Lisp object (too much API surface), advice system (over-engineering).

### Neovim (Lua)

The modern standard for pragmatic editor scripting.

- **`vim.api.*` namespace**: Clean C-to-Lua bridge. All operations are function calls, not imperative mutations. Our `editor/*` namespace follows this.
- **Two-phase loading**: `plugin/` (eager, minimal setup) vs `lua/` (lazy, on `require()`). We adopt this with `init.sema` (eager) and `scripts/` (imported on demand).
- **Autocommands**: Events + pattern matching + callbacks. Our hook system is simpler (no pattern matching on hook names) but sufficient.
- **Buffer handles**: Opaque integer IDs, not direct references. We don't need this yet (single-buffer API), but worth considering for multi-buffer scripts.

**What to adopt:** Namespace-based API (`editor/*`), explicit init file, lazy loading via imports.
**What to skip:** Msgpack-RPC remote plugins, buffer handles (premature for our scope).

### Helix (Steel -- Scheme in Rust)

The closest analog to our integration. Steel is a Scheme dialect embedded in a Rust editor.

- **`PluginSystem` trait**: Abstracts the scripting engine behind a trait. Allows swapping implementations. We should consider this for testability.
- **Two-file config**: `helix.scm` (define commands, no editor access) + `init.scm` (has editor context, sets up hooks). Our `init.sema` combines both roles -- simpler.
- **Context passing**: A `Context` object wraps editor state and is passed to every scripting call. Token's proposed `ScriptTransaction` instead owns a revision-stamped copy and commits through messages.
- **Thread-local context**: Helix stores context in thread-local storage. We can do the same since scripts run on the main thread.

**What to adopt:** Trait-based engine abstraction (for testing), context pattern.
**What to skip:** Two-file split (unnecessary complexity), direct state mutation.

### Kakoune (Shell Scripting)

- **Environment variable exposure**: `$kak_bufname`, `$kak_selection`, etc. Interesting for its simplicity but too limited for real scripting.
- **`%sh{}` inline shell**: Execute shell commands and use output. We explicitly deny shell access by default for security.
- **Comprehensive hook system**: 30+ hook types with regex filtering. Our hook set is smaller but covers the essential cases.

**What to adopt:** Nothing directly, but the hook categorization is useful reference.
**What to skip:** Shell-based approach entirely.

---

## Open Questions

### Design Decisions to Resolve

1. **Synchronous vs async script execution?**
   Use the cooperative runtime from the start. Choose an owning thread and connect bounded turns, wakeups, deadlines and cancellation to Token. Expose these through the public wrapper or use the lower-level interpreter explicitly.

2. **Should scripts see intermediate state changes?**
   Yes. Reads use the transaction's current text, cursors and selections. Commit only if all source revisions still match; otherwise report a conflict and discard the transaction.

3. **How to handle multi-cursor in the API?**
   Options: (a) API always operates on primary cursor, multi-cursor is implicit; (b) API exposes cursor index parameter; (c) API auto-applies to all cursors.

   **Recommendation:** (a) for simplicity. `editor/get-cursor` returns primary, `editor/get-cursors` returns all. Editing operations apply to all cursors (matching current editor behavior).

4. **Script error UX?**
   Options: (a) status bar message; (b) modal error dialog; (c) dedicated script output panel.

   **Recommendation:** (a) for MVP. Status bar shows `[Script Error] message` for 5 seconds. Add `token/log` output panel later.

5. **Should Sema's file I/O functions be available?**
   Sema's stdlib includes `file/read`, `file/write`, etc. These bypass the editor's file handling.

   **Recommendation:** Allow `file/read` scoped to workspace root. Deny `file/write` by default (use `editor/save` instead). Configurable via sandbox settings.

6. **Startup performance with no scripts?**
   Creating a Sema interpreter has a cost (stdlib registration). If no `init.sema` exists, we should skip interpreter creation entirely.

   **Recommendation:** Lazy initialization. Check for `init.sema` existence first. No file = no interpreter = zero overhead.

### Future Considerations

- **Script debugging**: Sema has a DAP (Debug Adapter Protocol) implementation. Could we expose a debug port for script development?
- **Script sharing**: A community repository of Token scripts (like Neovim's plugin ecosystem). Premature but worth designing for.
- **Bytecode compilation**: Sema supports ahead-of-time compilation to `.semac` files. Could speed up startup for complex init scripts.

---

## References

### Internal Docs

- [Macros design](macros.md) -- command recording/replay, complements scripting
- [Keymapping](../archived/KEYMAPPING_IMPLEMENTATION_PLAN.md) -- keymap architecture
- [Editor UI Reference](../EDITOR_UI_REFERENCE.md) -- UI component inventory

### External Resources

- [Sema Language](https://sema-lang.com/docs/) -- language documentation
- [sema-lang crate](https://crates.io/crates/sema-lang) -- Rust embedding API
- [Emacs Lisp Manual: Writing Primitives](https://www.gnu.org/software/emacs/manual/html_node/elisp/Writing-Emacs-Primitives.html)
- [Neovim Lua API](https://neovim.io/doc/user/lua.html)
- [Helix Plugin System PR #8675](https://github.com/helix-editor/helix/pull/8675)
- [Steel Scheme](https://github.com/mattwparas/steel) -- Helix's embedded Scheme
- [Kakoune Hooks](https://github.com/mawww/kakoune/blob/master/doc/pages/hooks.asciidoc)

---

## Appendix

### Design Decisions

| Decision | Options Considered | Chosen | Rationale |
|----------|-------------------|--------|-----------|
| Scripting language | Sema, Lua, Steel, WASM | Sema | Same author, native Rust, built-in sandbox |
| Execution model | Blocking calls, cooperative host turns, dedicated owner thread | Cooperative runtime | Rendering continues while scripts await I/O |
| State access | Direct mutation, queued-only snapshot, working transaction | Working transaction + commit message | Read-your-writes, rollback and one undo group |
| Script discovery | Auto-scan dir, manifest file, init script | Init script imports | Explicit, predictable, follows Neovim model |
| Hook model | Trait objects, closures, Sema lambdas | Sema lambdas in Vec | Simple, matches Emacs hook-as-list pattern |
| Sandbox default | Permissive, restrictive | Restrictive | Security by default, opt-in to capabilities |

### Example: Complete Script

```scheme
;; ~/.config/token-editor/scripts/rust-helpers.sema

;; Auto-format on save for Rust files (uses hook pattern filtering)
(token/on-hook :before-save {:pattern "*.rs"}
  (lambda (event)
    (token/log (string/concat "Rust file saving: " (get event :path)))))

;; Set Rust-specific editor options when a Rust file is opened
(token/on-hook :after-open {:pattern "*.rs"}
  (lambda (event)
    (editor/set-option "indent_width" 4)
    (editor/set-option "tab_style" "spaces")))

;; Wrap selection in dbg!() -- uses undo grouping
(token/register-command
  "rust-dbg-wrap"
  "Rust: Wrap in dbg!()"
  (lambda ()
    (let ((sel (editor/get-selection)))
      (when sel
        (editor/with-undo-group
          (editor/replace-selection
            (string/concat "dbg!(" sel ")")))))))

;; Open the same file in a side-by-side split (uses layout API)
(token/register-command
  "split-and-mirror"
  "Split: Mirror Current File"
  (lambda ()
    (let ((path (editor/get-file-path)))
      (when path
        (layout/split :vertical)
        (layout/open-file-in-tab path)))))

;; List all modified buffers (uses multi-buffer API)
(token/register-command
  "list-modified"
  "Buffers: Show Modified"
  (lambda ()
    (let ((modified (filter
                      (lambda (buf) (get buf :modified?))
                      (buffer/list))))
      (editor/show-message
        (if (null? modified)
          "No modified buffers"
          (string/concat
            (length modified) " modified: "
            (string/join ", "
              (map (lambda (buf) (or (get buf :path) "untitled"))
                   modified))))))))
```

---

## Known Limitations & Gaps

Compared to scripting systems in Emacs, Neovim, Helix, and Kakoune, this plan has notable gaps. Some are intentional scope limits; others are constraints of the Sema language that would need upstream work. This section documents both so they can inform prioritization.

### Plan Gaps (vs Other Editors)

#### Addressed in This Plan

The following gaps were identified during review and have been incorporated into the implementation phases:

- **Undo grouping** -- Added `editor/with-undo-group` to API surface; undo stack collapsing added to Phase 1 engine and Phase 2 API. Uses existing `EditOperation::Batch` variant.
- **Event data in hooks** -- Hooks now receive an event map argument with context (path, language, document-id, etc.). Added to Phase 4 with event map construction at each call site.
- **Hook pattern filtering** -- Optional `{:pattern glob}` filter added to `token/on-hook`. Added to Phase 4.
- **Multi-buffer API** -- `buffer/*` namespace added with `DocumentId` integer handles. Read-only access to any open buffer. Added to Phase 2.
- **Layout/window API** -- `layout/*` namespace added, mapping directly to existing `LayoutMsg` variants (split, close, focus, tabs). Added to Phase 2.
- **Filetype-specific config** -- `editor/set-option` and `editor/get-option` added. Combined with `:on-language-change` hook, scripts can set per-language settings. Added to Phase 2.
- **Timeout/interruption** -- use the current cooperative runtime and root cancellation. The wrapper still needs a host-driving API. Instruction budgets do not bound a blocking native callback or compilation; test these separately.

#### Remaining: Should Address Early

**Read-your-writes is required.** A queued-only snapshot would return old text after `editor/insert-text`. The transaction specified above includes text, cursors and selections together. Test sequential edits and reads, rollback on error, and rejection if the live document changes during suspension.

#### Remaining: Nice to Have (Can Defer)

**No custom UI / virtual text.** Neovim has floating windows, extmarks (virtual text, inline diagnostics), signs. Emacs has overlays and text properties. Our plan offers `editor/show-message` (status bar) and a deferred `editor/prompt`. Scripts cannot render inline annotations, diagnostic markers, code lenses, or any visual feedback beyond a status bar string.

**Process management needs host policy.** Sema provides cooperative `proc/*`, stream, task and channel operations. Token must drive the runtime, handle completion wakeups, and cancel resources on script shutdown. Deny process/shell capabilities by default; do not implement custom blocking callbacks on the render thread.

**No interactive input beyond prompt.** Emacs has `completing-read` (fuzzy completion list), `y-or-n-p`, `read-char`. Neovim has `vim.ui.select` (picker), `vim.ui.input`. Our plan has only a deferred `editor/prompt` with no completion, no picker, no confirmation dialog. Scripts that need user choices have no mechanism.

---

### Sema Language Gaps

These are constraints of the Sema language itself that limit what the scripting integration can offer. Some may warrant upstream changes to Sema. Specific upstream changes are documented in `sema-lisp/docs/plans/2026-03-11-embedding-api-improvements.md`.

#### Blockers for Future Features

**Thread ownership remains explicit.** Sema values use `Rc` and are `!Send`. Keep each interpreter and its values on one thread. This does not prohibit background execution: a dedicated thread can create and own an interpreter and exchange plain host messages. A render-thread host can instead use bounded runtime turns.

**Suspension is available.** Tasks, promises and channels support suspended operations. A prompt can be an explicit host request whose result resumes a task. General first-class continuations are not required for that design. The host must discard or cancel prompts when their script/root ends.

**Event-loop integration needs a Token adapter.** The lower-level interpreter exposes submission and bounded `drive_turn`. Connect completion wakeups and timer deadlines to winit, process results as messages, and keep drawing between turns. The public wrapper does not yet expose this interface; synchronous `eval_str` is not a substitute.

#### Development Friction

**No automatic Rust struct <-> Sema value mapping.** Every API function must manually construct `Value::map(...)` for return values and manually extract fields from argument maps. Steel (Helix's Scheme) has `#[steel_derive]` proc macros that auto-generate conversions. With 20+ API functions each returning cursor/selection/position maps, the boilerplate adds up quickly. A derive macro or conversion trait in Sema would significantly reduce the API module's size and error surface. Upstream plan: `sema-lisp/docs/plans/2026-03-11-embedding-api-improvements.md` Section 3.

**No typed function registration.** `register_fn` takes `fn(&[Value]) -> Result<Value>`. Every function must manually `check_arity!`, call `.as_str()`, `.as_int()`, handle `None`. There is no `register_fn_typed::<(String, i64), Value>(...)` that auto-extracts and validates arguments. This makes the API module tedious to write and every function a potential source of type-check bugs. Upstream plan: `sema-lisp/docs/plans/2026-03-11-embedding-api-improvements.md` Section 2.

**No dynamic library / native plugin loading.** Steel can load `.dylib` files over a stable ABI, letting plugins include Rust code loaded at runtime. Sema cannot. If a user wants a high-performance script (e.g., custom syntax analysis), they cannot drop to native code without recompiling the editor.

---

### Priority Matrix

Items marked with a checkmark have been incorporated into the implementation phases above.

| Priority | Gap | Category | Status |
|----------|-----|----------|--------|
| Must fix | Undo grouping | Plan | Addressed -- Phase 1 (engine) + Phase 2 (`editor/with-undo-group`) |
| Must fix | Event data in hooks | Plan | Addressed -- Phase 4 (event maps passed to callbacks) |
| Must fix | Timeout/interruption | Host + Sema wrapper | Runtime exists; host-driving API and cancellation integration still required |
| Must fix | Multi-buffer API | Plan | Addressed -- Phase 2 (`buffer/*` namespace with DocumentId handles) |
| Must fix | Layout/window API | Plan | Addressed -- Phase 2 (`layout/*` namespace, maps to existing LayoutMsg) |
| Must fix | Hook pattern filtering | Plan | Addressed -- Phase 4 (optional `{:pattern glob}` filter) |
| Must fix | Filetype config from scripts | Plan | Addressed -- Phase 2 (`editor/set-option`, `editor/get-option`) |
| Must fix | Read-your-writes | Plan | Working-copy transaction; validate revisions and commit one undo group |
| Should fix | Typed registration | Sema | Open -- upstream plan documented (`sema-lisp/docs/plans/2026-03-11-embedding-api-improvements.md`) |
| Nice to have | Custom UI / virtual text | Plan | Deferred -- design API surface when needed |
| Nice to have | Subprocess management | Host | Runtime operations exist; host policy, wakeups and cancellation required |
| Nice to have | Interactive input (picker, completion) | Plan | Deferred -- requires modal input system |
| Nice to have | Suspended prompts | Host | Task/channel support exists; prompt lifecycle adapter required |
| Nice to have | Dynamic library loading | Sema | Deferred -- upstream feature; not blocking MVP |
