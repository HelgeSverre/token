# Feature Name

Brief tagline describing the feature in one sentence.

> **Status:** 📋 Planned | ⚠️ In Progress | ✅ Complete
> **Priority:** P1 (Critical) | P2 (Important) | P3 (Nice-to-have)
> **Effort:** S (1-2 days) | M (3-5 days) | L (1-2 weeks) | XL (2+ weeks)
> **Created:** YYYY-MM-DD
> **Updated:** YYYY-MM-DD
> **Milestone:** N - Milestone Name

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

Describe what exists today. For new features, note "No existing implementation."
For enhancements, describe the current behavior and limitations.

### Goals

- Goal 1: What this feature will enable
- Goal 2: Specific capability or improvement
- Goal 3: User benefit

### Non-Goals

Explicitly state what this feature will NOT do to prevent scope creep:

- Non-goal 1: Feature we're intentionally excluding
- Non-goal 2: Future consideration, not this iteration

---

## Architecture

### Integration Points

Show how this feature integrates with existing architecture:

```
┌─────────────────┐     ┌─────────────────┐
│   User Input    │────►│    Keymap       │
└─────────────────┘     └────────┬────────┘
                                 │
                                 ▼
┌─────────────────┐     ┌─────────────────┐
│   New Module    │◄────│    Update       │
└────────┬────────┘     └─────────────────┘
         │
         ▼
┌─────────────────┐     ┌─────────────────┐
│    Renderer     │────►│    Display      │
└─────────────────┘     └─────────────────┘
```

### Module Structure

```
src/
├── existing_module/
│   └── mod.rs           # Existing code (modify)
└── new_module/          # New module (create)
    ├── mod.rs           # Public exports
    ├── types.rs         # Data structures
    └── logic.rs         # Core implementation
```

### Message Flow

Describe the message flow for key operations:

1. User triggers action (key press, click)
2. `Msg::Feature(FeatureMsg::Action)` dispatched
3. `update()` processes message, returns `Cmd`
4. Renderer displays updated state

---

## Data Structures

### FeatureState

```rust
/// State for this feature, stored in AppModel or relevant submodule.
pub struct FeatureState {
    /// Description of field purpose
    pub field_name: Type,

    /// Another field with its purpose
    pub another_field: Option<Type>,
}

impl Default for FeatureState {
    fn default() -> Self {
        Self {
            field_name: Type::default(),
            another_field: None,
        }
    }
}
```

### FeatureMsg

```rust
/// Messages for this feature
#[derive(Debug, Clone)]
pub enum FeatureMsg {
    /// Perform the primary action
    DoAction,

    /// Action with parameters
    DoActionWith { param: Type },

    /// Cancel or reset
    Cancel,
}
```

### Related Types

```rust
/// Supporting type used by this feature
pub struct SupportingType {
    pub data: Vec<Item>,
}

/// Configuration for this feature
pub struct FeatureConfig {
    pub enabled: bool,
    pub option: String,
}
```

---

## Keybindings

### Default Bindings

| Action | Mac | Windows/Linux | Context |
|--------|-----|---------------|---------|
| Primary action | `Cmd+Key` | `Ctrl+Key` | always |
| Secondary action | `Cmd+Shift+Key` | `Ctrl+Shift+Key` | has_selection |
| Toggle feature | `F1` | `F1` | always |

### Keymap Configuration

```yaml
# User can add to ~/.config/token-editor/keymap.yaml
- key: "cmd+k"
  command: FeatureAction
  when: ["in_editor"]
```

---

## Implementation Plan

### Phase 1: Core Foundation

**Effort:** S/M/L

- [ ] Create module structure (`src/feature_name/mod.rs`)
- [ ] Define data structures
- [ ] Add messages to `messages.rs`
- [ ] Implement basic `update()` handling
- [ ] Add unit tests for core logic

### Phase 2: UI Integration

**Effort:** S/M/L

- [ ] Add rendering in `view.rs`
- [ ] Wire up keybindings in `keymap.yaml`
- [ ] Handle user input
- [ ] Add command to palette

### Phase 3: Polish

**Effort:** S/M/L

- [ ] Error handling and edge cases
- [ ] Performance optimization
- [ ] Integration tests
- [ ] Documentation update

### Phase N: Future (Optional)

Items explicitly deferred to future iterations:

- [ ] Advanced feature X
- [ ] Integration with Y

---

## Testing Strategy

### Unit Tests

```rust
// tests/feature_name.rs

#[test]
fn test_basic_functionality() {
    let mut state = FeatureState::default();
    // Setup
    // Action
    // Assert
}

#[test]
fn test_edge_case() {
    // Edge case handling
}

#[test]
fn test_error_condition() {
    // Error handling
}
```

### Integration Tests

Test scenarios that involve multiple components:

1. **Scenario 1:** Description of end-to-end test
2. **Scenario 2:** Another integration scenario

### Manual Testing Checklist

- [ ] Basic functionality works as expected
- [ ] Keybinding triggers correct action
- [ ] Command palette integration works
- [ ] Edge case: empty document
- [ ] Edge case: large document
- [ ] Multi-cursor interaction (if applicable)
- [ ] Selection interaction (if applicable)
- [ ] Undo/redo preserves state correctly

---

## References

### Internal Docs

- [Related feature](../feature/related-feature.md)
- [Behavior contract](../dev/contracts-relevant.md)
- [ROADMAP](../ROADMAP.md)

### External Resources

- [Inspiration: VS Code feature](https://code.visualstudio.com/docs)
- [Relevant crate](https://docs.rs/crate-name)
- [Algorithm/technique](https://example.com)

---

## Appendix (Optional)

### Design Decisions

Document key decisions and their rationale:

| Decision | Options Considered | Chosen | Rationale |
|----------|-------------------|--------|-----------|
| How to X | A, B, C | B | Reason for B |

### Open Questions

Questions to resolve during implementation:

1. Question about specific behavior?
2. Question about edge case handling?

### Changelog

| Date | Change |
|------|--------|
| YYYY-MM-DD | Initial draft |
| YYYY-MM-DD | Updated after review |
