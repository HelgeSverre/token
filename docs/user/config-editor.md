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

## Example Configuration

```yaml
# ~/.config/token-editor/config.yaml
theme: "fleet-dark"
cursor_blink_ms: 500
auto_surround: true
bracket_matching: true
```
