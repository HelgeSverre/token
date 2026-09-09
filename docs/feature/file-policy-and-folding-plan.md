# Auto-save, EditorConfig, and code folding

> **Status:** Implemented and reviewed; repository and native automation checks passed, with manual GUI gaps recorded below.
> **Baseline investigated:** 2026-09-09 at `6948f3f`.
> **Integration reviewed:** 2026-09-09 with `a842c74` (`main`), including the docked Find and hover changes.
> **Branch:** `plan/autosave-editorconfig-folding`.
> **Worktree:** `/Users/helge/code/token-editor-feature-plan`.

This is the implementation plan for auto-save on window focus loss or an idle
delay, per-file EditorConfig rules, and code folding progressing from indentation
to syntax detection and saved state. It supersedes the implementation sketches
in [auto-save](auto-save.md), [EditorConfig](editorconfig.md),
[basic folding](folding-basic.md), and [advanced folding](folding-advanced.md).
Those documents retain historical ideas; their pseudocode is not the build plan.

The recommended order is a document-targeted save pipeline, auto-save,
per-document text settings and EditorConfig, then folding through the shared
viewport. Auto-save can ship independently. Folding should follow configurable
tab geometry so indentation detection and displayed columns agree.

## Evidence from the pre-implementation baseline

| Area                              | Existing source and behavior                                                                                                                                                                                  | Consequence for implementation                                                                                                                                                              |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Document identity and dirty state | [Document](../../src/model/document.rs): `DocumentId`, `revision`, saved Rope/path snapshots, `refresh_modified`, `begin_file_request`                                                                        | Target documents explicitly; a save completion must acknowledge its snapshot while preserving newer edits.                                                                                  |
| Saves                             | [update/app.rs](../../src/update/app.rs): `SaveFile`, `save_document`, `begin_save`, `finish_save`; [file request state](../../src/model/file_io.rs)                                                          | `begin_save` already takes a document ID, but the entry point and formatter continuation depend on focus. Refactor these boundaries, not the worker.                                        |
| File effects                      | [runtime/file_io.rs](../../src/runtime/file_io.rs): ordered `FileWorker`, `write_checked`, `file_matches`, draining `Drop`                                                                                    | Reuse the queue and exact-content conflict checks. Writes currently truncate the checked handle; they are not atomic replacement or crash recovery.                                         |
| External changes                  | [update/file_change.rs](../../src/update/file_change.rs), [runtime/file_watch.rs](../../src/runtime/file_watch.rs)                                                                                            | Known conflicts can block auto-save early; worker checks remain necessary when notifications arrive late. Watches currently cover parents of open files, not every EditorConfig ancestor.   |
| Runtime deadlines                 | [runtime/app.rs](../../src/runtime/app.rs): `WindowEvent::Focused`, `about_to_wait`, `next_wake`                                                                                                              | Add a per-document deadline map to the existing event loop; the old single-document timer-thread proposal is unnecessary.                                                                   |
| Shared mutations                  | [update/text_edits.rs](../../src/update/text_edits.rs): `PlannedEdit`, `apply_planned_edits`, `EditPositions`, `edit_effects`                                                                                 | Use the existing batch/position mapping for whitespace cleanup and fold anchors. Revision-aware scheduling must also cover undo/redo and CSV commits.                                       |
| Text defaults                     | [update/document.rs](../../src/update/document.rs): indentation inserts a tab, unindent removes a tab or up to four spaces, Enter inserts LF                                                                  | Existing behavior is not consistently “four spaces.” Preserve these defaults until an explicit file/user policy supplies alternatives.                                                      |
| Tab geometry                      | [util/text.rs](../../src/util/text.rs), [wrap.rs](../../src/wrap.rs), [view/geometry.rs](../../src/view/geometry.rs), [view/editor_text.rs](../../src/view/editor_text.rs) use `TABULATOR_WIDTH = 4`          | A per-file option must reach all column conversions, wrapping, rendering, guides, and hit testing.                                                                                          |
| Formatting                        | [update/lsp.rs](../../src/update/lsp.rs): `request_formatting`, `FormattingResolved`; [runtime/app.rs](../../src/runtime/app.rs): `PendingFormatting`                                                         | Formatting currently requests four-space indentation; its save continuation requires the original document to remain focused.                                                               |
| Viewport                          | [model/editor.rs](../../src/model/editor.rs): `TextViewportMap`, `EditorState::viewport_map`; [ghost projection](../../src/model/ghost_text.rs)                                                               | Compose folds with soft wrap, fractional scrolling, and inline suggestions. A separate fold-only line loop would break existing behavior.                                                   |
| Gutter and overview               | [geometry](../../src/view/geometry.rs): `GutterLayout`, `LaneId::Fold`; [mouse routing](../../src/runtime/mouse.rs); [marks](../../src/model/decorations.rs); [overview](../../src/view/editor_scrollbars.rs) | The fold lane is reserved with zero width, clicks are consumed without a feature action, and `LineMarks` has no fold slot yet. Overview caching currently keys on wrap identity, not folds. |
| Syntax                            | [syntax registry](../../src/syntax/registry.rs), [parser](../../src/syntax/parser.rs), [syntax updates](../../src/update/syntax.rs), [tree snapshots](../../src/syntax/selection.rs)                          | Reuse parsed trees, language profiles, injections, and stale-result checks. Outline entries alone do not cover all foldable blocks.                                                         |
| Preferences                       | [config.rs](../../src/config.rs), [settings.rs](../../src/settings.rs), [Settings page](../../src/view/settings_page.rs)                                                                                      | Extend the existing category/form design. The existing `EditorConfig` Rust type is Token's user configuration, not a `.editorconfig` resolver.                                              |
| Persistence                       | [session.rs](../../src/session.rs): version-1 `Session`, per-pane `SavedTab`; [runtime/session.rs](../../src/runtime/session.rs): bounded, atomic metadata storage                                            | Extend this metadata format, including per-pane folds. Do not create the old proposal's competing global `fold-state.json`.                                                                 |
| Close behavior                    | [update/layout.rs](../../src/update/layout.rs): `close_tab`; [update/app.rs](../../src/update/app.rs): `Quit`                                                                                                 | Dirty-tab confirmation is not implemented. Auto-save must not be described as protection for untitled buffers, edits closed before the delay, or crashes.                                   |

## Scope and recommended defaults

These product choices were used for the implementation. The user guides describe
the resulting settings and behavior.

| Choice              | Recommendation                                                                                                                                                                                                                                        |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Auto-save           | Off by default; modes `off`, `on_focus_loss`, `after_delay`, `on_focus_loss_and_delay`; 1,000 ms idle delay.                                                                                                                                          |
| Idle meaning        | Time since the last buffer mutation in each document. Cursor movement, scrolling, and activity in another file do not postpone it.                                                                                                                    |
| Focus meaning       | The native window loses focus. Switching tabs, panes, or moving into Settings/terminal does not constitute focus loss.                                                                                                                                |
| Format on auto-save | A separate `auto_save.format_on_save`, default false. Existing manual `format_on_save` remains independent. EditorConfig save cleanup applies to both.                                                                                                |
| Eligible content    | Dirty documents with a path, including new named files and committed CSV buffers. Skip untitled, image/binary, special pages, and documents with known disk conflicts or a pending load/Save As transition. Never commit a CSV cell draft implicitly. |
| EditorConfig        | Enabled by default once shipped; resolve supported properties independently per document, retaining their source. No unsolicited reindent of existing contents on open/reload.                                                                        |
| No project rules    | Retain current tab insertion and four-column display behavior; infer the newline used for subsequent Enter from the file, LF for new files. Preserve existing mixed endings until an explicit conversion policy applies.                              |
| Folding             | Enabled for plain-text editing only, initially expanded. Collapse state belongs to a pane; candidate regions belong to its document.                                                                                                                  |
| Saved folds         | Restore across restart and close/reopen, with independent session state for split panes. Invalid or ambiguous anchors open expanded.                                                                                                                  |

Charset conversion, custom region markers, manual selection folds, fold-to-level
commands, exclusion globs, and a per-file settings override editor are follow-ups.
The requested indentation, line-ending, whitespace, syntax-folding, and persistence
work is all included below. General whitespace visualization/conversion tools
remain in [whitespace management](../future/whitespace-management.md).

## Shared save pipeline

Introduce a save intent carrying document ID, source/destination path identity,
reason, and a unique operation token. Suggested reasons are manual, Save As, idle,
and window focus loss. Keep save-preparation state in the model and actual clocks,
resolution reads, and writes in runtime effects. Replies must validate the token
and path as well as the relevant revision; changing focus is not cancellation.

The pipeline is:

```text
Save intent for document D
  -> eligibility and conflict check
  -> resolve destination rules if needed (Save As)
  -> optional formatter for D
  -> compute and apply save cleanup to D as one undoable transaction
  -> capture D's resulting Rope and file request
  -> existing ordered, guarded file write
  -> acknowledge the exact saved snapshot; retain any newer dirty edits
```

Refactor `request_formatting` and its continuation to carry this intent. Keep
interactive Format Selection's selection/focus contract separate. Formatting and
cleanup continuations validate revision, policy generation, path, and operation
token. On a stale formatter result, discard its edits: manual save continues with
the current buffer and current policy, while idle auto-save waits for the newest
edit's deadline. Focus-loss mode preserves a pending save obligation for newer
edits while the window remains unfocused. Unsupported/timed-out formatting retains
the existing ability to save without formatter edits.

Track preparation as well as queued writes, allowing only one automatic save
operation per document at a time. Coalesce repeated automatic triggers, preserving
the newest mutation deadline and any pending focus-loss obligation. Manual Save
and Save As take precedence over automatic preparation; an already submitted write
cannot be retracted and stays ordered in the worker. Never change a document's
identity to a Save As destination before its successful write.

For Save As across languages, use the destination's whitespace policy but skip a
source-language formatter that does not apply to the destination. Switch language
and LSP routing through the existing successful-save path. Test this alongside
ordinary Save As within one language and formatting-request replacement in the
runtime's per-document `FeatureSlot`.

Cleanup runs before snapshot capture, so buffer contents, the saved Rope, undo,
and `LspDidSave` describe the same text. Do not trim only the outgoing string or
silently normalize inside `write_content`: saved-state comparison and conflict
guards currently depend on that text matching disk. A failed write leaves the
cleanup visible and undoable, with the document dirty. A save itself never clears
history. Emit syntax/LSP effects from cleanup normally, but tag its mutation origin
so it cannot endlessly schedule another auto-save.

No-op saves must not create cleanup history entries. A config change should update
effective policy without dirtying or rewriting an otherwise clean file; explicit
Save applies newly selected rules even when the buffer was clean.

## Auto-save

### Implementation

1. Add typed auto-save preferences in `src/config.rs` and form descriptors in
   `src/settings.rs`. Accept arbitrary valid delay values from YAML; display
   presets without overwriting a custom value merely by opening Settings.
   Validate delay conversion/overflow and use an effective 100 ms minimum.
2. Add `src/update/auto_save.rs` for eligibility, state transitions, and effects;
   add a small `src/runtime/auto_save.rs` deadline owner integrated with
   `App::next_wake` and `about_to_wait`. Use injected timestamps in scheduler
   tests, not sleeps. No `Instant::now()` or channel ownership in update handlers.
3. Extend the shared edit lifecycle to emit document/revision scheduling effects
   only when the buffer actually changed. Cover typing, paste, undo/redo, replace,
   completion, LSP workspace edits to background documents, and committed CSV
   edits. Existing redraw/syntax helpers are also called for no-ops, so blindly
   scheduling on every `edit_effects` invocation is insufficient.
4. Dispatch window-focus loss through a model message that enumerates document
   IDs once, including background tabs and shared documents. Preserve completion
   dismissal, scrollbar drag teardown, modifier clearing, and scroll cancellation.
5. Deadline messages carry document ID, revision, and scheduler/policy generation.
   Drop obsolete messages after edits, mode changes, close, reload, or path changes.
   Recheck eligibility immediately before preparation and writing.
6. Preserve dirty state and retain a newer deadline when a save completes. Clean
   documents cancel their timers; pending writes/preparations defer, rather than
   discard, newer eligible work. Consume due entries even when they are blocked
   so the event loop cannot spin on a past deadline.
7. Defer automatic saves while the target has an active IME composition or an
   uncommitted CSV edit, retrying eligibility when the interaction ends. Only
   committed buffer contents are candidates; focus loss is not an implicit commit.
8. Known conflicts skip automatically without launching a dialog. On write error,
   retain dirty state, surface a persistent per-document failure indication plus
   a status message, and suspend retries for that revision. New edits, a successful
   manual save, or explicit conflict resolution can rearm it; do not retry every
   tick/focus event. Retain the existing interactive conflict UI for manual actions.
9. Cancel pending preparation on document release; leave already queued writes to
   the existing draining worker. Quitting does not invent an additional save-all
   policy. Session settings still govern metadata, independently of auto-save.

Auto-save v1 includes committed CSV saving through the existing file path but does
not apply text indentation or whitespace cleanup to CSV serialization. If any
document view has a pending CSV draft, defer that document to avoid ambiguous
"saved" feedback while the visible cell edit remains uncommitted.

### Verification gate

Add `tests/auto_save.rs` and focused runtime scheduler tests. Prove both modes and
their combination, off/default behavior, distinct deadlines for two documents,
deduplication across splits, background saves, edits during formatting/write,
manual-save overlap, and disable/reload/close/Save As cancellation. A no-op or
cursor-only message must not reset a deadline. Test composition and CSV draft
deferral and resumption, and all error/conflict retry rules.

Extend [file I/O tests](../../tests/file_io.rs),
[file-change tests](../../tests/file_change.rs), and existing worker tests for a
late outside edit, deletion/recreation, symlink aliases, failure after cleanup,
and writes drained during shutdown. Assert disk bytes, saved snapshot, and dirty
state, not just that `Cmd::SaveFile` was emitted. Native verification switches
between Token and another app, including two dirty tabs and a read-only file.

## EditorConfig and document text policy

### Resolver choice and compatibility

Recommend an adapter around `ec4rs`, subject to a dependency spike before adding
it to `Cargo.toml`. Its published 1.2.0 documentation provides property parsing,
directory traversal, indentation fallbacks, and optional source tracking. The
`allow-empty-values` feature also needs evaluation for current spec compatibility.
These are useful adapter capabilities, not proof that every required case passes.
[ec4rs documentation](https://docs.rs/ec4rs/latest/ec4rs/).

Compare its observable results against pinned upstream fixtures, record the
supported spec version and features, and verify macOS/Linux/Windows builds and
license/toolchain fit. Exercise parsing through `ConfigParser` over worker-read
snapshots so pure policy tests require no filesystem. Use runtime-owned traversal
to collect source paths and missing candidate locations for live reload. Prefer
this adapter over maintaining another glob/INI implementation. If the spike fails,
record the failing fixtures and evaluate the official core behind the same adapter;
do not silently ship a suffix-only matcher. Context7 did not index `ec4rs` during
this investigation; the evidence here comes from its published crate docs.

The compatibility contract includes ancestor traversal through `root=true` or the
filesystem root, later/nearer rule precedence, relative glob matching, and `unset`.
The workspace root is not an automatic stopping point. Unknown/unsupported
properties are ignored. See the
[EditorConfig specification](https://spec.editorconfig.org/#file-processing) and
[official format overview](https://editorconfig.org/#file-format-details).

### Model and application

Add a distinct `DocumentTextSettings`/`ResolvedFilePolicy` type rather than reusing
Token's global `config::EditorConfig`. Store effective settings and origin metadata
on `Document`, including a policy generation; split views share them. Suggested
home: `src/model/text_settings.rs` for editor-facing values and
`src/editorconfig.rs` for the library adapter and pure resolution results.
Runtime discovery belongs in `src/runtime/editorconfig.rs`.

Resolve explicit EditorConfig values over user defaults per property. Do not fill
in absent project values too early: `unset`, detected line endings, user fallback
values, and explicitly disabled whitespace rules need distinct representations.
If manual per-file overrides are added later, they can form a separate highest
precedence layer without mutating the user's project files.

| Property                                   | Planned behavior                                                                                                                                                                           |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `indent_style`, `indent_size`, `tab_width` | Separate indentation step, insertion style, and display tab stops; honor the `tab` fallback and mixed tab/space indentation when widths differ.                                            |
| `end_of_line`                              | Support LF, CRLF, and CR for inserted line breaks and configured save conversion.                                                                                                          |
| `trim_trailing_whitespace`                 | True removes trailing whitespace; false disables cleanup. Test the supported whitespace set against upstream behavior.                                                                     |
| `insert_final_newline`                     | True ensures a terminal newline except in an empty file; false removes terminal newlines; absent preserves them.                                                                           |
| `charset`                                  | Initial text path remains UTF-8. Preserve existing BOM bytes; ignore unsupported conversion requests and expose them in policy details. General encoding conversion is a separate feature. |

The property contract follows the
[specification's supported pairs](https://spec.editorconfig.org/#supported-pairs).
Token-specific fallback and UI behavior above are implementation recommendations.

Avoid guessing that `tab_width` equals indentation size. Parameterize the existing
shared text helpers, retaining explicit defaults for generic UI text fields.
Thread the document's width through `WrapCache`, `TextViewportMap`, tab expansion,
rectangle selection, cursor reveal/desired columns, inline ghost projection and
completion postprocessing, indent guides, and LSP formatting options. Put tab width
and policy generation into geometry cache keys. Keep model character offsets and
LSP UTF-16 positions independent of rendered columns.

Choosing hard tabs as the fallback preserves current typing/indent behavior, but
aligning LSP formatting with that policy changes its current hard-coded spaces
request. Treat that as an intentional user-visible change in E1 and document it;
an explicit spaces default or EditorConfig rule produces spaces consistently.

Centralize line-ending classification/length handling and audit current literal LF
assumptions in document editing, paste distribution, shared editable cursor updates,
line operations, view helpers, syntax selection, ghost projection, and LSP
positions. CR-only needs explicit tests: current display trimming handles LF and
CRLF, not bare CR. Do not change Ropey's definition of logical lines incidentally
while adding byte serialization policy.

For CR-only files, derive syntax fold lines from byte offsets through the Rope,
not by assuming Tree-sitter row coordinates match editor logical lines. Include
syntax/navigation fixtures for this boundary as part of E3 and F3.

Retain original buffer line endings on load. Enter uses the resolved ending; absent
a rule, use the most common existing LF/CRLF/CR ending, breaking ties by first
occurrence, then LF for a file with no endings. Clipboard text keeps its literal
endings until configured cleanup; generated indentation uses the file policy.
This avoids a normalized-buffer/disk-byte mismatch in the current save guard.

Implement cleanup as non-overlapping `PlannedEdit`s, applied once via the shared
transaction. Build the final desired replacement for each affected range so
trailing-whitespace removal, line-ending conversion, and EOF changes never overlap
or delete CRLF twice. Preserve preexisting extra final blank lines when the true
rule is already satisfied; the false rule removes all terminal line endings.
Use the applicable upstream plugin fixtures to settle EOF/whitespace edge cases.

Resolution lifecycle:

1. Resolve during worker preparation for every new file path: startup, normal open,
   session restore, named nonexistent files, and LSP-created/opened documents.
   An already-open alias reuses the document's current policy and identity.
2. Use the owning document's resolved physical path when available so aliases do
   not produce competing policies for one buffer; named nonexistent files use an
   absolute normalized destination path. Keep display paths unchanged. Document
   this Token-specific symlink choice and test retargeting alongside file identity.
3. For Save As, resolve candidate-destination policy after the native dialog returns,
   before formatting/cleanup and snapshot capture. Cancellation does nothing. Keep
   destination policy provisional until write success; on failure restore source
   effective settings while leaving any cleanup transaction visible and undoable.
4. Watch the consulted ancestor directories, including absent `.editorconfig`
   candidates, independently of workspace ignore rules. Creation, deletion,
   replacement, root-boundary changes, and ancestor-directory replacement invalidate
   affected resolutions. Deduplicate watch roots across documents and release them
   when unused. A root change must rebuild the dependency chain.
5. Worker results carry document/path identity and resolution generation. Drop stale
   replies after rename, close, newer resolution, or disabling support. Revalidate
   before saving if config invalidation is pending; never save under a knowingly
   stale cleanup policy.
6. When effective geometry changes, reflow every pane showing that document and
   preserve logical viewport anchors. Recompute indentation fold candidates later.
   Changing cleanup rules alone waits for an eligible save. Invalid config produces
   a nonmodal diagnostic and a consistent fallback; distinguish missing files from
   read/parse failures, retaining last valid policy on transient reload failures.

Settings gains an EditorConfig enable switch and default indentation controls.
Keep resolved source/value information available from the file's status/policy
details without turning the separate Settings page into a command palette. Bound
config reads/cache memory with `ByteSize`; character counts and tab sizes remain
ordinary quantities. Reject zero or unrepresentable widths and fall back with a
diagnostic instead of dividing by zero or overflowing geometry.

### Verification gate

Add adapter tests and `tests/editorconfig.rs` covering the property contract and
filesystem lifecycle above, including files outside the workspace, missing parent
directories, Save As across projects, and multiple views of one document. The
[official core tests](https://github.com/editorconfig/editorconfig-core-test)
provide parser, glob, property, and file-tree cases; record the exact fixture
revision/license and compare actual resolved properties. The
[plugin tests](https://github.com/editorconfig/editorconfig-plugin-tests) exercise
the editor-side application. Do not equate passing a small local subset with full
EditorConfig conformance.

Extend [soft-wrap tests](../../tests/soft_wrap.rs),
[geometry tests](../../tests/geometry.rs),
[ordinary edit positions](../../tests/ordinary_edit_positions.rs),
[pane undo tests](../../tests/undo_pane_state.rs), and formatting/ghost tests for
tab widths 2/4/8 and differing indentation size. Assert painting/hit-test column
agreement with tabs and Unicode. Round-trip LF/CRLF/CR, mixed endings, empty files,
EOF whitespace, BOM, and both boolean whitespace values through actual disk saves.
Prove cleanup is one undo batch, stable on a second save, and correctly ordered
after formatting for manual and automatic saves. UTF-8/BOM preservation tests do
not establish support for UTF-16 or Latin-1 conversion.

## Basic code folding

### State and detection

Use `src/folding/` for region definitions, indentation detection, reconciliation,
and a pure projection index. Keep fold candidates on `Document` with revision,
language, and policy generation; keep collapsed region identities on `EditorState`.
A new split initially copies the originating pane's collapse state, then changes
independently. Folding never changes the Rope, dirty bit, or text undo history.

Represent a region with a visible header line and a half-open range of whole
hidden lines, plus provider/kind and stable in-session identity. Canonicalize into
a sorted forest: valid nested or disjoint regions, no crossing ranges or duplicate
header choices. Choose one deterministic primary candidate per header; expanding
an outer region preserves the collapse choices of its children.

Indentation detection compares visual indentation using the effective tab stops.
A nonblank header followed by more-indented content opens a region; punctuation
such as `{` or `:` is not a requirement. Blank lines do not create headers and
do not close a block. The first nonblank dedented line ends the range and remains
visible. Exclude trailing blank-only lines from a region, permit a single hidden
body line, and finish open blocks at EOF. This also works for unparsed text.

Run initial/full detection off the rendering path against an immutable snapshot,
with revision/policy checks on installation. Coalesce background recomputation;
start with a bounded full-scan implementation and measure it before choosing an
incremental indentation index. While results are pending, map unaffected existing
anchors through the shared edit transaction; expand/invalidate regions touched
by an edit or whose boundaries no longer exist. Undo/redo, bulk replacements,
reload, and edits from another pane use the same reconciliation rules.

### One viewport authority

Extend `TextViewportMap` rather than adding `doc_to_visual` calls in individual
features. Its consumers need a common answer for row count, row kind, logical
position to visible row, hidden position to containing header, and visible pixel
to logical position. Returning a visible row for hidden text must be distinguishable
from finding a real caret position so hit testing cannot invent hidden cursors.

The projection order is document lines with hidden intervals removed, visible
lines split into wrap segments when enabled, then the existing ghost insertion
at a visible anchor. Reuse `WrapCache` segmentation; it is acceptable initially to
retain cached segments for hidden lines while excluding them from the row index.
Ghost text at a hidden anchor is dismissed. A fold header remains normal text and
can wrap; its chevron appears on the first segment. Draw a short ellipsis/count
badge on its last segment as a decoration, with a hit rectangle from shared
geometry, without adding fake document characters or an independent row loop.

Use an interval/prefix index that skips entire collapsed subtrees. Rendering and
hit testing should scale with visible rows plus indexed lookups, not scan every
hidden line or every fold per row. Cache identity must include document/buffer,
wrap width, tab policy, collapse generation, and ghost projection. Invalidate the
overview even when two different collapse sets happen to have the same row count.

Audit every current `soft_wrap` fast-path branch: folding also needs visual
navigation when wrapping is off. Keep `EditorState::is_plain_text_mode()` as the
gate for these paths. Reuse `src/view/editor_text.rs` for text, placeholders,
selection, marks, and cursor painting; `Renderer` remains the orchestrator.

### Interaction contract

| Interaction                     | Behavior                                                                                                                                                                                                                                     |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Gutter/badge toggle             | Dispatch the clicked pane/editor and logical header; enable the existing fold lane only for eligible text views. Consume press/drag consistently with `LaneId::Fold`.                                                                        |
| Commands                        | Toggle, collapse, expand, collapse all, expand all; expose in the command registry, keymap actions, and command palette. A command inside a block selects the innermost containing region.                                                   |
| Collapse with carets/selections | Move affected empty carets to the header's end and deduplicate through existing helpers. Refuse/skip a region intersecting a nonempty selection, preserving selection contents.                                                              |
| Vertical/page movement          | Traverse visible rows, skipping hidden bodies; preserve desired visual column. Horizontal character/word movement that enters hidden text reveals it.                                                                                        |
| Explicit navigation             | Find next, go-to-line, outline/LSP jumps, diagnostic navigation, selection expansion, and restored cursors reveal all ancestors of the destination. Cursor correctness takes precedence over collapse state.                                 |
| Selection and editing           | Selection ranges remain logical. Copy/cut/replace across a folded body includes that text. Rectangular selection enumerates visible rows. Edits intersecting a collapsed body expand/invalidate it in affected panes before the next render. |
| Scrolling                       | Preserve the logical top anchor and pixel fraction across toggles/reflow; if the anchor becomes hidden, use its visible header. Clamp ranges and cancel stale scroll animation targets.                                                      |
| Diagnostics/find marks          | Search covers the full document. Aggregate hidden marks onto the header/overview location using shared priority rules; selecting a result reveals its real line.                                                                             |
| Overlays and damage             | Dismiss or re-anchor hover/completion overlays using the updated map. A fold toggle initially requests editor-area redraw; cursor-only redraw must still agree with a full repaint.                                                          |

Default shortcuts must be checked against the actual platform keymaps, including
chord prefixes and OS bindings; do not copy the old drafts' bindings blindly.
Command IDs and user rebinding can ship before any unambiguous default is chosen.

### Verification gate

Add `tests/folding.rs` with nested/sibling/EOF/blank-line/mixed-indent fixtures,
candidate normalization, edit reconciliation, and per-pane independence. Compare
projection results with a simple test-only reference iterator over all visible
segments for combinations of folds, wrap, tabs, ghost text, and fractional scroll.
Assert round trips for every visible logical position and explicit hidden-position
results; screenshots alone cannot prove the mapping.

Extend rendering/hit testing, scrollbar overview, navigation, rectangular selection,
ordinary edits, and undo tests. Verify collapse with multiple carets/selections,
hidden search/LSP targets, closing/opening folds above the viewport, same-row-count
cache invalidation, diagnostic lane width changes, and special tab exclusion.
Add repaint equivalence tests for cursor-only versus full frames after folding.
Native verification covers mouse dragging, narrow split panes, trackpads and scroll
animations, and toggling wrap while a nested region is collapsed.

## Syntax-aware folding and saved state

### Syntax providers

Add folding profiles to `LanguageDefinition` and detection under
`src/syntax/folding.rs`, following the registry's existing outline/selection
pattern. Compute regions in the syntax worker from the same parsed snapshot;
deliver them with `SyntaxMsg::ParseCompleted`. Accept only matching document,
revision, language, and policy generation. Language changes must invalidate fold
results even if the numeric document revision did not change.

Start with Rust, JavaScript/TypeScript including JSX/TSX, Python, JSON/YAML,
HTML/CSS, and Markdown. Cover bodies, declarations with multiline bodies,
collections, comments, multiline strings where appropriate, tags, headings, and
fenced blocks through language-specific fixtures. Publish the initial support
matrix; languages without a fold profile retain indentation folding.

Convert Tree-sitter byte ranges to document lines once. Respect exclusive end
positions: an end at column zero must not hide the following line. Keep an end
line visible if unrelated text follows the node on that line. Reject single-line,
empty, crossing, and invalid candidates. Reuse injected tree coordinate/range
information for fenced code and embedded HTML/component languages; clamp children
to their host range and normalize duplicates before building the forest.

Where a valid syntax provider exists, its structural regions are authoritative;
do not union every indentation guess back into syntax strings/comments. Use
indentation for unsupported languages and conservatively uncovered/error areas.
Reconcile surviving collapsed regions by mapped anchors and kind. Disappearing or
ambiguous regions expand. Do not collapse the entire file merely because its tree
root spans multiple lines. Syntax failure preserves usable indentation behavior.

### Persistence

Extend the existing session schema with optional, defaulted fold metadata on
`SavedTab`, plus a bounded per-file recent-fold record for close/reopen. Keep version
1 readable; either add backward-compatible optional fields or explicitly support
both versions if the schema version changes. Existing invalid-version rejection
must not make every prior session unloadable.

Save region identities as provider/kind, nesting context, line hint, and a versioned
digest of identifying header/boundary or body content. Do not serialize source
snippets or raw Tree-sitter IDs. Associate each record with the file's content
fingerprint and workspace/path identity. Compute fingerprints off the rendering
path and reuse revision-keyed results.

For unchanged files, exact region fingerprints restore collapsed state after
candidates exist. For modified files, allow relocation only when a unique matching
region fingerprint and context exist; otherwise expand. A bare saved line number
is never sufficient. Retain nested collapse identities even under a collapsed
parent. Cursor/selection reveal runs after restoration and wins where necessary.

Session restore already installs documents and pane positions before later syntax
results arrive. Hold pending fold/viewport restoration until initial matching
candidates are available; preserve the saved logical top position during this
transition. Timeout or parse failure settles on validated indentation candidates
or expanded state and completes restoration, rather than leaving it pending forever.
User navigation/fold interaction cancels late restoration for that pane.

Per-pane `SavedTab` state wins when restoring an existing layout. For ordinary
close/reopen, the most recently closed eligible pane supplies that file's default;
existing live panes keep their own state. Maintain the recent record in the same
workspace session metadata, prune it by recency with count and `ByteSize` limits,
and retain the existing total session-size guard. Document how disabling session
restore/save-on-exit affects fold persistence; there is no hidden second store.

Only capture new persistent anchors from content matching the saved disk snapshot.
For dirty documents, retain their last valid saved-content fold record instead
of recording positions against text that session restore will not recover. After
successful auto/manual save, refresh the eligible record without writing source
text to session files. Save As establishes a destination record after success;
old path records do not gain overwrite or identity privileges for the new path.

### Verification gate

Test the language support matrix and mixed-language injections with fixtures,
stale completions after edits/language changes, malformed syntax, and duplicate or
crossing node ranges. Verify current collapse state survives parsing updates when
the block survives and expands when it does not.

Extend `src/session.rs` and `src/runtime/session.rs` tests for old-session migration,
unchanged-file restore, unique relocation after inserted lines, duplicate headers,
deleted regions, dirty files, Save As, aliases, split panes, close/reopen, size
limits, and disabled persistence. Inspect serialized JSON to ensure it contains
metadata only. Native restart testing must include nested folds, soft wrap,
fractional scroll, a changed file, and an inline suggestion after restore.

## Build sequence and completion gates

Each row is a reviewable implementation slice. Size is relative and does not
promise elapsed delivery time. All nine slices are implemented. Final gate results are recorded below.

| Slice                              | Size | Dependencies                 | Deliverable and exit gate                                                                                                                                                |
| ---------------------------------- | ---- | ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [x] S1: document-targeted saves    | M    | None                         | Save intent and formatter continuations survive focus changes; existing file identity/conflict/ordering tests remain green.                                              |
| [x] S2: auto-save                  | M    | S1                           | Both triggers, Settings, all-document scheduling, coalescing, errors, composition/CSV handling, and runtime focus-loss coverage; native GUI switching remains unchecked. |
| [x] E1: per-document text geometry | L    | None; deliver before folding | Typed defaults, configurable indentation and all tab-width consumers; wrapped/unwrapped rendering, hit testing, ghost, and LSP agreement.                                |
| [x] E2: EditorConfig resolution    | M    | E1                           | Dependency spike completed, supported properties/provenance, all open paths, Save As staging, live invalidation, and compatibility fixtures pass.                        |
| [x] E3: save-time text rules       | M–L  | S1, E2; integrate S2         | Newline insertion and LF/CRLF/CR conversion, trim/EOF policy, one cleanup undo batch, exact disk/snapshot/notification agreement.                                        |
| [x] F1: fold model and projection  | L    | E1                           | Candidate forest and pane collapse state; shared viewport composition and reference-mapping tests before UI activation.                                                  |
| [x] F2: basic folding UI           | L    | F1                           | Indentation provider, commands/gutter/badge, selection/navigation/scroll/overview integration; automated hit/repaint and native command checks.                          |
| [x] F3: syntax-aware folding       | M–L  | F2                           | Registry profiles, language matrix, injections, malformed/stale result handling and reconciliation tests.                                                                |
| [x] F4: persistence                | M    | F3                           | Session migration, per-pane and close/reopen state, safe anchor matching and native restart verification.                                                                |

Suggested serial order: **S1 → S2 → E1 → E2 → E3 → F1 → F2 → F3 → F4**.
The E1 work is independent of S1/S2, but E3 must verify integration with auto-save.
Do not treat shipping only S2 or F2 as completing this three-feature effort.

For every substantial implementation slice: use targeted `just test-one <filter>`
while iterating, then `just fmt`, `just test`, and `just lint` before handoff.
Update `docs/CHANGELOG.md` only when application behavior actually changes, and
update the settings/keymap/language guides alongside the slice that ships them.
Add automation state/actions only where existing UI/runtime tests cannot observe
the behavior; any new automation commands must traverse the same update path.

Measure geometry and scheduler work with repository release/profile recipes:
`just bench-wrap`, `just bench-render`, `just profile-render`, and
`just profile-workloads` as applicable. Use large files with many nested folds,
many independent dirty tabs, long tabbed wrapped lines, and bulk whitespace edits.
Record baseline and changed-workload costs, allocations and idle wake behavior.
Any new render instrumentation belongs in `src/perf.rs`; debug `just workspace`
or an open F2 overlay cannot establish release frame rates. Performance thresholds
should be selected from these measurements, not invented now.

The largest implementation risks are the full set of tab-width consumers,
fold/wrap/ghost composition, and asynchronous save-policy changes. The first
spikes must settle EditorConfig compatibility and the fold projection API. The
existing checked writer's non-atomic truncation, lack of dirty-close prompts, and
absence of unsaved-text recovery are separately identified lifecycle limitations;
none is solved merely by adding a timer.

## Implementation verification

The implementation includes all nine slices above. Automated coverage exercises
multi-document saves and formatter supersession, EditorConfig reload and Save As,
198 pinned upstream compatibility cases, mixed newline cleanup and undo, reference
fold/wrap/tab projection, gutter and badge hit testing, repaint equivalence,
language providers, split independence, and session and recent-fold restoration.

The merge review found and fixed the following issues:

| Severity | File                                                          | Finding                                                                                    | Resolution                                                                                                           |
| -------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| Medium   | [editor_text.rs](../../src/view/editor_text.rs), glyph stages | Debug full frames omitted a folded-header badge drawn by cursor repaints.                  | Full and incremental frames share the glyph stages; wrapped/ghost repaint comparison passes.                         |
| Medium   | [app.rs](../../src/update/app.rs), `prepare_resolved_save`    | A policy reload could resume an idle save after a newer edit when formatting was disabled. | Recheck the automatic request's revision and policy before resuming.                                                 |
| Medium   | [editor.rs](../../src/model/editor.rs), `fold`                | Collapse All repeatedly scanned the growing collapsed-region list.                         | Index existing headers while building the new state; release benchmarks cover 100,000 lines.                         |
| Medium   | [text_edits.rs](../../src/update/text_edits.rs), `map_left`   | Text inserted after a fold could extend its hidden boundary.                               | Map the exclusive end with left affinity; regression coverage verifies the inserted line stays visible through undo. |
| Medium   | [text.rs](../../src/util/text.rs), `expanded_chars`           | Wide tabs could allocate a large expanded line to paint a small viewport.                  | Expand lazily through the visible edge; a regression test verifies bounded source traversal.                         |
| Medium   | [folding.rs](../../src/update/folding.rs), `action`           | A hover request could survive when folding hid its target without moving the caret.        | Dismiss hover intent and forward gutter-click effects to the runtime.                                                |

Native macOS checks used an isolated config/session directory and the real
application automation endpoint. They passed for nested collapse and explicit
navigation, independent split metadata, soft wrap, changed-file restart (87
logical lines restored to 27 visible rows), close/reopen, independent idle saves
of background files, live EditorConfig reload, and exact CRLF/CR disk cleanup.
The computer-use tool could not obtain macOS Accessibility/Screen Recording
permissions, so physical gutter clicks, drag/trackpad behavior and native focus-loss
switching remain manual verification gaps. Automated hit-testing, repaint, pixel
scrolling and runtime focus-loss tests cover those code paths.

`just bench-wrap` ran with release optimization on this machine. Median timings:

| Workload                                       | 10,000 lines | 100,000 lines |
| ---------------------------------------------- | -----------: | ------------: |
| Fold detection (nested indentation fixture)    |      3.72 ms |      40.51 ms |
| Collapse All (same fixture)                    |      0.36 ms |       4.07 ms |
| Full wrap layout (long-line baseline workload) |      8.85 ms |      88.96 ms |
| Incremental middle-line wrap update            |     20.37 µs |        160 µs |

Indexed folded row lookup was 16.79 ns; ordinary wrap lookup was 1.74 ns.
These are workload measurements, not release frame-rate or allocation claims.
A release-only unused-variable warning found by this run was corrected.

Final checks: `just test '--test-threads 2 --retries 1 --no-fail-fast'` passed
all 2,673 tests without retries before the final hover integration, with five pre-existing skips. Both doctests passed
(six ignored). `just lint`, `just fmt-check`, the explicit documentation Prettier
check and `git diff --check` passed. The reduced test concurrency followed a run
with fake-server startup timeouts under parallel load; no timeout thresholds or
production behavior were weakened. The release build passed. The combined result is receiving a final test and build pass after the integration fixes.

**Review verdict: Approve.** The identified issues are fixed and covered. No
outstanding critical or high-severity finding remains. Manual GUI verification
limitations are listed above and do not change the automated checks' scope.
