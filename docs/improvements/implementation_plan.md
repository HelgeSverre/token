# Implementation Plan — Purifying the Elm Architecture Boundaries

This plan outlines the changes required to address the architectural and API design violations identified in the review. Specifically, we will eliminate direct background thread spawning, synchronous clipboard accesses, and blocking filesystem writes from pure model transformations and update handlers, moving them entirely into Elm-style asynchronous `Cmd` variants executed by the application runtime.

---

## Proposed Changes

### Component 1: Commands & Messages (`src/commands.rs`, `src/messages.rs`)

We will introduce new command and message variants to represent asynchronous side effects and their corresponding completion events.

#### [MODIFY] [commands.rs](file:///Users/helge/code/token-editor/src/commands.rs)

- **Add command variants**:

  ```rust
  /// Save recent files list asynchronously
  SaveRecentFiles { recent: crate::recent_files::RecentFiles },

  /// Copy a string to the system clipboard
  CopyToClipboard(String),

  /// Request pasting text from the system clipboard
  RequestClipboardPaste,

  /// Create default keymap file asynchronously
  CreateDefaultKeymapFile { path: PathBuf },
  ```

#### [MODIFY] [messages.rs](file:///Users/helge/code/token-editor/src/messages.rs)

- **Add message variants to `AppMsg`**:

  ```rust
  /// Paste text retrieved from system clipboard
  PasteFromClipboard(String),

  /// Default keymap file was created asynchronously
  KeymapCreated { path: PathBuf, result: Result<(), String> },
  ```

- **Add message variant to `DocumentMsg`**:
  ```rust
  /// Paste given text into the document
  PasteText(String),
  ```
- **Add message variant to `CsvMsg`**:
  ```rust
  /// Paste given text into the active CSV cell editing buffer
  EditPasteText(String),
  ```
- **Add message variant to `ModalMsg`**:
  ```rust
  /// Paste given text into the active modal text input
  PasteText(String),
  ```

---

### Component 2: Application Runtime (`src/runtime/app.rs`)

We will implement execution handlers for the new commands inside the winit application event loop runner, ensuring all side-effects run on background threads.

#### [MODIFY] [app.rs](file:///Users/helge/code/token-editor/src/runtime/app.rs)

- **Update `process_cmd` to handle new command variants**:
  - `Cmd::SaveRecentFiles { recent }`: Spawn thread and call `recent.save()`.
  - `Cmd::CopyToClipboard(text)`: Spawn thread and use `arboard` to set text to system clipboard.
  - `Cmd::RequestClipboardPaste`: Spawn thread, use `arboard` to get text, and send `AppMsg::PasteFromClipboard(text)` to the main channel.
  - `Cmd::CreateDefaultKeymapFile { path }`: Spawn thread, call `crate::update::app::create_default_keymap_file(&path)`, and send `AppMsg::KeymapCreated` to the main channel.

---

### Component 3: Passive Model Layer (`src/model/mod.rs`)

We will make `AppModel` completely pure by eliminating thread spawning from the recent files tracking logic.

#### [MODIFY] [mod.rs](file:///Users/helge/code/token-editor/src/model/mod.rs)

- **Refactor `AppModel::record_file_opened`**:
  - Simplify to only perform synchronous, in-memory updates to `self.recent_files`:
    ```rust
    pub fn record_file_opened(&mut self, path: PathBuf) {
        let workspace = self.workspace.as_ref().map(|ws| ws.root.clone());
        self.recent_files.add(path, workspace);
    }
    ```

---

### Component 4: Pure Update Handlers (`src/update/`)

We will refactor update handlers to return the appropriate commands instead of running I/O or threading directly.

#### [MODIFY] [app.rs](file:///Users/helge/code/token-editor/src/update/app.rs)

- **Refactor `AppMsg::FileLoaded` & `CommandId` handlers**:
  - Add `Cmd::SaveRecentFiles` to batched return commands whenever `model.record_file_opened` is called.
  - Make `create_default_keymap_file` a `pub` function so it can be called from the runtime loop.
  - Update `CommandId::OpenKeybindings` to return `Cmd::CreateDefaultKeymapFile` when the file doesn't exist.
  - Implement handler for `AppMsg::KeymapCreated { path, result }`:
    - On success: Return `Cmd::OpenFileInEditor { path }`.
    - On error: Log error, show in status bar, and return `Cmd::redraw_status_bar()`.
  - Implement handler for `AppMsg::PasteFromClipboard(text)`:
    - Determine focus (active modal, active CSV, or document) and dispatch the respective `PasteText(text)` message.

#### [MODIFY] [layout.rs](file:///Users/helge/code/token-editor/src/update/layout.rs)

- **Refactor `update_layout`**:
  - Add `Cmd::SaveRecentFiles { recent: model.recent_files.clone() }` to the batched commands returned when opening files (images, binaries, text docs).

#### [MODIFY] [document.rs](file:///Users/helge/code/token-editor/src/update/document.rs)

- **Refactor clipboard document handlers**:
  - `DocumentMsg::Copy`: Return `Cmd::CopyToClipboard(text)` instead of calling `arboard` synchronously.
  - `DocumentMsg::Cut`: Return `Cmd::CopyToClipboard(text)` along with deletion changes.
  - `DocumentMsg::Paste`: Return `Cmd::RequestClipboardPaste` instead of retrieving clipboard text synchronously.
  - **Add `DocumentMsg::PasteText(text)`**: House the actual paste text insertion logic here.

#### [MODIFY] [csv.rs](file:///Users/helge/code/token-editor/src/update/csv.rs)

- **Refactor clipboard CSV cell handlers**:
  - `edit_copy` & `edit_cut`: Return `Cmd::CopyToClipboard(text)` instead of calling `arboard` synchronously.
  - `edit_paste` / `CsvMsg::EditPaste`: Return `Cmd::RequestClipboardPaste`.
  - **Add `CsvMsg::EditPasteText(text)`**: House the actual paste text cell insertion logic here.

#### [MODIFY] [ui.rs](file:///Users/helge/code/token-editor/src/update/ui.rs)

- **Refactor clipboard modal handlers**:
  - `ModalMsg::Copy` & `ModalMsg::Cut`: Return `Cmd::CopyToClipboard(text)`.
  - `ModalMsg::Paste`: Return `Cmd::RequestClipboardPaste`.
  - **Add `ModalMsg::PasteText(text)`**: House the actual paste text modal input insertion logic here.

---

## Verification Plan

### Automated Tests

- Build debug binary to verify compilation:
  ```bash
  make build
  ```
- Run full test suite:
  ```bash
  make test
  ```
- Run clippy lints to check for compiler/clippy warnings:
  ```bash
  make lint
  ```

### Manual Verification

- Launch application using `make run`.
- Open several files from explorer and verify they are successfully saved to the recent files list without thread lockups.
- Trigger Copy/Cut/Paste inside a text document, modal inputs (such as Go to Line), and CSV cells, and verify standard clipboard integration works perfectly.
- Trigger Open Keybindings when keymap doesn't exist, verify default keymap file is created and opened without latency.
