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

### `theme`

The active theme ID.

- **Type:** `string`
- **Default:** `"default-dark"`
- **Example:** `theme: "fleet-dark"`

See [config-theme.md](config-theme.md) for available themes and customization.

### `cursor_blink_ms`

Cursor blink interval in milliseconds.

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

### `completion.inline` and `completion.providers`

Ghost-text suggestions from a local [llama.cpp](https://github.com/ggml-org/llama.cpp)
server. Off until you point Token at one:

```yaml
completion:
  inline:
    enabled: true
    provider: local        # key into providers
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

## Example Configuration

```yaml
# ~/.config/token-editor/config.yaml
theme: "fleet-dark"
cursor_blink_ms: 500
auto_surround: true
bracket_matching: true
```
