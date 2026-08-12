# Autocomplete & Inline Suggestions

A pluggable completion system with two rendering surfaces — a popup menu at the cursor and ghost-text inline suggestions — fed by swappable providers: buffer words and snippets first, LSP when it lands, and LLM fill-in-the-middle backends (local or remote) behind one backend abstraction.

> **Status:** 🚧 In Progress — Phase 1 (menu completion: words + snippets) shipped; Phases 2+ (inline suggestions, FIM backends, LSP source) not started.
> **Priority:** P2 (Important)
> **Effort:** XL (phased — each phase ships independently)
> **Created:** 2026-08-11
> **Updated:** 2026-08-11
> **Milestone:** 4 - Hard Problems

---

## Overview

### Why

Completion is the highest-frequency assist feature an editor has — it fires on nearly every keystroke in insert flow. It is also two distinct features that editors routinely conflate:

1. **Menu completion** — a filtered list of discrete candidates (identifiers, snippets, LSP items) the user picks from. Deterministic, symbol-shaped, latency budget ~10 ms for local sources.
2. **Inline suggestion** — a single speculative continuation rendered as ghost text and accepted with Tab. Probabilistic, text-shaped, latency budget 100–1000 ms, produced by an LLM or any other predictor.

Every mature editor (Zed, VS Code, Neovim ecosystem) models these as **separate subsystems with separate data models that only touch at two points**: a key-routing precedence rule and a "menu suppresses ghost text" rule. We adopt the same split. Getting the split right up front is what makes the system pluggable: an LLM is just one inline provider, LSP is just one menu source, and a future tree-sitter-driven "syntax-aware" predictor slots into the same inline mechanism with zero new UI.

### Terminology

The ecosystem's names for these concepts, and what this document calls them:

| Our term | Concept | Elsewhere called |
| --- | --- | --- |
| **Menu completion** | Popup list of candidates, fuzzy-filtered as you type | IntelliSense / Suggest widget (VS Code), Code Completion (JetBrains), `textDocument/completion` (LSP), pum (Vim) |
| **Inline suggestion** | Ghost text after the cursor, insert-only, Tab to accept | Inline completions (VS Code), ghost text (everywhere), Full Line Code Completion (JetBrains), `virt_text` (Neovim) |
| **Edit prediction** | Suggested *rewrite* of existing code, possibly away from the cursor, shown as a diff | Edit Prediction / Zeta (Zed), Next Edit Suggestions (Copilot NES), Cursor Tab | 
| **Menu source** | A producer of menu items (words, snippets, LSP, paths) | source (nvim-cmp/blink.cmp), backend (company), capf (Emacs) |
| **Inline provider** | A producer of inline suggestions (FIM backend, heuristic predictor) | InlineCompletionProvider (VS Code), EditPredictionDelegate (Zed) |
| **FIM** | Fill-in-the-middle: prompting a code LLM with prefix + suffix around the cursor | infill (llama.cpp), insert mode (Ollama) |

Adjacent features that are **not** this document: signature help / parameter hints (LSP feature, popup but not completion), snippet *placeholder navigation* (`docs/feature/snippets.md`), postfix templates. Edit prediction (rewrites + jump targets) is designed-for but deferred — see Non-Goals.

### Current State

Nothing completion-shaped exists. Relevant infrastructure (verified against the codebase):

- **Elm loop + async worker pattern**: `src/runtime/app.rs::syntax_worker_loop` — `std::thread` + `mpsc`, results as `Msg` via `msg_tx` + `EventLoopProxy` wake, request coalescing per document, revision guards on both ends. The debounce mechanism (`syntax_deadlines: HashMap<DocumentId, (Instant, u64)>` checked in `about_to_wait`) is exactly what completion triggering needs — a second deadline map folded into the same `next_wake` min.
- **Popup surface**: [`overlay-surface.md`](overlay-surface.md) (Milestone 1) plans the unified `OverlaySurface` component with an `Anchor::Cursor` mode and a **Completion context** (kind badge + label + dim signature rows, flip/clamp, dismiss rules) built exactly for this feature and LSP's popups. This document consumes that shell and owns none of the popup painting. If autocomplete Phase 1 somehow lands first, the interim fallback is today's `selectable_list.rs` + a cursor-anchored overlay bound — but overlay-surface deletes `selectable_list.rs`, so don't build on it deliberately. `src/view/caret.rs::active_text_input_rect` computes the caret pixel rect either way.
- **Prefix extraction**: `src/update/document.rs::word_start_before` / `word_end_after` (char-class based) — the completion-query extractor, already written and tested.
- **Fuzzy matching**: `nucleo-matcher` is already a dependency (file finder). Lapce uses the same crate for completion filtering.
- **Multi-cursor atomic edits**: `EditOperation::Batch` in `src/model/document.rs` — accepting a completion at N cursors is one undo step, no new machinery.
- **Key routing**: `KeyContext` / `Condition` in `src/keymap/context.rs`; Tab is currently bound unconditionally to `InsertTab` with a standing TODO to make it context-conditional — completion lands exactly in that mechanism.
- **The hard rendering constraint**: layout is strictly **1 buffer line = 1 visual row** (`TextViewportMap` in `src/model/editor.rs` is pure arithmetic). There is no virtual-text mechanism. Consequence: *single-line* ghost text on the cursor row is cheap (draw after `render_line_text_stage` at `column_to_pixel_x`); *multi-row* ghost text (virtual lines below the cursor) requires the logical→visual mapping that `docs/feature/soft-wrap.md` introduces. This constraint drives the phasing.
- **Prior in-repo prose**: `docs/EDITOR_UI_REFERENCE.md` ch. 7 ("Autocomplete and Overlay Positioning") and `docs/feature/lsp-integration.md` Phase 5 both sketch a completion popup. This document supersedes and details both sketches; the LSP doc's Phase 5 becomes "plug the LSP menu source into this system" (see [LSP Integration](#integration-with-lsp)).

### Goals

- Two surfaces (menu, inline), each with a pluggable provider interface, shipped in that order.
- Menu completion works fully offline with zero configuration: buffer words + snippets, fuzzy-filtered, multi-cursor-correct, one undo step.
- Inline suggestions work against any of: llama.cpp `/infill`, Ollama, OpenAI-compatible `/v1/completions` (with `suffix`), Mistral FIM API — via one backend abstraction with independently pluggable **transport**, **prompt format**, and **context strategy** (Zed's factoring, validated across the ecosystem).
- Non-LLM inline providers are first-class: the provider interface takes a document snapshot and returns a suggestion; nothing in it assumes a network or a model.
- Every response is revision-guarded; no stale result ever inserts text or moves a cursor. The editor is never blocked on a provider.
- All commands palette-visible, rebindable, automation/MCP-invokable (`is_simple()` commands).
- Everything degrades to today's editor when disabled or unconfigured.

### Non-Goals

- **Edit prediction (rewrites, jump-to-edit, diff popovers)** — Zed's anchor-based edit lists and diff rendering are the right eventual model, but they require anchor infrastructure and overlay diff rendering we don't have. The inline provider interface is shaped so a rewrite-capable provider can be added later (see Design Decisions), but v1 suggestions are insert-at-cursor only.
- **Multi-row ghost text before soft-wrap lands.** First-line-inline + `+N lines` collapse indicator instead (blink.cmp's `show_first_line_only` pattern). Full multi-line rendering is explicitly sequenced after `soft-wrap.md`'s `logical_to_visual` mapping exists.
- Snippet placeholder navigation (converges with `snippets.md`; completions insert snippets flattened to plain text until then).
- Bundling or auto-downloading models. The user runs their own llama.cpp/Ollama/Tabby server or supplies an API key; we ship config, not weights.
- Telemetry beyond local acceptance logging. No network telemetry, ever.
- Semantic ranking (type-aware "smart completion"). That arrives for free with LSP `sortText`.

---

## Prior Art

Condensed from a survey of Zed, Neovim (nvim-cmp, blink.cmp, copilot.vim/lua, codeium.nvim), Emacs (capf, company, corfu, copilot.el), Helix, Lapce, and the Copilot/Continue/Tabby/llama.vim client pipelines. Full details in the references; what follows is what transfers.

### How each editor models it

| Editor | Menu model | Inline model | Rendering |
| --- | --- | --- | --- |
| **Zed** | One `CompletionProvider` (LSP) + built-in words/snippets merged into one list; fuzzy + tiered sort; no timer debounce (request-coalescing by ID + prefix-refinement skip) | `EditPredictionDelegate` trait: `refresh/suggest/accept/discard`, per-provider debounce (Copilot 75 ms, Codestral 150 ms, Zeta 300 ms throttle); predictions are **anchor-based edit lists**, survive typing via interpolation | Ghost text = real inlays in the display map (wraps/folds like buffer text); deletions = red highlight; mixed = diff popover. Conflicts resolved entirely in keymap contexts |
| **Neovim** | `ins-completion` two-phase provider protocol (find-start, then matches); nvim-cmp/blink.cmp bypass the built-in pum with floating windows; blink.cmp filters with SIMD Smith-Waterman (frizbee) in Rust | copilot.lua / codeium: extmarks with `virt_text` (line 1, `inline` position) + `virt_lines` (rest); 45–75 ms debounce | Extmark virtual text; `inline` shifts real text (correct mid-line), `overlay` paints over (EOL only) |
| **Emacs** | capf: `(START END COLLECTION . PROPS)` with `:exclusive no` chaining; company adds async via `(:async . FETCHER)`; corfu consumes capf directly, delegates matching to `completion-styles` | copilot.el: two overlays — ghost text (`display` + `after-string`) plus a priority-101 keymap overlay that scopes Tab rebinding to while-suggestion-visible | Overlays / child frames |
| **Helix** | Event-driven handler; LSP + word + path sources collected concurrently in a `JoinSet`; responses dropped on doc/view change | **None in core** — blocked on inline-rendering work (their virtual-text API exists but the state machine doesn't); interim solution is an LSP shim (helix-gpt) putting AI results in the normal menu | Popup component |
| **Lapce** | `CompletionData` with `request_id`/`input_id` staleness discrimination, per-input response cache, nucleo scoring of `filter_text` + `label` | "Completion lens" phantom text previews the *selected menu item* first line; native AI inline is an open PR | Phantom text (same primitive as inlay hints) |

### Cross-cutting lessons (these are the spec's load-bearing decisions)

1. **Invalidation is snapshot-based, never marker-based.** Every implementation snapshots `(document, revision, cursor)` at request time, drops responses whose snapshot moved, and clears + redraws on every edit. Nobody trusts marks/anchors to track edits for pending requests. This maps 1:1 onto our existing `ParseCompleted` revision-guard pattern.
2. **Ghost text survives typing via prefix-consumption, not re-request.** If the typed character equals the suggestion's next character, shorten the suggestion in place (copilot.el's `self-insert` optimization, Zed's `interpolate_edits`, Continue's `GeneratorReuseManager`). This is the difference between ghost text that feels solid and ghost text that flickers.
3. **Key conflicts are resolved by context conditions, not handler branching.** Zed: `showing_completions`, `edit_prediction` GPUI contexts. Emacs: keymap scoped to an overlay. Vim: `<expr>` mapping with fallback keys. Our `KeyContext`/`Condition` system is the same mechanism — Tab means "accept" only when a suggestion is visible, and users can rebind their way out of any conflict.
4. **Menu suppresses ghost text** (or the provider explicitly opts into rendering inside the menu, like Zeta's stacked row). Never render both.
5. **Two failure-prone spots get special treatment everywhere:** mid-line suggestions with text after the cursor (llama.vim refuses to auto-trigger with >8 chars right of cursor; nvim-cmp hides its overlay fallback), and the empty-suffix case (several FIM models degrade; Tabby substitutes `"\n"`).
6. **Post-processing beats model size.** JetBrains ships a usable product on a *100M-parameter* local model because of beam pruning plus a five-stage reject filter (too short, unsafe, low score, statically invalid, unbalanced). Tabby's 15-filter chain is split **pre-cache** (normalize once) vs **post-cache** (trim per serve, because a cached completion is replayed as the user types through it) — that split is the non-obvious structural insight, and each filter ships with a golden-file test corpus.
7. **Latency reality check:** GitHub targets sub-200 ms; JetBrains local FLCC averages ~150 ms with >90 % cache hits; a fully local llama.cpp + Qwen2.5-Coder-1.5B setup on Apple Silicon measures **~500 ms (empty context) to ~1150 ms (full ring-buffer context)** per suggestion. Local FIM is a 0.5–1 s experience, not 200 ms — debounce and cancellation policy must assume that.

---

## Background: FIM and the Model Landscape

### Fill-in-the-middle

Two papers define the technique every completion model now uses:

- **InCoder** (Fried et al., [arXiv:2204.05999](https://arxiv.org/abs/2204.05999)): *causal masking* — cut spans out of training documents, move them to the end behind sentinel tokens, so a plain left-to-right decoder learns to generate a missing middle after having seen the right context. An `<EOM>` token signals span completion — the ancestor of every FIM stop token.
- **OpenAI FIM** (Bavarian et al., [arXiv:2207.14255](https://arxiv.org/abs/2207.14255)): the *document-level FIM transformation* — split prefix/middle/suffix, reorder around `<PRE>/<SUF>/<MID>` sentinels. Two findings matter to us: **"FIM-for-free"** (training with FIM doesn't hurt normal generation, so every modern code model has it), and **SPM ordering** (suffix-first) exists specifically for editors — as the user types, the *prefix* changes but the suffix doesn't, so putting the suffix first keeps the server's KV-cache valid across keystrokes. Also: single-line infilling is dramatically more reliable than multi-line (~0.60 vs ~0.29–0.38 pass rates) — treat them as different products with different trigger rules.

Practical implication: a "FIM request" is universally `(prefix, suffix, extra_context) → middle`, but every model family has **different sentinel tokens** (StarCoder `<fim_prefix>`, Qwen `<|fim_prefix|>`, CodeLlama `<PRE>`, DeepSeek full-width-bar tokens, Codestral `[SUFFIX]…[PREFIX]` SPM, Mellum SPM + `<filename>` tags…). Getting one byte wrong silently produces garbage. Hence **prompt format is a first-class enum**, decoupled from transport, with an `Infer`-from-model-name variant (Zed's exact factoring).

### Models worth targeting

| Model | Size (Q4 GGUF) | License | Notes |
| --- | --- | --- | --- |
| **Qwen2.5-Coder-0.5B/1.5B base** | 491 MB / 1.12 GB | Apache 2.0 | The default recommendation. Documented repo-level FIM, llama.cpp ships `--fim-qwen-*` presets. Avoid the 3B (non-commercial Qwen-Research license). |
| **JetBrains Mellum-4b** (base/sft) | 2.6 GB | Apache 2.0 | The "JetBrains model". SPM + `<filename>` context tags. **GPU-only in practice** — JetBrains themselves measured up to 3 s on CPU. Quality tier, not default. |
| **deepseek-coder-1.3b-base** | 873 MB | DeepSeek license (commercial OK) | 16K ctx; no file-separator token (cross-file context goes in as comments). |
| **StarCoder2-3B** | 1.85 GB | OpenRAIL-M | Usable but license carries pass-through obligations. |
| **Codestral (Mistral)** | API | API: paid. Open 22B: **MNPL, non-production** | Use via the hosted FIM API (`/v1/fim/completions`, native SPM); the open weights can't ship in a product. |

Reference latencies: JetBrains' fully-local FLCC (100M params, ONNX INT8) ≈ 75–150 ms; Mellum served on cloud GPUs has a 90 %-under-500 ms SLO; local llama.cpp with the 1.5B ≈ 0.5–1.2 s. We should expose the provider's expected-latency class in config and tune debounce per class rather than pretending one number fits all.

### Serving APIs to abstract over

| Backend | Endpoint | Cross-file context | Distinguishing features |
| --- | --- | --- | --- |
| **llama.cpp** | `POST /infill` | ✅ `input_extra: [{filename, text}]` | Server builds the FIM prompt from the model's vocab (no client sentinels); `t_max_prompt_ms`/`t_max_predict_ms` time budgets returning partials; `n_indent` scope stop; `cache_prompt` + `--cache-reuse` chunked KV reuse; `id_slot` cache affinity. The richest target — build for it first. |
| **Ollama** | `POST /api/generate` with `suffix` | ❌ (inline into prompt) | `suffix` support is per-model (template must reference `.Suffix` — base tags yes, instruct mostly no); `keep_alive: -1` mandatory (cold load is seconds). Its OpenAI-compat `/v1/completions` also honors `suffix`. |
| **OpenAI-compatible** | `POST /v1/completions` with `suffix` | ❌ | Covers vLLM, DeepSeek `/beta`, and most self-hosted gateways. |
| **Mistral** | `POST /v1/fim/completions` | ❌ | `temperature` capped at 0.7; response is chat-completion-shaped. |
| **TabbyML** | `POST /v1/completions` (segments) | ✅ declarations + snippet lists + server-side search | Does all prompt building and post-processing server-side; also defines the acceptance-telemetry schema worth copying (`view`/`select`/`dismiss` events keyed on completion id). Future adapter, not v1. |

---

## Architecture

### The two surfaces, and where the pieces live

```
src/
├── completion/                    # New module — everything completion
│   ├── mod.rs                     # Public exports
│   ├── menu.rs                    # CompletionMenuState, filtering, sorting
│   ├── sources.rs                 # MenuSource impls: words, snippets (LSP later)
│   ├── inline.rs                  # InlineSuggestionState, prefix-consumption
│   ├── provider.rs                # InlineProvider abstraction + registry
│   ├── fim/
│   │   ├── mod.rs                 # FimRequest/FimResponse, provider impl
│   │   ├── transport.rs           # llama.cpp /infill, Ollama, OpenAI-compat, Mistral
│   │   ├── prompt.rs              # PromptFormat enum (sentinels, PSM/SPM, Infer)
│   │   ├── context.rs             # Context strategy: cursor window + recency ring
│   │   └── postprocess.rs         # Filter chain (pre-cache / post-cache split)
│   └── worker.rs                  # completion_worker_loop (syntax-worker pattern)
├── update/completion.rs           # CompletionMsg handlers, revision guards
├── view/completion.rs             # OverlaySpec builder for the menu (surface from overlay-surface.md) + ghost text paint stage
├── messages.rs                    # + Msg::Completion(CompletionMsg)
├── commands.rs                    # + completion Cmd variants, damage arms
└── runtime/app.rs                 # worker spawn, completion deadline map
```

`CompletionMenuState` and `InlineSuggestionState` live on `UiState` (view state, like modals — a popup is not document data). Provider handles (HTTP clients, child processes if we ever supervise a llama-server) live in the runtime, never the model — same non-`Clone` constraint as `PtyHandle` and the planned `LspManager`.

### Data structures

```rust
// ---- messages.rs ----
pub enum CompletionMsg {
    // user intents
    TriggerMenu { explicit: bool },          // Ctrl+Space or auto-trigger
    MenuNext, MenuPrev, MenuPageDown, MenuPageUp,
    AcceptMenuItem,                          // Enter/Tab while menu visible
    TriggerInline { explicit: bool },        // manual request or auto
    AcceptInline(AcceptGranularity),         // Full | Word | Line
    CycleInline { forward: bool },           // alternatives, if provider returned >1
    Dismiss,                                 // Escape — closes whichever is active
    // debounce lifecycle (internal)
    InlineDeadlineFired { document_id: DocumentId, revision: u64 },
    // worker → update (revision-guarded on arrival)
    MenuItemsReady   { snapshot: RequestSnapshot, items: Vec<MenuItem>, is_incomplete: bool },
    InlineReady      { snapshot: RequestSnapshot, suggestion: InlineSuggestion },
    InlineFailed     { snapshot: RequestSnapshot, error: String },   // status transient, never modal
}

pub enum AcceptGranularity { Full, Word, Line }

/// Captured at request time; every response carries it back. The universal
/// staleness guard (same shape as SyntaxMsg::ParseCompleted).
#[derive(Clone, Debug, PartialEq)]
pub struct RequestSnapshot {
    pub document_id: DocumentId,
    pub revision: u64,
    pub cursor: Cursor,          // line + char col
    pub request_id: u64,         // monotonic, supersede-and-drop
}

// ---- completion/menu.rs ----
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,           // displayed
    pub filter_text: String,     // matched against the typed query (defaults to label)
    pub insert: MenuInsert,      // what accepting does
    pub kind: MenuItemKind,      // Word | Snippet | Function | Variable | ... (LSP-compatible superset)
    pub source: MenuSourceId,    // Words | Snippets | Lsp — for sort tiering + icon
    pub sort_key: Option<String>,// LSP sortText passthrough; None sorts by score alone
    pub detail: Option<String>,  // right-aligned annotation (type, source)
}

pub enum MenuInsert {
    /// Replace [replace_start..cursor] with text. Covers words, snippets-as-plain-text,
    /// and LSP textEdit (converted to our coords on arrival, clamped).
    Replace { replace_start: Cursor, text: String },
}

pub struct CompletionMenuState {
    pub snapshot: RequestSnapshot,       // what the items were computed against
    pub query_start: Cursor,             // word start; query = text[query_start..cursor]
    pub items: Vec<MenuItem>,            // unfiltered, from all sources
    pub filtered: Vec<(u32 /*score*/, usize /*items idx*/)>,
    pub selected: usize,
    pub is_incomplete: bool,             // re-request instead of local refilter
    pub viewport_offset: usize,          // SelectableListViewport state
}

// ---- completion/inline.rs ----
#[derive(Clone, Debug)]
pub struct InlineSuggestion {
    /// v1: plain text inserted at the snapshot cursor. Multi-line allowed in the
    /// data model from day one; the renderer collapses rows it can't draw yet.
    pub text: String,
    pub alternatives: Vec<String>,       // for CycleInline; often empty
    pub provider: InlineProviderId,
}

pub struct InlineSuggestionState {
    pub snapshot: RequestSnapshot,
    pub suggestion: InlineSuggestion,
    pub consumed: usize,                 // chars of `text` already typed by the user
    pub alt_index: usize,
}
```

Deliberate simplifications, each with its upgrade path:

- `MenuInsert` is a single-variant enum (ponytail: one variant until LSP's `additionalTextEdits`/auto-import needs a second — the enum exists so adding it is non-breaking).
- `InlineSuggestion.text` is a string, not Zed's `Vec<(Range<Anchor>, Arc<str>)>`. Insert-at-cursor covers menu-less completion entirely; anchor-based edit lists arrive with edit prediction, as a new `InlineSuggestion` variant, and the accept/render plumbing is the only code that changes.
- `consumed` implements prefix-consumption (lesson #2): on each typed char, if it equals `text[consumed]`, increment `consumed` and redraw the (shorter) ghost text — **no re-request**. Any other edit, cursor move, or Escape clears the state. Backspace over consumed chars decrements `consumed` (copilot.el behavior) rather than clearing — cheap and much less flickery.

### Data flow

**Menu (synchronous sources — words, snippets):**

```text
typed char / Ctrl+Space
  → update/editor.rs inserts char (normal path, unchanged)
  → update/completion.rs: should_trigger? (word char, or explicit)
      collect: word_start_before(cursor) → query
      sources run inline in update (they're rope scans + static tables, <1 ms):
        WordsSource: words_in_range around cursor (±N lines), dedup vs query + other items
        SnippetsSource: static per-language table prefix match
      fuzzy-filter with nucleo-matcher, sort (tier: exact > prefix > score; then source; then label)
  → CompletionMenuState set on UiState, Cmd::redraw_editor()
subsequent typing
  → query grows → local refilter only (no re-collect unless word boundary crossed)
  → query empty or non-word char → dismiss
Enter/Tab
  → AcceptMenuItem → MenuInsert::Replace applied at every cursor via EditOperation::Batch
  → dismiss, Cmd::redraw + schedule_syntax_parse (normal edit path)
```

No worker and no debounce for v1 menu sources — they are microseconds of rope scanning. The worker enters the picture when a source is async (LSP), using `is_incomplete` + re-request, and the menu code path doesn't change because items always arrive via `MenuItemsReady` when async and via direct call when sync.

**Inline (async, debounced, cancellable):**

```text
edit lands (revision bumped)
  → update/completion.rs: auto-trigger check
      gates: inline enabled, provider configured, cursor at/near EOL
             (≤ N chars of non-closer text right of cursor — llama.vim's max_line_suffix),
             not mid-menu, document has a language the provider accepts
  → Cmd::DebouncedInlineRequest { document_id, revision, delay_ms }   (deadline map)
deadline fires (about_to_wait)
  → InlineDeadlineFired → re-check revision current → snapshot:
      RequestSnapshot + prefix/suffix strings (budgeted, char-window around cursor)
      + context chunks (recency ring, see below)
  → Cmd::RunInlineRequest(request) → completion worker thread
worker
  → supersede: newest request per document wins; in-flight HTTP gets its token cancelled
  → transport builds body (or renders sentinels via PromptFormat when transport is raw)
  → HTTP with client-side timeout (t_max budgets server-side where supported)
  → postprocess filter chain (trim, dedupe-vs-suffix, bracket sanity, stop trimming)
  → msg_tx.send(Msg::Completion(InlineReady{..})) + proxy wake
update
  → revision + cursor guard (drop stale), menu-visible guard (drop, lesson #4)
  → InlineSuggestionState set, Cmd::redraw_cursor_lines(vec![cursor_line])
typing through it → prefix-consumption (no request)
divergent edit / cursor move / Escape → clear state, maybe schedule new request
Tab (context: inline_suggestion_visible)
  → AcceptInline(Full) → insert remaining text as one EditOperation (undoable)
  → immediately schedule a follow-up request (chained-accept flow, Zed/llama.vim pattern)
```

Damage note: ghost text on the cursor row rides the existing `DamageArea::CursorLines` fast path only if the row count doesn't change; the `+N lines` collapse indicator keeps it single-row, so it does. The menu popup overlaps arbitrary rows → it forces `DamageArea::EditorArea` while visible (documented ceiling: no rectangle-granularity damage; acceptable because the popup is short-lived).

### Rendering

**Menu popup** — rendered through [`overlay-surface.md`](overlay-surface.md)'s **Completion context**, not bespoke drawing:

- The menu builds an `OverlaySpec` per frame: `Anchor::Cursor` at `query_start` (so the list aligns with what's being completed, from `active_text_input_rect` + `column_to_pixel_x`), `Body::List` with rows = `RowIcon::KindBadge(kind)` + label with `match_indices` (nucleo indices) + `Accessory::DimText(detail)`. Flip-above, edge clamping, no backdrop dim, and dismiss-on-edit rules are the surface's cursor-anchored behavior — defined there, consumed here.
- `MenuItemKind` maps onto the surface's kind-badge palette (`overlay.kind_*` theme keys, derived from syntax colors).
- Docs side panel (LSP `documentation`) is the surface's `ListWithPanel` v2 — deferred with it, not built twice.
- Sequencing: overlay-surface Phases 1 + 5 (painter primitives, cursor anchor) are a **prerequisite for this document's Phase 1 popup**. Its milestone (1) precedes ours (4); if that ordering ever inverts, ship the popup on the old `selectable_list.rs` shell and migrate with the other contexts.

**Ghost text** (inside `editor_text.rs`'s per-line stage pipeline — the `VisibleTextLine` doc comment explicitly invites decorations to plug into these stages):

- After `render_line_text_stage` for the cursor line, if `InlineSuggestionState` is current: take `text[consumed..]`, split at first `\n`. Draw the first segment at `column_to_pixel_x(cursor_visual_col)` in the theme's ghost color (a dimmed foreground — new theme key `syntax.ghost_text` with a computed fallback). If more lines exist, append a ` ⏎ +N lines` badge in the same dim color.
- Mid-line case: v1 draws ghost text only when the rest of the line after the cursor is empty or whitespace/closers — matching the trigger gate, so the "shift the following text right" problem (Neovim's `inline` vs `overlay` distinction) never arises. When soft-wrap's visual-row infrastructure lands, both mid-line shifting and true multi-row ghost text become one follow-up feature.
- Cursor stays a bar *before* the ghost text; ghost text never affects `TextViewportMap`, scrollbars, or hit-testing (it is paint-only — clicking "through" it targets the real buffer position).

### Key handling

Key routing uses the `KeyContext`/`Condition` mechanism that exists today, with the flag ownership split along the surface boundary:

- **Menu**: the menu is a cursor-anchored overlay context, so it is covered by [`overlay-surface.md`](overlay-surface.md)'s generalized `overlay_routes_keys` flag (its evolution of the LSP plan's `completion_visible`) — one flag for all cursor-anchored popups routing Up/Down/Enter/Escape while visible. This document does **not** introduce a separate `completion_menu_visible` field; bindings below that say `menu_visible` compile to `overlay_routes_keys` + the active overlay context being Completion.
- **Inline**: ghost text is not an overlay, so it gets its own `inline_suggestion_visible` field + `Condition` variant — the one genuinely new flag this document adds.

Default bindings:

```
Ctrl+Space                          → TriggerMenu (explicit)
menu_visible: Down/Up/PgDn/PgUp     → MenuNext/MenuPrev/…
menu_visible: Enter                 → AcceptMenuItem
menu_visible: Tab                   → AcceptMenuItem          (menu wins over inline — lesson #4)
menu_visible: Escape                → Dismiss
inline_visible && !menu_visible:
    Tab                             → AcceptInline(Full)
    Cmd+Right (or Ctrl+Right)       → AcceptInline(Word)
    Alt+]/Alt+[                     → CycleInline
    Escape                          → Dismiss
(unconditional Tab → InsertTab remains the fallback, resolving the standing keymap TODO)
```

Routing lives in the keymap via conditions, **not** in `runtime/input.rs` branches, so every conflict (Tab-vs-indent in leading whitespace, snippet tabstops later) is user-rebindable — the single clearest lesson from Zed. Typing printable characters is never captured: it flows to the document, and the menu/inline state reacts (refilter / consume / dismiss).

---

## The Provider Model

### Menu sources

```rust
pub enum MenuSourceId { Words, Snippets, Lsp }

/// Synchronous sources are plain functions called from update.
/// Async sources (LSP) go through the request/response Msg cycle.
/// Both produce Vec<MenuItem>; the menu doesn't know the difference after collection.
```

Not a trait for v1 (two hardcoded sources; a trait with one call site is speculation). The seam where a trait appears is the moment a third *async* source exists — the `MenuItem`/`MenuItemsReady` contract is the actual pluggability boundary, and it's already source-agnostic.

- **WordsSource**: identifiers within ±1000 lines of the cursor (Zed scans ±5000; we start smaller), char-class word extraction (reuse `util::text::char_type`), min length 3, dedup against the query and LSP items, capped count. Runs on the UI thread — it's a bounded rope scan.
- **SnippetsSource**: per-language static table in the syntax registry (a `&'static [(prefix, body)]` on `LanguageDefinition`, following how `selection`/`outline` behaviors are attached). Bodies flattened to plain text until `snippets.md` lands, then this source becomes its client unchanged.
- **LspSource** (when LSP Phase 1–2 exist): see [Integration with LSP](#integration-with-lsp).

Filtering and ranking (all client-side, uniform across sources): nucleo-matcher over `filter_text`, sort by (exact match, word-start match tier, score desc, source tier LSP > Snippets > Words, `sort_key`, label). Blink.cmp's frizbee and Zed's tiering agree this ordering is right; nucleo is already in-tree and is what Lapce ships.

### Inline providers

```rust
pub enum InlineProviderId { Fim(String /*backend name from config*/), /* future: Heuristic, EditPrediction */ }

/// The worker-side contract. Implementations run on the completion worker thread.
pub trait InlineProvider: Send {
    /// Build + execute one suggestion request against a snapshot. Blocking is fine —
    /// the worker thread owns the wait; supersession cancels via the token.
    fn suggest(&mut self, req: &InlineRequest, cancel: &CancelToken)
        -> Result<InlineSuggestion, ProviderError>;
    /// Called on accept/dismiss — cache upkeep, future local telemetry. Default no-op.
    fn feedback(&mut self, _event: FeedbackEvent) {}
}

pub struct InlineRequest {
    pub snapshot: RequestSnapshot,
    pub prefix: String,              // budgeted window before cursor (chars, from rope)
    pub suffix: String,              // budgeted window after cursor ("\n" if empty — FIM quirk)
    pub language: LanguageId,
    pub file_path: Option<PathBuf>,
    pub context: Vec<ContextChunk>,  // {filename, text} — recency ring output
    pub explicit: bool,              // manual trigger: skip debounce, allow longer budget
}
```

This trait is the pluggability guarantee the user asked for: it does not mention HTTP, models, or FIM. A syntax-aware heuristic provider (e.g. tree-sitter-driven "close this block / repeat this pattern") implements the same two methods and plugs into identical triggering, rendering, acceptance, and guards. The FIM provider is merely the first implementation.

### The FIM provider: three independent axes

Zed's factoring, adopted wholesale because it's the only one that survives new backends:

```rust
// completion/fim/transport.rs — WHICH HTTP SHAPE
pub enum Transport {
    LlamaCppInfill { url: String },          // /infill: input_prefix/suffix/extra, t_max_*, id_slot
    Ollama         { url: String, model: String },          // /api/generate + suffix, keep_alive: -1
    OpenAiCompat   { url: String, model: String, api_key: Option<Secret> }, // /v1/completions + suffix
    MistralFim     { api_key: Secret, model: String },      // /v1/fim/completions, temp ≤ 0.7
}

// completion/fim/prompt.rs — WHICH SENTINELS/ORDER (only used when the transport
// doesn't build the prompt server-side; llama.cpp does, Ollama-with-suffix does)
pub enum PromptFormat {
    Infer,           // from model name — the default
    Qwen, StarCoder, CodeLlama, DeepSeek, Codestral, Mellum, /* extend as needed */
}

// completion/fim/context.rs — WHAT EXTRA CONTEXT
pub enum ContextStrategy {
    None,
    RecencyRing {    // llama.vim's design: no ranking, pure recency → stable prompt prefix
        max_chunks: usize,       // default 8
        chunk_lines: usize,      // default 64
        // chunks enqueued on file switch, save, large cursor jumps; deduped by
        // token-set similarity (>0.9 evicts); updated on idle only, so the
        // prompt's stable region stays stable and server KV-cache reuse works
    },
}
```

Capability flags per transport (`builds_fim_prompt`, `supports_extra_context`, `supports_time_budget`, `supports_slot_affinity`) steer the request builder — e.g. when `supports_extra_context` is false, ring chunks are inlined into the prefix as commented snippets (`// Path: …` headers, the Copilot/DeepSeek convention).

Why RecencyRing and not retrieval: retrieval that re-ranks per keystroke invalidates the server's prefix cache — the single biggest local-latency lever. llama.vim's ring (stable chunks first in the prompt, `cache_prompt: true`, `--cache-reuse`) turns a large repo context into a one-time prompt-eval cost. Retrieval (BM25 à la Zeta/Tabby) is a future `ContextStrategy` variant; the enum is the seam.

### Post-processing

Small, ordered, individually golden-file-tested filter chain (`fim/postprocess.rs`), the highest-leverage quality component per every surveyed system:

1. Strip leaked sentinel tokens and everything after them (all vocabularies).
2. Trim to stop: blank-line-at-lower-indent boundary (Tabby's `limitScopeByIndentation` — never suggest past the current block).
3. Drop if the suggestion duplicates the text already following the cursor (rolling comparison vs the first ~30 suffix chars — Continue's `stopAtStartOf`).
4. Drop degenerate results: empty/whitespace, < 2 alphanumerics, immediate repetition (same line ≥3×).
5. Bracket sanity: truncate at the first closer that has no opener in suggestion-or-prefix scope.
6. Normalize indentation to the document's tab/space style.

Cache: LRU (~256 entries) keyed on `(document, cursor position, prefix tail hash)` — replay on backspace-and-retype and on the chained-accept flow. Filters 1–4 run pre-cache, 5–6 post-cache (the Tabby split).

---

## Integration with LSP

This section amends `docs/feature/lsp-integration.md` (its Phase 5 sketched a standalone completion popup; this document now owns all completion UI/state, and LSP Phase 5 shrinks to "implement the source"):

- **LSP is a menu source, not the menu.** `lsp-integration.md` Phase 5's dropdown, `completion_visible` KeyContext, nucleo filtering, and textEdit application are all *this* document's Phase 1–2 machinery. The LSP work that remains: send `textDocument/completion` with the server's trigger characters and the debounced request cycle, convert LSP items → `MenuItem` (label/filterText/sortText/kind map directly; `textEdit` → `MenuInsert::Replace` through `lsp/position.rs`'s UTF-16 conversion, clamped on arrival), respect `isIncomplete` → menu's `is_incomplete` re-request path, and resolve lazily (`completionItem/resolve` for documentation on the selected item only — LSP's own lazy-fields protocol, mirroring Zed's visible-range resolution).
- **Shared guards**: `RequestSnapshot` is the same revision-guard shape the LSP doc already specifies for definition/hover; one convention everywhere.
- **Trigger characters**: the menu's auto-trigger gate gains "typed char ∈ server trigger characters" once a server is attached; until then it's word-char-only. `CompletionTriggerKind` (Invoked/TriggerCharacter/Incomplete) maps onto `TriggerMenu { explicit }` + the incomplete re-request.
- **Merging**: LSP items enter the same list as words/snippets with source-tier priority; words dedupe against LSP `insert` texts (Zed's exact rule); words become a fallback tier that can be configured out (`words: enabled | fallback | disabled`).
- **LLM completion *through* LSP**: shims like helix-gpt prove AI completions can be served into the LSP menu path — a valid low-effort integration for users who run such servers, and it costs us nothing: it's just another language server. Our native inline surface exists because ghost text UX (Tab-through, partial accept, prefix consumption) is strictly better for speculative text than a menu.
- **`additionalTextEdits` / auto-import**: the known gap in `MenuInsert` v1 (documented above). When LSP completion lands, add the second variant; until a server exists, there is no producer.
- **Snippet-format LSP items** (`insertTextFormat: Snippet`): flattened to plain text — same Non-Goal and convergence plan as the LSP doc.
- The LSP doc's data sketch (`CompletionResolved { items }`) is superseded by `MenuItemsReady { snapshot, items, is_incomplete }`.

---

## Configuration

`config.yaml` (note: the config system is YAML — `src/config.rs` / `serde_yaml` — not TOML as `lsp-integration.md` assumed; that doc needs the same correction):

```yaml
completion:
  menu:
    enabled: true            # auto-trigger on typing; Ctrl+Space always works
    min_word_length: 3
    words: fallback          # enabled | fallback (only when no LSP items) | disabled
  inline:
    enabled: false           # off until a backend is configured
    provider: local          # key into providers below
    debounce_ms: 300         # raise for slow backends; explicit trigger bypasses
    max_line_suffix: 8       # suppress auto-trigger with more chars right of cursor
  providers:
    local:
      transport: llama_cpp   # llama_cpp | ollama | openai_compat | mistral
      url: http://127.0.0.1:8012
      prompt_format: infer
      max_tokens: 128
      context: recency_ring
    mistral:
      transport: mistral
      model: codestral-latest
      api_key_env: MISTRAL_API_KEY   # env var name — never the key itself in config
```

Status bar: completion state joins the planned `SegmentId` set — a small spinner/glyph while an inline request is in flight, provider name on error (transient). No segment when disabled.

Automation/MCP: all commands (`TriggerMenu`, `AcceptInline`, …) are `is_simple()` `Command`s → invokable via `execute_action` for free; the automation snapshot gains `completion: { menu_visible, item_count, selected, inline_visible, inline_text }` so end-to-end tests can assert open → filter → accept without pixel scraping.

---

## Implementation Plan

### Phase 1: Menu completion — words + snippets (fully offline)

**Effort:** M — proves the entire UI with zero async complexity

- [x] `CompletionMenuState` on `UiState`; `Msg::Completion` + `update/completion.rs`. **Deviation:** `selected`/`viewport_offset` aren't duplicated on `CompletionMenuState` — they live on `ui.cursor_overlay` (`CursorOverlayState`, added by the overlay-p5 unit after this doc was written), the same shared home every other cursor-anchored popup uses. `CompletionMenuState` owns `document_id`/`revision`/`query_start`/`items`/`filtered` only.
- [x] WordsSource (rope scan, dedup, cap) + SnippetsSource (a handful of snippets for Rust/JavaScript+TypeScript/Python to prove the path). **Deviation:** snippets are a plain `match` in `completion/sources.rs`, not a new `&'static [(prefix, body)]` field on `LanguageDefinition` — the registry's `language!` macro has ~40 call sites, and threading a new field through all of them is a large mechanical diff for "a handful of snippets to prove the path." Add the `LanguageDefinition` field (following `selection`/`outline`'s pattern) if/when the per-language snippet count outgrows a match arm.
- [x] nucleo filtering + tiered sort; refilter-on-type; dismiss rules (non-word char, cursor line change, Escape). **Partial:** no "focus loss" dismiss hook (e.g. window losing OS focus) — not wired to anything in this unit; low-risk gap since the popup is also killed by the next keystroke/click almost always.
- [x] Popup rendering: build the `OverlaySpec` for the overlay-surface Completion context; `EditorArea` damage while visible (for free — `view::mod::compute_effective_damage` already forces `Damage::Full` whenever `ui.cursor_overlay.is_some()`, generically for every cursor-anchored popup kind since overlay-p5). Rows carry real `match_indices` from `Matcher::fuzzy_indices` (`filter_and_sort`'s `filtered` now stores `(score, index, indices)`), so the typed substring is bolded, matching this section's spec. (A verifier fix-up: the version that first shipped this checkbox passed `match_indices: &[]`, ticked here without recording the gap.)
- [x] Key routing: Ctrl+Space (`Command::TriggerCompletionMenu`, keymap-bindable) opens explicitly; arrows/Enter/Tab/Escape are claimed by the existing pre-keymap `handle_cursor_overlay_key` dispatch (overlay-p5's `overlay_routes_keys` mechanism) when `cursor_overlay.kind == Completion`, exactly as this doc's Key Handling section specified ("this document does not introduce a separate `completion_menu_visible` field ... `menu_visible` compiles to `overlay_routes_keys` + the active overlay context being Completion") — no new `Condition` variant needed. Tab falls through to `InsertTab` when the menu isn't open, resolving the standing keymap TODO for this one case.
- [x] Accept via `EditOperation::Batch` at all cursors; single undo step; multi-byte-safe (tested with an emoji elsewhere on the line and rope char-offsets throughout, never byte offsets).
- [~] Config block (menu), automation snapshot fields, palette entries. Automation (`EditorSnapshot.completion`) and a palette entry (`Trigger Completion`) are done. **Not done:** no `completion.menu.*` YAML config block — Phase 1 menu completion is always-on with the constants in `completion/sources.rs` (`MIN_WORD_LEN`, `WINDOW_LINES`, `MAX_WORDS`) hardcoded, not user-configurable. Add `src/config.rs` wiring when a real need for tuning surfaces.
- [x] **Gate:** covered by unit tests in `src/update/completion.rs` driving the exact same `update()` entry point automation uses (type → menu opens → filter → `MenuNext` wraps → `AcceptMenuItem` → `Undo`), including a multi-cursor case (one undo reverts both cursors) and a multi-byte case (emoji elsewhere on the line, char-offset correctness). Plus `runtime::app::tests::automation_flow_triggers_menu_and_reports_completion_snapshot` in `src/runtime/app.rs`, which pushes real `AutomationRequest`s (`SetCursor`, `ExecuteAction("TriggerCompletionMenu")`, `State`) through `automation_tx` → `process_automation_requests` — the same path the socket/MCP server feeds — and asserts on `EditorSnapshot.completion`.

### Phase 2: Inline suggestions — infrastructure + first FIM backend

**Effort:** L

- [ ] `InlineSuggestionState`; ghost-text paint stage (first line + `+N` badge, theme key, cursor-line damage); paint-only (no hit-test/layout impact).
- [ ] Completion worker thread (syntax-worker pattern) + inline deadline map in `about_to_wait`; supersede-and-cancel policy.
- [ ] `RequestSnapshot` guards on every arrival; trigger gates (EOL rule, menu-suppression, language check).
- [ ] Prefix-consumption on typing; backspace un-consume; clear rules.
- [ ] `InlineProvider` trait + FIM provider with **llama.cpp `/infill` transport only** (richest API, no prompt rendering needed); `ureq` or raw `std` HTTP — decide by dependency weight, no async runtime.
- [ ] Post-processing chain filters 1–4 with golden-file tests.
- [ ] `inline_suggestion_visible` KeyContext; Tab accept (Full), Escape dismiss; chained follow-up request on accept.
- [ ] Config (`inline`, `providers`), in-flight status glyph, error transients (never modal, capped retry/backoff like the LSP crash policy).
- [ ] **Gate:** against a local llama-server + Qwen2.5-Coder-1.5B: type in a Rust file, ghost text appears, type through it, Tab-accept, undo restores; kill the server mid-request → editor unaffected.

### Phase 3: Inline maturity

**Effort:** M

- [ ] Remaining transports: Ollama (+`keep_alive`, per-model suffix-capability error surfaced clearly), OpenAI-compatible, Mistral FIM. `PromptFormat` rendering for raw-prompt paths; `Infer` from model name.
- [ ] RecencyRing context strategy (+ inline-as-comments fallback for transports without `input_extra`); idle-only ring updates.
- [ ] Partial accept (Word/Line granularity — Zed's leading-run rule), alternative cycling when `n > 1` returned.
- [ ] LRU completion cache with pre/post-cache filter split; postprocess filters 5–6.
- [ ] Automation coverage: suggestion visible → partial accept → full accept assertions.

### Phase 4: LSP menu source *(sequenced with lsp-integration.md Phases 1–2)*

**Effort:** M — the menu machinery already exists; this is item conversion + async plumbing

- [ ] LSP source: request on trigger chars + debounce, `MenuItem` conversion (UTF-16 positions, kind map, sortText), `isIncomplete` re-request, lazy resolve for docs.
- [ ] Words demoted to `fallback` tier when LSP items present; dedup rule.
- [ ] `additionalTextEdits` → second `MenuInsert` variant (auto-import).
- [ ] Documentation side-card (shared with LSP hover overlay work).

### Phase 5+: Future

- [ ] Multi-row ghost text + mid-line suggestions — after soft-wrap's `logical_to_visual` mapping exists.
- [ ] Edit prediction: anchor-based edit-list suggestion variant, deletion highlighting, diff popover, jump targets (the Zed model); candidate providers: Zeta-style rewrite models, Copilot NES-compatible backends.
- [ ] Retrieval context strategy (BM25 over workspace, Tabby-style declaration extraction via tree-sitter or LSP).
- [ ] TabbyML transport; supervised local llama-server child process (spawn/own like PTY).
- [ ] Local acceptance stats (accept/dismiss counts per provider — Tabby's event shape, stored locally) to judge provider quality.
- [ ] Path completion source; menu documentation panel richness; commit characters.

---

## Testing Strategy

### Unit

- Fuzzy filter + tiered sort: ordering fixtures (exact > word-start > score; source tiers; stable across equal scores).
- Word extraction windows: boundaries, unicode identifiers, dedup, min length.
- Prefix-consumption state machine: type-through, backspace, divergent char, multi-byte chars, newline in suggestion.
- Prompt formats: byte-exact golden strings per `PromptFormat` (the silent-garbage failure mode makes these the highest-value tests in the module).
- Post-process chain: golden corpus per filter (Tabby's model) — leaked sentinels, over-long blocks, suffix duplication, unbalanced closers, repetition.
- Revision guards: stale `MenuItemsReady`/`InlineReady` dropped; superseded request ids dropped.

### Integration: fake backend

A stub HTTP server (few dozen lines, `std::net`) speaking canned `/infill` and `/v1/completions` responses from fixtures — deterministic, offline, CI-safe; mirrors the LSP plan's fake-server approach. Scenarios: happy path; slow response superseded by typing; error → transient → recovery; cancellation observed server-side; empty-suffix substitution.

### Manual checklist

- [ ] Menu: filters as typed, Escape/click-away dismisses, works in a 200k-line file without hitching.
- [ ] Ghost text invisible to selection/click hit-testing; correct at viewport edges and with horizontal scroll.
- [ ] Tab: accepts when suggestion visible, indents otherwise, unindents with selection — no dead keys.
- [ ] Multi-cursor: menu accept applies at all cursors, one undo; inline suggestions render for the active cursor only.
- [ ] Kill/absent backend: no error spam, editing unaffected, transient shown once.
- [ ] CJK/emoji around the cursor: popup anchor and ghost x-position correct.
- [ ] Blink fast path: cursor blink over a visible ghost suggestion doesn't ghost-duplicate or erase it.

### Performance

- Menu collect+filter budget: < 2 ms at 10k unique words (it's a rope scan + nucleo — verify, don't assume).
- Ghost-text paint rides `CursorLines` damage; a suggestion arriving for an unfocused/off-screen document must not redraw the focused editor.
- Worker request build (rope→string windows) is bounded by the prefix/suffix budgets, not document size.

---

## Acceptance Criteria

- With no configuration: typing in any file offers word/snippet completion; Ctrl+Space always answers; zero network activity.
- With a local llama.cpp server configured: ghost text appears within debounce+model latency, survives typing-through, Tab-accepts as one undoable edit, and never blocks or corrupts editing under server kill/restart/timeout.
- Menu and ghost text are never visible simultaneously; every completion key conflict is resolvable in `keymap.yaml`.
- No stale response is ever applied (revision + cursor + request-id guarded).
- All completion commands are palette-visible and automation-invokable; the snapshot exposes enough state for end-to-end tests.
- With `completion.menu.enabled: false` and `inline.enabled: false`, behavior is byte-for-byte today's editor.

---

## Design Decisions

| Decision | Options | Chosen | Rationale |
| --- | --- | --- | --- |
| Surface split | one unified completion system / menu + inline as separate subsystems | separate, touching only at key-precedence + suppression | Every surveyed editor converged here; different data shapes, latencies, UX |
| Suggestion data model | string @ cursor / anchor-based edit lists (Zed) | string + `consumed`, enum-extensible | Insert-only covers all v1 providers; edit lists arrive with edit prediction; anchors don't exist in our model yet |
| Invalidation | marker/anchor tracking / snapshot + guard | `RequestSnapshot` guard, clear-on-edit + prefix-consume | Universal ecosystem practice; identical to our syntax-worker guards |
| Menu source abstraction | trait registry now / two functions + shared item type | functions; trait deferred to third async source | One implementor per shape today; `MenuItem`/`MenuItemsReady` is the real boundary |
| Inline provider abstraction | FIM-specific / provider trait over snapshots | `InlineProvider` trait, FIM as first impl | Explicit requirement: non-LLM providers use the same mechanism |
| FIM factoring | monolithic per-backend / transport × prompt format × context strategy | three independent axes | Zed's proven factoring; only one that survives new backends |
| First transport | Ollama / OpenAI-compat / llama.cpp `/infill` | llama.cpp | Server-side prompt building (no sentinel risk), time budgets, `input_extra`, cache reuse — most capability for least client code |
| Default model guidance | Mellum / Codestral / Qwen2.5-Coder base | Qwen2.5-Coder 0.5B/1.5B base | Apache 2.0, smallest viable, llama.cpp presets exist; Mellum documented as GPU tier; Codestral open weights are non-production licensed |
| Context strategy | retrieval (BM25) / recency ring | recency ring first | Stable prompt prefix preserves server KV-cache — the dominant local-latency lever; retrieval is an enum variant later |
| Fuzzy matcher | new SIMD matcher (frizbee-like) / nucleo | nucleo | Already a dependency; Lapce ships it for exactly this; revisit only on measured lag |
| Key conflicts | input.rs branching / KeyContext conditions | conditions | Existing mechanism, user-rebindable, matches keymap TODO |
| Menu debounce | timer / none for sync sources | none (sync); request-coalescing for async | Zed ships menu completion with no timer debounce; our sync sources are sub-ms |
| Async runtime | tokio / std thread + mpsc worker | std thread | House pattern (syntax worker, PTY, planned LSP) |
| Multi-line ghost text | build virtual rows now / first-line + `+N` badge | badge until soft-wrap | Virtual rows without `logical_to_visual` would fork the layout model; soft-wrap doc owns that seam |

## Open Questions

1. **HTTP client**: smallest viable — `ureq` (blocking, tiny) vs hand-rolled over `std::net` (llama.cpp/Ollama are localhost HTTP/1.1; TLS only needed for hosted APIs). Decide in Phase 2 by whether hosted-API support ships before Phase 3.
2. Should explicit `TriggerInline` with the menu open dismiss the menu (Zed: menu has precedence; VS Code: inline can render inside the suggest widget)? Start with dismiss-menu; revisit with usage.
3. Word-source scope: active document only vs all open documents (Copilot's neighboring-tabs evidence says same-language open tabs help). Start single-document; the source signature doesn't change.
4. Does `SnippetsSource` ship user-defined snippets from config in Phase 1, or static tables only until `snippets.md`? Leaning static-only to avoid designing snippet config twice.
5. Ghost-text color: derived from theme (blend fg/bg) vs explicit theme key per theme file. Derived-with-override is likely right; needs a pass over bundled themes.

## References

### Papers
- [InCoder: A Generative Model for Code Infilling and Synthesis](https://arxiv.org/abs/2204.05999) — causal-masking infilling, EOM sentinel
- [Efficient Training of Language Models to Fill in the Middle](https://arxiv.org/abs/2207.14255) — PSM/SPM, FIM-for-free
- [Productivity Assessment of Neural Code Completion (Copilot acceptance study)](https://arxiv.org/abs/2205.06537)
- [Mellum: production-scale FIM at JetBrains](https://arxiv.org/abs/2510.05788) · [JetBrains Full Line Code Completion](https://arxiv.org/html/2405.08704v1)

### Editors & plugins
- Zed: [Edit Prediction docs](https://zed.dev/docs/ai/edit-prediction) · [Zeta blog](https://zed.dev/blog/edit-prediction) · [pluggable providers](https://zed.dev/blog/edit-prediction-providers) · `crates/editor/src/completions.rs`, `crates/edit_prediction_types/`
- Neovim: [insert.txt (ins-completion)](https://github.com/neovim/neovim/blob/master/runtime/doc/insert.txt) · [nvim-cmp](https://github.com/hrsh7th/nvim-cmp) · [blink.cmp](https://cmp.saghen.dev) · [frizbee](https://github.com/Saghen/frizbee) · [copilot.lua suggestion module](https://github.com/zbirenbaum/copilot.lua/blob/master/lua/copilot/suggestion/init.lua)
- Emacs: [Completion in Buffers (capf)](https://www.gnu.org/software/emacs/manual/html_node/elisp/Completion-in-Buffers.html) · [company backends](https://company-mode.github.io/manual/Backends.html) · [corfu](https://github.com/minad/corfu) · [copilot.el](https://github.com/copilot-emacs/copilot.el)
- Helix: [handlers/completion.rs](https://github.com/helix-editor/helix/blob/master/helix-term/src/handlers/completion.rs) · [inline completion tracking #13039](https://github.com/helix-editor/helix/issues/13039)
- Lapce: [completion.rs](https://github.com/lapce/lapce/blob/master/lapce-app/src/completion.rs)

### Backends & clients
- [llama.cpp server `/infill`](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md) · [llama.vim](https://github.com/ggml-org/llama.vim) · [cache-reuse design PR #9787](https://github.com/ggml-org/llama.cpp/pull/9787)
- [TabbyML](https://github.com/TabbyML/tabby) (post-process filter chain, adaptive debounce, event schema)
- [Continue.dev autocomplete internals](https://github.com/continuedev/continue) (streaming filter chain, generator reuse)
- [Copilot internals (deobfuscated)](https://thakkarparth007.github.io/copilot-explorer/posts/copilot-internals.html) · [VS Code Next Edit Suggestions](https://code.visualstudio.com/blogs/2025/02/12/next-edit-suggestions)
- [Mistral FIM API](https://docs.mistral.ai/api/endpoint/fim) · [JetBrains Mellum-4b-base](https://huggingface.co/JetBrains/Mellum-4b-base) · [Qwen2.5-Coder](https://huggingface.co/Qwen/Qwen2.5-Coder-1.5B)
- [VS Code IntelliSense](https://code.visualstudio.com/docs/editor/intellisense) · [LSP 3.18 `textDocument/completion`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.18/specification/#textDocument_completion)

### Internal
- [overlay-surface.md](overlay-surface.md) — owns the completion popup surface (`Anchor::Cursor`, Completion context, kind badges); prerequisite for the Phase 1 popup. Irrelevant to ghost text, which is in-text-flow paint, not an overlay
- [lsp-integration.md](lsp-integration.md) — Phase 5 superseded by this document's Phase 4; also note config is YAML, not TOML
- [soft-wrap.md](soft-wrap.md) — prerequisite for multi-row ghost text
- [snippets.md](snippets.md) — convergence point for snippet bodies and placeholder navigation
- `docs/EDITOR_UI_REFERENCE.md` ch. 7 — earlier positioning prose, superseded here
