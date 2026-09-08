# Editor Configuration Reference

General editor settings for Token.

---

## Configuration File

| Platform | Path |
|----------|------|
| macOS | `~/.config/token-editor/config.yaml` |
| Linux | `~/.config/token-editor/config.yaml` |
| Windows | `%APPDATA%\token-editor\config.yaml` |

---

## Settings

The page uses the standard themed scrollbar: drag its thumb or click the track
to jump. Scrolling leaves the selected setting unchanged and moves continuously
in pixels, including partially visible rows. Trackpad deltas retain their pixel
distance; keyboard navigation reveals the selected row without snapping the
rest of the page to section boundaries.

Open the separate Settings page with `Cmd+,` (`Ctrl+,` on Windows/Linux) or
the **Open Settings** command. Categories appear in the left navigation, with
compact category buttons in small windows. Search filters the selected category.
Use Up/Down to select a row, Left/Right to cycle presets, or click a switch or
preset control. Clicking the row label only selects it.
Changes save immediately; Escape closes the page without undoing them. The Theme
row opens the existing theme picker.

Tab/Shift+Tab cycles categories, including [Keymap](config-keymap.md#editing-in-settings).
The Keymap category records shortcuts with an explicit Save/Cancel step; its base-preset
chips save immediately. General controls and keymap overrides use separate files.

The file accepts values outside the offered presets. Such values show no active
chip and stay unchanged until you choose a preset. Saves preserve unknown YAML
keys, but not comments or formatting.

The LSP section includes a master switch and an enabled switch for each registered
server. Disabling stops the affected servers; enabling permits startup on the next
matching file open/edit. A server's switch retains its own preference when the
master switch is Off. Selecting an unchanged choice does not save again.

Command rows show the configured command or registry default and the YAML key to
edit (`lsp.servers.<id>.command`). They are read-only, as are the live process-state
rows; Left/Right and Enter cannot change them. Status updates without reopening
Settings. The existing Language Servers picker remains available.

### Workspace symbol search

With a workspace open and a running language server that supports workspace
symbols, Search Everywhere enables its **Symbols** tab. Select that tab or type
`@` into an empty query. The **All** tab also includes up to five symbol results.
Use arrows/Page Up/Page Down, the mouse wheel, and Enter or a row click to
navigate. Jumps share the normal Back/Forward history and preserve UTF-16 cursor
positions when opening another file.

Search uses the language server's symbol index; it does not launch additional
servers or scan source files itself. Queries debounce for 150 ms and each server
has a five-second response timeout. Partial failures remain visible alongside
usable results. Change the query to retry. Results are deduplicated and limited
to 2,000 rows; queries are limited to 256 characters. Refine the query when a limit
is reported. Servers must return complete file locations; lazy symbol resolution
is not currently supported.

### Usages

Run **Find Usages** at a symbol in a named text file to open a persistent Usages
dock panel. It uses the file's language server and groups results by file; it
does not scan the workspace itself. **Show Usages** keeps the transient popup
behavior. Starting either action supersedes the pending references search.

In the panel, use Up/Down or Page Up/Page Down to select rows, Left/Right to
collapse/expand a file, and Enter to toggle a file or open a location. A single
click selects, a double click opens, and a file's chevron toggles its group.
The mouse wheel scrolls; Escape returns focus to the editor. Results remain
available after navigation or closing the dock; **View: Toggle Usages** reopens
it without rerunning the search.

Results are a snapshot, not a live index. Run Find Usages again after edits.
Changing or closing the source while a search is pending cancels that search;
moving the caret, changing focus or closing the panel does not. Late responses
never reopen the panel or steal focus. Loading, empty results, unavailable
servers, timeouts and the 200-location result limit are shown in the panel.
Previews use unsaved open buffers when available; bounded background reads may
leave other previews blank without preventing navigation.

### `theme`

The active theme ID.

- **Type:** `string`
- **Default:** `"default-dark"`
- **Example:** `theme: "fleet-dark"`

See [config-theme.md](config-theme.md) for available themes and customization.

### `cursor_blink_ms`

Cursor blink interval in milliseconds. Set `0` for a steady, non-blinking caret.

- **Type:** `integer`
- **Default:** `600`
- **Example:** `cursor_blink_ms: 500`

### `auto_surround`

Automatically surround selected text when typing an opening bracket or quote character. When enabled, selecting text and typing `(`, `[`, `{`, `"`, `'`, or `` ` `` wraps the selection instead of replacing it (e.g., `hello` → `(hello)`). Works with multi-cursor selections.

- **Type:** `boolean`
- **Default:** `true`
- **Example:** `auto_surround: false`

### `bracket_matching`

Highlight matching brackets when the cursor is adjacent to `(`, `)`, `[`, `]`, `{`, or `}`. Both the bracket under/before the cursor and its matching pair are highlighted with a background color (configurable via the `bracket_match_background` theme color).

- **Type:** `boolean`
- **Default:** `true`
- **Example:** `bracket_matching: false`

---

### Completion dropdown

The dropdown uses language-server results for member access such as `builder.`.
Local buffer words and snippets are excluded in that context; they cannot infer
the receiver's methods. The server's ordering is retained, and typing a prefix
filters its results. Selected items can show a signature and documentation.
Badges distinguish methods (`M`), functions (`f`) and modules (`m`).

Documentation cards share Markdown formatting with hover and signature help:
headings and emphasis, literal code examples, numbered/nested lists and task
markers, quoted lines and text tables. Links show their labels; images show alt
text, without fetching resources. HTML stays literal text, and strikethrough is
shown dimmed. Plain-text server documentation is not parsed as Markdown.

For completion documentation, scroll over the card without moving the selected
suggestion. The footer shows the visible row range, including wrapped code and
prose. Click it or press F1 to expand/collapse the card; Alt+PageUp/PageDown moves
one visible page. Ordinary PageUp/PageDown still navigates the suggestion list.
Choosing a different suggestion resets its documentation view. Cards narrow to
fit beside the menu and stay within the window height; if no text area fits,
enlarge the window to see the card. These controls apply to the completion side
card, not the separate hover/signature-help popups.

For example, `ar_flag` and `archiver` after `cc::Build::new().` are valid builder
methods, not words extracted from your file. Typing `comp` narrows that list to
compiler-related methods. `completion.menu.words` controls local word suggestions
(`fallback`, `enabled` or `disabled`), but does not make them semantic member
completions.

```yaml
completion:
  enabled: true             # master switch for dropdown and inline suggestions
  menu:
    enabled: true           # automatically open the dropdown while typing
    min_word_length: 3      # candidate identifier length, in characters
    words: fallback        # fallback | enabled | disabled
```

`completion.menu.enabled: false` stops new automatic dropdowns, including on
server trigger characters and paths. Ctrl+Space still opens one, and an already
open session continues to filter while typing. Accepting a directory from a
manual path session continues into its children. Signature help and configured
inline suggestions remain available. `completion.enabled: false` retains its
master-switch behavior, disabling both automatic and explicit completion.

`min_word_length` filters local candidate identifiers, not language-server items,
snippets or filenames. It counts characters rather than UTF-8 bytes, defaults to
three and treats zero as one. It does not change the two-character typed-prefix
threshold for automatic word completion. The existing syntax/context restrictions,
scan window and result cap still apply.

Legacy `completion.words` remains readable. An explicitly supplied
`completion.menu.words` wins if both keys exist; otherwise the legacy value is
used, including with a partial `menu` block. Saving writes only the nested key
while preserving unknown settings. Invalid settings are not overwritten by save.

Single-cursor LSP acceptance includes additional edits such as auto-imports,
whether supplied in the initial response or by item resolution. The primary
completion and non-overlapping additional edits form one undo step. If resolution
fails, edits already supplied by the server are retained. Multi-cursor acceptance
currently uses the item's plain-text fallback; absolute additional edits are not
repeated at each cursor.

Typing a commit character explicitly declared by the selected LSP item (or its
server default) accepts that item, then inserts the character. Token does not
guess a punctuation set for local words, snippets or filesystem paths. The result, character and
known auto-imports form one Undo step; single-cursor snippet caret placement is
preserved. Multi-cursor acceptance uses the same plain-text fallback described
above, with the character appended at each completed prefix.

When item resolution is still pending, the character appears immediately. If
you type again, navigate, dismiss the menu, switch panes/focus or change the file,
the late reply cannot replace your newer work. Paste, text/IME insertion messages
and multi-character keyboard payloads do not accept a highlighted menu item.
An ordinary one-character keyboard event can commit; native IME/platform behavior
still needs the separate platform verification matrix.

#### File path suggestions

The dropdown recognizes forward-slash paths such as `./assets/`, `../src/`,
`~/Documents/` and absolute paths. Relative paths use the current file's parent
directory, or the workspace root for an untitled buffer. Without either base,
untitled buffers can still complete absolute and home-relative paths.

Code paths must be inside an ordinary quoted string identified by current syntax; local directory
reads wait for parsing. Plain-text paths and Markdown link destinations also
work. Ctrl+Space can explicitly list a directory inside a string or Markdown
link even without a slash; ordinary prose words and code expressions do not
trigger filesystem suggestions. Comments, regexes, URLs and Markdown fragments/query strings are
excluded. Language-server completions remain available, including import aliases
that have no corresponding local directory, and matching local/server insertions
are deduplicated in favor of the server.

File (`F`) and directory (`/`) badges distinguish entries. Selecting a filesystem
directory inserts its trailing slash and requests its children. Acceptance
replaces the current filename component, including an existing suffix after the
caret, as one Undo step. Multiple carets must have compatible path contexts;
peer panes follow the shared edit transaction. Spaces work inside quoted strings
and are percent-encoded in Markdown links; accented filenames use literal prefix
matching. Hidden names appear only when the component starts with `.`.
Already-complete local filenames are omitted when acceptance would do nothing.

The source reads one directory, never recursively, on a speculative worker that
does not delay saves. It keeps at most one active and one pending request, up to
500 returned names and 1 MiB of filename bytes. Scanning stops after 20,000 entries
or a cooperative 50 ms budget, reporting when results are limited. A blocked OS
filesystem call can exceed that budget, but neither typing nor shutdown waits
for it. No completion data is persisted or sent to an inline provider.

Path recognition is limited to lines of at most 4,096 characters. Escaped source
paths, backslash/UNC forms, angle-bracket Markdown destinations and names requiring
language-specific quoting are omitted; use forward slashes and ordinary Markdown
destinations. Non-UTF-8 names and unsupported filesystem entry types are omitted.
Changing the file, pane, language, focus, selection or revision invalidates old
results; Escape prevents an outstanding reply from reopening the menu.

### `completion.inline` and `completion.providers`

Inline results have an in-memory, per-window cache: at most 256 entries and
8 MiB of retained source/result payload, plus bounded entry metadata. Matching
automatic requests can replay after backspace/retype, or reuse a longer
alternative's remainder after typing or acceptance. The full bounded prefix,
suffix, document/file, language and provider configuration must match. Request
revisions are refreshed and normal visibility/staleness checks still apply.
The ordinary debounce is unchanged. An explicit “Trigger Inline Suggestion”
request bypasses reuse and asks the backend again. Entries are evicted by recent
use and disappear when the window's worker exits; nothing is persisted.

After cache lookup, inline results receive syntax-aware bracket and indentation
checks. Recognized literals/comments are left alone; an impossible closing
bracket truncates the candidate. In Rust, Go, JavaScript, C and C++, leading
tabs/spaces follow the document's majority style without changing visual columns.
Other languages and mixed/tied styles keep their indentation unchanged; visual
tab stops must not rewrite indentation-sensitive syntax. This is not a formatter
or a semantic correctness guarantee.

Analysis uses a local document snapshot, never added to the provider's HTTP
request or retained in the result cache. Documents and individual candidates
over 1 MiB, unsupported syntax, ambiguous recovery involving quotes/slashes,
and exhausted analysis budgets preserve the original result. Alternatives share
a cooperative 50 ms parsing/traversal budget on the existing worker. Exact cache
hits are checked against fresh local context; partial replay matches the
normalized text that was shown, including tabs/spaces.

Ghost-text suggestions from a local [llama.cpp](https://github.com/ggml-org/llama.cpp)
server. Off until you point Token at one:

```yaml
completion:
  inline:
    enabled: true
    provider: local        # key into providers
    statistics: true       # local aggregate counts; disable to opt out
    debounce_ms: 300       # quiet time after the last keystroke
    max_line_suffix: 8     # auto-trigger only near the end of the line
  providers:
    local:
      transport: llama_cpp
      url: http://127.0.0.1:8012
      max_tokens: 128
      timeout_ms: 5000
```

Start the server with a fill-in-the-middle model, for example
`llama-server -m Qwen2.5-Coder-1.5B-Instruct-Q8_0.gguf --port 8012`. Tab
accepts the ghost text, Escape dismisses it, and Option+\\ asks for one
without waiting for the debounce. After three failed requests Token pauses
automatic suggestions until you trigger one manually.

Suggestions show all their lines as dimmed text in the focused pane, wrapping
with that pane's soft-wrap setting. Existing text after the cursor and following
lines move out of the way; the document is unchanged until you accept. Clicking
the speculative text places the cursor at its insertion point and dismisses it.
Navigation also dismisses it. Typing matching characters consumes the preview
without opening a competing automatic dropdown. Other panes keep their normal
document view. Explicit requests work mid-line; automatic requests still respect
`completion.inline.max_line_suffix`.

### Local completion statistics

`completion.inline.statistics` defaults to `true`; inline completion itself
remains disabled by default. Settings → Completion → Local completion statistics
controls the same value. No source, suggested text, file paths, endpoint settings,
credentials, or network telemetry are collected. The JSON contains only a format
version, the configured provider names, and these aggregate counters:

- `accepted`: the first successful explicit acceptance of any portion of a
  response, including word/line acceptance.
- `dismissed`: a displayed response ends without explicit acceptance, such as
  Escape, navigation, changing focus, or incompatible typing.
- `typed_through`: the entire suggestion was typed manually without explicit
  acceptance.

Each response contributes at most once. Cycling alternatives, accepting more
portions, and undoing an acceptance do not add or reverse outcomes. Pending,
empty, failed and stale responses do not count. Unresolved offers lost in a crash
do not count either, so the sum is completed observations, not all requests or
all generated alternatives. Counts alone are not a controlled quality comparison.

“Open Inline Completion Statistics” in the command palette opens
`inline-statistics.json` in the editor's configuration directory. This is a normal
file tab, not a live dashboard: close and reopen it to read newer counts. Reusing
an open tab preserves its buffer and unsaved edits. Do not save an old snapshot
over newer counts; to reset counts, close the tab and remove the JSON while the
editor is stopped. Leave the stable `inline-statistics.lock` sidecar alone while
any editor window is running.

Writes run on the ordered background file worker and merge across windows under
a bounded lock. Storage is capped at 256 KiB, 256 provider names and 256 UTF-8
bytes per name; counters saturate rather than wrap. Malformed, unsupported-version
and symlinked statistics files are left untouched. Failed writes show a non-modal
status message and are not replayed; editing and completion continue normally.
Turning collection off discards the current observation; turning it on starts
counting with newly requested responses. Existing stored counts are retained.

## Example Configuration

### Additional inline providers

Providers share request cancellation, deadlines and a 1 MiB response limit.
By default, the OpenAI-compatible transport uses native `prompt` + `suffix`
fields, not Chat Completions or Responses. Native mode needs a server/model that
supports suffix insertion. Raw FIM is an explicit alternative described below;
neither mode makes arbitrary chat models compatible. The [OpenAI Completions
reference](https://developers.openai.com/api/reference/resources/completions/methods/create)
documents the wire shape and model-specific suffix restriction.

```yaml
completion:
  inline:
    enabled: true
    provider: compatible
  providers:
    compatible:
      transport: open_ai_compat
      url: http://127.0.0.1:8080/v1
      model: your-suffix-capable-model
      # api_key_env: TOKEN_COMPLETION_KEY  # optional environment-variable name
      max_tokens: 128
      n: 1  # 1..8; higher values request more alternatives and may cost more
      timeout_ms: 5000
```

For [Mistral FIM](https://docs.mistral.ai/api/endpoint/fim), set the selected provider
to a separately configured entry such as:

```yaml
transport: mistral_fim
url: https://api.mistral.ai/v1
model: codestral-latest
api_key_env: TOKEN_MISTRAL_KEY
max_tokens: 128
timeout_ms: 5000
```

`url` is a base URL: the appropriate `/v1/completions` or `/v1/fim/completions`
route is appended, without duplicating a trailing `/v1`. Gateway path prefixes
are preserved. Do not include the complete endpoint, query parameters or
credentials in the URL.

Set the named credential variable in the environment inherited by the editor
process before starting it. A GUI launcher may not inherit your shell environment.
The config stores only its name; the worker resolves its value when starting a
request. Missing, empty or invalid credentials fail with a status message and no
request. No ambient API key is discovered automatically. Authenticated non-loopback
URLs must use HTTPS, and redirects are never followed. Selecting a hosted provider
sends the bounded source prefix/suffix to that provider and may incur API charges;
support does not enable it or change your selected provider automatically.

### TabbyML

For [TabbyML](https://tabby.tabbyml.com/api/completion/), select a provider such as:

```yaml
transport: tabby
url: http://127.0.0.1:8080
# api_key_env: TOKEN_TABBY_KEY  # when your server requires a bearer token
timeout_ms: 5000
```

The native `/v1/completions` request uses `segments.prefix` and `segments.suffix`.
Model selection and generation length are configured on the Tabby server;
the common `model`, `max_tokens` and `keep_alive` fields are not sent. Use
`prompt_format: native` and `n: 1`; raw prompts and requesting multiple results
are rejected. Returned alternatives still share filtering, cycling, partial
acceptance and Undo with other providers. The same base-path, credential,
deadline, cancellation and response-size rules above apply.
An empty `choices` list means no suggestion and does not trigger error backoff.

The adapter sends a language ID when known, but no absolute file paths, clipboard,
user identifier, Git URL or acceptance telemetry. Workspace-relative path and
declaration retrieval are not yet attached. Opt-in recent-buffer context uses
the existing commented-prefix fallback, not Tabby's declaration/search fields.
The adapter does not start a server, download a model or change your selected
provider. Automated local HTTP fixtures cover the contract; live Tabby model
quality has not been validated.

### Raw FIM prompts

`prompt_format: native` is the default and preserves the transport's existing
prefix/suffix handling. On `ollama` and `open_ai_compat`, an explicit format
instead serializes both parts into one raw prompt and omits the native `suffix`
field. Ollama receives `raw: true` to bypass its model template. Both transports
receive format-specific stop strings and retain existing cancellation, limits,
credentials and acceptance behavior. Raw formats are rejected for `llama_cpp`
and `mistral_fim` or `tabby`, which build FIM prompts server-side.

| `prompt_format` | Layout |
| --- | --- |
| `qwen` | Qwen FIM tokens, prefix then suffix |
| `star_coder` | StarCoder FIM tokens, prefix then suffix |
| `code_llama` | CodeLlama prefix-first infilling, including marker word-boundary spaces |
| `deep_seek` | DeepSeek's Unicode FIM tokens, prefix then suffix |
| `codestral` | Suffix then prefix |
| `mellum` | Suffix then prefix, with optional current-file basename |

For example, with a compatible raw-completion server already serving this model:

```yaml
completion:
  inline:
    enabled: true
    provider: raw
  providers:
    raw:
      transport: open_ai_compat
      url: http://127.0.0.1:8080/v1
      model: Qwen/Qwen2.5-Coder-7B
      prompt_format: qwen
```

`infer` recognizes bounded family names for Qwen2.5-Coder, StarCoder/StarCoder2,
CodeLlama 7B/13B (including their infilling-capable Instruct variants),
DeepSeek-Coder, Codestral and Mellum. Unknown aliases and unrecognized/ambiguous
variants fail with an explicit message; use an explicit format when you know
the served model's FIM vocabulary. Inference is a name mapping, not a capability
probe. It does not load a model or contact a hosted service to test compatibility.

The server must recognize the format's special tokens and provide its model's
normal beginning-of-sequence handling; Token does not add a second BOS tag or a
chat template. Prefix/suffix bytes are retained, with the format's own separators.
Raw source containing that format's control tokens is rejected before HTTP to
avoid ambiguous prompt boundaries. Mellum includes only a usable basename, not
an absolute path; names with control characters or angle brackets are omitted.
Changing `prompt_format` invalidates result-cache reuse. Live model/server quality
still needs validation for the selected deployment.

### Extra context from recently visited buffers

Extra context is off by default. To opt in for one provider, add:

```yaml
completion:
  providers:
    local:                      # your existing inline provider
      context:
        strategy: recency_ring  # none (default) | recency_ring
        max_chunks: 8           # 1–32
        chunk_lines: 64         # 1–256
```

This permits sending snippets from other open text buffers, including **unsaved
text**, to that provider. It does not scan unopened files or persist a context
index. With a workspace open, named files must have a resolved identity inside
its root; symlinks resolving outside are excluded. Without a workspace, the
scope is the current window's open text buffers. Untitled buffers are included.
Labels use workspace-relative paths or buffer-qualified basenames, never absolute
directories; invalid/oversized labels are omitted.

Enabling the strategy, switching files, saving, or jumping at least `chunk_lines`
logical lines queues positions. After 750 ms without cursor/document changes,
the ring takes up to `chunk_lines` around each queued position, capped at 8 KiB
per snippet and 1 KiB per filename. Pending positions and retained chunks are
both bounded by `max_chunks`. The maximum text payload is 256 KiB, plus labels
and bounded metadata. A runtime-only index retains sorted token byte ranges so
pairwise deduplication does not re-tokenize existing chunks. On 64-bit builds its
retained range storage is bounded above by 2 MiB at the maximum configuration,
plus bounded capture scratch space, separate from the source payload; indexes
are not transmitted or copied into the result
cache. Empty snippets are skipped; token-set similarity strictly
above 90% removes the older snippet. Revisiting an overlapping region of the same
buffer replaces its old snapshot even after a substantial rewrite or deletion.
Ordering is by recency, not relevance ranking.

Committed text remains stable during ordinary typing. It represents an idle
snapshot, not a continuously updated copy; a subsequent save, switch or large
jump refreshes context after idle. Closing/renaming a source removes its old ring
entry. Disabling inline completion or changing provider settings/workspace clears
the ring. These actions cannot retract requests already sent to the provider.

llama.cpp receives `input_extra`. Other transports prepend line-commented snippets
to the active prefix, including in raw FIM mode. Common `//`, `#`, `--` and `;`
languages are supported. Languages without a supported comment fallback, such
as JSON, HTML and CSS, report an explicit configuration error when extra context
is present; use native llama.cpp or `strategy: none` for those languages.
The active prefix/suffix and cursor positions remain unchanged internally.
Extra context participates in exact and partial result-cache equality and the
existing 8 MiB cache payload limit. Changing context therefore prevents reuse of
an answer generated with different snippets; there is no new cache setting.

### Cycling inline alternatives

For `open_ai_compat`, `n` selects 1–8 results per request and defaults to 1.
Other transports currently require `n: 1`; they do not fan out extra requests.
All returned alternatives pass through the same output filters. Empty and
duplicate suggestions are omitted, with at most eight retained in provider order.

While ghost text is visible, Alt+] selects the next compatible alternative and
Alt+[ the previous one (Option on macOS), wrapping at either end. The indicator
`[2/3]` means the second of three currently compatible choices. Cycling does not
edit, create an undo entry or ask the provider again. After typing through or
partially accepting a suggestion, only alternatives with the exact same inserted
prefix and a nonempty remainder remain eligible. Backspacing can make previously
incompatible choices eligible again. Tab accepts the selected remainder.

`NextInlineSuggestion` and `PrevInlineSuggestion` are named actions for automation
and user keybindings. Automation reports `inline_choice: [position, count]` only
while a suggestion is visible, alongside the existing remaining ghost text.

### General settings example

```yaml
# ~/.config/token-editor/config.yaml
theme: "fleet-dark"
cursor_blink_ms: 500
auto_surround: true
bracket_matching: true
```
