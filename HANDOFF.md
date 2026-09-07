# Handoff

Written 2026-09-04, at `9d34829` (7 commits past the `v0.6.0` tag, none pushed).

Read `AGENTS.md` first — it is the canonical instruction file and this document
does not repeat it. The short version: Elm-style `Message -> Update -> Command ->
Render`, update handlers stay deterministic and I/O-free, `just fmt && just test
&& just lint` before handing off, record user-visible changes in
`docs/CHANGELOG.md` under `Unreleased`.

---

## 1. Where things stand

**v0.6.0 shipped 2026-09-02** (tag pushed, cargo-dist release workflow ran). It
carried the whole August LSP wave plus two new subsystems:

- **CLI launcher** (`src/launcher.rs`): `token file` returns to the shell
  immediately and hands paths to a running editor; a detached child starts one
  when none is running. `-w/--wait` blocks until the opened documents close or
  the editor exits (the git `core.editor` contract). Directories and
  `--new-window` always get their own process. Also `path:line:col`, `token -`
  for stdin, `--foreground`, and macOS Finder/`open -a` delivery via
  `application:openURLs:` (`src/macos_open.rs`).
- **Per-instance automation** (`src/automation.rs`): every editor process listens
  on `$TMPDIR/token-<uid>/instances/<pid>.sock` (a loopback port file on
  Windows). `token automate instances` lists them, `--instance <pid>` targets
  one, the MCP bridge gained `list_instances` and an optional `instance` argument
  on every tool. The snapshot field is `instance_id` (renamed from `process_id`)
  plus `workspace_root` and `focused_at_ms`.

**Since the tag** (7 unpushed commits):

| Commit                                  | What                                                                                      |
| --------------------------------------- | ----------------------------------------------------------------------------------------- |
| `0416ed0`                               | Archived seven shipped feature plans into `docs/archived/`, refreshed the roadmap indexes |
| `ea19fd7` `df6615f` `0b9f539` `06c97a7` | find-enhancements Phases 5 + 7 (below)                                                    |
| `76e5b94` `9d34829`                     | autocomplete Phase 2, inline ghost text (below)                                           |

Suite is 2102 tests, green. `just lint` and `just fmt-check` clean.

### find-enhancements: done, doc archived

`docs/archived/find-enhancements.md`. The Find label row carries a status
("3 of 42" / "No matches" / "Invalid regex: unclosed group" in the error colour)
through a new `Field::trailing` slot; the footer legend lists the four options
with ⌥⌘C/W/R/L and a check when on. Selection scope captures the primary
selection as char offsets and `FindReplaceState::matches()` is the single
filtered list navigation, replace, replace-all, and the highlight decorations all
read.

Deliberately **not** done: mouse-clickable toggle chips (needs a new
`OverlayHit` variant and a `UiKey`; keyboard reaches everything today), and the
`SearchResults` cache the original doc specified (search is stateless and
viewport-bounded — see the doc's Phase 1/3 notes).

### autocomplete Phase 2: inline ghost text, shipped with gaps

`docs/feature/autocomplete.md` (still active — Phases 3 and 5+ remain). What
landed:

- `src/completion/inline.rs` — `InlineSuggestionState`, `RequestSnapshot`,
  `InlineRequest`, the `postprocess` filter chain (filters 1–4), prefix
  consumption, the end-of-line trigger rule.
- `src/completion/fim.rs` — llama.cpp `/infill` over a ~100-line `std::net`
  HTTP/1.1 client. **No HTTP dependency was added** and none should be until a
  TLS/hosted transport actually needs one.
- `src/runtime/inline_worker.rs` — worker thread cloned from the syntax worker's
  shape; newest request per document wins.
- `src/update/inline.rs` — triggering, revision-guarded arrival, accept as one
  undo step, failure/backoff policy.
- Paint: `render_ghost_text_stage` in `src/view/editor_text.rs`, inside
  `render_line_content_stages` so the cursor-lines-only damage path draws it too.
- Config `completion.inline` + `completion.providers` (`src/config.rs`), theme
  key `editor.ghost_text`, `inline_suggestion` in the automation snapshot and in
  screenshot scenarios, keymap `inline_suggestion_visible` condition with Tab /
  Escape / ⌥\.

Verified live against `llama-server -hf
Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF:Q8_0 --port 8012`: ghost text in ~500 ms,
type-through, Tab-accept, single-step undo, and a responsive editor after the
server was killed mid-request. The model is cached on this machine; the server
does **not** auto-start.

---

## 2. What remains

Ordered as I would do it. Items 1 and 2 are the user's stated priorities.

### Priority 1 — Soft wrap (`docs/feature/soft-wrap.md`, XL)

The largest single item in the tree and the gate on several others. See §3.

### Priority 2 — Finish inline completion (autocomplete Phases 3 and 5+)

See §4. Phase 3 is M-sized and independent of soft wrap; the multi-row half of
Phase 5+ is blocked on it.

### Priority 3 — Settings page (`docs/feature/settings-page.md`)

Its Phase 1 is the `keep_unknown` config merge, which is shippable on its own and
small.

### Priority 4 — Remaining LSP surface

Workspace symbols into the Search Everywhere Symbols tab (the tab exists,
disabled); a usages _panel_ (the popup shipped).

### Known debt carried forward

Eight bundled themes still ride derivation fallbacks (only `default-dark` is
hand-tuned); no `completion.menu.*` config
block (Phase 1 menu constants are hardcoded in `src/completion/sources.rs`); two
`#[ignore]`d load-sensitive process-spawn tests (`--include-ignored`).

Also: Windows is compiled-by-reasoning only for the launcher (`DETACHED_PROCESS`,
always-detach) and the per-instance port file. Neither has been run on Windows.

---

## 3. Soft wrap — the brief

**Status:** Planned, P2, XL, Milestone 4. Eight phases, ~21–27 days as estimated
in the doc.

### Why it is a prerequisite

Three separate features are parked behind it, all for the same reason: they need
a mapping between _logical_ positions (line, column in the buffer) and _visual_
rows (what the renderer draws), and today those are the same thing.

1. **Multi-row ghost text.** Today a multi-line suggestion draws its first line
   and collapses the rest into a `⏎ +2 lines` badge
   (`src/view/editor_text.rs:render_ghost_text_stage`). Drawing the real
   remaining rows means inserting visual rows that do not exist in the document,
   which is exactly what a wrap cache does. `autocomplete.md` Phase 5+ says so
   explicitly: _"Multi-row ghost text + mid-line suggestions — after soft-wrap's
   `logical_to_visual` mapping exists."_
2. **Mid-line suggestions.** Same reason: ghost text mid-line has to shift the
   following text right (Neovim's `inline` vs `overlay` distinction), which is a
   visual-row layout problem.
3. **Code folding** (`docs/feature/folding-basic.md`) needs the identical seam —
   a fold hides logical lines and renumbers visual rows.

### The one architectural decision that matters

**Do not build a render-only wrap cache bolted onto the draw loop.** The doc was
written before `TextViewportMap` existed and its own Data Structures section now
says this in a preamble.

`TextViewportMap` (`src/model/editor.rs`, struct at the `/// No-wrap mapping
between viewport rows/columns and logical document positions` comment) is already
the single seam every consumer asks "which logical line/column does this visible
row or pixel map to?". Its API:

```
doc_line_for_visible_row   visible_row_for_doc_line   contains_doc_line
doc_line_for_pixel_y       visual_column_for_x_offset
reveal_line_no_padding     reveal_line_with_mode      reveal_column
clamp_top_line  max_top_line  end_line  bottom_line   scroll_vertical_by
```

Consumers, all of which get wrap support for free if the map becomes wrap-aware:

- `src/view/editor_text.rs:118` (the renderer)
- `src/view/hit_test.rs:740` (clicks)
- `src/view/geometry.rs:247` and `:285` (layout)
- `src/view/caret.rs:65` (IME rect, completion popup anchor)

So: build `WrapCache` as the doc specifies (Phases 1–2), then make
`TextViewportMap` wrap-aware — or add a wrap-aware variant answering the same
queries in visual-row terms — and let the four consumers follow. Phase 3's
implementation note says the same thing.

### Phase order from the doc

| Phase | Work                                                                                                        | Est.  |
| ----- | ----------------------------------------------------------------------------------------------------------- | ----- |
| 1     | `src/wrap.rs`: `WrapCache`, `WrapSegment`, `compute_line_wraps`, `logical_to_visual` / `visual_to_logical`  | 3–4 d |
| 2     | `EditorState.soft_wrap` + cache, toggle message, invalidate on `push_edit`                                  | 2 d   |
| 3     | Wrap-aware viewport map; renderer iterates visual rows; gutter shows logical numbers + continuation markers | 4–5 d |
| 4     | Cursor Up/Down by visual line, `desired_column` across wraps, Home/End stay logical                         | 3–4 d |
| 5     | Selection rects per visual row                                                                              | 2–3 d |
| 6     | Mouse: click Y → visual row → logical position; double/triple click across wraps                            | 2 d   |
| 7     | Viewport tracks visual lines; scroll, reveal, PageUp/Down in visual rows                                    | 2 d   |
| 8     | Incremental cache updates on edit; benchmark large files                                                    | 3–4 d |

Phases 1–3 are the vertical slice that first shows something on screen. Ship
that, then 4, then the rest.

### Traps the doc already flags

- **Tab expansion must match the renderer.** `compute_line_wraps` takes a
  `tab_width` parameter in the sketch; reuse `TABULATOR_WIDTH` /
  `expand_tabs_for_display` from `src/view/geometry.rs` instead of a free
  parameter, or wrapping and painting will disagree.
- Wide Unicode is a `// TODO` in the sketch (`char_width = 1`). Decide
  deliberately; the editor is char-column based throughout.
- An empty line still occupies one visual row.
- Cache invalidation triggers: content edit, viewport width change (resize _and_
  split), and toggling wrap on.
- Non-goals, per the doc: hard wrap, configurable wrap width (use viewport
  width), character-level-only wrapping, bidi.
- `EditorState` is per-pane, so `soft_wrap` is per-pane — the doc's goal #1.
  Two splits on the same document can disagree, which means the wrap cache
  cannot live on `Document`.

---

## 4. Inline completion — what "fully complete" means

### Gaps in what shipped (fix these before Phase 3)

1. **Mid-line paint is unguarded.** `line_tail_is_short`
   (`src/completion/inline.rs`) gates _auto_-trigger only — it sits in the
   `if !explicit` branch of `schedule()` in `src/update/inline.rs`. An explicit
   ⌥\ trigger mid-line therefore produces ghost text that paints _over_ the
   text following the cursor, because `render_ghost_text_stage` overdraws.
   Either apply the same tail rule to the paint stage (cheap, matches the doc's
   v1 scope) or accept it only once soft wrap can shift the trailing text.
2. **No in-flight status glyph.** `ui.inline_in_flight` is maintained but nothing
   draws it. `docs/feature/autocomplete.md` Configuration asks for a spinner
   segment; `SegmentId` lives in `src/model/status_bar.rs`.
3. **No cancel token.** An in-flight `std` socket read cannot be aborted, so
   supersession only drops _queued_ requests and the revision guard discards late
   replies. Fine against localhost; a slow remote transport will make the wait
   visible. Documented as a deviation in the doc's Phase 2 list.
4. **`InlineProvider` is not a trait yet.** One implementation does not earn one
   — add it with the second transport, as the doc's deviation note says.

### Phase 3 (M, unblocked, no soft wrap needed)

- Transports: Ollama (`keep_alive`, per-model suffix capability surfaced as a
  clear error), OpenAI-compatible, Mistral FIM. This is where `PromptFormat`
  (sentinels, PSM/SPM, `Infer` from model name) becomes real, because those
  transports do not build the FIM prompt server-side the way `/infill` does.
  **This is also the point where an HTTP client dependency may finally be
  justified** (TLS for hosted APIs); until then keep `src/completion/fim.rs`.
- `RecencyRing` context strategy — llama.vim's design: no ranking, pure recency,
  so the prompt prefix stays stable and the server's KV cache is reused. Idle-only
  ring updates. Inline-as-comments fallback for transports without `input_extra`.
- Partial accept (Word / Line granularity, Zed's leading-run rule) and
  alternative cycling when the provider returns more than one.
- LRU cache (~256 entries) keyed on `(document, cursor, prefix tail hash)`, with
  the pre/post-cache filter split; postprocess filters 5–6 (bracket sanity,
  indentation normalisation).
- Automation coverage: suggestion visible → partial accept → full accept.

### Phase 5+ (after soft wrap)

Multi-row ghost text and mid-line suggestions first. Then, in rough order of
value: edit prediction (anchor-based edit lists, the Zed model), retrieval
context (BM25 / tree-sitter declaration extraction), TabbyML transport, a
supervised local llama-server child process, local acceptance stats, path
completion, commit characters.

### The manual checklist nobody has run

`docs/feature/autocomplete.md` → Testing Strategy → Manual checklist. Seven
items, none ticked. The two most likely to find real bugs:

- CJK/emoji around the cursor: popup anchor and ghost x-position.
- Blink fast path over visible ghost text (the `CursorLines` damage route) — the
  stage is inside `render_line_content_stages` specifically so this works, but it
  has not been eyeballed.

---

## 5. Working notes for whoever picks this up

**Build and verify**

```bash
just build        # debug
just test         # nextest + doctests, 2102 tests today
just lint         # clippy, same strictness as CI
just fmt          # rust + markdown
just release && just install   # ~/.local/bin/token
```

**Driving the real editor.** The automation socket is the fastest way to check
behaviour without a human at the keyboard:

```bash
export TOKEN_AUTOMATION_SOCKET=/tmp/token-dev.sock     # isolates from your own editor
target/release/token --foreground file.rs &
target/release/token automate state | jq .state
target/release/token automate cursor 10 4
target/release/token automate text "x"
target/release/token automate action AcceptInlineSuggestion
```

`docs/AUTOMATION.md` has the full command list. `EditorSnapshot`
(`src/automation.rs`) is the assertion surface — add a field there rather than
scraping pixels.

**Headless screenshots** for visual checks:
`cargo run --release --bin screenshot -- --scenario <yaml> --out-dir <dir>`.
Scenarios live in `screenshots/scenarios/`; the config struct in
`src/bin/screenshot.rs` supports `modal`, `inline_suggestion`, view modes,
workspaces and splits.

**Inline suggestions need a server**:

```bash
llama-server -hf Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF:Q8_0 --port 8012
```

The model is already cached. `~/.config/token-editor/config.yaml` has
`completion.inline.enabled: true` pointing at `http://127.0.0.1:8012`. With the
server down the editor shows one transient per failure and pauses auto-trigger
after three (`MAX_CONSECUTIVE_FAILURES`).

**Test conventions.** Unit tests drive `update()` directly with real `Msg`s
(`src/update/completion.rs` and `src/update/inline.rs` are good models). Runtime
tests build a headless `App::new(800, 600, cfg, None, None, None)` and push
through `automation_tx` + `process_automation_requests`
(`src/runtime/app_tests.rs`); `pump_until` drives `process_async_messages` for
worker replies. Fake servers bind `127.0.0.1:0` in-process — see
`fake_infill_server` in `app_tests.rs` and `fake_server` in
`src/completion/fim.rs`.

**Docs discipline.** Feature plans in `docs/feature/` are active; move one to
`docs/archived/` with a dated `> **Status:**` line when it ships, and fix the
inbound links such as `docs/README.md`.
`0416ed0` is the pattern.

**Release process** is in `AGENTS.md` §Releases. Preparing a release does not
authorize publishing it.
