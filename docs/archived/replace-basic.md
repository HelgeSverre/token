# Basic Replace

Core replace functionality with replace and replace all.

> **Status:** ✅ Implemented
> **Implemented:** v0.3.11
> **Keybindings:** Cmd+F (open with Tab to replace field), Enter (replace & next)

---

## Implemented Features

### Replace Operations

- **Replace field** - Text input for replacement text
- **Replace & Find Next** - Replace current match and move to next
- **Replace All** - Replace all matches in document
- **Case sensitivity** - Respects find bar case sensitivity setting

### Keybindings

| Action | Keybinding |
|--------|------------|
| Open Find/Replace | Cmd+F |
| Replace & Find Next | Enter (when in replace field) |
| Replace All | Cmd+Shift+Enter (in find modal) |

### Code Location

- **State:** `src/model/ui.rs` - `FindReplaceState`
- **Messages:** `src/messages.rs` - `ModalMsg::ReplaceAndFindNext`, `ModalMsg::ReplaceAll`
- **Update:** `src/update/ui.rs:735-770` - Replace handlers

### Implementation Details

Replace operations:
1. Get current match position from find state
2. Replace text at match position with replacement text
3. Re-search to find next match
4. Navigate to next match (for Replace & Next)

Replace All:
1. Find all matches
2. Replace in reverse order (to preserve offsets)
3. Single undo operation for all replacements

---

## Related Docs

- **Advanced features:** `docs/feature/replace-enhancements.md` - Preserve case, regex captures
