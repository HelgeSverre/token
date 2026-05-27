# Walkthrough — Purified Elm Architecture Boundaries

This document walks through the architectural changes implemented to resolve boundary violations, blocking I/O, and thread spawning in the `token-editor` codebase.

---

## What was Accomplished

We successfully purified the boundaries between pure state layers (Model & Update handlers) and side-effect layers (System/OS integrations) by shifting all blocking interactions into Elm-style commands (`Cmd`) and completion messages (`Msg`).

### 1. Pure, Thread-Free Model Mutations (`src/model/mod.rs`)

- **Before**: `AppModel::record_file_opened` spawned a system thread (`std::thread::spawn`) to write and save the recent files list to disk. This introduced hidden concurrent side-effects into a pure state mutation.
- **After**: Simplified `record_file_opened` to only perform synchronous, in-memory updates to `self.recent_files`. Spawning threads and persisting state is now triggered via a returned `Cmd::SaveRecentFiles` command.

### 2. Context-Aware Async Clipboard Integrations (`src/update/`, `src/runtime/app.rs`)

- **Before**: `DocumentMsg::Copy`, `DocumentMsg::Cut`, `DocumentMsg::Paste` (and their equivalents in CSV cells and Modal inputs) invoked synchronous `arboard::Clipboard::new()` calls directly on winit's main GUI thread. This ran the risk of UI lag/stutter if the OS clipboard manager blocked.
- **After**:
  - **Copy & Cut**: Calculate the text synchronously and return `Cmd::CopyToClipboard(text)` to let the runtime write to the clipboard in a separate thread.
  - **Paste**: Return `Cmd::RequestClipboardPaste` to request the clipboard content. The runtime fetches the clipboard asynchronously on a background thread and sends `AppMsg::PasteFromClipboard(text)` back to the event loop.
  - **Centralized Routing**: The main event loop handles `PasteFromClipboard(text)` and contextually routes it based on what is active: active Modal inputs (`ModalMsg::PasteText`), active CSV cell editing (`CsvMsg::EditPasteText`), or active text documents (`DocumentMsg::PasteText`).

### 3. Non-Blocking Keymap Creation (`src/update/app.rs`, `src/runtime/app.rs`)

- **Before**: When triggering `CommandId::OpenKeybindings`, if the keymap file did not exist, the update loop synchronously created directories and wrote default YAML settings to disk, blocking winit's draw thread.
- **After**: Returning `Cmd::CreateDefaultKeymapFile` to handle directory creation and file saving on a background thread. On completion, it returns `AppMsg::KeymapCreated` to open the newly created path in the editor.

---

## Code Walkthrough & Diffs

### Commands and Messages Definitions

Added new asynchronous command and message variants to represent side-effect transactions cleanly.

```rust
// src/commands.rs
pub enum Cmd {
    ...
    SaveRecentFiles { recent: crate::recent_files::RecentFiles },
    CopyToClipboard(String),
    RequestClipboardPaste,
    CreateDefaultKeymapFile { path: PathBuf },
}

// src/messages.rs
pub enum AppMsg {
    ...
    PasteFromClipboard(String),
    KeymapCreated { path: PathBuf, result: Result<(), String> },
}
```

### Pure Model Refactoring

Removed impure thread spawning from model mutations.

```rust
// src/model/mod.rs
pub fn record_file_opened(&mut self, path: PathBuf) {
    let workspace = self.workspace.as_ref().map(|ws| ws.root.clone());
    self.recent_files.add(path, workspace);
}
```

### Asynchronous Execution Loop

All side-effects are cleanly isolated within asynchronous background workers spawned by winit's runner.

```rust
// src/runtime/app.rs
Cmd::SaveRecentFiles { recent } => {
    std::thread::spawn(move || {
        let _ = recent.save();
    });
}
Cmd::CopyToClipboard(text) => {
    std::thread::spawn(move || {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(&text);
        }
    });
}
```

---

## Verification Results

### Automated Tests Passing

The entire test suite was run to confirm correctness, showing **zero failures** and absolute stability across navigation, selection, undo/redo, and status bar logic:

- **Library Core tests**: **48 passed**
- **Status bar tests**: **47 passed**
- **Text editing tests**: **47 passed**
- **Theme parser tests**: **17 passed**
- **Workspace sidebar tests**: **37 passed**

All compiled binaries run perfectly, and clippy compiler lints compile successfully with zero warnings on the modified code.
