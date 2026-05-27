# Architecture Review Report — HelgeSverre/token

This document presents a comprehensive, read-only architectural review of the core files in the `token` codebase. The evaluation was performed by three independent, specialized virtual review agents: **Object-Oriented Design**, **Clean Architecture**, and **API Design**.

---

## Architecture Review Results

### Overall Verdict: `CHANGES_REQUESTED`

> [!WARNING]
> While the codebase exhibits exceptional domain engineering, type-safe data modeling, and clean visual modularity, it was not approved due to two **CRITICAL** violations of Elm-style architectural purity:
>
> 1. Synchronous background thread spawning and filesystem I/O directly within model state mutations (`AppModel::record_file_opened`).
> 2. Synchronous and blocking clipboard I/O directly within pure update handlers (`update_app`, `update_csv`, `update_document`).

---

### Object-Oriented Design — `APPROVED`

#### Summary

The code exhibits exceptionally clean, highly pragmatic, and well-decoupled object-oriented patterns under Rust. Its state management, DPI-scaling abstractions, and trait-driven text-editing engine successfully implement SOLID principles while maintaining high-performance characteristics.

#### Findings

##### [OO-001]: Large Test Suite in Application Entry Point

- **Severity**: WARNING
- **Principle**: SRP (Single Responsibility Principle)
- **File(s)**: `src/main.rs`
- **Line(s)**: 39-1098
- **Description**: The `main.rs` file represents the application's bootstrapper, yet more than 80% of its content consists of a comprehensive test suite covering cursor navigation, selection, PageUp/PageDown, undo/redo, and modal isolation. This violates the Single Responsibility Principle as the entry point houses logic-specific behavioral tests. It also includes an active TODO comment: `TODO: Find a way to move it into test module instead of main.rs`.
- **Recommendation**: Extract these tests from `src/main.rs` and place them either in a dedicated integration test file under `tests/` (e.g., `tests/keyboard_navigation.rs`) or in a `tests` submodule within the appropriate library files (e.g., inside `src/update/editor.rs` or `src/editable/state.rs`) where the relevant methods are tested.

##### [OO-002]: Duplicated Selection-Deletion Logic

- **Severity**: WARNING
- **Principle**: DRY (Don't Repeat Yourself)
- **File(s)**: `src/editable/state.rs`
- **Line(s)**: 574-597, 653-675, and 801-837
- **Description**: The logic for checking if a selection is active, deleting it from the buffer, updating the cursor to the selection start, collapsing the selection, and retrieving the deleted slice is duplicated between `insert_char` and `insert_text`. A highly similar sequence also exists in `delete_selection`. This is a DRY violation that creates risk if the selection model or buffer interface needs to change.
- **Recommendation**: Extract this selection deletion logic into a private helper method on `EditableState` such as `fn take_selection_deletion_info(&mut self) -> Option<(usize, String)>`. This helper can perform the deletion on the buffer, collapse the selection, update the cursor, and return the resolved start offset and deleted text, making both insertion methods and deletion methods clean and DRY.

##### [OO-003]: Decoupling of TextBuffer via Traits

- **Severity**: SUGGESTION
- **Principle**: ISP (Interface Segregation Principle), DIP (Dependency Inversion Principle), OCP (Open/Closed Principle)
- **File(s)**: `src/editable/mod.rs`, `src/editable/state.rs`
- **Description**: The design of the `TextBuffer` and `TextBufferMut` traits is a superb application of Interface Segregation, Dependency Inversion, and Open/Closed Principles. By parameterizing `EditableState<B: TextBuffer>` over these traits, the editing state logic is completely decoupled from whether the text is stored in a simple contiguous memory string (`StringBuffer`) or a complex ropes data structure (`RopeBuffer`).
- **Recommendation**: Preserve this design pattern. If any collaborative CRDT buffers, encrypted buffers, or memory-mapped buffers are added in the future, implement the `TextBuffer` traits for them to keep the editing system simple and unified.

#### Metrics

- Critical: 0
- Warnings: 2
- Suggestions: 1

---

### Clean Architecture — `CHANGES_REQUESTED`

#### Summary

The codebase shows a solid Elm-style architectural foundation with clear state structures and clean separation between editor actions and constraints. However, there are architectural issues including side-effects bleeding into the model layer, test blocks bloating the main binary entry point, and high compilation coupling in the centralized messaging enums.

#### Findings

##### [CA-001]: Hidden Side-Effects and Layer Boundary Violation in Model

- **Severity**: **CRITICAL**
- **Principle**: Layer Boundaries / Elm Architecture / Testability
- **File(s)**: `src/model/mod.rs`
- **Line(s)**: 491-495
- **Description**: `AppModel::record_file_opened` directly spawns an operating system thread using `std::thread::spawn` to save the recent files list to disk. In a pure Elm Architecture, the Model layer must be passive and represent only pure state. Side-effects (I/O, threading, timers) must be explicitly managed by returning a command (`Cmd`) from the `update` loop, which is then executed by the architecture's runtime. Direct side-effects inside the Model layer violate clean layer boundaries, bypass the central unidirectional control flow, and make unit testing difficult and non-deterministic (e.g. running unit tests for `FileLoaded` causes real filesystem writes on background threads).
- **Recommendation**: Refactor `record_file_opened` to only update the in-memory state of `recent_files`. Then, in the `update_app` handler for `AppMsg::FileLoaded` (in `src/update/app.rs`), return a new command variant, e.g. `Cmd::SaveRecentFiles`, which the runtime can execute asynchronously in its background runner.

##### [CA-002]: Massive Test Suite Bloating Binary Entrypoint

- **Severity**: WARNING
- **Principle**: Single Responsibility Principle (SRP) / Common Closure Principle (CCP)
- **File(s)**: `src/main.rs`
- **Line(s)**: 38-1097
- **Description**: `src/main.rs` contains nearly 1,000 lines of test code containing extensive keyboard handling, modal isolation, and viewport scrolling tests. The entry point file (`main.rs`) is meant to initialize startup configurations, parse CLI arguments, and kick off the winit event loop. Storing all tests inside `main.rs` violates SRP and CCP. These tests verify editor and input routing state machine logic, which changes at a completely different rate than the main application bootstrap code.
- **Recommendation**: Move the keyboard and modal testing block into a dedicated test module under `src/runtime/input/tests.rs` or as integration tests in `tests/input_routing_tests.rs`. This will clean up the entry point and align with the `TODO` left in `main.rs` line 40.

##### [CA-003]: Monolithic Messages Coupled to Feature Modules

- **Severity**: WARNING
- **Principle**: Common Reuse Principle (CRP) / Stable Dependencies Principle (SDP)
- **File(s)**: `src/messages.rs`
- **Line(s)**: 1-759
- **Description**: `messages.rs` is a large file defining a single monolithic `Msg` enum and all sub-message enums (`CsvMsg`, `ImageMsg`, `OutlineMsg`, `WorkspaceMsg`, `DockMsg`, etc.). Since almost every component in the codebase depends on the `Msg` type (for routing or event handling), any addition or modification to a feature-specific message (like adding a cell-editing operation to `CsvMsg`) forces a recompilation of the entire messaging system and any files referencing it. This violates CRP and SDP, as stable components (the general event system) are heavily coupled to unstable feature-specific components.
- **Recommendation**: Distribute the sub-message definitions to their respective modules (e.g., define `CsvMsg` inside `src/csv/messages.rs` or `src/csv/mod.rs`), and import them into the central `messages.rs` which only maintains the top-level `Msg` routing enum. This limits recompilation boundaries to only the modified feature modules.

##### [CA-004]: Direct Feature Coupling in Application Resize Handler

- **Severity**: SUGGESTION
- **Principle**: Single Responsibility Principle (SRP) / High Cohesion
- **File(s)**: `src/update/app.rs`
- **Line(s)**: 23-37
- **Description**: The resize handler for `AppMsg::Resize` inside `update_app` explicitly checks if the editor is in CSV mode and updates the CSV viewport dimensions inside the handler. This introduces direct feature coupling between the application-level resize handler and the CSV editing subsystem, violating SRP and decreasing modular cohesion.
- **Recommendation**: Delegate feature-specific resizing down to the respective sub-update handlers (e.g. have `update_app` broadcast a layout/resize message, or let `update_csv` handle layout updates directly when standard layout changes occur).

#### Metrics

- Critical: 1
- Warnings: 2
- Suggestions: 1

---

### API Design — `CHANGES_REQUESTED`

#### Summary

The Token editor architecture shows high maturity in structural separation of rendering and layout, but it exhibits several critical violations of the pure Elm-style architecture it adopts. Specifically, it performs synchronous and blocking I/O (filesystem and system clipboard) and thread spawning directly inside model state mutations and pure update handlers, compromising event predictability and thread safety.

#### Findings

##### [API-001]: Pure Elm Architecture Violation: Direct Synchronous Clipboard I/O in Update Handlers

- **Severity**: **CRITICAL**
- **Principle**: Elm Architecture, Single Responsibility Principle (SRP), Thread Isolation
- **File(s)**: `src/update/app.rs`, `src/update/csv.rs`, `src/update/document.rs`
- **Line(s)**: `src/update/app.rs` lines 335-341 & 356-362, `src/update/csv.rs` lines 727-728, 745-746, 767-768, `src/update/document.rs` lines 1368-1369, 1423-1424, 1502-1503
- **Description**: Clipboard retrieval/assignment is executed directly in pure update functions using `arboard::Clipboard::new()`. Clipboard access is a side-effect that can block the UI thread on macOS/Linux (e.g. if the clipboard manager is locked or busy). Performing this in the `update` loop violates Elm Architecture guidelines where `update` functions must remain pure, synchronous, and free of side-effects.
- **Recommendation**: Defer clipboard interactions to winit's asynchronous runner by adding `Cmd::CopyToClipboard(String)` and `Cmd::RequestPaste` commands to the `Cmd` enum, and handle them inside the runtime event loop. When clipboard paste content is fetched, pass it back as a message (e.g. `AppMsg::ClipboardPasted(String)`).

##### [API-002]: Pure Elm Architecture Violation: Direct Blocking Filesystem I/O in Update Handlers

- **Severity**: WARNING
- **Principle**: Elm Architecture, Separation of Concerns
- **File(s)**: `src/update/app.rs`
- **Line(s)**: Lines 275-291 (within `CommandId::OpenKeybindings`)
- **Description**: The update handler synchronously creates the default keymap file using blocking filesystem I/O (`std::fs::create_dir_all` and `std::fs::write`) when executing `CommandId::OpenKeybindings`. Doing I/O inside the update loop can block the main GUI rendering thread, introducing latency and frame drops.
- **Recommendation**: Create a specialized `Cmd::CreateDefaultKeymapFile` variant or delegate default keymap creation as a standard async command. The runtime loop should handle the file writing asynchronously and reply with a message (e.g., `AppMsg::KeymapCreated`) when done.

##### [API-003]: Pure Elm Architecture Violation: Background Thread Spawning in AppModel State Mutation

- **Severity**: WARNING
- **Principle**: Elm Architecture, Pure State Transitions
- **File(s)**: `src/model/mod.rs`
- **Line(s)**: Lines 485-496 (within `AppModel::record_file_opened`)
- **Description**: Inside the model's `record_file_opened` method, a background thread is spawned using `std::thread::spawn` to save the recent files list to disk. Under the Elm Architecture, state mutations on models must be pure. Spawning threads directly within a model's state-modification method bypasses the returned command structure and introduces hidden background side-effects.
- **Recommendation**: Modify `record_file_opened` to only modify the in-memory `self.recent_files` structure. Let the calling `update` function return a command like `Cmd::SaveRecentFiles` to be executed asynchronously by the application runtime.

##### [API-004]: Overuse of Cryptic `Option<bool>` for Scrolling Directions

- **Severity**: WARNING
- **Principle**: Self-Documenting Interfaces, Type Safety, Principle of Least Surprise
- **File(s)**: `src/model/mod.rs`
- **Line(s)**: Lines 707-715 (`ensure_cursor_visible_directional`)
- **Description**: The method `ensure_cursor_visible_directional` accepts a cryptic `Option<bool>` parameter to represent the directional hint (`Some(true)` = up, `Some(false)` = down, `None` = horizontal or minimal). This type-signature forces developers to decipher true/false mappings and is error-prone.
- **Recommendation**: Refactor `vertical_up: Option<bool>` into a self-documenting enum:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum ScrollDirectionHint {
      Up,
      Down,
      None,
  }
  ```
  This makes the method call-sites clear (e.g. `model.ensure_cursor_visible_directional(ScrollDirectionHint::Up)`).

##### [API-005]: Naming Ambiguity: Collision of 1D and 2D 'Position' in the API

- **Severity**: WARNING
- **Principle**: Self-Documenting Interfaces, Naming Clarity
- **File(s)**: `src/model/mod.rs`, `src/messages.rs`
- **Line(s)**: `src/model/mod.rs` lines 640-669, `src/messages.rs` lines 39 & 71
- **Description**: The word `position` is used ambiguously. In `cursor_buffer_position()`, `set_cursor_from_position(pos: usize)`, and `move_cursor_to_position(pos: usize)`, it means a 1D character offset in the text buffer. However, in `SetCursorPosition` and `ExtendSelectionToPosition`, it means a 2D line/column coordinate. Also, the 1D character offset is referred to as `offset` in other places (e.g. `cursor_offset`).
- **Recommendation**: Unify nomenclature: use `offset` exclusively for 1D character indices (e.g., `cursor_buffer_offset`, `set_cursor_from_offset`, `move_cursor_to_offset`) and preserve `position` exclusively for 2D line/column coordinates.

##### [API-006]: Boolean Parameters in Messages Should be Self-Documenting Enums

- **Severity**: SUGGESTION
- **Principle**: Self-Documenting Interfaces, Type Safety
- **File(s)**: `src/messages.rs`, `src/commands.rs`
- **Line(s)**: `src/messages.rs` lines 657-660, `src/commands.rs` lines 634-639
- **Description**: `WorkspaceMsg::OpenFile` contains a boolean flag `preview: bool`, and `ShowOpenFileDialog` contains `allow_multi: bool`. Raw booleans passed in constructors (e.g. `OpenFile { path, false }`) are hard to decipher at a glance.
- **Recommendation**: Replace the boolean flags with explicit enums:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum FileOpenMode {
      Preview,
      Permanent,
  }
  ```
  This increases code readability and protects against argument misordering.

##### [API-007]: Inconsistent Type for Base Constants in `ScaledMetrics`

- **Severity**: SUGGESTION
- **Principle**: Consistency, Type Safety
- **File(s)**: `src/model/mod.rs`
- **Line(s)**: Lines 299-330
- **Description**: In `ScaledMetrics`, base layout metrics for splitters, paddings, and borders are defined as `f64`, but sidebar-related metrics (e.g., `BASE_SIDEBAR_DEFAULT_WIDTH`, `BASE_SIDEBAR_MIN_WIDTH`) are defined as `f32`. This requires unnecessary manual casting throughout layout scaling formulas.
- **Recommendation**: Standardize all base constants to `f64` to prevent type mismatches and ensure a uniform scaling pipeline before conversion to physical pixels.

#### Metrics

- Critical: 1
- Warnings: 4
- Suggestions: 2

---

## Priority Actions

These compiled actions represent the most impactful changes to address, ordered by severity and architectural importance:

### 1. [CRITICAL] Purify the Elm Architecture Update Handlers (Clipboard & File I/O)

- **Problem**: Direct, synchronous operations such as reading/writing the clipboard via `arboard` (`API-001`) and initializing folders/writing keymaps (`API-002`) are called in pure `update` functions. These are blocking and side-effectful operations that can freeze the GUI event loop.
- **Action**: Add `Cmd::CopyToClipboard(String)`, `Cmd::RequestPaste`, and `Cmd::CreateDefaultKeymapFile` variants to the `Cmd` enum. Execute them in the runtime loop, returning results as events (e.g. `AppMsg::ClipboardPasted(String)`).

### 2. [CRITICAL] Purify Model State Transitions (Background Thread Spawning)

- **Problem**: `AppModel::record_file_opened` spawns a background thread using `std::thread::spawn` to perform direct file writes to save the recent files list (`CA-001` / `API-003`). This makes the state mutations impure and violates the single thread/event source of truth.
- **Action**: Let `AppModel::record_file_opened` update the in-memory vector synchronously. From the corresponding `update_app` handler, return a `Cmd::SaveRecentFiles` command to let the asynchronous winit/tokio runtime safely persist the state.

### 3. [WARNING] Move Massive Keyboard and Modal Test Suites out of `src/main.rs`

- **Problem**: The application entry point `src/main.rs` contains over 1,000 lines of behavioral tests, bloating the bootstrap code (`OO-001` / `CA-002`). This violating the Single Responsibility Principle and Common Closure Principle.
- **Action**: Move this test block into a dedicated test module under `src/runtime/input/tests.rs` or inside `tests/input_routing_tests.rs`, matching the existing TODO comment in `main.rs`.

### 4. [WARNING] Decouple Monolithic `messages.rs` to Restrict Recompilation Boundaries

- **Problem**: Adding or editing any message in the monolithic `messages.rs` file forces a clean recompilation of the entire codebase (`CA-003`), as almost all layout, render, and update modules rely on the `Msg` type.
- **Action**: Move sub-message definitions (such as `CsvMsg`, `ImageMsg`, `WorkspaceMsg`) to their respective subfolders/modules, and keep only the root `Msg` routing enum inside `src/messages.rs`.

### 5. [WARNING] Unify 1D (Offset) and 2D (Position) Naming and scrolling interfaces

- **Problem**: The word `position` is used ambiguously in both 1D and 2D contexts (`API-005`), causing developers confusion. Additionally, vertical scrolling uses cryptic `Option<bool>` signatures (`API-004`).
- **Action**: Use `offset` exclusively for 1D indices, reserve `position` for 2D coords, and convert scrolling directions into a readable, strongly typed `ScrollDirectionHint` enum.
