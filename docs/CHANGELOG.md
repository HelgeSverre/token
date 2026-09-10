# Changelog

All notable changes to rust-editor are documented in this file.

---

## Unreleased

### Sema language support

- Parse Unicode identifiers and numeric-tower literals, and refresh builtin and
  macro highlighting from the current Sema documentation and prelude.
- Show Sema definitions in Outline and fold balanced forms without indentation.
- Use `sema lsp` semantic colors, line-end parameter hints, and code-lens actions
  through Code Actions (Alt+Enter). Explicit Run actions show evaluation results.
- Keep Run actions in Code Actions without repeating inline labels beside every
  top-level Sema form.
- Discard stale annotations and action menus after edits or server restarts.

### Go language server

- Go files automatically use `gopls` when installed, with `go.work`/`go.mod`
  project-root detection and the standard Language Servers settings and
  `lsp.servers.gopls` configuration overrides.

### Code folding

- Collapse blocks using gutter chevrons, hidden-line badges, or command-palette
  actions. Folding composes with soft wrap, pixel scrolling, selection, navigation,
  inline suggestions, and diagnostic/Find overview marks.
- Detect structural regions in Rust, JavaScript/TypeScript/JSX/TSX, Python,
  JSON/YAML, HTML/CSS, and Markdown, including supported embedded languages.
  Other text and malformed syntax use indentation folding.
- Keep collapse choices independent across split panes, preserve unaffected folds
  through edits and undo, and restore matching folds from saved-file sessions and
  recent close/reopen metadata.

### Text settings and EditorConfig

- Apply per-file EditorConfig indentation and line-ending rules, with live reload
  and user defaults in Settings.
- Keep tab geometry consistent across wrapping, drawing, navigation and inline
  suggestions; formatting requests use the same indentation preferences.
- Apply trailing-whitespace, line-ending and final-newline rules as undoable save
  cleanup. Support CR-only files and resolve destination rules for Save As.

### Auto-save

- Optional automatic saving when the window loses focus, after each document's
  idle delay, or both. Configure modes, delay, and optional formatting in Settings.
  Background tabs share the same guarded, ordered save pipeline as manual saves.
- Formatting before save follows its original document after tab switches. Newer
  saves supersede old formatter replies, and formatter shutdowns fall back to
  saving the current buffer.
- Failed saves retain dirty state and show a persistent failure indicator;
  automatic retries wait for new edits or a successful manual save. Untitled and
  image/binary tabs are excluded, and unfinished CSV cell edits defer saving.

### Docked Find and Replace

- Find and Replace now open beneath the active editor pane's tabs instead of
  in a modal. The editor remains available for clicking, editing and scrolling.
- Cmd+F focuses Find; Cmd+R opens Replace (Ctrl on Windows/Linux). The bar has
  match counts, previous/next navigation, search-option toggles and replacement
  actions, with a stacked layout in narrow panes. Escape closes it.
- Search inputs share the existing text-editing and caret primitives, including
  selection, clipboard shortcuts and pointer selection. Queries are remembered
  when closing; captured selection scopes reset when switching documents.
- Viewport sizing and pointer targets stay aligned with the Find bar at
  fractional display scales and split-pane sizes. Clicking another split keeps
  the clicked text position when the Find bar moves to that pane.

### Pixel scrolling

- Plain-text panes scroll vertically and horizontally by pixels, including
  partially visible rows and characters. Text, gutter, selections, decorations,
  pointer hit testing and scrollbar interaction share the viewport offsets.
- Discrete mouse-wheel steps ease toward their target; trackpad pixel events and
  scrollbar dragging remain direct. Navigation, edits and layout changes cancel
  pending animation. Settings, terminal and special-document scrolling retain
  their existing behavior.
- Saved sessions retain within-row/column positions across display metric
  changes; older session files default to aligned positions.
- Dismissing a wide inline suggestion clamps horizontal scrolling back to the
  source text while retaining the vertical pixel offset.

### Session restore

- Restore saved-file tabs, split layout and ratios, focused tabs, selections,
  multiple cursors and per-pane scroll positions on startup. Each workspace has
  separate session metadata; windows without a workspace use a default session.
- Files are loaded from current disk contents. Missing or unreadable files are
  skipped, empty split branches collapse, and positions clamp to changed files.
  Wrapped viewports retain a document anchor across window-size changes.
- Session settings independently control restore and saving on exit. `--new`
  skips restoration; explicit command-line files take focus after restoration.
  Metadata is written atomically after queued saves finish. Unsaved text, undo
  history, terminal sessions and preview panes are not persisted.

### External-file protection

- Ordinary saves now check the file's bytes against the last loaded/saved
  snapshot before overwriting it. Outside edits, deletion, and another program
  creating a previously missing file cancel the save without replacing either
  version. Ordered saves from Token remain supported.
- Save As can replace a different destination chosen in the native dialog;
  symlink aliases of the original file retain its conflict check.
- Open text files are checked after filesystem changes and window refocus,
  including files outside the workspace. Clean buffers reload by default;
  `auto_reload` in Settings → Editor disables silent reload. CSV grids retain
  their view mode, and in-progress cell edits are not replaced.
- External conflicts show a persistent `!` in the tab title. The resolution
  dialog defaults to Keep Editing and offers explicit Reload, Overwrite,
  Recreate (for deleted files), or Save As actions as appropriate. Save and the
  Resolve External File Change command reopen deferred conflicts. Overwrite
  rechecks the approved disk version before replacing it.

### Documentation cards

- Hover and completion documentation use a draggable scrollbar instead of the
  row-count/Expand footer. Click the track or use the mouse wheel to read more;
  short cards show no scrollbar. F1 expansion and Alt+PageUp/PageDown still work.
- Leading code snippets form a full-width header with rounded top corners and
  balanced vertical padding; their text stays aligned with the documentation body.
- Language-tagged code snippets in hover, completion and signature documentation
  use the editor's syntax highlighting and current theme colors. Unknown languages
  remain plain code, and inline identifiers retain their compact code styling.
- Hover documentation now scrolls through the full signature and prose instead
  of cutting off long content, without moving the editor caret.
- Hover prose stays left-aligned at the same size with or without signatures,
  diagnostics or inline formatting. Signature help uses the configured editor
  font throughout, including the highlighted parameter.
- Automatic hover targets actual text, stays stable within a word, and leaves a
  short grace period for moving into the card. Typing, scrolling, selecting,
  switching panes or leaving the window cancels pending mouse documentation;
  late replies no longer reopen dismissed cards.
- Explicit Show Hover opens immediately and survives incidental pointer
  movement. Completion documentation takes visual priority over hover and
  signature help; automatic empty/unsupported hover results no longer flash
  status messages.
- Hover cards have a wider responsive layout and an opaque reading surface;
  completion documentation side cards are opaque too.
- Documentation code spans use the configured editor font at a slightly smaller
  size, while prose keeps the UI font. Shared wrapping measures the same font
  roles used for painting, and inline backgrounds no longer spill into nearby
  prose. Standalone code avoids stacked backgrounds, and signatures have balanced
  vertical padding. Screenshot fixtures use both bundled fonts and the native
  cursor-overlay renderer.
- Markdown paragraphs reflow across source soft breaks, while explicit hard
  breaks and code-block newlines remain intact.

### Indent guides

- Added indentation guides with an Appearance toggle (`indent_guides`, enabled
  by default). Spaces and tabs use the existing text-column geometry, with
  horizontal clipping and no guides on soft-wrap continuation rows. Guide
  spacing follows common indentation increases, including two-space Lisp/YAML,
  instead of assuming every document uses four-space indentation.
- Added `ui.editor.indent_guide` to theme definitions, with subdued colors in
  all 14 bundled themes and a foreground/background-derived fallback for older
  custom themes. RGB and translucent RGBA overrides are supported.

### Internal cleanup

- Rendering and editing now share tab-column conversion, and split-pane layout,
  hit testing and resize handling share child-rectangle calculations.
- Removed the duplicate navigation command-merging helper. Problems-panel
  grouping uses borrowed traversal without temporary group lists, and opening
  a diagnostic no longer clones its full payload.
- UI labels, Settings controls and overlay text now share one UTF-8-safe
  truncation routine with matching font measurements. Text that fits stays
  borrowed where possible; widths too small for an ellipsis produce empty text
  instead of overflowing.

### Context menus

- Context menus size to their labels and shortcuts, up to 520 logical pixels
  and within the window. Iconless lists no longer reserve an empty icon column.
- Menu shortcut chips are 20% smaller and vertically centered using the shared
  keycap geometry. Rendering and pointer hit targets use the same measured width.

### Fonts

- Separate `editor_font` and `ui_font` preferences. Code and terminal grids
  default to JetBrains Mono, as do file explorer text, tab titles and all text inputs; the
  remaining application UI now defaults to bundled Inter.
  Installed families can be selected in configuration and applied through
  Reload Configuration. Invalid choices retain safe bundled fallbacks.
- Code and UI share the text painter with independent glyph caches. UI labels
  use measured glyph advances; inputs retain matching text, selection and caret
  metrics in the editor font.
- Cursor redraws, text fields and mixed code/UI surfaces retain the correct
  font and baseline.

### Linux builds

- Fixed missing Janet and AppleScript scanner symbols when linking with GNU ld.
  Their shared compatibility build now retains the scanner objects independently
  of archive order.
- Gate the macOS-only open-file sender to its platform, keeping Linux builds
  free of unused-code warnings while retaining the shared automation tests.

### Clipboard

- Keep copied text available on Linux desktops without a clipboard manager.
  Copy and paste now share a persistent, ordered background worker instead of
  dropping clipboard ownership after each operation. This applies to editor,
  terminal and other surfaces using the existing clipboard commands.

### Keyboard shortcuts

- Settings shortcuts now work from terminals, docks and other dialogs. Chords
  for global commands work in those contexts too; editor-only bindings are
  filtered before matching so they cannot capture input or block an eligible
  global binding.
- Alt-containing chord shortcuts no longer trigger the bare-Alt double-tap
  multi-cursor gesture. Actual double-tap-and-arrow navigation is preserved.

### Terminal links

- Hold Cmd on macOS (Ctrl elsewhere) to underline a terminal web link and show
  a pointer cursor; modifier-click opens it in the default browser. Ordinary
  clicks still select text. Both plain URLs and explicit OSC 8 links work,
  including wrapped text and scrollback.
- Preview and terminal navigation share a validated HTTP/HTTPS browser launcher;
  terminal output cannot launch file, script or other custom URL schemes.

### Terminal selection

- Drag to select terminal output; double-click selects a word and triple-click
  selects a line. Copy with Cmd+C on macOS or Ctrl+Shift+C elsewhere, without
  changing the editor document or sending input to the shell. Plain Ctrl+C
  still interrupts the shell.
- Selection stays with its terminal tab. Clipboard extraction uses the terminal
  engine's wrapped-line, scrollback and Unicode handling; painting and pointer
  selection share cell geometry. Hidden terminal glyphs are no longer painted.

### Terminal tabs

- Create, switch and close terminal tabs using the session strip or the
  `Terminal: New/Close/Next/Previous Tab` commands. Each tab retains its shell
  and scrollback; previous/next controls keep overflowed tabs reachable.
- Terminal tabs share dock layout for rendering, hit testing and PTY sizing.
  Session IDs are no longer reused after closing a tab, so late output cannot
  land in a replacement session. Hiding a dock does not cancel a pending shell.

### Performance investigation

- Shared edit transactions reuse coordinate conversions when a caret and its
  selection endpoints coincide, reducing repeated Rope lookups for multi-cursor
  edits across split panes without changing selection or Undo/Redo semantics.
  [Before/after profiling](benchmark/2026-09-08-multicursor.md) measured
  two-pane line duplication at 1,000 cursors about one-third faster.
- Recency context reuses an unchanged snippet's token index instead of repeating
  whole-ring similarity checks. It still captures current text, refreshes recency
  order and region metadata, and fully deduplicates changed snippets.
  [Optimized profiling](benchmark/2026-09-08-recency-refresh.md) measured the
  unchanged 32-snippet refresh at 0.66–0.67 ms, down from 4.54 ms; edited
  refresh remains about 4.6 ms.
- Plain Find queries can use a non-overlapping literal matcher when both query
  and document are ASCII. Unicode, regex and whole-word queries retain the
  existing engine and character-offset semantics.
  [Profiling](benchmark/2026-09-08-find-literals.md) measured dense 100,000-line
  cold scans at 3.4–3.5 ms, down from about 7.1 ms, with a small matcher-allocation
  tradeoff and no document-sized case-folding copy.
- Find overview markers reuse Rope chunk coordinates instead of constructing a
  Rope slice for every matched line. Newline semantics and snapshot-bound,
  lazy overview caching remain unchanged.
  [Before/after profiling](benchmark/2026-09-08-find-overview.md) measured dense
  100,000-line worker medians of 10.2–10.3 ms, down from 11.1–11.6 ms.
- Added focused cold Find and worker-computation profiling modes, with
  [dense/sparse/absent results and CPU samples](benchmark/2026-09-08-cold-find.md).
  This adds reproducible diagnostics without changing search behavior.

### Completion efficiency

- LSP completion rows retain the original typed protocol item through filtering
  and resolve debouncing. JSON is built only when an actual resolve request is
  sent, avoiding eager copies of every candidate's fields and opaque server data.
  Snippet normalization, initial edits and resolve round-trip contents are unchanged.
- Completion-response debug traces now report identity and item count without
  formatting or logging the entire candidate payload.
- The [before/after benchmark](benchmark/2026-09-08-completion-responses.md)
  measured about 81% fewer freshly allocated bytes for 1,000-item conversion;
  this is not an end-to-end menu-latency measurement.

### Workspace-aware inline suggestions

- Providers can opt into `workspace_retrieval` context: bounded, background BM25
  ranking of workspace declarations using the existing syntax/outline registry.
  Respects ignore files, skips hidden/generated directories and symlinks, and uses
  unsaved open buffers instead of stale disk text. Requests are revalidated before
  provider submission. No workspace source is collected by default.

### Managed local suggestions

- Inline providers can opt into a window-owned llama-server using a configured
  executable and local GGUF model. Startup is on demand and loopback-only, with
  offline mode, health checks, a startup deadline and explicit retry after failure.
  Canceling generation keeps the model loaded; disabling/changing its provider
  or exiting stops the owned child. Externally managed servers are unchanged.

### Theme polish

- Hand-tuned the full overlay palettes for Fleet Dark, GitHub Dark/Light,
  Dracula, Mocha, Nord, Tokyo Night and Gruvbox Dark: distinct selected rows,
  readable secondary text and shortcut chips, and coordinated diagnostic colors.
  Custom-theme fallback behavior and the opaque Settings page are unchanged.

### Scrollbar consistency

- Settings scrolls continuously in pixels, with clipped partial rows at both
  edges. Trackpad movement and thumb dragging no longer snap to settings or
  section headings; keyboard navigation reveals rows with minimal movement.

- Settings and list modals now use the shared scrollbar geometry, standard-width
  hit area, theme colors and renderer instead of separately painted indicators.
  Thumbs can be dragged and tracks clicked, including to the last settings row.
  Settings wheel scrolling preserves the incoming scroll amount instead of
  forcing a three-row jump. Section headings are included in scroll positioning.
- Consolidated vertical/horizontal scrollbar messages and pointer-position math.
  Drag capture ends on release, focus loss, modal changes and window resizing.

### Settings design correction

- Restored the separate Settings page: category navigation, spacious preference
  rows, descriptions beneath labels, right-aligned controls and boolean switches.
  Small windows use compact categories. The command-palette-style replacement
  is removed; keybinding controls live in the page's Keymap category.

### Performance

- Settings and centered overlays skip backdrop blending behind opaque panel
  interiors. Rounded edges and translucent overlays retain correct compositing.
- Settings is always a solid page, preserving the theme's background RGB without
  allowing editor content to show through a translucent overlay theme.

- Modal backdrop dimming now reuses the clipped rectangle-blend primitive,
  removing repeated per-pixel clip-stack checks while preserving rendered colors.
- Added Settings scrolling CPU probes for debug and optimized builds, with
  separate update, hit-layout, backdrop and rendering measurements.

- Usages previews are prepared off the event loop with replaceable work and
  bounded file reads (1 MiB per file, 4 MiB per response). Slow or unreadable
  previews fall back to navigable file/line entries after 250 ms. Results retain
  unsaved-buffer previews, deduplicate identical locations before the 200-row
  limit, and discard previews from superseded queries.

- Forward edit batches index pristine offsets once per mapped position set,
  replacing per-cursor scans through every edit with binary searches. Typing,
  deletion, duplication, completion and peer-pane selections share this mapper;
  duplication records each copy's final start instead of repeatedly scanning
  later edits. Insertion affinity, replacement clipping and exact undo snapshots
  are unchanged. Selection-only Find keeps its distinct boundary behavior.

- Undo/Redo skips per-edit cursor mapping for panes restored from exact history
  snapshots. Newly opened panes and selection-only Find scopes still follow the
  edits. Position capture now uses one filtered path shared with ordinary edits.
  Release probes cover deletion, selection/line duplication and their separate
  Undo/Redo stages at up to 1,000 cursors in one or two panes.

- Completion benchmarks now cover multi-row ghost arrival, cycling, blink,
  type-through, width reflow and on-screen CPU rendering. Fixtures assert that
  previews remain projected and visible after viewport initialization; source
  rewrapping is measured separately from ordinary projection updates.

- Idle completion-context deduplication indexes each captured snippet once and
  compares retained token ranges, eliminating repeated token-tree construction
  for every pair. The exact similarity rule, recency order and capture limits
  are unchanged. Token indexes stay runtime-local; request/cache payloads remain
  text only. Completion benchmarks now cover idle fill/refresh, ordinary
  observation and attachment at 8- and 32-snippet sizes.

- Inline suggestions reuse normalized results from a worker-local LRU cache,
  bounded to 256 entries and 8 MiB of retained source/result payload. Matching
  context can replay after backspace/retype or return a compatible alternative's
  remainder after typing/acceptance. Document, file, language, provider settings,
  prefix and suffix are checked; cached replies use the current request snapshot.
  Explicit requests still fetch a fresh result. Errors and empty results are not
  cached, and no cache data is written to disk.

- Find display scans for documents of at least 256 KiB run on a coalescing
  background worker. Pending searches show “Searching…” and hide stale marks;
  replies are checked against the document, buffer revision, query and scope.
  Explicit navigation and replacement retain a fresh synchronous fallback when
  current results are not ready.

- Range decorations reuse visible-row geometry and lazily prepared text across
  highlights and diagnostics, visiting only intersecting wrapped rows. Tint and
  stroke order is unchanged. The existing text-decoration timing stage now also
  includes the range-decoration pass.

- Shared edit transactions skip old-position mapping for a pane whose carets
  already have explicit final offsets. Typing and completion avoid mapping those
  positions only to overwrite them. Paste distribution checks allocate no line
  vector for one cursor and bound it by cursor count for multiple cursors.

- Find computes overview lines only when needed, walking adjacent matches without
  repeated whole-rope position lookups. Scrollbar marks from Find and diagnostics
  share a per-pane pixel-row cache across full and caret-only redraws, refreshed
  when search inputs, document content, diagnostics, wrapping or track size change.

### Changed

- Bindable command names now derive from the command enum, so new actions such
  as ToggleUsages and RestartLanguageServer cannot be omitted from YAML parsing.
  Default keymaps share the embedded YAML registry and return independent
  snapshots for user overrides; the duplicate hardcoded keymap is removed.
  Save/Open/Quit remain as minimal emergency bindings for invalid embedded YAML.

- Loaded documents retain their original and resolved file identity. LSP
  diagnostics and current-file Problems use the same snapshot without repeated
  filesystem lookup, including differently named symlinks and missing targets.
  File URI encoding preserves Unix filename bytes, roots and parent components;
  malformed encodings and NUL paths are rejected. Async open/save integration
  follows separately.

- The LSP client decodes complete workspace-symbol responses, rejecting unusable
  locations and bounding retained rows. Shared ranking removes duplicate symbols
  independently of server response order. This protocol foundation does not yet
  enable the Search Everywhere Symbols tab on its own.

- Startup files use the same preparation and tab installation as later opens:
  images get image tabs, binaries get placeholders, and duplicate/symlink paths
  reuse one document. Successful tabs keep CLI order; failed paths remain visible
  in the status message. CLI cursor positions clamp to the first successful file.
  Workspace startup also records the workspace with recent files; recent entries
  reuse prepared document identities without another filesystem lookup.

- Model construction no longer reads configuration, histories or file paths.
  Rust callers use `AppModel::new(width, height, scale)` for an empty model or
  `with_document` for prepared text. Runtime startup owns disk preparation.

- Removed the unused `TextEditMsg`/`EditContext` routing API and `RopeBuffer`
  wrapper. Document, modal and CSV input continue through their existing message
  handlers, sharing editing primitives without a second dispatch surface.

- Editor documents and small text fields now share cursor, position and selection
  types. Duplicate definitions were removed while preserving selection direction,
  half-open ranges, Unicode text extraction and desired-column behavior.

- Dropdown settings now live under `completion.menu`: automatic opening,
  minimum candidate word length and local-word policy. Turning automatic menus
  off still allows Ctrl+Space, manual path continuation, signature help and
  configured inline suggestions. The existing `completion.enabled` master switch
  is unchanged. Legacy `completion.words` loads compatibly and is migrated on
  save; explicit nested values win and unknown settings remain preserved.

- Completion documentation scrolls independently from the suggestion list,
  including long signatures and code examples. F1 expands/collapses it, and
  Alt+PageUp/PageDown scrolls a page.
  Cards wrap into available side space without covering the menu. Selection
  changes reset the view; late local-path results preserve an unchanged server
  item's position. Wheel events use current hit-test geometry after resizing.

- Completion, hover and signature documentation use the preview's Markdown
  parser for nested formatting, matching code fences, escaped text, reference
  links, lists, quotes and tables. Code examples retain literal links and markup;
  cards remain native text with no HTML execution or remote resource loading.

- Context-aware file-path suggestions use the existing completion dropdown and
  Undo transaction, alongside language-server results. Relative paths resolve
  from the current file or workspace; directories continue into their children.
  Markdown links encode spaces, Unicode filenames retain exact prefix matching,
  and duplicate local/server insertions are shown once. Directory reads use a
  bounded, replaceable worker separate from ordered saves; stale replies cannot
  reopen a dismissed menu or edit a different document or pane.

- Find and path completion share the latest-request worker lifecycle, including
  cancellation, non-blocking shutdown and panic-to-failure replies. Find's
  synchronous fallback and mid-scan interruption remain separate follow-ups.

- Undo and Redo restore each existing pane's exact selections, cursor order,
  active cursor and desired columns, including positions clipped by deletions
  and overlapping selections merged while typing. History is tied to editor
  identity rather than whichever split invokes Undo. Newly opened panes keep
  mapped live positions; closed panes are not recreated.

- Inline suggestions occupy real visual rows, including wrapped continuations,
  and shift existing text after the cursor instead of painting over it. Explicit
  requests can preview mid-line insertions; automatic requests retain the
  configured end-of-line gate. Rendering, mouse placement, IME anchors and
  scrolling share the same projection. Ghost clicks map to the insertion point;
  source selections and diagnostics do not include the speculative text.
  Compatible type-through keeps the inline suggestion instead of opening a
  competing automatic dropdown. Escape and navigation remove the projection;
  acceptance remains an undoable document edit.

- Inline providers can opt into idle recency context from open text buffers.
  File switches, saves and large cursor jumps queue bounded snippets; the ring
  updates after 750 ms of editor inactivity and removes near-duplicates by token
  similarity. llama.cpp receives native extra context; other transports receive
  commented snippets for supported languages. Context is disabled by default,
  stays in memory, and participates in result-cache identity and memory bounds.
  Provider/workspace changes clear the ring; closed and renamed sources are evicted.

- Inline providers can opt into raw FIM prompts with `prompt_format`: Qwen,
  StarCoder, CodeLlama, DeepSeek, Codestral or Mellum, plus conservative model-name
  inference. Ollama bypasses its template in raw mode; OpenAI-compatible requests
  send the rendered prompt without a native suffix field. Native behavior remains
  the default. Prompt construction and leaked-token cleanup share vocabulary data,
  including DeepSeek's Unicode tokens. Unsupported combinations fail explicitly.

- Inline suggestions apply syntax-aware bracket sanity and indentation filters
  after cache lookup. Recognized strings/comments are preserved; ambiguous
  parser recovery leaves candidates unchanged. Rust, Go, JavaScript, C and C++
  indentation follows the document's tab/space tendency, retaining visual columns.
  Other languages keep their indentation unchanged. Analysis uses a bounded,
  local-only snapshot and a shared cooperative work budget, not extra HTTP context.
  Partial cache replay matches the normalized text actually shown to the user.
  Empty/rejected refreshes remove an older cached answer at the same context.

- Completion rows distinguish methods (`M`) from functions (`f`) and modules
  (`m`), preserving the language server's kind. Sources and rendering now share
  one completion-kind type instead of mirrored enums and a conversion table.

- Inline suggestions retain and cycle multiple provider results. OpenAI-compatible
  providers accept `n: 1..8` (default 1); unsupported transports reject `n > 1`.
  Alt+]/Alt+[ cycle compatible alternatives without editing or making a request.
  Empty/duplicate results are omitted, and already typed or accepted text is
  preserved when cycling. Ghost text shows the compatible choice position/count.
  macOS Option shortcuts try the typed character first, then the layout's
  unmodified key; unbound character input keeps its composed text. Chord state
  advances once even when the event has both interpretations.

- Inline suggestions support OpenAI-compatible native-suffix completions and
  Mistral FIM through the existing cancelable HTTP/TLS worker. Optional bearer
  credentials are referenced by environment-variable name (required for Mistral),
  never stored as resolved values in editor configuration. Authenticated remote
  endpoints require HTTPS; redirects remain disabled.

- Settings now includes LSP master/per-server switches, read-only command
  overrides with their YAML keys, and live server states. Switches share the
  existing configuration/lifecycle effects; unchanged choices do not save.
  Process-state updates refresh an open Settings or Language Servers modal
  without changing the query or selection.
  Shared overlay rows keep a gap between their text and right-hand accessories.

- Added searchable Settings (`Cmd+,` / “Open Settings”) on the shared modal
  surface. Preset chips support keyboard cycling and direct clicks, save changes
  immediately, and leave hand-written off-preset values untouched until a choice
  is made. Theme selection opens the existing picker. Cursor blink “Off” keeps a
  steady caret without a zero-delay event-loop wake cycle.
  Sectioned overlays retain their first heading at the top; narrow footers fit
  navigation hints and hide secondary text instead of overlapping it.

- Find replacements now share ordinary editing's undo transaction and position
  mapping. Replace All is one undo step, keeps split-pane carets aligned and
  places the primary caret at the actual first replacement, including multiline
  Unicode text. Selection-only bounds track edits and undo/redo, and
  Replace-and-Find no longer skips adjacent matches. Identical replacements
  preserve dirty state and redo history.

- Character/word deletion, cut, line deletion, duplication and indentation now
  use the shared edit transaction. Overlapping removals delete text once; all
  carets follow same-line and multiline edits, and no-op deletes add no undo
  entry. Backspace/Delete join CRLF lines in one operation. Non-contiguous line
  deletion retains the active caret, and duplication captures every source before
  editing so one cursor cannot change another's copied text. Copy/cut preserve
  per-selection clipboard order and report character counts rather than bytes.

- Typing, newline insertion and paste share one multi-cursor edit planner. They
  replace each selection, keep same-line sibling carets aligned, and apply a batch
  as one undo step. Surround preserves peer positions inside the selected text
  and places the accepting caret correctly after Unicode text. Paste character
  counts now report characters rather than UTF-8 bytes.

- Ordinary edits now map other split panes' cursors and selection endpoints from
  the actual edit operations, sharing completion and undo/redo mapping. This
  fixes multiline paste columns, newline joins, selection replacement and batch
  edits leaving peer positions behind. No-op edits preserve peer navigation state.

- Open documents now share a boundary-resolved file identity across tab reuse,
  navigation, LSP diagnostics and Problems scope. Differently named symlinks no
  longer lose current-file Problems, and lookups do not recanonicalize files.
  Successful saves/reloads refresh identity; stale replies and path changes
  cannot keep using aliases belonging to an old file.

- Configuration directory preparation, default-keymap creation and log selection
  now run on the ordered file worker, sharing the normal file-open request/reply
  path. Delayed config-file opens retain their original split and respect newer
  tab choices; closed groups and duplicate replies cannot reveal a directory.

- New-tab file reads, validation, alias lookup and image decoding run on the
  ordered background file worker. Delayed opens retain their requesting split;
  navigation converts and clamps coordinates only after the destination loads.
  Newer tab/cursor choices are not displaced by an older open reply. Reusing an
  image or binary file in another split preserves its viewer/placeholder mode.
  Split image views share immutable decoded pixels instead of copying them.
- Native file-dialog selections open in their original group, even if focus
  moves while the dialog is open. CLI/automation open acknowledgements wait for
  tab installation; `--wait` still waits for those documents to close.
- Workspace edits prepare closed text files in the background before applying
  edits or acknowledging success. Changed/closed targets and load failures
  reject the deferred operation; code-action follow-up commands wait for edits.

- Save, Save As and explicit reload replies are tied to the initiating document.
  Switching tabs cannot apply them to another buffer, and a stale reload cannot
  replace intervening edits. Save As keeps the document selected when its dialog
  opened and changes its path only after a successful write.
- Save and Save As share an ordered background writer, preventing older writes
  from overtaking newer ones within a window. Save completion and undo/redo use
  the saved text snapshot, preserving edits made during a save and distinguishing
  different undo branches at the same history depth. LSP save notifications carry
  the actual saved text.
  File-reply tracing excludes buffer contents.
- Save/Save As reject image and binary placeholders instead of writing their
  placeholder text buffer over the file. Text-backed CSV editing remains savable.

- Configuration-opening actions share one runtime preparation command. Keyboard
  and palette log actions now follow the same path; preparation failures appear
  in the status bar. Log selection ignores backup files and directories and
  reports when no log is available.
- Opening keybindings or logs now opens/reuses a normal tab, preserving the
  previously focused buffer and unsaved edits in an already-open resource.
  Default keymap creation uses exclusive creation and never replaces an existing
  file, including an empty keymap or a symlink to a custom keymap.

- Palette and context-menu shortcut hints now come from the loaded keymap,
  respecting rebindings, unbinding, conditions and shadowed sequences. Keycaps
  support platform modifier symbols/text.
- User keymaps accept space-separated chords such as `ctrl+k ctrl+c`. Editor
  dispatch resolves each keystroke once, fixing lost non-global chord completions.
  Manual keymap changes load when reopening Settings → Keymap or restarting;
  automatic file watching is not implemented.

- Document cursor movement now shares one internal target/selection-policy path
  for arrows, words, line/document boundaries and paging. Existing shortcuts,
  selection behavior and wrapped navigation are unchanged.
- Rust integration callers must send messages through `update(model, Msg)`;
  individual message handlers and internal LSP/syntax scheduling exports are no
  longer public. Runtime/view helpers with existing callers remain available.

### Added

- Keymap preferences share the existing YAML parser and merge engine for base
  presets, typed conflict checks and override serialization. Optional
  `base: conventional` changes `cmd+p`, `cmd+shift+p` and `cmd+d`; user entries
  still win. Chord parsing now shares one sequence parser, supports Unicode
  character keys, literal `plus`/`literal_space` aliases and F1–F24, with bounded
  keymap-file reads. Bindable names and the unassigned-command list derive from
  the command enum rather than another registry.

- Settings has a Keymap category with searchable merged bindings, shared
  shortcut chips, context-aware exact/prefix conflict warnings, and explicit
  four-stroke capture/save/cancel. Captured shortcuts do not execute editor or
  debug commands. Saves update the live keymap and persist OS-local overrides;
  sibling contexts, foreign-platform entries and unknown YAML keys are preserved.
  Stale, invalid, read-only and linked files are not overwritten. The optional
  Common base changes only Command+P, Shift+Command+P and Command+D;
  user overrides retain precedence. YAML comments/formatting are not retained.

- Search Everywhere searches workspace symbols from running, capable language
  servers. The Symbols tab (or `@` on an empty query) offers keyboard/mouse
  navigation; All includes a capped symbol group. Queries are debounced and
  cancelled on change, with stale/restarted-server replies rejected. Results
  are deduplicated, capped, and opened through shared UTF-16-aware navigation;
  loading, partial failure and result-limit states are visible.

- TabbyML inline completions through `transport: tabby`, using its native segments
  API and the shared cancellation, bounded-response, credential, filtering and
  acceptance pipeline. Model selection and generation limits stay server-side.

- Local inline completion statistics record one outcome per offered response:
  accepted (including partial acceptance), dismissed, or fully typed through.
  Only configured provider names and aggregate counts are stored; no source,
  suggestion text, connection settings, or network telemetry. Counts are merged
  on the background file worker. “Open Inline Completion Statistics” opens the
  JSON file; `completion.inline.statistics` and the corresponding Settings
  control can disable collection.

- Find Usages opens persistent, file-grouped results in the Usages dock panel.
  Results stay available while navigating; Show Usages retains the transient
  popup. The panel supports mouse and keyboard navigation, collapse/expand,
  paging and scrolling, and can be reopened with View: Toggle Usages. Loading,
  cancellation, unavailable servers, timeouts, empty results and the 200-result
  limit are shown explicitly. Late responses do not steal focus or reopen a
  closed panel.

- Markdown preview renders fenced `mermaid` diagrams, using the preview theme
  and a pinned, on-demand Mermaid renderer. Loading requires a connection to
  jsDelivr; offline/loading failures and invalid syntax keep the source visible
  with an explanation. Other code fences retain syntax highlighting. See
  `samples/mermaid.md`, including diagram-local spacing for self-loop labels.

- **Cancelable inline providers:** Ollama `/api/generate` joins llama.cpp
  `/infill`, with model selection, configurable `keep_alive` (default `-1`),
  and a clear error for models without suffix support. Both transports support
  HTTPS with certificate validation. Superseding, dismissing, changing panes,
  moving/selecting, disabling or reconfiguring the provider, losing focus, and
  quitting cancel pending work. Replies are size-bounded and redirects are not
  followed; canceled requests do not count toward failure backoff.

- **Partial inline acceptance:** Cmd+Right (Ctrl+Right on Windows/Linux) accepts
  the next leading word run while a suggestion is visible. “Accept Inline
  Suggestion Line” accepts through the next newline and is available in the
  palette, automation, and custom keybindings. Each portion is one undo step;
  the remaining ghost text stays visible without another backend request.
  Acceptance uses the active cursor and preserves other cursors and selections
  in split panes. Full accept and dismiss are now palette-visible as well.

- **Per-pane soft wrap**: Alt+Z or “Toggle Soft Wrap” in the command palette
  wraps at whitespace without changing document text. Rendering, selection,
  mouse placement, caret anchors, arrow/page movement, and scrolling use the
  shared visual-row mapping. Continuation rows have gutter markers; wrapped
  panes disable horizontal scrolling. Layout updates reuse unaffected lines
  after edits, including edits seen by another split. Automation reports
  `soft_wrap` and `visual_row_count`; screenshot files accept `soft_wrap`.

- **Inline suggestions (ghost text)** (autocomplete Phase 2): a
  `completion.inline` config block plus a `completion.providers` entry point
  Token at a local llama.cpp server's `/infill` endpoint. Typing at the end
  of a line schedules a debounced request on a worker thread; the reply
  appears as dimmed ghost text after the cursor (multi-line suggestions show
  the first line and a `⏎ +N lines` badge). Typing through it consumes it,
  Backspace un-consumes, Tab accepts the rest as one undo step and chains
  the next request, Escape dismisses, and ⌥\\ asks explicitly. Replies are
  guarded by document, revision, and cursor; a menu completion wins over
  ghost text. Backend errors are status transients, and auto-trigger pauses
  after three consecutive failures until triggered manually. New theme key
  `editor.ghost_text` (derived from foreground/background when absent),
  `inline_suggestion` in the automation `state` snapshot, and an
  `inline_suggestion` field in screenshot scenarios.

- **Find options, match count, and selection scope** (find-enhancements
  Phases 5 and 7): the Find label row shows "3 of 42", "No matches", or the
  regex error; the footer lists the case, whole-word, regex, and selection
  options with their keys and a check when on. ⌥⌘C, ⌥⌘W, ⌥⌘R, and ⌥⌘L toggle
  them inside the modal. Selection scope captures the primary selection when
  switched on and restricts navigation, highlights, replace, and replace-all
  to it; reopening the modal re-captures the scope from the live selection.
  The automation overlay snapshot reports `status` and `options`, and
  screenshot scenarios accept `whole_word` and `use_regex`.

### Fixed

- Modal pointer actions now preserve their returned runtime commands. Choice
  saves, tab loads, row activation and outside-click theme restoration use one
  path instead of silently replacing effects with a redraw.

- Conditional chord prefixes now check the same context rules as completed
  bindings. Inactive branches no longer capture keys or keep a pending sequence
  alive; eligible alternatives and existing precedence are preserved. The
  documented `sidebar_focused` keymap condition is now accepted by YAML parsing.

- Typing a server-declared commit character accepts the selected LSP completion
  and inserts the character in one Undo step, including auto-imports and snippet
  caret placement. Punctuation remains visible during item resolution; later
  typing, navigation, focus or file changes invalidate the pending acceptance.
  Paste and multi-character keyboard text do not accept highlighted items.
  Navigating the menu also withdraws an older Enter acceptance, and synchronous
  resolve fallback preserves the final syntax-update revision.

- LSP completion now retains auto-import and other `additionalTextEdits` sent
  in the initial completion response, including when the server cannot resolve
  items or a later resolve fails. Those edits apply with the primary completion
  in the existing single-cursor transaction and undo step.

- Context menus now highlight the row under the pointer and repaint immediately
  when it changes or the pointer leaves. Completion, code-action and reference
  popups share this hover state without changing keyboard selection. Separators
  remain unselectable.

- Multi-split render profiling now uses independent documents, cycles supplied
  files correctly, activates CSV/TSV grid mode, and uses real syntax highlights
  and mode-specific scrolling. Invalid input files fail setup instead of yielding
  misleading measurements. Run it with `just profile-render`.

- Dropdown completion no longer mixes buffer words or generic snippets into
  member access (`.`, `::`, `->`). Local suggestions require a matching prefix;
  code fallback excludes syntax-highlighted comments, strings and keywords,
  ignores numbers/symbols, and prefers nearby identifiers. Empty local results
  no longer prevent language-server requests or intercept editing keys. Server
  relevance and preferred selection are retained, and method parameters/return
  metadata are displayed when supplied by the server.

- Completion now places every same-line cursor correctly and keeps split-pane
  cursors and selections aligned. Overlapping word prefixes complete once.
  LSP boundary inserts no longer get swallowed by the primary completion;
  snippet caret placement is retained through redo. Planned edits preserve
  selection direction, and undo/redo transform live positions in peer panes.

- Cursor movement no longer copies undo/redo history. Find navigation, status,
  highlights and overview marks share revision-checked search results; visible
  highlights are filtered before position conversion. Wrapped rendering copies
  only the current visual segment, avoiding repeated whole-line allocations.
- Resize and font changes now size split-pane viewports from their actual group
  rectangles. Configuration save failures are reported in the status bar.

- Inline suggestions now show a status-bar progress glyph while a provider
  request is running. Explicit requests obey the same paint-time tail limit
  as automatic requests, hidden suggestions cannot intercept Tab, and debug
  builds render ghost text on full redraws. Superseded replies cannot clear
  a newer request's progress or replace its suggestion.
- Configuration saves preserve unknown YAML keys, including nested keys,
  without restoring deliberately removed known settings. Invalid or unreadable
  existing files are left untouched. YAML comments and formatting are not retained.

## v0.6.0 - 2026-09-02

### Added

- **CLI handoff and `--wait`**: `token file` now returns to the shell
  immediately and opens the file in the running editor (over the automation
  socket), starting a detached editor when none is running. `-w/--wait`
  blocks until every opened tab is closed or the editor exits, Zed-style, so
  `git config core.editor "token -w"` works. Directories and `--new-window`
  always start a separate editor process (`-w` then waits for that window).
  `path:line:col` suffixes position the cursor; `token -` reads stdin into a
  new tab; `--foreground` runs the editor in the calling process.
- `token automate open <paths…>` and an `open_paths` MCP tool open files in
  the running editor.
- **Multi-instance automation**: every editor process now listens on its own
  endpoint (`$TMPDIR/token-<uid>/instances/<pid>.sock`, a loopback port file
  on Windows) instead of one shared socket that only the first process could
  own. `token automate instances` lists running editors, `--instance <pid>`
  targets one, and the MCP bridge gains `list_instances` plus an optional
  `instance` argument on every tool; the default target is the most recently
  focused editor. `token file` opens in the editor whose workspace contains
  the file, and `token dir` focuses the editor already showing that workspace
  instead of starting a duplicate.
- macOS: `Token.app` declares document types and handles
  `application:openURLs:`, so Finder "Open With", the Dock, and
  `open -a Token file` deliver files to the running editor.

### Changed

- The automation `state` snapshot reports `instance_id` instead of
  `process_id` (same value: the editor's process id) and adds
  `workspace_root` and `focused_at_ms`.

### Fixed

- `Cmd::Quit` triggered from automation only took effect on the next window
  event; the event loop now exits from `about_to_wait` as well.
- The Windows automation server handled connections serially, so one slow
  client stalled every other; it now uses a thread per connection like Unix.

- **LSP completion (Phase 5)**: `textDocument/completion` now feeds the
  autocomplete menu in Rust, TypeScript/JavaScript, Python, and PHP files.
  Requests are debounced while typing (120 ms) with flush-before-request,
  revision-guarded responses, and server trigger characters (e.g. `.`)
  keeping the menu open with a fresh member query. Server ordering
  (`sortText`) ranks LSP items above buffer words and snippets; matching
  uses `filterText ?? label`. Accepting an item whose server advertises
  `resolveProvider` resolves first, so ts-ls auto-imports apply;
  `textEdit` + `additionalTextEdits` land as one undo step with the
  primary range re-anchored to the live cursor. Incomplete lists re-request
  on every keystroke instead of trusting local filtering.
- Per-server LSP settings: `lsp.servers.<id>.initialization_options` is
  sent verbatim as `initialize`'s `initializationOptions`, and
  `lsp.servers.<id>.settings` answers `workspace/configuration` section
  lookups (dotted paths; missing sections reply `null`). Previously every
  server got all-null configuration and no init options, making pyright and
  rust-analyzer effectively unconfigurable.
- PageUp/PageDown navigate the completion popup by a full visible page.
- **Toggle Autocomplete** command (palette): turns the whole completion
  menu off, Ctrl+Space included; persisted as `completion.enabled` in
  `config.yaml`. New `completion.words` mode: `fallback` (default) hides
  buffer words once the language server answers, `enabled` always lists
  them, `disabled` never does.
- **Set Language...** command (palette): a picker over every registered
  language that overrides the inferred language of the focused file for the
  session. The override is pinned to that document and survives external
  reload and Save As. Save As on an unpinned file now re-detects the
  language from the new extension (it never did before).
- **Next / Previous Diagnostic** (F2 / ⇧F2): walks the focused file's
  diagnostics in order, wrapping, and flashes the message in the status bar.
- **Problems panel scope**: a palette command switches between the focused
  file (default) and every file with diagnostics; the tab reads
  "Problems · N files" when workspace-wide.
- A persistent status-bar segment shows the focused file's language server
  state (`rust-analyzer: ready` / `indexing` / `not found` / `LSP off`).
- LSP completion items sent in snippet format now insert readable text:
  placeholders are flattened (`${1:arg}` → `arg`), tab stops removed, and the
  caret lands at `$0`.
- **Styled text in overlay cards.** Hover cards, the completion docs card,
  and signature help now keep structure from the server's markdown instead
  of flattening it: inline and fenced code render as recessed chips,
  emphasis and headings as bold, and diagnostic banners chip their
  backticked identifiers. Signature help marks the active parameter as an
  accent run (the previous `‹›` brackets are gone) and dims the "(n of m)"
  counter. Intraword underscores such as `snake_case` are no longer eaten
  as emphasis. Completion rows chip the server's `detail` (the type
  signature), and a docs card that opens with a code fence shows it as a
  code block above the prose, like the hover card.
- **Completion documentation card**: selecting an item resolves its
  `documentation` lazily (150 ms debounce) and shows it in a card beside the
  menu, flipping to the left when there is no room. Accepting an already
  resolved item no longer waits for a second round trip.
- **Signature help** (`textDocument/signatureHelp`): opens above the caret
  on the server's trigger characters (`(`, `,`) and on ⌘P, retriggers while
  typing inside the call, marks the active parameter, and closes when the
  caret leaves the line or on Escape.
- Server-initiated `workspace/applyEdit` requests are now applied (one undo
  step per file, closed files opened in place) and acknowledged, instead of
  being refused.
- **Rename Symbol** (⇧F6): prompts with the symbol under the caret (via
  `prepareRename` when the server supports it), then applies the server's
  `WorkspaceEdit` across open and closed files with one undo step per file
  and reports "Renamed in N files, M edits".
- **Show Code Actions** (⌥↩): lists the server's quick fixes and refactors
  for the selection or caret, preferred actions first; Enter applies the
  edit or runs the server command.
- **Format Document** (⌥⌘L) and **Format Selection** (palette) via the
  language server, applied as one undo step. New `format_on_save` config
  (default off) formats before writing and falls back to saving unformatted
  if the server does not answer within two seconds.
- CSV mode mouse editing: clicking inside the cell being edited places the
  caret at the pressed character (Shift extends the selection), double-click
  opens the cell editor with the caret at the pressed character, and
  double/triple-click inside an open editor selects the word / everything.

### Changed

- The completion menu auto-trigger now requires a two-character prefix
  before it opens (Ctrl+Space is unaffected and still works on an empty
  query). Buffer-word self-exclusion is case-insensitive: typing `Value`
  no longer suggests `value`.
- Scrolling the editor or losing window focus dismisses the completion
  popup instead of leaving it visually detached from its word / claiming
  keys while unfocused. Completion rows highlight under the mouse like
  modal rows.
- Explicitly triggering completion in a file type without support now
  flashes "Completion unavailable for this file type" instead of doing
  nothing silently.
- A go-to-definition reply that lands after focus moved to another
  tab/split is now dropped, matching hover and references, instead of
  jumping the editor that is no longer focused.
- Added a right-click context menu for the editor text area, tab bar, and
  file tree, plus Shift+F10 to open the editor menu at the caret. Reuses the
  command palette's popup chrome and keycap hints; Up/Down/Enter/Escape
  navigate, any other key or an outside click dismisses it.
- New `src/layout/` module: a pure-Rust adaptation of the Clay layout engine
  (declarative element tree, fit/grow/percent/fixed sizing, floating anchored
  elements, clip chains, measure-callback text wrapping, virtualized row
  lists) producing a queryable geometry snapshot shared by rendering,
  hit-testing, and update-layer queries.
- `Frame` clipping is now a nesting stack (`push_clip`/`pop_clip`);
  `set_clip`/`clear_clip` keep their absolute semantics.

### Changed

- The top-level window shell (sidebar, editor area, right/bottom docks, and
  status bar) now lays out through the Clay snapshot. Rendering, screenshots,
  hit-testing, and editor split layout consume that shared shell geometry.
- Dock chrome (headers, tabs, panel content) and the Problems/Outline panels
  now lay out through the new layout engine: one solved geometry snapshot is
  shared by rendering, hit-testing, and scroll/capacity logic, replacing six
  independently rebuilt layout chains. Panel content is clipped to its rect,
  and a partial bottom row is now painted (it was already clickable).
- Centered and cursor-anchored overlays now declare their panel, tabs, header,
  list rows, fields, content zones, and footer through Clay. Painting and
  pointer routing consume the resulting snapshot instead of rebuilding boxes.
- Sidebar file-tree painting, hit testing, traversal, and scroll clamping now
  share one Clay `RowListView`; the duplicate legacy tree viewport was
  removed.
- Editor tab strips and preview chrome now use narrow Clay layout trees. Tab
  painting, hit testing, drag targets, wheel scrolling, and active-tab reveal
  share solved tab geometry; preview painting, pointer routing, hosted webview
  placement, and screenshot compositing share the solved header/content split.
  The superseded `TabBarLayout` and generic `Pane` geometry were removed.
- The hover card and drop overlay now wrap their text with real glyph
  advances measured through the font (via the layout engine's measure
  callback) instead of a fixed 8px-cell approximation; the measured plan is
  computed once per layout and shared with painting.

### Fixed

- Clicking another CSV cell while one was being edited left the editor
  overlay and IME caret stranded on the old cell; the click now commits the
  edit in place and selects the clicked cell.
- CSV viewports are sized from their own group's content height, so a
  top/bottom split no longer thinks more rows fit than are drawn.
- The modal IME caret rect now lands on the painted caret (it was inset a
  second time and ignored the palette prompt glyph).
- Outline, Problems, and Terminal keyboard routing—and Terminal spawn/resize
  sizing—now follow panels when they move away from their default docks.
- Long single-line drop-overlay messages now paint the measured wrapped-line
  plan instead of overflowing as one centered line.
- The dock header separator is drawn at the scaled chrome border width
  again, restoring its 2px thickness on HiDPI displays after the clay
  layout migration briefly hardcoded it to 1px.
- Fixed a debug-build crash when rendering an active terminal inside a dock
  panel whose content already established an enclosing clip.
- Dock tabs now advance by their clamped widths, so a width-clamped tab no
  longer leaves a phantom gap before the next tab.
- Outline expand/collapse now clamps scrolling with the same
  count-minus-capacity formula as every other path, so collapsing near the
  end of a long outline can no longer scroll the panel past its own content.
- Problems/Outline scroll, capacity, and row hit-testing now follow the
  panel to whichever dock hosts it instead of assuming bottom/right.

- Allowed manual outline scrolling to move beyond the selected symbol without
  the viewport snapping back to keep that symbol visible.
- Restored outline keyboard navigation by preventing focused outline keys from
  being handled as editor keybindings.
- Matched outline tree traversal to the file tree: Left collapses or selects
  the parent, while Right expands or advances to the next visible symbol.
- Added the copyright notice to the standard macOS About panel.

## v0.5.1 - 2026-08-12

### Fixed

- Constrained Markdown and HTML preview webviews to the editor area when the
  bottom dock is opened or resized on macOS.

### Changed

- Reduced the workspace-plus-large-Rust-file startup stress test from a 166.4
  ms release median to 86.1 ms by preparing fonts, application state, and the
  workspace concurrently with AppKit, then moving the macOS application menu
  and file-system watcher off the first-frame path. Startup-critical font
  dependencies are also optimized for the `just workspace` debug workflow.
- Audited the website homepage against the current language registry, release
  artifacts, installer behavior, keymap, rendering architecture, and public
  development record; replaced the unsupported `<50ms` startup claim with a
  measured `~86ms` figure and corrected stale stats, shortcuts, download
  links, and AI-development copy.
- Updated the website's Astro and JavaScript dependencies to remediate known
  Dependabot security advisories.
- Updated transitive `rand` dependencies to remediate two Dependabot security
  advisories.
- Improved the legibility of inline website keybindings with system-font
  modifier symbols and wider spacing.
- Added plain-English tooltips to inline website keybindings.

## v0.5.0 - 2026-08-11

### Added

- Human-readable, const-friendly `ByteSize` quantities now centralize binary
  size limits and B/KiB/MiB/GiB display formatting.
- AppleScript syntax support using the pinned HelgeSverre Tree-sitter grammar,
  including `.applescript` detection, highlighting, structural selection
  expansion, handler/property outlines, and a representative sample fixture.
- Tree-sitter parsing, highlighting, file detection, fenced-code aliases, and
  structural selection coverage for C#, Ruby, Lua, R, Swift, Elixir, Gleam,
  Solidity, Kotlin, Dart, Julia, Haskell, OCaml, D, Objective-C, VHDL, Odin,
  Fish, Assembly, SCSS, CMake, Make, Common Lisp, Zig, GLSL, GraphQL,
  HCL/Terraform, Nix, Ada, Erlang, Clojure, Nushell, sed, Tcl, Roc, Janet,
  Forth, Protobuf, Dhall, Pkl, Hurl, WIT, Standard ML, Nim, Astro, AWK, KDL,
  Tera, Typst, Dockerfile/Containerfile, WGSL, SQL, V, CUE, Fennel, Pest, and
  Pony. Every syntax fixture is now covered by a registered language.
- Syntax-tree snapshots can now retain embedded language trees alongside their
  host tree. Markdown fences and HTML-family script/style regions use
  document-relative included ranges so expand-selection can move through both
  embedded-language and host-language scopes.
- macOS releases now include native `Token.app` archives for both Apple
  architectures alongside the existing CLI artifacts. The app bundle supplies
  the icon, version, category, copyright, and product metadata used by Finder
  and the standard About panel.
- Syntax-aware expand selection now walks current tree-sitter nodes through
  identifiers, expressions, delimiter interiors, strings, blocks, and owning
  declarations before falling back to line/document scopes. Revision checks
  preserve the existing fallback for stale or unavailable trees, and shrink
  now restores complete multi-cursor selection snapshots.
- Deterministic `--demo` launching and cursor-free local automation through CLI
  and MCP clients, including document/state inspection, cursor and selection
  control, text insertion, scrolling, named keymap action discovery and
  dispatch, bounded real-renderer stage profiling, and end-to-end syntax
  pipeline profiling.
- Embedded terminal dock panel with async PTY spawning, VT/ANSI terminal emulation, keyboard and paste routing, scrollback, resize-aware grid sizing, and native rendering in the bottom dock.
- First-class Svelte syntax support, including `.svelte` detection, Svelte/HTML
  highlighting, TypeScript and CSS injection, outline extraction, syntax-aware
  selection, and a mixed-language sample fixture.

### Changed

- Syntax-aware selection behavior is now supplied through composable language
  profiles, preserving the existing markup, YAML, INI, Rust, delimiter, and
  plaintext fallback behavior while giving new grammars explicit extension
  points for normalization and additional semantic ranges.
- Each language now has one authoritative registry descriptor composing
  metadata and detection, lazy parser/query construction, selection, outline,
  and injection behavior. This removes parallel language switches while
  retaining explicit per-language extension points.
- The language registry now generates `LanguageId`, descriptor modules, and
  the complete inventory from one declaration. Selection profiles are stored
  directly, and overlapping injected regions deterministically prefer the
  smallest successful containing tree.
- Desktop product metadata is consistently branded as “Token” while preserving
  `token` as the command name. Direct macOS launches now use the capitalized
  name in the application menu, the native About panel links to the Token
  website, and Windows executables carry complete version and product resources.
- Developer tasks have moved from Make to a grouped, self-documenting Justfile.
  All former build, run, test, benchmark, profiling, coverage, sample,
  cross-compilation, installation, and packaging workflows remain available as
  `just` recipes, with filterable tests and portable profiling output hints.
- Document search now avoids per-character hash maps and redundant
  case-sensitive document copies, with an allocation-light ASCII
  case-insensitive fast path.
- Syntax worker completion now wakes the window event loop immediately, rather
  than waiting for an unrelated input or cursor-blink event before applying
  and presenting updated highlights.
- Incremental tree-sitter updates now rerun highlight queries only for expanded
  changed-line ranges and merge those patches into existing highlights.
  Outline extraction is demand-driven while its panel is closed and refreshes
  immediately when the outline is opened.

### Fixed

- Windows release builds no longer fail when compiling SCSS, Standard ML, and V
  Tree-sitter grammars with MSVC.

- macOS text-input accessories, including the Caps Lock indicator and IME
  candidate window, now follow the active editor, modal, or CSV cell caret
  instead of appearing at the top-left of the window.
- Syntax-aware expansion at the end of a code line now recovers the nearest
  completed named syntax node instead of jumping to an enclosing block. Opening
  delimiters retain right affinity, ambiguous separators fall back to the
  current line, and CSS/SCSS completed rules start with their declaration block.
- Expand selection inside Svelte script blocks no longer interleaves malformed
  host-tree ranges with the injected TypeScript tree. Expanding within
  `toggleTag` now selects that function before moving to the enclosing script
  and component scopes.

- Opening multiple files from the CLI now creates a distinct tab for every
  path again; deferred startup loads previously raced to overwrite the focused
  tab, which made `just test-syntax` appear to open only one sample.
- Expand-selection history is now invalidated by document edits and all
  cursor/selection-changing commands, preventing shrink from restoring stale
  positions. Syntax string interiors now handle Python triple quotes, Rust raw
  strings, and C++ raw-string delimiters correctly.
- Syntax-aware expansion now excludes tree-sitter YAML comments attached to a
  preceding sibling scope and INI whitespace around `=` values. Rust items add
  a documented/attributed scope containing attached doc comments and attributes
  such as `#[test]` and `#[should_panic]`.
- Syntax-aware expansion now applies reusable boundary-affinity profiles to
  XML, HTML, Vue, Svelte, JSX, and TSX: expansion at the end of a completed
  element selects that element without indentation or trailing whitespace,
  while expansion after an opening tag retains right affinity to its content.
- HTML-family void elements such as `<source>`, `<br>`, and `<img>` now stop
  selection at the end of their opening tag instead of including the newline
  and indentation that tree-sitter attaches before the following sibling.
- Sema highlighting is synchronized with upstream through commit `39a7dc9`,
  adding `def`/`defn`, workflow and policy definitions, newer special forms,
  and workflow-related builtins.
- Terminal spawn lifecycle now tracks in-flight PTY creation, avoids duplicate spawns while one is pending, and discards late spawn results if the terminal panel has been closed.
- Dock resizing now grows the right dock when dragging its handle left and grows the bottom dock when dragging its handle up.
- Terminal cursor rendering now uses the scrolled grid row instead of the visible row, so the cursor glyph stays correct when viewing scrollback.

---

## v0.4.1 - 2026-07-05

### Fixed

- CSV: `sync_cell_edit_to_document` used byte offsets (from `char_indices`) directly with `Rope::remove`/`insert`, which expect char offsets — corrupting or panicking on any CSV cell edit near multi-byte UTF-8 content (accents, CJK, emoji).
- Opening keybindings/log file while a non-text tab (image/CSV/binary) was focused silently overwrote its document without resetting `view_mode`/`tab_content`, leaving the wrong renderer active.
- Split views: `DeleteForward`, `IndentLines`, `UnindentLines`, `Duplicate`, and single-cursor `PasteText` now sync peer editors' cursors on the same document; previously only some paths did, causing split-view cursor drift.
- Multi-cursor auto-surround (typing a bracket/quote around a selection) could corrupt the buffer when selections overlapped (e.g. from "Select All Occurrences" with overlapping matches); overlapping selections are now merged first.
- `hit_test_ui` cloned the entire `EditorArea` (every open document's undo/redo history included) on every mouse move/click; it now derives splitter geometry read-only instead.
- Rectangle (block) selection dragged down-and-right used to skip any intermediate line shorter than the live drag column instead of clipping the selection to that line's length.
- `OpenFileDialogResult` discarded each opened file's syntax-parse/recent-files commands when opening multiple files at once.
- Undo/Redo unconditionally marked the document as modified, so undoing back to the exact saved state still showed a dirty indicator.
- `CopyAbsolutePath`/`CopyRelativePath` performed clipboard I/O directly in the pure update layer instead of returning a command.
- `ReloadConfiguration` only triggered a status-bar redraw, so theme changes didn't visually apply until another event.
- Large text inserts (e.g. IME commit) were split into a loop of per-character `InsertChar` messages; they're now a single atomic insert with one undo record.
- `TextEditMsg::Paste` discarded its already-captured clipboard text and re-read the OS clipboard asynchronously instead.
- `SetCursorPosition`, `ClickRow`, and `JumpToSymbol` could write out-of-range cursor positions from a stale outline; all three now clamp to document bounds.
- CRLF files: cursor-column math and line-length clamping counted the trailing `\r` while rendering stripped it, causing off-by-one cursor placement near the end of a line.
- Binary-file placeholder button hover state was a single global flag, so hovering one split group's button also highlighted another visible group's button; focus ring was also hardcoded on.
- Theme picker modal drew every theme with no scrolling or clipping, and used an uncapped vertical position unlike other modals.
- Outline panel didn't auto-scroll to keep the keyboard-selected item visible.
- Modal text inputs hardcoded zero horizontal scroll, so the cursor could scroll out of view and become invisible on a long query.
- Various minor correctness/performance cleanups: redundant unwrap in tab-move, O(n²) cursor-line dedup, unchecked arithmetic in viewport/scroll math, full-content-rect clearing instead of the editor sub-rect, degenerate-rect handling in frame clipping, redundant per-pixel alpha re-derivation in blend/dim, jumpy (non-minimal) list scroll behavior, and a focus ring that was skipped on small buttons.

## v0.4.0 - 2026-07-05

### Added

- Pure Elm Boundaries: Shifted side-effect layers (like thread spawning, clipboard integrations, and filesystem writes) out of `AppModel` mutations and pure `update` handlers into non-blocking, asynchronous command (`Cmd`) definitions and completion messages (`Msg`).
- Asynchronous Clipboard integrations: Implemented asynchronous clipboard copy/cut operations (`Cmd::CopyToClipboard`) and non-blocking clipboard paste requests (`Cmd::RequestClipboardPaste`) running in background runtime workers.
- Centralized Clipboard routing: Main runner processes clipboard paste asynchronously and contextually dispatches `AppMsg::PasteFromClipboard` to either active Modal inputs, CSV cell editing buffers, or active text documents.
- Async config operations: Shifted keymap configuration creation (`Cmd::CreateDefaultKeymapFile`) to background threads, returning `AppMsg::KeymapCreated` on completion to cleanly load keybindings.
- Profiling: Chrome trace export via `make profile-chrome` — all 28 render stages emit named `tracing` spans (`frame`, `render_stage`) visible in Perfetto UI. Feature-gated (`profile-chrome` / `profile-tracing`) with zero overhead when disabled.
- CLI: `--version` now shows git commit info (e.g. `token 0.3.19-48-g33d40cd`) for easier build traceability.
- Image Viewer: Open image files (PNG, JPG, GIF, BMP, WebP, ICO) inline in editor tabs with aspect-ratio-preserving scaling and checkerboard transparency background.
- Binary Placeholder: Unsupported binary files now open a tab with a centered placeholder showing filename, file size, and a clickable "Open with Default Application" button.
- Syntax: Added Just/Justfile language support with syntax highlighting via tree-sitter-just. Detects `justfile`, `Justfile`, `.justfile`, and `*.just` files.
- Themes: `image_preview` section with `checkerboard_light`, `checkerboard_dark`, and `checkerboard_size` for customizing the image viewer transparency background. Per-theme values in all 9 builtin themes.
- Themes: `button` section with 6 color properties (background, background_hover, background_pressed, foreground, border, focus_ring) for UI button styling.

### Improved

- Pure, thread-free state transitions: Refactored `AppModel::record_file_opened` to be a pure, thread-free state mutation, deferring disk writes to `Cmd::SaveRecentFiles`.
- Compiler Lints: Resolved 10 pre-existing compiler warnings and Clippy lints (including unnecessary sorting, manual checked divisions, and loop counters) to ensure a warning-free compilation.
- Rendering: Frame clipping system — sidebar and other panels no longer render outside their bounds. TextPainter pixel writes are routed through the Frame clip rectangle.
- Syntax: Hierarchical highlight name resolution now walks the full capture name chain (e.g. `keyword.control.import` → `keyword.control` → `keyword`) instead of only trying one parent level.
- Syntax: Sema now uses its own canonical grammar (vendored [`tree-sitter-sema`](https://github.com/sema-lisp/tree-sitter-sema)) instead of piggybacking on the Racket grammar, with the upstream highlight query covering Sema's LLM/agent builtins and namespaced stdlib functions.

### Added

- Tabs: drag a tab to reorder it within its group (live reorder past a 4px threshold), or drag it onto another pane's tab bar or content area to move it there — the move happens live at the hovered position and focuses the target pane. A semi-transparent ghost of the dragged tab follows the cursor.
- Tabs: tab bars now scroll horizontally when tabs overflow — the mouse wheel scrolls the tab strip under the cursor, and switching/opening/closing tabs automatically keeps the active tab in view.

### Fixed

- Sidebar text no longer overflows into the editor area on long filenames.
- Syntax: overlapping query captures now resolve with tree-sitter's last-match-wins convention (specific overrides beat generic fallbacks). Previously the first capture won, which painted every symbol with the `@variable` fallback — Sema files rendered almost entirely uncolored and JavaScript function names lost their color.
- Syntax: silenced the two `-Wunused` compiler warnings from the generated tree-sitter-sema parser, and added Sema to the query-compilation test suite.
- Dock panels: clicking a dock header tab no longer closes the dock — header tab clicks now activate the panel (toggle behavior remains on the keybindings).
- Tabs: tab title text is clipped to the tab, so the last tab in a narrow pane no longer bleeds into the neighboring pane.
- Scrollbar: no longer panics when a pane's scrollbar track is shorter than the minimum thumb size (e.g. deeply split panes or tiny windows) — the thumb is now clamped to the track length.
- Scrollbar: resizing the window/pane smaller than the scrollbar itself no longer crashes the editor — the horizontal track width is clamped to zero and degenerate (negative-size) tracks yield zero thumb travel instead of panicking.
- Sidebar: clicking the folder chevron now uses the same `TreeListLayout` geometry as the renderer, fixing misaligned chevron hit zones on HiDPI displays (previously hardcoded unscaled pixel offsets).
- Sidebar: collapsing a folder while scrolled down no longer leaves the file tree blank — the scroll offset is clamped after the visible item count shrinks.
- Sidebar: collapsing a folder now collapses all folders beneath it, so collapsing the top-level folders resets the whole tree instead of restoring stale expansion state on re-expand.
- Sidebar: scroll and auto-scroll-to-selection now use the actual sidebar viewport height (excluding the status bar), so the last row can't hide behind the status bar.
- Mouse: double/triple-click detection is now keyed by click target (editor pane, sidebar row, outline row, binary placeholder), so rapid clicks on unrelated UI regions no longer register as double-clicks (e.g. a sidebar click followed by a binary-placeholder click no longer opens the file in an external app).

---

## v0.3.19 - 2026-02-19

### Added

- Code Outline: Tree-sitter based symbol extraction panel in the right dock. Supports 10 languages (Rust, TypeScript, JavaScript, Python, Go, Java, PHP, C/C++, Markdown, YAML). Collapsible tree with click-to-select, double-click-to-jump, and scroll support.
- Code Outline: Dock panel hit-testing fix — clicks on dock panels no longer fall through to the editor.
- Code Outline: Added HTML outline support — shows structural/semantic elements (`html`, `body`, `nav`, `section`, `div`, `form`, `table`, etc.) with enriched labels (e.g. `div#app`, `section.hero`).
- Code Outline: Added Blade outline support — shows structural directives (`@section`, `@fragment`, `@push`, `@verbatim`, `@once`, `@livewire`, etc.) with parameter names, and Blade components (`<x-*>`). Control flow directives (`@if`, `@foreach`, `@switch`) are excluded to reduce noise.
- Syntax: Added Laravel Blade (`.blade.php`) language support with syntax highlighting for directives, comments, echo delimiters, components, and HTML structure. Uses [tree-sitter-blade](https://github.com/EmranMR/tree-sitter-blade) by [Emran MR](https://github.com/EmranMR).

### Improved

- Code Outline: Jumping to a symbol (double-click or Enter) now centers the target line in the viewport instead of minimal scrolling, for better orientation.

### Fixed

- Icons: Fixed all Nerd Font file type icons (were empty strings — PUA codepoints lost during copy/paste). Now using explicit Unicode escapes with codepoints from nvim-web-devicons. Also fixed folder/folder_open icons and improved Markdown/YAML icon variants.

---

## v0.3.18 - 2026-02-19

### Added

- Recent Files: Persistent recent files list (Cmd+E) — tracks files opened via any method (CLI, file dialog, quick open, drag-and-drop) and persists to `~/.config/token-editor/recent.json`. MRU-ordered list of up to 50 entries with fuzzy filtering, file type icons, and "time ago" timestamps. Pressing Cmd+E then Enter instantly swaps to the previously active file.
- Editing: Auto-surround selection — select text and type `(`, `[`, `{`, `"`, `'`, or `` ` `` to wrap it (e.g., `hello` → `(hello)`). Works with multi-cursor.
- Editing: Matching bracket highlighting — when cursor is adjacent to `()`, `[]`, or `{}`, both brackets are highlighted with a colored background.
- Config: `auto_surround` setting to enable/disable auto-surround behavior (default: `true`)
- Config: `bracket_matching` setting to enable/disable bracket match highlighting (default: `true`)
- Themes: `bracket_match_background` editor color for customizing bracket highlight appearance

### Performance

- Syntax highlighting pipeline rewritten: replaced thread-per-debounce with event-loop deadline timers for lower latency.
- Immediate highlight shifting on edits (InsertNewline, DeleteBackward, Paste, DeleteForward, DeleteWordBackward, DeleteWordForward, DeleteLine) eliminates highlight flashing on keystrokes.

### Fixed

- Viewport no longer scrolls unexpectedly when clicking near viewport edges or past the midpoint on newly opened editors.
- Modal layout bugs (geometry, hit-testing) fixed with comprehensive test coverage.

### Changed

- Refactored modal layout system: replaced ad-hoc layout code with VStack-based `ModalLayout` for cleaner, testable geometry.

---

## v0.3.17 - 2026-02-18

### Added

- Themes: 5 new builtin themes — Dracula, Catppuccin Mocha, Nord, Tokyo Night, Gruvbox Dark
- Syntax: Added sema-lisp syntax sample file

---

## v0.3.15 - 2026-01-09

### Added

- Preview: HTML file preview support. Opening preview on `.html` files now displays the rendered HTML in the webview pane, reusing the existing markdown preview infrastructure.
- Preview: Local resource loading for HTML preview. Images, CSS, and JS files are now served from the document's directory via a custom `token://` protocol handler.
- Preview: Preview now updates to show the new file when switching tabs (if the new file supports preview), instead of closing.
- API: Added `LanguageId::supports_preview()` method to check if a file type supports live preview.
- API: Added `content_to_preview_html()` function that handles both Markdown and HTML content.

---

## v0.3.14 - 2026-01-09

### Added

- Docs: Updated developer OVERVIEW and archived obsolete docs to reflect recent refactors and module moves.
- Tests: Comprehensive damage computation tests added for redraw/damage logic.

### Fixed

- Layout: Editor viewport now correctly shrinks to accommodate dock panels (bottom/right). Fixed issue where dock panels would obscure part of the editor area because the logical viewport didn't account for dock sizes.
- Layout: Viewport dimensions now recalculated when dock visibility or size changes.
- Preview: Fixed dual rendering issue where both native markdown and webview were drawn. Native rendering is now properly disabled when webview is active.
- Preview: Fixed webview misalignment by correctly converting physical pixel coordinates to logical points with proper Y-axis flipping for macOS.
- Scroll: Scroll events over dock panels and preview panes are now properly consumed instead of bleeding into the editor.
- Rendering: Minor fixes to selection/cursor rendering behavior related to the cursor-blink optimization and partial redraws.
- Sidebar: Fixed syntax highlighting not triggering when opening files via sidebar double-click. Commands from update() calls in mouse handlers are now properly propagated.

### Changed

- Refactor (mouse): Unified mouse event handling with new hit-test system in `src/view/hit_test.rs` and centralized dispatch in `src/runtime/mouse.rs`. Replaces ad-hoc if/else chains with explicit `HitTarget` enum and priority-ordered hit-testing.
- Refactor (app): Introduced specific redraw helpers and tighter damage accumulation to reduce unnecessary full redraws; clearer separation between damage computation and command processing.
- Refactor (layout): Added `redraw_editor`-style helpers to limit redraw scope when layout changes are localized.
- Refactor (ui): Optimized cursor-blink related redraw behavior to prevent selection flicker and avoid unnecessary redraws of unrelated regions.
- Cleanup: Removed dead code from view module (`is_in_tab_bar`, `is_in_modal`, unused Renderer getters, wrapper functions superseded by ViewportGeometry).

---

## v0.3.13 - 2026-01-07

### Fixed - CSV Cell Editing

**Column Truncation:**

- Fixed column truncation issue during CSV cell editing
- Cells now properly handle content that exceeds initial column width
- No data loss when editing longer values

**Horizontal Scrolling:**

- Added horizontal scroll support for CSV cell editing
- Long cell content now scrollable within the edit field
- Improved UX for editing cells with long text values

**Log File Discovery:**

- Fixed log file discovery to find most recent dated log file
- Improved config path resolution for logging

### Added - Command Palette Enhancements

**New Commands:**

- Added "Open Folder..." command to open workspace directories
- Added "Quit" command (⌘Q) to close the application
- Added "Toggle Performance Overlay" command (F2) - debug builds only
- Added "Toggle Debug Overlay" command (F8) - debug builds only

**Implementation:**

- Added `Cmd::Quit` for proper application exit flow
- Debug commands use conditional compilation (`#[cfg(debug_assertions)]`)
- Added `DEBUG_COMMANDS` array for debug-only command definitions

### Added - Configuration and Command System

**Configuration Reload:**

- Implemented `ReloadConfiguration` command for hot-reloading config without restart
- Command palette now includes "Reload Configuration" command
- Allows changes to config.yaml and keymap.yaml to take effect immediately

**Application Commands:**

- Added `ApplicationCommand` enum for app-level operations (quit, reload config)
- Added `WorkspaceCommand` enum for workspace operations (toggle sidebar, refresh, etc.)
- Unified command structure with `Cmd::Application` and `Cmd::Workspace` variants

**Theme Picker Enhancements:**

- Implemented theme preview with live updates
- Theme changes are applied immediately while browsing
- Restore original theme on cancel (Escape)
- Confirm theme selection with Enter
- Updated theme picker tests for new preview/restore API

**Configuration Integration:**

- Cursor blink interval now configurable via `editor.cursor_blink_interval_ms` in config.yaml
- Performance overlay toggle now configurable via `editor.show_perf_overlay` in config.yaml
- Runtime respects user configuration preferences

### Fixed - Rendering Bugs

**Selection Highlights Disappearing:**

- Selection highlights were disappearing after ~1 second due to cursor blink optimization
- `render_cursor_lines_only()` now properly renders selection highlights and rectangle selections
- Cursor blink interval changed from 500ms to 600ms

**CSV Cell Editor Cursor:**

- Fixed cursor appearing at top of editor instead of inside cell when editing CSV
- Skip cursor-lines-only optimization for CSV mode (falls back to full render)

### Changed - Build System

**Cargo.toml Enhancements:**

- Added package metadata: authors, readme, keywords, categories, exclude
- Added `default-run = "token"` to handle multiple binaries
- New build profiles:
  - `dev`: Fast compile with `debug = "line-tables-only"`, no debug info for deps
  - `debugging`: Full debug info for debuggers
  - `release`: Thin LTO, `panic = "abort"` for local testing
  - `dist`: Fat LTO, `codegen-units = 1`, stripped for distribution
  - `profiling`: Debug symbols, no LTO for flamegraph/samply

**Makefile Updates:**

- Added `make dist` and `make debugging` targets
- Cross-compilation targets now use `dist` profile for maximum optimization
- `make bundle-macos` uses `dist` profile for smallest binary
- Updated help text

---

## v0.3.12 - 2025-12-20

### Added - Find/Replace Implementation

Complete implementation of the Find/Replace modal (Cmd+F):

**Core Search Functions:**

- `find_all_occurrences_with_options()` with case sensitivity support
- `find_next_occurrence_with_options()` for forward search with wrapping
- `find_prev_occurrence_with_options()` for backward search with wrapping
- Full Unicode support (Greek, Japanese, emoji, accented characters)
- Overlapping match detection

**New Modal Messages:**

- `ToggleFindReplaceField` - Tab to switch between query/replace fields
- `ToggleFindReplaceCaseSensitive` - Toggle case-sensitive search
- `FindNext` / `FindPrevious` - Navigate through matches
- `ReplaceAndFindNext` - Replace current match and find next
- `ReplaceAll` - Replace all occurrences at once

**UX Improvements:**

- Query persists when reopening Cmd+F (like command palette)
- Transient messages show "No matches found" or "Replaced N occurrences"
- Selection highlights the current match
- Cursor scrolls to ensure match is visible

**Test Coverage:**

- 25+ new tests for find functionality
- Case sensitivity tests
- Unicode edge cases (emoji, CJK, Greek letters)
- Overlapping pattern matching
- Empty document and needle handling

---

## v0.3.11 - 2025-12-20

### Fixed - Event Loop Performance

Critical fix for the event loop spinning issue that caused ~7 FPS in multi-split scenarios:

**Root Cause:** `ControlFlow::Poll` was spinning the event loop constantly at ~100% CPU even when idle.

**Fix:** Changed to `ControlFlow::WaitUntil` in `src/runtime/app.rs`:

- Event loop now sleeps until the next cursor blink timer (500ms)
- Wakes immediately for user input, async messages, or file system changes
- Idle CPU usage dropped from ~100% to ~0%
- Multi-split FPS improved from ~7 to 60 (VSync limited)

**Performance Profile (30-second live session):**

| Category       | Time  |
| -------------- | ----- |
| Idle/Waiting   | 77.6% |
| Event Handling | 21.5% |
| Rendering      | ~0.9% |

### Fixed - Debug Overlay HiDPI Scaling

Fixed hard-coded pixel values in the performance overlay (`src/runtime/perf.rs`) that didn't scale on Retina displays:

- Overlay dimensions now scale based on `line_height` ratio
- Chart widths derived from approximate character width
- Stacked bar calculation fixed with `saturating_sub` to prevent overflow panic

### Added - Profiling Documentation

Enhanced [docs/PROFILING.md](PROFILING.md) with recommended workflow:

- Step-by-step process: headless benchmark → sample command → Instruments
- Interpreting `sample` output (mach_msg2_trap, CFRunLoop patterns)
- Common issues & solutions troubleshooting table
- Example healthy profile output

### Changed

- Archived detailed allocation analysis to [performance-analysis-v1.md](archived/performance-analysis-v1.md)
- Created new [performance-analysis.md](archived/performance-analysis.md) with summary of all optimizations

---

## v0.3.10 - 2025-12-19

### Added - File System Watcher & New Languages

**Workspace File Watching:**

- Integrated `notify` crate for real-time file system monitoring
- `FileSystemChange` event for workspace updates
- Automatic refresh when files change externally

**New Language Support:**

- **Scheme** - tree-sitter-scheme parser and highlighting
- **INI** - tree-sitter-ini parser and highlighting
- **XML** - tree-sitter-xml parser and highlighting

**File Operations:**

- Support for opening files via sidebar
- Support for creating new files
- `Document::new_with_path` constructor with comprehensive tests

**Theme Improvements:**

- Added CSV highlighting colors to all themes
- Restructured CSV theme initialization

**Documentation:**

- Added comprehensive documentation suite
- Updated workspace feature documentation
- Added workspace performance benchmarks and testing guide

### Changed

- Removed legacy highlight query files
- Cleaned up obsolete assets

---

## v0.3.9 - 2025-12-19

### Added - SelectWord for EditableState

- Implemented `select_word()` method for `EditableState` in `src/editable/state.rs`
- Wired `TextEditMsg::SelectWord` in `src/update/text_edit.rs` (previously a TODO stub)
- Added 4 tests for select_word covering middle-of-word, on-space, at-start, and at-end scenarios

### Fixed - Sidebar Folder Indicator Spacing

- Increased spacing between +/- folder indicators and folder names in sidebar
- Changed `text_x` offset from 16px to 20px for better visual separation

---

## v0.3.8 - 2025-12-19

### Added - Unified Text Editing System

Major refactoring to unify text editing across all input contexts (modals, CSV cells) with consistent behavior:

**Core Architecture (`src/editable/` module):**

- **`TextBuffer` / `TextBufferMut` traits** - Abstract over String and Rope buffer backends
- **`StringBuffer`** - Efficient single-line buffer for modals and CSV cells
- **`EditableState<B>`** - Unified state container with cursor, selection, and undo history
- **`EditConstraints`** - Context-specific restrictions (multiline, multi-cursor, char filters)
- **`TextEditMsg` / `MoveTarget`** - Unified message types for all editing operations
- **`EditContext`** - Identifies which input area is being edited
- **`TextFieldRenderer`** - Unified text field rendering with selection support

**Modal Input Improvements:**

- Full cursor navigation in all modals (Left/Right, Home/End)
- Selection support (Shift+Arrow) in command palette, goto line, find/replace
- Word movement (Option+Arrow) in all modals
- Word deletion (Option+Backspace/Delete) in all modals
- Select all (Cmd+A) in all modals
- Undo/redo within modal inputs
- Delete forward (Delete key) now works in modals
- Clipboard integration (Cmd+C/X/V) in all modals

**CSV Cell Editor Enhancements:**

- Migrated `CellEditState` to use `EditableState<StringBuffer>`
- Word movement (Option+Left/Right) while editing cells
- Word deletion (Option+Backspace/Delete) while editing cells
- Select all (Cmd+A) while editing cells
- Undo/redo (Cmd+Z / Cmd+Shift+Z) within cell editing session
- Selection support (Shift+Arrow, Shift+Home/End, Shift+Option+Arrow) while editing cells
- Clipboard integration (Cmd+C/X/V) while editing cells

**New Messages:**

- `CsvMsg::EditCursorWordLeft`, `EditCursorWordRight`
- `CsvMsg::EditDeleteWordBackward`, `EditDeleteWordForward`
- `CsvMsg::EditSelectAll`, `EditUndo`, `EditRedo`
- `CsvMsg::EditCursorLeftWithSelection`, `EditCursorRightWithSelection`, etc. - Selection movement
- `CsvMsg::EditCopy`, `EditCut`, `EditPaste` - Clipboard operations
- `ModalMsg::Copy`, `Cut`, `Paste` - Modal clipboard operations
- `Msg::TextEdit(EditContext, TextEditMsg)` - Unified text editing dispatch

**Main Editor Bridge:**

- `bridge_text_edit_to_editor()` maps `TextEditMsg` to legacy `EditorMsg`/`DocumentMsg`
- Enables unified message system to control main editor via bridge pattern
- All movement, selection, editing, clipboard, undo/redo, and multi-cursor operations bridged
- Allows gradual migration without breaking existing functionality

**New Files:**

- `src/editable/mod.rs` - Module exports
- `src/editable/buffer.rs` - TextBuffer traits and implementations
- `src/editable/cursor.rs` - Position and Cursor types
- `src/editable/selection.rs` - Selection operations
- `src/editable/history.rs` - EditOperation and EditHistory
- `src/editable/constraints.rs` - EditConstraints
- `src/editable/state.rs` - EditableState implementation
- `src/editable/context.rs` - EditContext enum
- `src/editable/messages.rs` - TextEditMsg and MoveTarget
- `src/update/text_edit.rs` - TextEditMsg routing, application, and editor bridge
- `src/view/text_field.rs` - TextFieldRenderer

---

## v0.3.7 - 2025-12-17

### Added - Workspace Management & Focus System

Complete workspace management with sidebar file tree and comprehensive focus handling:

**Sidebar Resize:**

- Click-and-drag to resize sidebar width
- ColResize cursor shown on hover over resize border
- `SidebarResizeState` tracks drag operation
- Width persists in logical pixels for DPI-independence

**File Tree Keyboard Navigation:**

- Arrow Up/Down to navigate between items
- Arrow Right expands folders or moves to next item
- Arrow Left collapses folders or jumps to parent folder (standard file tree behavior)
- Enter opens files or toggles folders
- Space toggles folder expansion
- Escape returns focus to editor

**Workspace Root Display:**

- Workspace root folder is now displayed as the first item in the file tree
- Root folder is auto-expanded when workspace opens
- Root path is canonicalized to ensure proper display name (fixes "." showing as empty)
- Folder expand/collapse indicators: `-` for expanded, `+` for collapsed

**Focus Management System:**

- New `FocusTarget` enum: `Editor`, `Sidebar`, `Modal`
- Explicit focus tracking in `UiState.focus`
- Click on sidebar transfers focus to sidebar
- Click outside sidebar returns focus to editor
- Modals automatically capture/release focus on open/close
- Hiding sidebar while focused returns focus to editor
- `KeyContext.sidebar_focused` and `editor_focused` now reflect actual focus state

**Global Shortcuts:**

- Command palette (Cmd+Shift+A), Save (Cmd+S), Quit (Cmd+Q), and other global shortcuts now work regardless of focus state
- New `Command::is_global()` method identifies shortcuts that bypass focus-based input routing
- Global commands include: ToggleCommandPalette, ToggleGotoLine, ToggleFindReplace, ToggleSidebar, Quit, SaveFile, NewTab, CloseTab

**Cursor Icon Cleanup:**

- I-beam cursor only appears over editable text areas
- Default pointer for sidebar, tab bars, status bar, modals, gutter
- ColResize/RowResize for splitters and sidebar resize border

**Keyboard Routing Fixes:**

- CSV cell editing now properly bypasses keymap (arrow keys work in cell editor)
- Added `AppModel::is_csv_editing()` helper method for consistent checking

**New Commands:**

- `FileTreeSelectPrevious`, `FileTreeSelectNext`
- `FileTreeOpenOrToggle`, `FileTreeRefresh`

**New Messages:**

- `WorkspaceMsg::OpenOrToggle` - opens file or toggles folder
- `WorkspaceMsg::SelectParent` - navigate to parent folder

---

## v0.3.6 - 2025-12-16

### Added - CSV Cell Editing (Phase 2)

Full cell editing support for CSV mode with document synchronization:

**Editing:**

- **Enter or typing** starts editing the selected cell
- **Typing replaces** cell content when starting with a character
- **Edit cursor** navigation with Left/Right arrows, Home/End
- **Backspace/Delete** for character deletion
- **Enter confirms** edit and moves to next row
- **Tab confirms** edit and moves to next cell
- **Escape cancels** edit, restoring original value

**Document Sync:**

- Edits update the underlying text buffer in real-time
- Proper CSV escaping for values with delimiters, quotes, or newlines
- Quoted fields are handled correctly (embedded commas don't break parsing)
- File becomes "modified" after edits, triggers save prompt

**New types:**

- `CellEditState` - tracks edit buffer, cursor position, original value
- `CellEdit` - represents a completed edit for sync/undo

**New messages:**

- `CsvMsg::StartEditing`, `StartEditingWithChar(char)`
- `CsvMsg::ConfirmEdit`, `CancelEdit`
- `CsvMsg::EditInsertChar`, `EditDeleteBackward`, `EditDeleteForward`
- `CsvMsg::EditCursorLeft`, `EditCursorRight`, `EditCursorHome`, `EditCursorEnd`

---

## v0.3.5 - 2025-12-16

### Added - CSV Viewer Mode (Phase 1)

Spreadsheet-like view for CSV, TSV, and PSV files with full navigation support:

**Core Features:**

- **Grid rendering** with row numbers (1, 2, 3...) and column headers (A, B, C...)
- **Cell selection** via mouse click with proper hit-testing
- **Delimiter detection** from file extension or content sniffing
- **Column width auto-calculation** based on content (sampled from first 100 rows)
- **Theme integration** with configurable colors for headers, grid lines, selection
- **Memory-efficient storage** using delimited strings (Tablecruncher pattern)

**Navigation:**

- Arrow keys move cell selection
- Tab/Shift+Tab for next/previous cell with row wrapping
- Home/End for first/last column in row
- Cmd+Home/End for first/last cell in document
- Page Up/Down jumps by viewport height
- Mouse wheel scrolling (vertical and horizontal)
- Click to select cell

**Integration:**

- Command palette: "Toggle CSV View" command
- Escape exits CSV mode
- Status bar shows CSV mode indicator
- Works with split views

**New files:**

- `src/csv/` module with `mod.rs`, `model.rs`, `parser.rs`, `render.rs`, `navigation.rs`, `viewport.rs`
- `samples/large_data.csv` - 10,001-line test file
- `make csv` target for testing

**Messages added:**

- `CsvMsg` enum with Toggle, Move\*, NextCell, PrevCell, FirstCell, LastCell, RowStart, RowEnd, PageUp, PageDown, Exit, SelectCell, ScrollVertical, ScrollHorizontal

**Documentation:** See [docs/feature/csv-editor.md](archived/csv-editor.md)

---

## v0.3.4 - 2025-12-16

### Fixed - HiDPI Display Switching

Fixed critical issues when switching between displays with different DPI/scale factors (e.g., moving window between Retina and non-Retina monitors):

- **Surface resize on display change** - The softbuffer Surface is now explicitly resized after creation, fixing incorrect rendering when switching displays
- **Buffer bounds checking in Frame** - Frame::new now validates buffer size matches dimensions, preventing panics during display transitions
- **Dynamic tab bar height** - Tab bar height is now computed from actual glyph metrics (`line_height + padding * 2`) instead of hardcoded values
- **Scaled metrics throughout** - Model's `resize()` and `set_char_width()` now use properly scaled metrics for viewport calculations

### Changed

- `ScaleFactorChanged` now triggers both `ReinitializeRenderer` and `Redraw` commands to ensure immediate visual update
- `reinit_renderer` recomputes tab bar height and viewport geometry after font metrics change
- Viewport visible lines calculation now accounts for tab bar height

### Technical Details

- `Renderer::with_scale_factor` explicitly calls `surface.resize()` after creation
- `Frame::new` adjusts height if buffer is smaller than expected (`width * height`)
- `recompute_tab_bar_height_from_line_height()` added to AppModel for font-metric-based sizing
- `group_content_rect_scaled()` now used in rendering for DPI-aware content areas

---

## v0.3.3 - 2025-12-15

### Added - Phase 3-5 Languages for Syntax Highlighting

Added 11 new languages to syntax highlighting, completing the roadmap phases 3-5:

**Phase 3 (Priority):**

- **TypeScript** - `tree-sitter-typescript` v0.23 with custom queries
- **TSX** - Shares queries with TypeScript, separate parser
- **JSON** - `tree-sitter-json` v0.24 with custom queries
- **TOML** - `tree-sitter-toml-ng` v0.7 with custom queries

**Phase 4 (Common):**

- **Python** - `tree-sitter-python` v0.25 using built-in highlights query
- **Go** - `tree-sitter-go` v0.25 using built-in highlights query
- **PHP** - `tree-sitter-php` v0.24 using built-in highlights query

**Phase 5 (Extended):**

- **C** - `tree-sitter-c` v0.24 using built-in highlights query
- **C++** - `tree-sitter-cpp` v0.23 using built-in highlights query
- **Java** - `tree-sitter-java` v0.23 using built-in highlights query
- **Bash** - `tree-sitter-bash` v0.25 using built-in highlights query

**Total languages now supported: 17** (PlainText excluded)

### Changed

- Upgraded `tree-sitter` from 0.24 to 0.25 for ABI compatibility with newer grammars
- `LanguageId` enum now includes all 17 language variants
- `from_extension()` recognizes extended file extensions (.tsx, .mts, .cts, .pyw, etc.)
- `from_path()` recognizes special filenames (Makefile, Dockerfile, .bashrc, etc.)

### Technical Details

- Custom queries created for TypeScript, JSON, TOML (in `queries/` directory)
- Phase 4-5 languages use built-in `HIGHLIGHTS_QUERY` or `HIGHLIGHT_QUERY` constants
- Query compilation tests added for all 17 languages
- Parsing tests added for all new languages
- 671 total tests passing

---

## v0.3.2 - 2025-12-15

### Added - Incremental Parsing with tree.edit()

Implemented proper incremental parsing for significantly faster syntax highlighting on edits:

**Tree Caching:**

- `ParserState` now caches parsed trees per document in `doc_cache: HashMap<DocumentId, DocParseState>`
- Each `DocParseState` stores the tree, source text, and language
- Cache enables incremental parsing on subsequent edits

**Edit Diffing:**

- `compute_incremental_edit()` computes `InputEdit` by diffing old vs new source
- Finds common prefix/suffix to minimize the edited region
- `byte_to_point()` converts byte offsets to tree-sitter `Point` (row, column)

**Incremental Parse Flow:**

1. On edit, diff cached source against new source
2. Call `tree.edit(&input_edit)` on cached tree
3. Pass edited tree to `parser.parse(source, Some(&old_tree))`
4. Tree-sitter reuses unchanged nodes, only re-parsing the changed region

**Performance Results:**

| File Size  | Full Reparse | Incremental |
| ---------- | ------------ | ----------- |
| 100 lines  | 67µs         | 68µs        |
| 500 lines  | 330µs        | 356µs       |
| 1000 lines | 660µs        | 728µs       |
| 5000 lines | 3.4ms        | 3.6ms       |

_Note: Incremental is similar speed due to highlight extraction dominating; benefit increases for larger files._

**New Benchmark Suite** (`benches/syntax.rs`):

- `parse_sample` - Parse small samples for each language
- `parse_only_sample` - Isolated parse time (pre-initialized parser)
- `parse_only_large_rust` - Large file scaling (100-10000 lines)
- `incremental_parse_small_edit` - Incremental with append
- `incremental_parse_middle_edit` - Incremental with mid-file edit
- `full_reparse_comparison` - Fresh parser baseline

**New Makefile Target:**

- `make bench-syntax` - Run syntax highlighting benchmarks

### Changed

- `ParserState` now includes `doc_cache` for tree caching
- `parse_and_highlight()` uses cached trees for incremental parsing
- Added `clear_doc_cache()` method for document cleanup

---

## v0.3.1 - 2025-12-15

### Fixed - Syntax Highlighting Bugs

Critical fixes for the syntax highlighting system:

**Tree-sitter Incremental Parsing Bug:**

- Fixed misaligned highlights after document edits (e.g., pressing Enter)
- Root cause: Passing old cached tree to tree-sitter without calling `tree.edit()`
- Tree-sitter incorrectly reused nodes from stale tree, producing wrong line/column positions
- Fix: Always do full reparse by passing `None` instead of cached tree
- Removed unused tree caching from `ParserState` until proper incremental parsing is implemented

**Flash of Unstyled Text (FOUC):**

- Fixed jarring unstyled flash during syntax re-parsing
- Old highlights are now preserved until new ones arrive
- Revision checks ensure only matching highlights are applied
- Reduced debounce from 50ms to 30ms for snappier updates

**Tab Expansion in Highlighting:**

- Fixed highlight token columns not accounting for tab expansion
- Token character columns are now converted to visual columns using `char_col_to_visual_col()`

### Changed

- `SYNTAX_DEBOUNCE_MS` reduced from 50ms to 30ms
- `ParserState` no longer caches syntax trees (simplified until incremental parsing)

---

## v0.3.0 - 2025-12-15

### Added - Benchmark Suite Improvements

Comprehensive audit and improvement of the benchmark suite:

**Phase 1: Fixed Inaccurate Benchmarks**

- **Rewrote `glyph_cache.rs`** — Now uses actual fontdue rasterization instead of fictional patterns
  - `glyph_rasterize` tests real character rasterization at various font sizes
  - `glyph_cache_realistic_paragraph` tests actual cache hit/miss patterns
  - `glyph_cache_code_sample` tests with code-like content
  - `font_metrics_extraction` tests line width measurement
- **Created `token::rendering::blend_pixel_u8`** — Shared blend function in lib.rs
- **Updated `rendering.rs` and `support.rs`** — Use shared blend function instead of duplicated code

**Phase 2: Added Missing Benchmarks**

- **Multi-cursor benchmarks in `main_loop.rs`:**
  - `multi_cursor_setup` (10, 100, 500 cursors)
  - `multi_cursor_insert_char`, `multi_cursor_delete`, `multi_cursor_move_down`
  - `multi_cursor_select_word`, `multi_cursor_add_cursor_above_below`
- **Large file scaling in `rope_operations.rs`** (100k, 500k, 1M lines):
  - `insert_middle_large_file`, `insert_start_large_file`, `insert_end_large_file`
  - `delete_middle_large_file`, `navigate_large_file`
  - `sequential_inserts_large_file`, `sequential_deletes_large_file`
- **New `benches/search.rs`** — Search operation benchmarks:
  - `search_literal_string`, `search_case_insensitive`, `search_whole_word`
  - `count_occurrences`, `find_first_occurrence`, `search_visible_range`
- **New `benches/layout.rs`** — Text layout benchmarks:
  - `measure_line_width`, `calculate_visible_lines`, `char_position_in_line`
  - `column_from_x_position`, `full_viewport_layout`, `viewport_layout_with_cache`

**New Makefile Targets:**

- `make bench-loop` — Main loop benchmarks
- `make bench-search` — Search benchmarks
- `make bench-layout` — Layout benchmarks
- `make bench-multicursor` — Multi-cursor specific benchmarks
- `make bench-large` — Large file (500k+) benchmarks

---

### Added - Syntax Highlighting MVP

Tree-sitter based syntax highlighting with async background parsing:

**New `src/syntax/` module:**

- `highlights.rs` - `HighlightToken`, `LineHighlights`, `SyntaxHighlights` data structures
- `languages.rs` - `LanguageId` enum with extension-based language detection
- `worker.rs` - `SyntaxWorker` with background thread, mpsc channels, debouncing

**Features:**

- **Async parsing** - Background worker thread prevents UI blocking
- **Debouncing** - 50ms timer prevents parsing on every keystroke
- **Revision tracking** - Staleness checks discard outdated parse results
- **Phase 1 languages** - YAML, Markdown, Rust support
- **Theme integration** - `SyntaxTheme` struct in theme.rs with VS Code-like default colors
- **Auto-trigger** - Parsing on document load and content changes

**New messages:**

- `SyntaxMsg::ParseCompleted` - Delivers highlights from background thread
- `Cmd::ParseSyntax` - Triggers async syntax parsing

**Dependencies added:**

- `tree-sitter = "0.24"`
- `tree-sitter-yaml = "0.6"`
- `tree-sitter-md = "0.3"`
- `tree-sitter-rust = "0.23"`

---

### Added - CLI Arguments with clap

Full command-line argument parsing using clap:

- **New `src/cli.rs` module** with `CliArgs`, `StartupConfig`, `StartupMode`
- **Supported flags:**
  - `token file.rs` - open file
  - `token --new` / `-n` - start with empty buffer
  - `token --line 42 file.rs` - open file at line 42
  - `token --line 42 --column 10 file.rs` - open at line 42, column 10
  - `token --wait` / `-w` - wait mode for git integration (parsed, not yet implemented)
  - `token ./src` - open directory as workspace (sets workspace_root)
- 1-indexed user input converted to 0-indexed internal representation
- 7 new CLI tests

### Added - Duplicate File Detection

Already-open files now focus existing tab instead of creating duplicates:

- Added `find_open_file()` and `is_file_open()` methods to `EditorArea`
- Uses canonicalized paths to handle symlinks and relative paths
- Status bar shows "Switched to: filename" when focusing existing tab
- Integrated into `open_file_in_new_tab()` in layout.rs

### Added - Visual Feedback for File Drag-Hover

File drag-and-drop now shows visual feedback overlay:

- **`DropState` struct** in `UiState` with `hovered_files` and `is_hovering`
- **New `UiMsg` variants:** `FileHovered(PathBuf)`, `FileHoverCancelled`
- Handles `WindowEvent::HoveredFile` and `HoveredFileCancelled`
- Semi-transparent overlay centered in window with "Drop to open: filename" text
- Overlay disappears when drag leaves window

### Changed

- Test count: 703 (was 603)
- Dependencies: Added `clap = "4"` with derive feature

---

## v0.2.1 - 2025-12-15

### Added - Centralized Config Paths

Single source of truth for all configuration directories:

- **New `src/config_paths.rs` module** with all config path functions:
  - `config_dir()` → `~/.config/token-editor/` (Unix) or `%APPDATA%\token-editor\` (Windows)
  - `config_file()` → `config.yaml` path
  - `keymap_file()` → `keymap.yaml` path
  - `themes_dir()` → themes subdirectory
  - `ensure_all_config_dirs()` → creates directory structure
- Respects `XDG_CONFIG_HOME` on Unix if set
- Explicitly uses `~/.config` on macOS (not `~/Library/Application Support`)

### Changed - Test Organization

Moved inline tests to integration test folder:

- Extracted tests from `config.rs`, `config_paths.rs`, `keymap/defaults.rs`
- New `tests/config.rs` with 22 tests covering config paths, editor config, and keymap merge logic
- Total test count: 597

### Fixed - Command Palette Navigation

- Selection index now properly clamped to filtered item count
- Prevents selecting beyond visible items when filter reduces list

### Changed

- Renamed "Open Keybindings" → "Open Keymap" in command palette

---

## 2025-12-15

### Added - Configurable Keymapping System

Complete data-driven keybinding system with YAML configuration:

**Core Module** (`src/keymap/`):

- `KeyCode`, `Keystroke`, `Modifiers` - platform-agnostic key representation
- `Command` enum - 52 bindable editor commands with `to_msgs()` conversion
- `Keybinding` struct - binds keystroke to command with optional conditions
- `Keymap` - lookup engine with context-aware binding resolution
- `KeyContext` - captures editor state (has_selection, has_multiple_cursors, modal_active, editor_focused)
- `Condition` enum - 7 conditions for context-aware bindings
- YAML parser with platform-specific binding support

**Default Bindings** (`keymap.yaml`):

- 74 default bindings embedded at compile time
- Platform-aware `cmd` modifier (Cmd on macOS, Ctrl elsewhere)
- macOS-specific bindings (meta+arrow for line navigation)
- Context-aware Tab (indent with selection, insert tab without)
- Context-aware Escape (collapse multi-cursor → clear selection → nothing)

**Integration** (`src/runtime/app.rs`):

- Keymap tried first for all key events
- Fallback to `input.rs` for complex behaviors (option double-tap, selection collapse on arrows)
- `KeyContext` extracted from model state for binding evaluation

**Commands Added**:

- `DeleteWordForward` - Option+Delete deletes word after cursor
- `InsertTab` - Insert tab character (for Tab without selection)
- `EscapeSmartClear` - No-op fallback for Escape key

### Fixed - Expand Selection Line Behavior

Fixed line selection to exclude the newline character:

**Before**: Expanding selection to line selected through the newline, placing cursor at start of next line
**After**: Line selection ends at last character of line content, cursor stays on same line

- Changed `select_line_at()` to end at last character of line content
- Reordered checks in `expand_selection()` to check `is_line_selection_at` before `is_word_selection_at`
- This fixes single-word lines where word selection equals line selection
- Simplified `is_line_selection_at()` to only check same-line selection ending at line length

### Fixed - Option Double-Tap for Multi-Cursor

Fixed Option double-tap gesture being intercepted by keymap:

**Before**: Keymap would handle Alt+Up/Down before input.rs could check for double-tap
**After**: Keymap lookup is skipped when `option_double_tapped && alt` is true

This preserves the Option+Option+Arrow gesture for adding cursors above/below.

### Changed

- Test count: 539 (was 537)
- 66 keymap-specific tests
- 2 new expand selection tests for line behavior

---

## 2025-12-09

### Added - Command Palette (GUI Phase 4)

Fully functional command palette with searchable command list:

**Command Registry** (`src/commands.rs`):

- `CommandId` enum with 17 commands (file, edit, navigation, view operations)
- `CommandDef` struct with id, label, keybinding
- `COMMANDS` static registry with all available commands
- `filter_commands(query)` for fuzzy substring matching

**Command Execution** (`src/update/app.rs`):

- `execute_command(model, cmd_id)` dispatches to appropriate update functions
- Commands routed through existing message handlers (DocumentMsg, LayoutMsg, etc.)

**Command Palette UI** (`src/view/mod.rs`):

- Filtered command list displayed below input field
- Selected item highlighted with selection background
- Keybindings shown right-aligned (dimmed)
- Up/Down arrows navigate list
- Enter executes selected command
- Shows "... and N more" when list is truncated

**Available Commands**:
New File, Save File, Undo, Redo, Cut, Copy, Paste, Select All, Go to Line, Split Editor Right/Down, Close Editor Group, Next/Prev Tab, Close Tab, Find, Show Command Palette

---

### Added - Mouse Blocking & Compositor (GUI Phase 5)

Modal overlays now properly capture mouse events:

**Mouse Blocking** (`src/runtime/app.rs`):

- Click outside modal closes it
- Click inside modal is consumed (doesn't leak to editor)
- Uses centralized `geometry::point_in_modal()` for hit-testing

**Modal Geometry** (`src/view/geometry.rs`):

- `modal_bounds()` - calculates modal position and size
- `point_in_modal()` - hit-test for modal area
- Shared between rendering and input handling

---

### Added - Frame Helpers

New drawing primitives for cleaner rendering:

- `Frame::draw_bordered_rect()` - fill with 1px border in single call
- Reduces code duplication in modal rendering

---

### Added - Basic Modal/Focus System (GUI Phase 3)

Added minimal modal overlay infrastructure with focus capture:

**Modal State Types** (`src/model/ui.rs`):

- `ModalId` enum: `CommandPalette`, `GotoLine`, `FindReplace`
- `ModalState` enum with per-modal state structs
- `CommandPaletteState`, `GotoLineState`, `FindReplaceState`
- `UiState::active_modal: Option<ModalState>` field
- Helper methods: `has_modal()`, `open_modal()`, `close_modal()`

**Modal Messages** (`src/messages.rs`):

- `ModalMsg` enum with variants: `Open*`, `Close`, `SetInput`, `InsertChar`, `DeleteBackward`, `SelectPrevious`, `SelectNext`, `Confirm`
- `UiMsg::Modal(ModalMsg)` and `UiMsg::ToggleModal(ModalId)`

**Focus Capture** (`src/runtime/input.rs`):

- `handle_modal_key()` routes all keyboard input to modal when active
- Modal consumes Escape, Enter, arrows, backspace, and character input
- Editor key handling bypassed when modal is open

**Modal Rendering** (`src/view/mod.rs`):

- `render_modals()` draws modal overlay layer
- 40% dimmed background over entire window
- Centered modal dialog with title, input field, and blinking cursor
- Rendered after status bar, before debug overlays

**Keyboard Shortcuts**:

- `Cmd+P` / `Ctrl+P` - Toggle Command Palette
- `Cmd+G` / `Ctrl+G` - Toggle Go to Line
- `Cmd+F` / `Ctrl+F` - Toggle Find/Replace
- `Escape` - Close modal
- `Enter` - Confirm (Go to Line jumps to entered line number)

#### Benefits

- Foundation for Command Palette (Phase 4) and other overlays
- Focus capture prevents editor input while modal is open
- Modals are first-class layers with proper z-ordering
- Clean separation of modal state, messages, and rendering

---

### Added - Widget Extraction & Geometry Centralization (GUI Phase 2)

Transformed monolithic render function into composable widget functions:

**New `src/view/geometry.rs` module** - centralized geometry helpers:

- Constants: `TAB_BAR_HEIGHT`, `TABULATOR_WIDTH`
- Viewport sizing: `compute_visible_lines()`, `compute_visible_columns()`
- Tab handling: `expand_tabs_for_display()`, `char_col_to_visual_col()`, `visual_col_to_char_col()`
- Hit-testing: `is_in_status_bar()`, `is_in_tab_bar()`, `tab_at_position()`, `pixel_to_cursor()`
- Layout helpers: `group_content_rect()`, `group_gutter_rect()`, `group_text_area_rect()`

**Extracted widget renderers:**

- `render_editor_area_static()` - top-level: all groups + splitters
- `render_editor_group_static()` - orchestrates tab bar, gutter, text area
- `render_tab_bar_static()` - tab bar background, tabs, active highlight
- `render_gutter_static()` - line numbers, gutter border
- `render_text_area_static()` - current line highlight, selections, text, cursors
- `render_splitters_static()` - splitter bars between groups
- `render_status_bar_static()` - status bar with segments and separators

**Updated hit-testing** to delegate to `view::geometry`:

- `Renderer::is_in_status_bar()`, `is_in_tab_bar()`, `tab_at_position()`, `pixel_to_cursor()`

#### Benefits

- Clear widget hierarchy with single responsibility per function
- Centralized geometry calculations - single source of truth
- Hit-testing and rendering share the same geometry logic
- Prepared for future modal system (Phase 3)

---

## 2025-12-08

### Fixed - Debug Tracing Message Names

Fixed `msg_type_name()` to show human-readable variant names instead of opaque discriminants:

**Before:** `msg=Ui::Discriminant(1)`, `msg=Document::Discriminant(0)`  
**After:** `msg=Ui::BlinkCursor`, `msg=Document::InsertChar('a')`

- Changed from `std::mem::discriminant()` to Debug formatting (`{:?}`)
- Includes variant arguments which helps debug multi-cursor/selection issues
- Zero dependencies, zero maintenance overhead

### Added - Frame/Painter Abstraction (GUI Phase 1)

Centralized drawing primitives for cleaner, more maintainable rendering code:

- **`Frame` struct** (`src/view/frame.rs`) - wraps pixel buffer with safe drawing methods:
  - `clear()`, `fill_rect()`, `fill_rect_px()` - solid color fills
  - `set_pixel()`, `get_pixel()` - single pixel operations
  - `blend_pixel()`, `blend_rect()` - alpha blending
  - `dim()` - modal background dimming
  - `draw_sparkline()` - debug chart rendering

- **`TextPainter` struct** - wraps fontdue + glyph cache:
  - `draw()` - render text at position with color
  - `measure_width()` - calculate text width in pixels

- **Migrated all rendering functions** to use Frame/TextPainter:
  - `render_all_groups_static()` - takes Frame + TextPainter
  - `render_editor_group_static()` - all pixel ops use Frame
  - `render_tab_bar_static()` - uses Frame/TextPainter
  - `render_splitters_static()` - simplified from ~15 lines to 4 lines
  - `render_perf_overlay()` - fully migrated
  - Status bar rendering - uses Frame/TextPainter

- **Removed legacy functions**: `draw_text()`, `draw_sparkline()` standalone functions

#### Benefits

- Simpler APIs with automatic bounds checking
- Fewer parameters passed through render functions
- Consistent abstraction for all pixel operations
- Prepared for future widget extraction (Phase 2)

---

## 2025-12-07

### Added - File Dropping & Multi-File Arguments

Open multiple files from command line or by drag-and-drop:

- **Multi-file CLI**: `cargo run -- file1.rs file2.rs file3.rs` opens all files as tabs
- **Drag-and-drop**: Drop files onto the window to open them in new tabs
- **LayoutMsg::OpenFileInNewTab(PathBuf)**: New message for opening files as tabs
- First file becomes active tab, additional files added to same group

#### Implementation

- `src/messages.rs`: Added `LayoutMsg::OpenFileInNewTab(PathBuf)`
- `src/update/layout.rs`: Added `open_file_in_new_tab()` handler
- `src/app.rs`: Handle `WindowEvent::DroppedFile` events
- `src/main.rs`: Parse all CLI args as file paths (removed TODO)
- `src/model/mod.rs`: `AppModel::new()` now accepts `Vec<PathBuf>`

### Fixed - Tab Click to Switch

- Clicking on tabs now switches to the clicked tab
- Added `Renderer::tab_at_position()` for tab hit-testing
- Tab bar click handler now detects tab index and dispatches `SwitchToTab`

### Refactored - Document Display Name

- Added `Document::display_name()` method centralizing naming logic
- Added `tab_title()` helper in view.rs to avoid duplication
- Tab bar rendering and hit-testing now use the same helper
- Keeps numbered untitled names (Untitled, Untitled-2, etc.) for UX

---

## 2025-12-07

### Changed - Test Extraction

Extracted inline tests from production code to `tests/` folder:

- `tests/editor_area.rs` - 7 tests (Rect, layout, hit testing)
- `tests/overlay.rs` - 7 tests (anchor positioning, alpha blending)
- `tests/theme.rs` - 10 tests (Color parsing, YAML themes, builtins)

Tests remaining in `src/main.rs` (14 tests) cannot be moved - they test `handle_key()` which is binary-only code.

### Fixed - Multi-Cursor Duplicate

- **Duplicate** (Cmd+D) now works on all cursors, not just primary
- Line duplication: duplicates line at each cursor position
- Selection duplication: duplicates selected text at each cursor
- Processes in reverse document order, records as Batch for proper undo
- 3 new tests in `tests/multi_cursor.rs`

### Fixed - Multi-Cursor Indent/Unindent

- **IndentLines** now works on all cursors/selections, not just primary
- **UnindentLines** now works on all cursors/selections, not just primary
- Both use `lines_covered_by_all_cursors()` helper for unique line collection
- Proper Batch undo/redo with cursor state restoration
- 5 new tests in `tests/multi_cursor.rs`

### Fixed - Multi-Cursor DeleteLine

- **DeleteLine** (Cmd+Backspace) now deletes lines at all cursor positions
- Uses same `lines_covered_by_all_cursors()` pattern
- Collapses to single cursor after deletion
- 3 new tests in `tests/multi_cursor.rs`

### Fixed - Multi-Cursor Edge Expansion

- **AddCursorAbove** now expands from top-most cursor, not primary
- **AddCursorBelow** now expands from bottom-most cursor, not primary
- Added `top_cursor()`, `bottom_cursor()`, `edge_cursor_vertical()` helpers
- 2 new tests in `tests/multi_cursor.rs`

---

## 2025-12-07

### Fixed - Multi-Cursor Selection Rendering & Cmd+J

Fixed three bugs in multi-cursor functionality:

#### Selection Rendering

- **Fixed**: All selections now render, not just the primary selection
- Previously only `editor.selection()` (primary) was rendered
- Now iterates over `editor.selections` to render all multi-cursor selections

#### Cmd+J (SelectNextOccurrence)

- **Fixed**: First invocation now searches from current selection position, not offset 0
- **Fixed**: Loop now skips already-selected occurrences instead of doing nothing
- Shows "All occurrences selected" message when all are already selected
- Proper wrap-around detection to avoid infinite loops

---

## 2025-12-06

### Changed - Codebase Organization

Major restructuring of large files for improved maintainability:

#### Update Module (`update/`)

Converted monolithic `update.rs` (2900 lines) into a module directory:

| File          | Lines | Contents                                  |
| ------------- | ----- | ----------------------------------------- |
| `mod.rs`      | 36    | Pure dispatcher only                      |
| `editor.rs`   | 1123  | Cursor movement, selection, expand/shrink |
| `document.rs` | 1231  | Text editing, undo/redo helpers           |
| `layout.rs`   | 472   | Split views, tabs, groups                 |
| `app.rs`      | 83    | File operations, window resize            |
| `ui.rs`       | 55    | Status bar, cursor blink                  |

#### Binary Modules

Extracted from `main.rs` (was 3100 lines, now ~20 lines entry + 669 tests):

| File       | Lines | Contents                                 |
| ---------- | ----- | ---------------------------------------- |
| `app.rs`   | 520   | App struct, ApplicationHandler impl      |
| `input.rs` | 402   | handle_key, keyboard→Msg mapping         |
| `view.rs`  | 1072  | Renderer, drawing functions, tab helpers |
| `perf.rs`  | 406   | PerfStats, debug overlay (debug only)    |

#### Benefits

- `main.rs` is now a clean ~20 line entry point
- `update/mod.rs` is a pure 36-line dispatcher
- Clear separation: Model → Messages → Update → View
- Prepared for future Frame/TextPainter abstraction
- All 401 tests pass

---

## 2025-12-06

### Added - Multi-Cursor Selection Gaps

Fixed remaining selection operations to work with multiple cursors:

- **`merge_overlapping_selections()`**: New method in `EditorState` that merges overlapping or touching selections into single selections, maintaining cursor/selection invariants
- **`SelectWord`**: Now operates on ALL cursors, selecting word at each position, then merging overlaps
- **`SelectLine`**: Now operates on ALL cursors, selecting line at each position, then merging overlaps
- **`SelectAll`**: Properly collapses to single cursor + single full-document selection
- **`ExtendSelectionToPosition`**: Collapses multi-cursor first, then extends from primary cursor
- **`word_under_cursor_at(doc, idx)`**: New helper refactored from `word_under_cursor()` for per-cursor word detection

#### Tests Added

- 6 tests for `merge_overlapping_selections()` (non-overlapping, overlapping, touching, multiline, duplicates, invariants)
- 4 tests for `SelectWord` (single cursor, whitespace, multi-cursor different words, same word merges)
- 4 tests for `SelectLine` (single cursor, last line, multi-cursor different lines, same line merges)
- 2 tests for `SelectAll` (single cursor, collapses multi-cursor)
- 2 tests for `ExtendSelectionToPosition` (single cursor, collapses multi-cursor)

### Changed

- Test count: 401 (was 383)
- Added 18 new selection tests

---

## 2025-12-06

### Added - Expand/Shrink Selection (Already Implemented)

Progressive selection expansion with history stack:

- **Option+Up**: Expand selection (cursor → word → line → all)
- **Option+Down**: Shrink selection (restore previous from history)
- Selection history stack in `EditorState.selection_history`
- 18 tests in `tests/expand_shrink_selection.rs`

_(Feature was already implemented, roadmap updated to reflect completion)_

### Added - Multi-Cursor Movement

All cursor movement operations now work with multiple cursors:

- **Arrow keys** (Up/Down/Left/Right) move ALL cursors simultaneously
- **Home/End** moves all cursors to their respective line starts/ends (smart behavior preserved)
- **Word navigation** (Option+Arrow) moves all cursors by word
- **Page Up/Down** moves all cursors
- **Shift+movement** extends selection for ALL cursors
- **Cursor deduplication** when cursors collide after movement
- Each cursor preserves its own `desired_column` for vertical movement through ragged lines

#### Implementation Details

- Per-cursor primitives in `EditorState`: `move_cursor_*_at(doc, idx)`
- All-cursors wrappers: `move_all_cursors_*(doc)`
- Selection variants: `move_all_cursors_*_with_selection(doc)`
- Removed legacy single-cursor movement functions from `update.rs`
- 10 new multi-cursor movement tests in `tests/cursor_movement.rs`

### Changed

- Test count: 383 (was 351)
- Added 10 multi-cursor movement tests, 22 other improvements

---

## 2025-12-06

### Added - Split View Implementation (All 7 Phases)

Complete multi-pane editor with split views, tabs, and shared documents.

#### Phase 1: Core Data Structures

- `DocumentId`, `EditorId`, `GroupId`, `TabId` - typed identifiers
- `Tab` struct with editor reference, pinned/preview flags
- `EditorGroup` with tabs, active tab index, layout rect
- `LayoutNode` enum: `Group(GroupId)` or `Split(SplitContainer)`
- `SplitContainer` with direction, children, ratios, min_sizes
- `EditorArea` managing documents, editors, groups, and layout tree

#### Phase 2: Layout System

- `Rect` type for layout calculations with `contains()` hit testing
- `compute_layout()` recursive algorithm for layout tree
- `group_at_point()` for mouse hit testing
- `SplitterBar` struct for splitter positions
- `splitter_at_point()` for resize handle detection
- `SPLITTER_WIDTH` constant (4px)

#### Phase 3: AppModel Migration

- Replaced single `Document`/`EditorState` with `EditorArea`
- Backward-compatible accessor methods: `document()`, `editor()`, etc.
- `ensure_focused_cursor_visible()` helper avoiding document cloning
- `resize()` now updates ALL editors (fixes multi-pane viewport bug)

#### Phase 4: LayoutMsg Handlers

- `SplitFocused(SplitDirection)` - split current group
- `SplitGroup { group_id, direction }` - split specific group
- `CloseGroup`, `CloseFocusedGroup` - close with layout cleanup
- `FocusGroup`, `FocusNextGroup`, `FocusPrevGroup` - navigation
- `FocusGroupByIndex(usize)` - keyboard shortcuts (1-indexed)
- `CloseTab`, `CloseFocusedTab`, `MoveTab` - tab operations
- `NextTab`, `PrevTab`, `SwitchToTab` - tab navigation

#### Phase 5: Multi-Group Rendering

- `render_all_groups_static()` iterates over layout
- `render_editor_group_static()` renders single pane
- Tab bar rendering with active/inactive styling
- Splitter bar rendering between groups
- Focus indicator (border) on focused group

#### Phase 6: Document Synchronization

- Documents shared across views (same `DocumentId`)
- Independent cursor/viewport per `EditorState`
- Edits reflect immediately in all views of same document

#### Phase 7: Keyboard Shortcuts

- Numpad 1-4: Focus group by index
- Numpad -/+: Split horizontal/vertical
- Cmd+W: Close tab
- Option+Cmd+Left/Right: Previous/Next tab
- Ctrl+Tab: Focus next group
- `physical_key` support in `handle_key()` for numpad detection

### Fixed - Split View Bugs

- `close_tab` on last group's only tab now prevented (was leaving invalid state)
- `move_tab` to invalid group now no-op (was losing tabs)
- Viewport resize updates all editors, not just focused one

### Added - Performance Overlay Sparklines

- Historical sparkline charts for render timing breakdown
- 60-frame rolling history per metric (clear, highlight, gutter, text, cursor, status, present)
- `draw_sparkline()` function with 1px bar visualization
- `record_render_history()` pushes timing to VecDeque histories

### Added - Multi-Cursor Batch Undo/Redo

- `EditOperation::Batch` for atomic multi-cursor operations
- InsertChar, InsertNewline, DeleteBackward, DeleteForward now batch
- Proper cursor restoration on undo/redo
- 6 new tests for multi-cursor undo behavior

### Added - SelectAllOccurrences (Cmd+Shift+L)

- Finds all occurrences of word/selection in document
- Creates cursor+selection for each occurrence
- Status message shows count: "Selected N occurrences"

### Added - Layout Tests

- 47 new tests in `tests/layout.rs`
- Split operations, close operations, focus navigation
- Tab operations (close, move, switch)
- Independent viewport/cursor per editor
- Edge cases (nested splits, invalid IDs)

### Changed

- Test count: 351 (was 246)
- Added 47 layout tests, 6 multi-cursor undo tests, selection tests

---

## 2025-12-06

### Added - Caret Count in Status Bar

- Shows "X carets" segment when multiple cursors are active
- New `SegmentId::CaretCount` variant
- Auto-syncs via `sync_status_bar()` when cursor count changes

### Fixed - Multi-Cursor Click Modifier

- Changed from Cmd+Click to Option+Click for adding/removing cursors
- Matches standard macOS editor conventions

### Added - Click+Drag Selection

- Standard click-and-drag text selection with left mouse button
- `left_mouse_down` state tracking in App struct
- CursorMoved handler extends selection while dragging
- Reuses existing `ExtendSelectionToPosition` message

### Added - Delete Line Command

- `DocumentMsg::DeleteLine` for deleting entire current line
- Cmd+Backspace keybinding (Ctrl+Backspace on non-Mac)
- Smart cursor positioning after delete:
  - First/middle line: stays on same line number
  - Last line: moves to end of previous line
  - Empty line after trailing newline: moves up
- Full undo/redo support
- 8 new tests in `tests/text_editing.rs`

### Added - Duplicate Line/Selection (Cmd+D)

- `DocumentMsg::Duplicate` for duplicating current line or selection
- No selection: duplicates entire line below cursor
- With selection: duplicates selected text in place
- Full undo/redo support
- 4 new tests in `tests/text_editing.rs`

### Added - Atomic Replace for Selection Editing

- `EditOperation::Replace` variant for atomic undo of selection replacement
- When typing over selection, undo restores both deleted text and removes inserted text in one operation
- Prevents "two-step undo" bug where user had to undo twice

### Fixed - Undo/Redo Keybindings on macOS

- Cmd+Z now properly triggers Undo (was inserting 'z')
- Cmd+Shift+Z now properly triggers Redo
- Fixed by adding `logo` modifier support alongside `ctrl`

### Fixed - Overflow Panics in Edge Cases

- `move_cursor_down()`: Fixed overflow when `visible_lines` is 0
- `ensure_cursor_visible_with_mode()`: Fixed horizontal scroll overflow
- `StatusBarLayout`: Fixed separator position overflow
- All arithmetic now uses `saturating_add`/`saturating_sub`

### Added - Expanded Monkey Tests

- 12 new window resize edge case tests in `tests/monkey_tests.rs`:
  - Maximum u32 dimensions
  - Very wide/narrow and very tall/narrow
  - Resize then cursor movement/scrolling
  - Oscillating zero/non-zero sizes
  - Resize with active selection
  - Cursor beyond viewport after resize
  - Powers of two dimensions
  - Interleaved resize and text operations
  - Status bar edge (height = line_height)

### Added - Status Bar Click Capture

- Clicks on status bar no longer propagate to editor
- `Renderer::is_in_status_bar(y)` method for hit testing

### Changed

- Test count: 246 (was 227)
- Added 8 delete line tests, 4 duplicate tests, 12 resize tests

---

## 2025-12-06

### Added - Status Bar System

- Structured, segment-based status bar per `docs/feature/STATUS_BAR.md`
- `StatusBar`, `StatusSegment`, `SegmentId`, `SegmentContent` types
- `sync_status_bar()` auto-updates segments from model state
- `StatusBarLayout` for rendering with separator positions
- Transient messages with auto-expiry (`TransientMessage`)
- Left/right segment alignment with separators
- 47 new status bar tests (`tests/status_bar.rs`)

### Added - Overlay System

- Reusable overlay rendering module (`src/overlay.rs`)
- `OverlayAnchor` enum (TopLeft, TopRight, BottomLeft, BottomRight, Center)
- `OverlayConfig` with builder pattern for configuration
- `render_overlay_background()` with alpha blending
- `render_overlay_border()` for optional 1px borders
- `blend_pixel()` for ARGB alpha compositing
- 7 overlay unit tests

### Added - Overlay Theme Integration

- `OverlayTheme` with themed colors: background, foreground, highlight, warning, error, border
- `OverlayThemeData` for YAML parsing (all fields optional for backward compatibility)
- Perf overlay now uses theme colors instead of hardcoded values
- Optional border rendering when theme specifies border color
- Added overlay sections to all 4 theme files

### Fixed

- Status bar separator lines now span full height (was inset 4px)
- Direction-aware scroll reveal with `ScrollRevealMode` enum
- `ensure_cursor_visible_with_mode()` primitive for scroll behavior
- Arrow key viewport snap-back behavior
- MoveCursor now properly calls ensure_cursor_visible()
- Directional reveal: Up→TopAligned, Down→BottomAligned for natural UX

### Changed

- Test count: 227 (was 185)
- Added 11 scroll reveal tests, 47 status bar tests, 7 overlay tests

---

## 2025-12-05

### Added - Selection & Multi-Cursor (Phase 7)

#### Phase 7.1: Basic Selection

- Theme support for `selection_background` and `secondary_cursor_color`
- ~25 new EditorMsg variants for selection/multi-cursor operations
- Shift+Arrow extends selection, Shift+Home/End, Shift+Click
- Selection rendering with blue highlight
- Escape clears selection or collapses multi-cursor

#### Phase 7.2: Selection Editing

- `delete_selection()` helper for selection range deletion
- InsertChar/InsertNewline deletes selection before inserting
- DeleteBackward/DeleteForward deletes selection instead of single char

#### Phase 7.3: Word & Line Selection

- SelectWord handler using `char_type` for word boundaries
- SelectLine handler (selects entire line including newline)
- Double-click selects word, triple-click selects line
- Click count tracking with wrap at 4

#### Phase 7.4: Multi-Cursor Basics

- `toggle_cursor_at()` in EditorState
- ToggleCursorAtPosition handler for Cmd+Click
- Multi-cursor rendering (primary=white, secondary=semi-transparent)

#### Phase 7.5: Multi-Cursor Editing

- `cursors_in_reverse_order()` helper
- InsertChar/InsertNewline at all cursors in reverse order
- DeleteBackward/DeleteForward at all cursors in reverse order

#### Phase 7.6: Clipboard

- arboard dependency for clipboard support
- Copy (Cmd+C) - copies selection or entire line
- Cut (Cmd+X) - copies and deletes selection
- Paste (Cmd+V) - multi-cursor aware, line-per-cursor distribution

#### Phase 7.7: Rectangle Selection

- `RectangleSelectionState` in EditorState
- Middle mouse down starts rectangle mode
- Mouse drag updates rectangle, mouse up finishes
- Creates cursors/selections for each line in rectangle
- Ghost cursor preview during drag

#### Phase 7.8: AddCursorAbove/Below

- Selection helper methods: `extend_to`, `collapse_to_start/end`, `contains`
- `deduplicate_cursors()` removes duplicate positions
- `assert_invariants()` for debug builds
- AddCursorAbove/Below handlers with column preservation
- Option+Option+Arrow double-tap detection (300ms threshold)

### Changed

- Moved 101 tests to tests/ folder (8 remaining in main.rs)
- Total test count: 185 (10 theme + 11 keyboard + 164 integration)

---

## 2025-12-04

### Added - Architecture Refactoring (Phases 1-6)

#### Phase 1: Split Model

- Created `model/` module hierarchy
- `Document` struct (buffer, undo/redo, file_path)
- `EditorState` struct (cursor, viewport)
- `UiState` struct (status, cursor blink)
- `AppModel` struct composing all state

#### Phase 2: Nested Messages

- `Direction` enum (Up, Down, Left, Right)
- `EditorMsg`, `DocumentMsg`, `UiMsg`, `AppMsg` enums
- Top-level `Msg` enum with sub-message dispatch
- Updated `handle_key()` for nested messages

#### Phase 3: Async Cmd System

- `Cmd::SaveFile` and `Cmd::LoadFile` variants
- `std::thread` + `mpsc` for async operations
- `process_cmd()` and `process_async_messages()` in event loop

#### Phase 4: Theming

- `src/theme.rs` with Color, Theme, YAML parsing
- All hardcoded colors replaced with theme lookups
- 6 new theme tests (96 total at this point)

#### Phase 5: Multi-Cursor Prep

- `Position` and `Selection` types in editor.rs
- `EditorState` uses `Vec<Cursor>` and `Vec<Selection>`
- Accessor methods: `cursor()`, `cursor_mut()`, `selection()`, `selection_mut()`
- ~220 cursor accesses updated across files

#### Phase 6: Performance Monitoring

- `PerfStats` struct with frame timing, cache stats
- `#[cfg(debug_assertions)]` gating
- Rolling 60-frame window for FPS calculation
- Semi-transparent perf overlay
- F2 toggle for overlay visibility

### Changed

- 90 tests passing after Phase 1-2
- 96 tests passing after Phase 4-5
