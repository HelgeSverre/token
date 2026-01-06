# Basic Find

Core find functionality with case sensitivity and navigation.

> **Status:** ✅ Implemented
> **Implemented:** v0.3.11
> **Keybindings:** Cmd+F (open), Enter/Cmd+G (next), Shift+Cmd+G (prev)

---

## Implemented Features

### Find Modal (`Cmd+F`)

- **Query input field** - Text input for search term
- **Case sensitivity toggle** - Match case checkbox
- **Find Next** - Navigate to next match (`Enter` or `Cmd+G`)
- **Find Previous** - Navigate to previous match (`Shift+Cmd+G`)
- **Current match highlighting** - Highlights the active match
- **Close** - `Escape` closes the find bar

### Code Location

- **State:** `src/model/ui.rs` - `FindReplaceState`
- **Messages:** `src/messages.rs` - `ModalMsg::FindNext`, `ModalMsg::FindPrevious`
- **Update:** `src/update/ui.rs` - Find navigation handlers
- **Keybindings:** `keymap.yaml` - `cmd+f` opens find modal

### Data Structure

```rust
pub struct FindReplaceState {
    pub query_editable: EditableState<StringBuffer>,
    pub replace_editable: EditableState<StringBuffer>,
    pub focused_field: FindReplaceField,
    pub replace_mode: bool,
    pub case_sensitive: bool,
}
```

---

## Related Docs

- **Advanced features:** `docs/feature/find-enhancements.md` - Regex, whole word, match count
