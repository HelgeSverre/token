# Autocomplete & Inline Suggestions

A pluggable completion system with two rendering surfaces — a popup menu at the cursor and ghost-text inline suggestions — fed by swappable providers: buffer words and snippets first, LSP when it lands, and LLM fill-in-the-middle backends (local or remote) behind one backend abstraction.

> **Status:** 🚧 In Progress — Phase 1 (menu: words + snippets), Phase 4 (LSP source), and Phase 2 (inline ghost text + llama.cpp `/infill`) shipped. Phase 3 now includes partial acceptance, cancellation, the provider trait, Ollama, OpenAI-compatible native-suffix completions, Mistral FIM, alternative cycling, bounded LRU reuse, conservative syntax/indentation filters, explicit raw FIM formats/inference and opt-in idle recency context (2026-09-07). Phase 5 multi-row/mid-line projection is implemented, with isolated macOS keyboard/pointer/resize checks and release profiling recorded; IME/platform verification and the rest of Phase 5+ remain open. Fixture tests do not prove live model/server compatibility.
> **Priority:** P2 (Important)
> **Effort:** XL (phased — each phase ships independently)
> **Created:** 2026-08-11
> **Updated:** 2026-09-07
> **Milestone:** 4 - Hard Problems

> **IME deferral (2026-09-08):** The user explicitly assigned native IME
> composition/candidate-window work near-zero priority. References to that
> verification below are deferred follow-up, not current acceptance or handoff
> closeout gates. Do not resume without a user request. The isolated Linux
> IBus/Anthy attempt did not establish a working control: both Token and a GTK
> entry received raw Roman input. No IME fix or native certification is claimed.

---

## Dropdown context policy — implemented 2026-09-06

The `build.rs` screenshot's `ar_flag`, `archiver`, `asm_flag`, `cargo_*` and
`ccbin` entries are actual `cc::Build` methods (verified against locked cc 1.2.67).
Their bare names and alphabetical ordering looked arbitrary. Separately, local
buffer-word/snippet fallback really could pollute member completion, and an
empty local match set prevented the LSP request altogether.

The current implementation supersedes the original Phase 1 ranking sketch below:

- Code member access is LSP-only, including multiline chains. An empty query does not
  dump every buffer word or generic snippet into the popup.
- Local candidates require a case-insensitive ASCII prefix. Plain text/Markdown
  retain word suggestions; code waits for fresh syntax highlights, excludes
  comments, strings and non-identifier captures, and accepts uncolored Unicode
  identifier-shaped words (Rust does not color ordinary local variables).
  This is conservative fallback, **not** lexical-scope or type inference.
- Nearby lines win local word ties. Filtering precedes the 500-word cap.
- LSP `sortText` leads server ordering; `preselect` chooses the initial row,
  without overriding deliberate navigation. Structured `labelDetails` displays
  parameters and return/owner metadata without changing insertion or matching.
  A full legacy signature is retained when structured data contains only an owner.
  See the [LSP completion contract](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_completion).
- A completion session can wait invisibly for LSP or syntax. Only visible rows
  claim navigation/accept keys, appear in automation snapshots, or suppress inline
  suggestions. Escape cancels the pending session. Parse completion refreshes local
  candidates without postponing the LSP debounce or reviving dismissed sessions.
- Syntax remains the shared code/literal classifier; the language server remains
  responsible for receiver types and scope. No language-specific method whitelist
  or second lexer was introduced. Word shape uses Unicode XID properties through
  [unicode-ident](https://docs.rs/unicode-ident/1.0.22/unicode_ident/).

Verification covers chained member access, prefix noise, comment/string exclusion,
stale syntax/caret replies, deferred local results, server preference vs navigation,
metadata conversion/render rows, and hidden-session input/inline behavior. Runtime
tests use the existing fake server; the live rust-analyzer GUI check remains manual.
`just bench-completion` measures response conversion, LSP filtering, typing with
carried items, and fresh syntax-filtered fallback independently of parser setup.

Optimized local CPU measurements (2026-09-06, 100 samples, allocation profiler
enabled; medians, not application frame times):

| Workload | Median |
| --- | ---: |
| Convert 1,000 server items | 971.8 µs |
| Filter/sort 1,000 LSP items | 79.62 µs |
| Type with 1,000 carried LSP items | 103.7 µs |
| Collect/filter 500 code words from fresh syntax | 310.7 µs |

The word-fallback fixture has 2,000 declaration/comment lines and asserts that
comment words are excluded. The typing fixture uses a 5,000-line document with
syntax pending. Debouncing, language-server round trips, parsing and rendering
are outside these measured operations. No before/after speedup is claimed.
Final suite: 2,153 passed, 7 skipped; 6 ignored doctests; strict all-target,
all-feature Clippy and formatting checks passed.

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
- **Popup surface**: [`overlay-surface.md`](../archived/overlay-surface.md) (Milestone 1) plans the unified `OverlaySurface` component with an `Anchor::Cursor` mode and a **Completion context** (kind badge + label + dim signature rows, flip/clamp, dismiss rules) built exactly for this feature and LSP's popups. This document consumes that shell and owns none of the popup painting. If autocomplete Phase 1 somehow lands first, the interim fallback is today's `selectable_list.rs` + a cursor-anchored overlay bound — but overlay-surface deletes `selectable_list.rs`, so don't build on it deliberately. `src/view/caret.rs::active_text_input_rect` computes the caret pixel rect either way.
- **Prefix extraction**: `src/update/document.rs::word_start_before` / `word_end_after` (char-class based) — the completion-query extractor, already written and tested.
- **Fuzzy matching**: `nucleo-matcher` is already a dependency (file finder). Lapce uses the same crate for completion filtering.
- **Multi-cursor atomic edits**: `EditOperation::Batch` in `src/model/document.rs` — accepting a completion at N cursors is one undo step, no new machinery.
- **Key routing**: `KeyContext` / `Condition` in `src/keymap/context.rs`; Tab is currently bound unconditionally to `InsertTab` with a standing TODO to make it context-conditional — completion lands exactly in that mechanism.
- **Rendering foundation (updated 2026-09-05):** soft wrap now supplies the shared logical→visual mapping in `TextViewportMap`. It does not yet insert virtual suggestion rows or shift suffix text. Ghost text still uses first-line paint plus the multi-line badge; Phase 5+ must extend this shared mapping rather than add a separate render-only row loop.
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
| **TabbyML** | `POST /v1/completions` (segments) | Server supports declarations + snippet lists + search; current adapter uses recency comment fallback | Native segments adapter implemented. Prompt building and generation limits stay server-side. Declaration/search attachment and event transmission are not enabled. |

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

**Menu (synchronous sources — words, snippets; LSP items arrive async and merge into the same state):**

```text
typed char / Ctrl+Space
  → update/editor.rs inserts char (normal path, unchanged)
  → update/completion.rs: should_trigger? (word char, or explicit)
      auto-open additionally requires a two-character query prefix
      (MIN_AUTO_TRIGGER_PREFIX — one char flashing the popup read as noise;
      Ctrl+Space is unaffected and still works on an empty query)
      collect: word_start_before(cursor) → query
      sources run inline in update (they're rope scans + static tables, <1 ms):
        WordsSource: words_in_range around cursor (±N lines), dedup vs query
          (case-insensitive: typing `Value` must not suggest `value`) + other items
        SnippetsSource: static per-language table prefix match
      fuzzy-filter with nucleo-matcher, sort (tier: exact > prefix > score; then source; then label)
  → CompletionMenuState set on UiState, Cmd::redraw_editor()
subsequent typing
  → query grows → local refilter only (no re-collect unless word boundary crossed)
  → query empty or non-word char → dismiss
    (exception: a server trigger character keeps the menu open with an empty
     query and a tagged re-request — lsp-integration.md Phase 5)
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

Damage note: changing a ghost projection redraws the editor area because source
rows can move. An unchanged projection participates in the existing cursor-line
blink fast path, which repaints all visible rows belonging to that source line.
The menu popup overlaps arbitrary rows and also requires editor-area damage.

### Rendering

**Menu popup** — rendered through [`overlay-surface.md`](../archived/overlay-surface.md)'s **Completion context**, not bespoke drawing:

- The menu builds an `OverlaySpec` per frame: `Anchor::Cursor` at `query_start` (so the list aligns with what's being completed, from `active_text_input_rect` + `column_to_pixel_x`), `Body::List` with rows = `RowIcon::KindBadge(kind)` + label with `match_indices` (nucleo indices) + `Accessory::DimText(detail)`. Flip-above, edge clamping, no backdrop dim, and dismiss-on-edit rules are the surface's cursor-anchored behavior — defined there, consumed here.
- `MenuItemKind` maps onto the surface's kind-badge palette (`overlay.kind_*` theme keys, derived from syntax colors).
- Docs side panel (LSP `documentation`) is the surface's `ListWithPanel` v2 — deferred with it, not built twice.
- Sequencing: overlay-surface Phases 1 + 5 (painter primitives, cursor anchor) are a **prerequisite for this document's Phase 1 popup**. Its milestone (1) precedes ours (4); if that ordering ever inverts, ship the popup on the old `selectable_list.rs` shell and migrate with the other contexts.

**Ghost text** (inside `editor_text.rs`'s per-line stage pipeline — the `VisibleTextLine` doc comment explicitly invites decorations to plug into these stages):

- The focused pane's derived `GhostText` reflows only the anchor logical line as
  prefix + remaining suggestion + suffix, using the same segmentation as soft
  wrap. `TextViewportMap` splices these rows into the existing document map.
- `render_line_text_stage` draws source fragments and `render_ghost_text_stage`
  draws inserted glyphs once in `editor.ghost_text`, with shared tab expansion.
  Source syntax, selections, brackets and diagnostics exclude ghost fragments.
- The cursor stays before the insertion; suffix glyphs at that same source
  column appear after it. Hit testing, IME, scrollbars and reveal use the shared
  projection. Ghost hits map to the source anchor. Navigation dismisses before
  computing source movement; rectangle anchors are rebased on dismissal.
- Explicit requests may insert mid-line. The automatic suffix gate is unchanged.
  Compatible typing consumes the preview before automatic dropdown collection.

### Key handling

Key routing uses the `KeyContext`/`Condition` mechanism that exists today, with the flag ownership split along the surface boundary:

- **Menu**: the menu is a cursor-anchored overlay context, so it is covered by [`overlay-surface.md`](../archived/overlay-surface.md)'s generalized `overlay_routes_keys` flag (its evolution of the LSP plan's `completion_visible`) — one flag for all cursor-anchored popups routing Up/Down/Enter/Escape while visible. This document does **not** introduce a separate `completion_menu_visible` field; bindings below that say `menu_visible` compile to `overlay_routes_keys` + the active overlay context being Completion.
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

**Implementation checkpoint (2026-09-06):** the sketch below describes the
intended capability, not the current Rust signature. The real `InlineProvider`
in `src/completion/provider.rs` returns a cancelable boxed future. Dropping it
cancels local work; there is no separate cancel-token API. `InlineRequest` is
provider-neutral, and `InlineJob` carries configuration separately. A non-HTTP
heuristic test uses the same response/postprocess path. Feedback hooks await
the cache/statistics work, where they gain a consumer.

The runtime has one latest-value request slot and one focused-pane debounce.
New requests, dismissal, edits, selection/pane/focus changes, provider changes
and shutdown invalidate the session and cancel outstanding waits. Tests verify
TCP disconnection, not just stale-response filtering. Whether a remote server
also stops GPU generation on disconnect is server-dependent.

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
    WorkspaceRetrieval { // bounded, ignore-aware BM25 declaration retrieval
        max_chunks: usize,       // default 8
        chunk_lines: usize,      // default 64
    },
}
```

Capability flags per transport (`builds_fim_prompt`, `supports_extra_context`, `supports_time_budget`, `supports_slot_affinity`) steer the request builder — e.g. when `supports_extra_context` is false, ring chunks are inlined into the prefix as commented snippets (`// Path: …` headers, the Copilot/DeepSeek convention).

Recency remains useful when prompt-prefix stability matters: retrieval can change
the selected chunks while typing, reducing the server's KV-cache reuse.
`WorkspaceRetrieval` is now an opt-in alternative on the same strategy boundary.
It uses the existing syntax/outline registry for declarations, with bounded line
windows where no outline exists. An ignore-aware, cancelable runtime worker
collects source; the update layer revalidates the request before provider
submission. See [configuration, limits and transmission scope](../user/config-editor.md#workspace-retrieval-context).

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
  enabled: true              # master switch, including explicit and inline requests
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
      transport: llama_cpp   # llama_cpp | ollama | open_ai_compat | mistral_fim
      url: http://127.0.0.1:8012
      prompt_format: native  # raw formats are opt-in on Ollama/OpenAI-compatible
      max_tokens: 128
      context:
        strategy: recency_ring  # opt-in; omitted defaults to none
        max_chunks: 8
        chunk_lines: 64
    mistral:
      transport: mistral_fim
      url: https://api.mistral.ai
      model: codestral-latest
      api_key_env: MISTRAL_API_KEY   # env var name — never the key itself in config
```

Status bar: completion state joins the planned `SegmentId` set — a small spinner/glyph while an inline request is in flight, provider name on error (transient). No segment when disabled.

The menu block above is implemented. `min_word_length` controls local candidate
identifier length (default three, effective floor one), not the two-character
auto-trigger prefix. Disabling automatic menus still allows Ctrl+Space and
refinement of an open session; signature help and inline suggestions remain
independent. Legacy `completion.words` migrates into `menu.words` on save, with
explicit nested values taking precedence and unknown keys preserved.

Currently supported Ollama configuration:

```yaml
completion:
  enabled: true
  inline:
    enabled: true
    provider: ollama
  providers:
    ollama:
      transport: ollama
      url: http://127.0.0.1:11434
      model: your-fim-model
      keep_alive: -1
      max_tokens: 128
      timeout_ms: 5000
```

In native mode, the model must already be installed and its template must support
suffix insertion. Explicit raw formats instead bypass that template and serialize
both prefix and suffix; see the [user guide](../user/config-editor.md#raw-fim-prompts).
Token does not pull models or silently retry with prefix-only generation.
`keep_alive` is seconds (`-1` keeps the model resident; `0` allows unloading).
See Ollama's [generate API](https://docs.ollama.com/api/generate) and
[residency documentation](https://docs.ollama.com/faq#how-do-i-keep-a-model-loaded-in-memory-or-make-it-unload-immediately).

The implemented transports accept HTTP or HTTPS base URLs, including a reverse
proxy base path. The shared reqwest/rustls client validates certificates, uses
one total request deadline and limits response bodies to 1 MiB (also for
chunked replies). URL credentials/query/fragment are rejected; redirects and
automatic system/environment proxies are disabled to keep source context on the
configured direct connection. Named credential references are implemented;
raw-format implementation and verification status are tracked in Phase 3 below.
TLS behavior follows the
[reqwest client contract](https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html);
no hosted endpoint or native HTTPS UI flow has been exercised in this checkpoint.

Automation/MCP: all commands (`TriggerMenu`, `AcceptInline`, …) are `is_simple()` `Command`s → invokable via `execute_action` for free; the automation snapshot gains `completion: { menu_visible, item_count, selected, inline_visible, inline_text }` so end-to-end tests can assert open → filter → accept without pixel scraping.

---

## Implementation Plan

### Phase 1: Menu completion — words + snippets (fully offline)

**Effort:** M — proves the entire UI with zero async complexity

- [x] `CompletionMenuState` on `UiState`; `Msg::Completion` + `update/completion.rs`. **Deviation:** `selected`/`viewport_offset` aren't duplicated on `CompletionMenuState` — they live on `ui.cursor_overlay` (`CursorOverlayState`, added by the overlay-p5 unit after this doc was written), the same shared home every other cursor-anchored popup uses. `CompletionMenuState` owns `document_id`/`revision`/`query_start`/`items`/`filtered` only.
- [x] WordsSource (rope scan, dedup, cap) + SnippetsSource (a handful of snippets for Rust/JavaScript+TypeScript/Python to prove the path). **Deviation:** snippets are a plain `match` in `completion/sources.rs`, not a new `&'static [(prefix, body)]` field on `LanguageDefinition` — the registry's `language!` macro has ~40 call sites, and threading a new field through all of them is a large mechanical diff for "a handful of snippets to prove the path." Add the `LanguageDefinition` field (following `selection`/`outline`'s pattern) if/when the per-language snippet count outgrows a match arm.
- [x] nucleo filtering + tiered sort; refilter-on-type; dismiss rules (non-word char, cursor line change, Escape). The focus-loss dismiss gap is closed (runtime `WindowEvent::Focused(false)` → `CompletionMsg::Dismiss`), and editor scroll now dismisses too — the cursor-anchored popup would otherwise visually detach from its word. lsp-integration.md Phase 5 adds the LSP source tier and server trigger characters as a keep-open exception to the non-word-char rule.
- [x] Popup rendering: build the `OverlaySpec` for the overlay-surface Completion context; `EditorArea` damage while visible (for free — `view::mod::compute_effective_damage` already forces `Damage::Full` whenever `ui.cursor_overlay.is_some()`, generically for every cursor-anchored popup kind since overlay-p5). Rows carry real `match_indices` from `Matcher::fuzzy_indices` (`filter_and_sort`'s `filtered` now stores `(score, index, indices)`), so the typed substring is bolded, matching this section's spec. (A verifier fix-up: the version that first shipped this checkbox passed `match_indices: &[]`, ticked here without recording the gap.)
- [x] Key routing: Ctrl+Space (`Command::TriggerCompletionMenu`, keymap-bindable) opens explicitly; arrows/Enter/Tab/Escape are claimed by the existing pre-keymap `handle_cursor_overlay_key` dispatch (overlay-p5's `overlay_routes_keys` mechanism) when `cursor_overlay.kind == Completion`, exactly as this doc's Key Handling section specified ("this document does not introduce a separate `completion_menu_visible` field ... `menu_visible` compiles to `overlay_routes_keys` + the active overlay context being Completion") — no new `Condition` variant needed. Tab falls through to `InsertTab` when the menu isn't open, resolving the standing keymap TODO for this one case.
- [x] Accept via `EditOperation::Batch` at all cursors; single undo step; multi-byte-safe (tested with an emoji elsewhere on the line and rope char-offsets throughout, never byte offsets).
- [x] Config block (menu), automation snapshot fields, palette entries. `completion.menu` controls automatic opening, minimum candidate word length and local-word policy; legacy word settings migrate on save. Automation (`EditorSnapshot.completion`) and the palette's `Trigger Completion` entry are implemented. Scan-window and count caps stay internal bounds rather than additional configuration surface.
- [x] **Gate:** covered by unit tests in `src/update/completion.rs` driving the exact same `update()` entry point automation uses (type → menu opens → filter → `MenuNext` wraps → `AcceptMenuItem` → `Undo`), including a multi-cursor case (one undo reverts both cursors) and a multi-byte case (emoji elsewhere on the line, char-offset correctness). Plus `runtime::app::tests::automation_flow_triggers_menu_and_reports_completion_snapshot` in `src/runtime/app.rs`, which pushes real `AutomationRequest`s (`SetCursor`, `ExecuteAction("TriggerCompletionMenu")`, `State`) through `automation_tx` → `process_automation_requests` — the same path the socket/MCP server feeds — and asserts on `EditorSnapshot.completion`.

### Phase 2: Inline suggestions — infrastructure + first FIM backend

**Effort:** L — **shipped 2026-09-03.**

- [x] `InlineSuggestionState` on `UiState` (`src/completion/inline.rs`); ghost-text paint stage `render_ghost_text_stage` inside `render_line_content_stages` so both full and cursor-lines-only paths draw it; theme key `editor.ghost_text` (derived when absent). The initial paint-only first-line/`+N` badge implementation was superseded by Phase 5 projected rows on 2026-09-07.
- [x] Completion worker thread (`src/runtime/inline_worker.rs`, syntax-worker pattern) + `inline_deadlines` map replayed from `about_to_wait` and folded into `next_wake`; newest request per document wins. **Deviation:** an in-flight std socket cannot be aborted, so supersession drops queued requests and the revision guard discards late replies — no cancel token until a slow remote transport needs one.
- [x] `RequestSnapshot` guards (document, revision, cursor) on every arrival; trigger gates: end-of-line rule (`max_line_suffix`, closers ignored), menu suppression, plain-text mode, backend configured.
- [x] Prefix consumption on typing; Backspace un-consumes; any other edit clears; cursor moves hide it (the state lingers until the next edit or Escape — paint and accept check `applies_to`).
- [x] `InlineProvider` boundary is the `InlineRequest` value + `fim::infill` (llama.cpp `/infill` only) over a ~100-line `std::net` HTTP/1.1 client — no dependency added. **Deviation:** no trait yet; one implementation does not earn one (add it with the second transport).
- [x] Post-processing filters 1–4 (`postprocess`) with unit tests; filters 5–6 and the LRU cache are Phase 3.
- [x] `inline_suggestion_visible` `KeyContext` condition; Tab → `AcceptInlineSuggestion`, Escape → `DismissInlineSuggestion`, ⌥\\ → `TriggerInlineSuggestion` in `keymap.yaml`; accept chains a follow-up request.
- [x] Config (`completion.inline`, `completion.providers`), error transients, pause after `MAX_CONSECUTIVE_FAILURES` until an explicit trigger. **Not done:** the in-flight status-bar glyph (`ui.inline_in_flight` exists; no segment draws it yet).
- [x] **Gate (fake backend):** `runtime::app::tests::inline_suggestion_round_trips_through_the_worker_and_accepts` runs the real worker thread against an in-process `/infill` server: type → debounce → request → ghost text → type-through → Tab accept → one undo step; `..._backend_failure_is_a_transient` covers a dead server. **Gate (live llama-server):** run 2026-09-03 against `llama-server -hf Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF:Q8_0` through the automation socket: ghost text ` + b` 500 ms after typing, type-through consumed it, Tab produced `a + b`, Undo restored the line in one step, and killing the server mid-request left the editor responsive with no modal. Not in CI (needs the model).

**Phase 2 follow-up:** shared visibility guards cover explicit requests and Tab acceptance, the status bar draws request progress, superseded request IDs cannot overwrite current state, and debug full redraws paint ghost text. Automation reports `inline_in_flight`. Cancellation and the provider trait were added on 2026-09-06 with Ollama; the original std-only transport notes above are historical.

### Phase 3: Inline maturity

**Effort:** M

- [x] Cancelable provider boundary and Ollama (`keep_alive`, clear suffix-capability errors), shared with llama.cpp over HTTP/TLS. One bounded latest-request slot; generation-tagged debounces; cancellation on supersession/dismissal/edit/pane/focus/config changes and shutdown.
- [x] OpenAI-compatible native-suffix completions and Mistral FIM share the existing HTTP/TLS worker, cancellation, deadlines and response bounds. Credentials use an explicit `api_key_env` reference resolved in the worker, with no credential values in model/config serialization. Authenticated remote endpoints require HTTPS; redirects stay disabled. Local fixtures cover wire shapes, auth validation and worker acceptance. Hosted service/model compatibility has not been exercised with live credentials.
- [x] Raw `PromptFormat` rendering and conservative `infer`: native remains the backward-compatible default; Qwen, StarCoder, CodeLlama, DeepSeek, Codestral and Mellum formats opt into raw Ollama/OpenAI-compatible requests. Canonical PSM/SPM layouts, tokenizer marker spaces, stop strings and leaked-token cleanup share one definition table. Raw mode is rejected for server-formatted llama.cpp/Mistral paths. Golden, HTTP, config, cache and worker acceptance/Undo tests pass, with the full suite and strict lint. This is not a silent fallback, arbitrary chat-model support or proof of live model compatibility.
- [x] Opt-in `RecencyRing`: runtime queues open-buffer positions on activation/file switch, save and large cursor jumps, committing bounded snippets after 750 ms without cursor/document changes. Strict >0.9 token-set similarity evicts older duplicates; order is stable between idle updates, without retrieval/ranking. Provider/workspace changes clear the ring; close/path changes evict old sources. Native llama.cpp receives `input_extra`; other transports use commented prefixes, with explicit errors for unsupported comment languages. Raw FIM checks include extra context; active prefix/suffix remain separate. Context order/text participates in cache equality and payload accounting. Config, idle/lifecycle, bounds, wire, cache and worker acceptance/Undo tests cover the implementation. See the [user guide](../user/config-editor.md#extra-context-from-recently-visited-buffers) for limits and transmission scope; native GUI and live model quality remain unverified. [Release-stage profiling](../dev/refactoring-audit-2026-09-06.md#recency-release-profiling-and-token-index-consolidation--2026-09-07) measured and removed repeated token-set construction; retained indexes preserve exact deduplication decisions.
- [x] Partial accept (Word/Line granularity — leading alphabetic/non-alphabetic run; line includes newline). Cmd+Right (Ctrl+Right off macOS) accepts a word run while ghost text is visible; `AcceptInlineLine` is palette/automation-invokable and user-bindable without a default shortcut. Every accepted portion is one undo batch; the remainder stays visible without another backend request. Acceptance now uses the active cursor and preserves other cursors and split-pane selection ranges.
- [x] Alternative cycling when multiple results are returned. `open_ai_compat` accepts `n: 1..8` (default 1); unsupported transports reject larger counts. Worker filtering and bounded, deduplicated state preserve provider order. Alt+]/Alt+[ cycle only choices matching the already typed/accepted prefix, without editing or requesting; backspace can restore eligibility. Painting and automation share the compatible position/count. Named actions and tests cover cycling, partial/full acceptance, Unicode/CRLF, no-op/invisible guards and undo. An isolated native Norwegian-layout macOS check verified both shortcuts, selected-result Tab acceptance, undo and unbound composed text. Option lookup is logical-first with an unmodified-layout fallback and one chord-state transition. US-layout fallback is unit-tested; broader IME and other native platforms remain unverified. See the [verification audit](../dev/refactoring-audit-2026-09-06.md#inline-alternatives-and-option-key-dispatch--2026-09-06).
- [x] Worker-local LRU completion cache: at most 256 entries and 8 MiB retained source/result payload, plus bounded metadata. Filters 1–4 run before storage. Exact-context hits and typed/accepted-prefix replay retain provider order, with full bounded prefix/suffix, document/file, language and provider configuration checks. Hashes are never sufficient proof of equality. Hits carry the current snapshot; errors/empty results are not cached. Explicit requests bypass reuse. Unit/worker tests cover Unicode, CRLF, sliding prefix windows, eviction, limits and invalidation; an isolated native backspace/retype check replayed after the backend closed and accepted/undid the result. This adds no cache setting or persistent data.
- [x] Post-cache filters 5–6: registry-parser bracket sanity and conservative leading-whitespace normalization run on every serve. A bounded local-only document snapshot supplies context beyond the provider window, without entering HTTP requests or cache entries. Recognized literals/comments are opaque; quote/comment-bearing parser recovery, unsupported or oversized input and exhausted cooperative budgets preserve the original result. Tab/space majority inference preserves visual columns in Rust, Go, JavaScript, C and C++, skips tied styles, and leaves other languages' indentation untouched. Exact cache hits use fresh context; prefix replay matches the served, normalized text. This is not semantic validation or general formatting; native language/platform coverage remains incomplete.
- [x] Automation coverage: fake-backend suggestion visible → named `AcceptInlineWord` → `AcceptInlineLine` → `AcceptInlineSuggestion`, asserting document text and remaining ghost text at each step. Additional tests cover UTF-8 runs, CRLF/blank lines, undo, stale replies, active multi-cursor acceptance, peer selections and conditional keybinding fallbacks.

### Phase 4: LSP menu source *(sequenced with lsp-integration.md Phases 1–2)*

**Effort:** M — the menu machinery already exists; this is item conversion + async plumbing

- [x] LSP source: request on trigger chars + debounce, `MenuItem` conversion (UTF-16 positions, kind map, sortText), `isIncomplete` re-request. Built in lsp-integration.md Phase 5 (`completion/lsp.rs`, `update/completion.rs`).
- [x] `additionalTextEdits` → `MenuInsert::Lsp` (auto-import), resolve-before-accept, one undo step.
- [x] Words demoted to `fallback` mode when LSP items present (`completion.menu.words: fallback|enabled|disabled`; legacy `completion.words` remains readable).
- [x] `completion.menu` configuration: automatic opening independent of explicit requests and inline suggestions, configurable minimum candidate word length, canonical nested word policy and preserving-save migration. Context restrictions, server trigger behavior, manual paths and signature help retain their distinct roles.
- [x] Lazy resolve for docs + documentation side-card (anchored to the menu panel's right edge, flips left).
- [x] Native macOS rust-analyzer validation of the reported `cc::Build` chain (2026-09-06): typing `.` opened the member list, `comp` filtered to seven compiler-related methods, Tab accepted `compile`, and undo restored the prefix. Local parser/scanner names were absent. Selected signatures/docs were inspected. Methods now keep their `M` badge through a shared source/view kind type. This uses an isolated build-script fixture with the repository's `cc` version, not a modification to the user's file; other language/platform matrices remain open. Evidence and reproduction details are in the [audit](../dev/refactoring-audit-2026-09-06.md#real-rust-analyzer-dropdown-validation--2026-09-06).

### Phase 5+: Future

- [ ] Multi-row ghost text + mid-line suggestions — implemented on shared `TextViewportMap` geometry, with real-insertion oracle, lifecycle and blink-pixel tests plus an inspected headless screenshot. Isolated macOS keyboard/pointer/resize checks and release-stage profiling are now recorded; actual IME composition/candidate-window behavior and the remaining platform matrix are still unverified. See the [native/profiling audit](../dev/refactoring-audit-2026-09-06.md#ghost-native-checks-and-release-profiling--2026-09-07).
- [ ] Edit prediction: anchor-based edit-list suggestion variant, deletion highlighting, diff popover, jump targets (the Zed model); candidate providers: Zeta-style rewrite models, Copilot NES-compatible backends.
- [x] Opt-in workspace retrieval: BM25 over bounded workspace source, declaration
  extraction through the existing Tree-sitter/outline registry, ignore-aware
  collection on the shared latest-request worker, unsaved-buffer precedence,
  and a guarded preparation-to-provider handoff. Ranking, ignore/cache refresh,
  scope, wire formatting and the real background preparation/provider pipeline
  have fixture coverage. Live model relevance is not established by those checks.
- [x] TabbyML native segments transport, sharing the HTTP provider, credentials, cancellation, response limits, recency comment fallback and acceptance pipeline. No automatic startup or telemetry. Wire fixtures and worker acceptance/cancellation tests cover the adapter; live server/model quality is unverified. See [configuration and limits](../user/config-editor.md#tabbyml).
- [x] Supervised local llama-server child process: opt-in executable/model configuration, on-demand startup, health checks, bounded loading, warm reuse across generation cancellation, explicit failure retry and owned-child teardown on configuration changes/exit. Real-child fixtures and an isolated macOS run with a cached Qwen Coder model pass; Windows/Linux process behavior and broader model quality remain unverified. See [configuration](../user/config-editor.md#managed-local-llama-server) and the [verification record](../dev/refactoring-audit-2026-09-06.md#managed-local-llama-server--2026-09-08).
- [x] Local acceptance stats: one terminal outcome per offered response (accepted, dismissed, or fully typed through), attributed to its original configured provider name. Alternatives and partial accepts do not inflate totals. Versioned aggregate-only JSON is merged on the ordered file worker; a config/Settings opt-out and palette action expose the feature. Lifecycle, bounded storage, concurrent writers, failure recovery and queue-draining tests pass. No source, connection settings or network telemetry are collected. See [semantics and storage limits](../user/config-editor.md#local-completion-statistics); these are descriptive counts, not a controlled provider-quality score.
- [x] Context-aware filesystem path source on the shared dropdown, alongside LSP results: bounded speculative directory reads, stale-request guards, Unicode/Markdown encoding, component replacement, directory continuation and multi-cursor Undo. Current syntax gates code-string reads. Supported forms and explicit limits are in [File path suggestions](../user/config-editor.md#file-path-suggestions); native platform verification remains separate. See the [path-source audit](../dev/refactoring-audit-2026-09-06.md#context-aware-path-completion--2026-09-07).
- [x] Menu documentation panel richness — shared CommonMark/GFM-to-native-text
  conversion now preserves nested styles, code examples, lists, quotes, tables,
  escapes and reference-link labels (2026-09-07). This reuses the preview parser
  and existing `StyledText`/card renderer, not a second Markdown grammar or an
  HTML surface. The side card now has independent scrolling across code/prose,
  a row-range footer, expansion via footer/F1 and Alt+PageUp/PageDown paging.
  Measured layout is shared by painting and input; narrow cards do not cover
  their menu. State, layout, input-routing and pixel regressions pass. Native
  keyboard/pointer/IME verification remains part of the separate platform gate.
- [x] Server/item commit characters: ordinary single-character keyboard input accepts the selected LSP item and types the character, preserving imports, single-cursor snippet placement and one-step Undo/Redo. Deferred resolution keeps the character visible and rejects stale/focus-changed replies. Paste/text payloads are not synthesized into acceptance keystrokes. The existing multi-cursor plain-text fallback is unchanged; native IME/platform verification remains a separate open gate above. See [user semantics](../user/config-editor.md#completion-dropdown) and the [acceptance audit](../dev/refactoring-audit-2026-09-06.md#commit-character-acceptance--2026-09-07).

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
| Context strategy | retrieval (BM25) / recency ring | opt-in recency or workspace retrieval | Recency favors stable prompt prefixes; retrieval favors lexical relevance and respects workspace ignore rules |
| Fuzzy matcher | new SIMD matcher (frizbee-like) / nucleo | nucleo | Already a dependency; Lapce ships it for exactly this; revisit only on measured lag |
| Key conflicts | input.rs branching / KeyContext conditions | conditions | Existing mechanism, user-rebindable, matches keymap TODO |
| Menu debounce | timer / none for sync sources | none (sync); request-coalescing for async | Zed ships menu completion with no timer debounce; our sync sources are sub-ms |
| Async runtime | tokio / std thread + mpsc worker | std thread | House pattern (syntax worker, PTY, planned LSP) |
| Multi-line ghost text | shared projected rows / first-line + `+N` badge | shared projection (2026-09-07) | Uses soft-wrap segmentation and `TextViewportMap`; no independent paint-only row loop |

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
- [overlay-surface.md](../archived/overlay-surface.md) — owns the completion popup surface (`Anchor::Cursor`, Completion context, kind badges); prerequisite for the Phase 1 popup. Irrelevant to ghost text, which is in-text-flow paint, not an overlay
- [lsp-integration.md](../archived/lsp-integration.md) — Phase 5 superseded by this document's Phase 4; also note config is YAML, not TOML
- [soft-wrap.md](../archived/soft-wrap.md) — prerequisite for multi-row ghost text
- [snippets.md](snippets.md) — convergence point for snippet bodies and placeholder navigation
- `docs/EDITOR_UI_REFERENCE.md` ch. 7 — earlier positioning prose, superseded here
