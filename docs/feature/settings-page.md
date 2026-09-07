# Settings Page

A searchable, preset-driven settings modal on the existing `OverlaySurface` — no new rendering surface, no config file editor, no free-text inputs. One static descriptor table drives search, sectioning, and rendering; the YAML file stays the single source of truth and keeps accepting values the UI doesn't offer as presets.

> **Status:** Implemented v1 (2026-09-06); keymap tab remains future
> **Priority:** P3
> **Effort:** M
> **Created:** 2026-08-13
> **Milestone:** 6 - Productivity

---

## Implementation Notes

### Visual revision (2026-09-07)

The initial compact palette presentation was replaced at user request with a
Zed-inspired, window-sized preferences form: category navigation on the left,
section headings and descriptions beneath setting labels, boolean switches,
and right-aligned presets. On narrow windows categories wrap above the form
and controls move below labels. Tab/Shift+Tab cycles categories; search filters
the current category (All Settings searches everything). This remains an in-app
modal, not a separate operating-system window.

Short windows also reflow categories into a grid. When only one display row
fits, navigation shows the setting rather than its section heading. Theme and
server-command values are separate from optional descriptions and stay visible
below their labels in compact rows; the selected value also appears in the footer.

`src/view/settings_page.rs` owns the settings presentation within OverlaySurface.
Its resolved rectangles are used by both painting and hit testing, and its
viewport capacity is shared with keyboard, wheel, and screenshot navigation.
The historical design below describes the original compact presentation;
this revision supersedes its width, row layout, and navigation decisions.

- `src/settings/descriptors.rs` declares the editor, appearance, status bar,
  completion, and LSP master presets. Theme metadata lives in the same table and
  opens the existing Theme Picker.
- `resolve_settings_rows` produces the row identities cached by `SettingsState`.
  Rendering, navigation, and chip commits use that same order. Per-server rows
  come from the compile-time server registry.
- Update handlers mutate `EditorConfig` and return `Cmd::SaveConfiguration`;
  the runtime saves each snapshot immediately in click order. Existing invalid
  files are left untouched. The merge compares the old typed configuration to
  avoid restoring deliberately removed known options or map entries.
- Control geometry is shared by rendering and hit testing. Narrow windows move
  controls below labels before shrinking slots and truncating labels. The footer shows the selected row's description
  or the command override's YAML key.
- Server lifecycle rows read the live model and request a full modal redraw on
  status changes. Settings does not add server restart/management actions.
- Regression tests live in `src/settings/tests.rs`, `src/config.rs`, and the
  runtime/overlay test modules. Screenshot scenarios cover the default modal,
  LSP search, and narrow windows under `screenshots/scenarios/settings*.yaml`.

## Overview

### Why

`EditorConfig` (`src/config.rs`) already has real, user-facing knobs — theme, cursor blink, auto-surround, bracket matching, scrollbar visibility, status bar font size, and the whole `lsp.*` block — but the only way to change most of them today is hand-editing `~/.config/token-editor/config.yaml` and restarting or triggering a reload. The one exception is the theme picker, which is a bespoke list modal wired directly to `theme` and nothing else. As `EditorConfig` grows (LSP per-server overrides already exist; more editor/UI knobs are inevitable) that gap gets worse, and every new setting either needs its own bespoke modal (theme-picker style, non-scaling) or stays YAML-only forever.

A Settings context on `OverlaySurface` gives every discrete-choice setting a UI for the cost of one table row, reusing infrastructure (fuzzy search, sectioned lists, accessory chips) that already exists for the palette and pickers.

### Baseline Before Implementation

- `EditorConfig` (`src/config.rs`) is the single struct persisted to `~/.config/token-editor/config.yaml`. Fields use `#[serde(default = "...")]` so missing keys fall back cleanly on load. `LspConfig` nests `enabled: bool` and `servers: HashMap<String, LspServerOverride>` (keyed by `LspServerDef::id`, overriding `command`/`enabled` per server).
- `EditorConfig::save()` (`src/config.rs:190`) is `serde_yaml::to_string(self)` followed by a plain file write — it serializes *only* what the struct knows about. Any key a user hand-added, or that a newer build wrote and this build doesn't have a field for, is silently dropped on the next save. This is fine today because saves are rare (theme picker only); it becomes a data-loss bug the moment settings UI writes on every click.
- The theme picker is the only existing config-mutating list modal: a `Body::List` context on `OverlaySurface` with User Themes / Built-in Themes sections and a `Check` accessory on the active row (`docs/feature/overlay-surface.md`, Contexts table). It is the closest reference pattern for a Settings context, but it is single-purpose — one field, one section split, no search-across-settings behavior.
- `OverlaySurface` (`src/view/overlay_surface.rs`) already provides everything a Settings context needs structurally: `Anchor::Centered`, `Header` with a live-filtering input, `Body::List { sections, selected, scroll, max_visible }`, row `Accessory` variants (`Keycaps`, `DimText`, `Check`, `Tag`), and the resolve-rows ordering-authority pattern (`update/ui.rs`) that keeps view order and Enter/confirm order from diverging. Keycap chips (`binding_chips`) render existing keybindings; segmented-chip choice rendering (multiple selectable options in one row) is new but is the same accessory-composition idea as `Keycaps`.
- Keymap layering: the embedded `keymap.yaml` (`src/keymap/defaults.rs`, `include_str!`) is loaded first, then a user `~/.config/token-editor/keymap.yaml` is parsed and merged over it via `merge_bindings` (`defaults.rs:74`). This two-layer (compiled defaults → user file) shape is the layering model Settings should mirror for v1 — no project-level layer yet.
- LSP state: `LspConfig.enabled` / `LspConfig.servers` in `EditorConfig` is the persisted half; `LspUiState { servers: HashMap<LspServerId, ServerState> }` (`src/model/mod.rs:437`) is the render-only live-status mirror, updated by `LspMsg::ServerStateChanged`. `ServerState` (`src/lsp/mod.rs:45`) is `Starting | Indexing | Ready | Restarting{attempt} | Failed | Missing | ShuttingDown`.

### Goals

- Every discrete-choice `EditorConfig` field gets a searchable settings row with 2-5 preset choices, without hand-rolling a new modal per field.
- Zero risk of clobbering hand-written or forward-compatible config keys on save.
- One place (a static table) to add a new setting — no new modal code, no new search wiring.
- LSP section surfaces `lsp.enabled`, per-server enabled/status, and command overrides, cross-referencing the interim Language Servers picker modal.

### Non-Goals

- A settings file editor, JSON/YAML schema viewer, or raw text editing of `config.yaml`.
- Free-text/numeric inputs, validation, or error states — v1 is presets only (see Preset/File Asymmetry).
- A new rendering surface — this is an `OverlaySurface` context, not a new component.
- Multiple config layers (project-level `.token/config.yaml`) — noted as future, not built here.
- Settings sync, import from other editors, per-setting reset buttons, live file-watch config reload — see Phase 2 / Explicitly Out of Scope.
- A macro/registration-based settings-declaration system (see Design Decisions).

---

## Design / Architecture

### Shape

Settings is a **context on the existing `OverlaySurface`**, the same way the palette, theme picker, and pickers are:

- **Anchor:** Centered, same width class as the palette/pickers.
- **Header:** input = live fuzzy search across all settings, matched against name + description + keywords (nucleo, same as every other list context per `overlay-surface.md`'s Behaviour section).
- **Body:** `Body::List` with `Section`s as categories — Appearance, Editor, Status Bar, LSP, ... — matching the descriptor table's `section` field. No search query shows all sections in table order; typing filters rows and hides empty sections, same behavior as the palette's tab-empty-state handling.
- **Row accessory:** a segmented-chip control — 2-5 preset choices rendered as adjacent chips in the row's `Accessory` slot, active choice highlighted (same visual family as `binding_chips`/keycaps: bordered, small-radius chips). Selecting a row and pressing Left/Right (or clicking a chip) cycles/sets the choice; the row commits immediately (see Persistence).
- **Entry points:** `Cmd+,` keybinding (added to `keymap.yaml`, following the same binding-declaration path as every other command) and a palette command "Open Settings".

Explicitly not built: a dedicated settings panel, sidebar, or full-page view. It is a modal like every other picker.

### Declaration

One static descriptor table, one file (`src/settings/descriptors.rs` or similar), modeled on sourcefour's `Choice<T>` pattern:

```rust
pub struct SettingDescriptor {
    pub id: &'static str,
    pub section: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub choices: &'static [(&'static str, Choice)],   // label, value — 2-5 entries
    pub read: fn(&EditorConfig) -> Choice,
    pub apply: fn(&mut EditorConfig, Choice),
}
```

(`Choice` here stands for a small per-setting value type or enum — the exact shape is an implementation detail; the load-bearing part is that `read`/`apply` round-trip through `EditorConfig` directly, no intermediate settings model.)

Search, sectioning, and row rendering all *derive* from this one table — `resolve_settings_rows(query, table)` is the ordering authority (same pattern as `resolve_palette_rows`), producing the sections/rows the view renders and the same list the input-cycle/click handler indexes into. `EditorConfig` remains the single authoritative struct; the table only describes how to present and mutate it.

**Rejected: macro-registry / registration-with-annotations.** An earlier direction considered a `#[setting(...)]`-style macro on `EditorConfig` fields, generating UI metadata at the field-declaration site. Rejected because Zed tried exactly this — UI annotations attached to settings structs — and walked it back because UI concerns (labels, choice sets, section names) leaked into crates that have no business knowing about UI (zed.dev/blog/settings-ui). A single external descriptor table keeps `EditorConfig` a plain config struct and keeps all UI concerns in one file that can be deleted or reworked without touching persistence code. Centralized strong typing (one table, checked against `EditorConfig` by the `read`/`apply` function signatures) won over convenience-at-declaration-site.

### Preset / File Asymmetry

The YAML file accepts any value `EditorConfig`'s `serde` layer accepts; the UI only ever offers the presets listed in a descriptor's `choices`. A hand-edited off-preset value:

- Is read correctly by the editor (serde doesn't care about the UI's opinions).
- Lights **no chip** in the settings row (none of the choices match) — the row shows no active selection rather than guessing or snapping to a nearest value.
- Never errors, never gets silently overwritten by opening the settings modal (no chip active means no chip write happens until the user picks one).

Examples from existing fields:

| Setting | Choices |
| --- | --- |
| `cursor_blink_ms` | Off (0) / Slow (1000) / Normal (600) / Fast (300) |
| `status_bar_font_size` | Small (11) / Medium (12) / Large (13) |
| `theme` | chips sourced from the theme registry, or a single row that opens the existing Theme Picker context |

Open-ended values that aren't preset-shaped — LSP server command overrides being the clearest example — are **read-only status rows**: they display the current value (or "default" if unset) and name the YAML key to hand-edit (`lsp.servers.<id>.command`). No text input is built for v1; this is what eliminates validation and error-state UI from the whole feature.

### Persistence

Immediate write on change, no save button — every chip selection calls `EditorConfig::save()` right after mutating the in-memory config, same as the theme picker does today.

**Hard prerequisite (Phase 1, shippable alone): unknown-key preservation in `EditorConfig::save()`.** Today's `save()` is `serde_yaml::to_string(self)` — a plain struct serialize that silently drops any YAML key the struct doesn't have a field for. That's tolerable when saves are rare; it is not tolerable once every settings click writes the file, because the first click would permanently delete any hand-written key, any section from a newer build this binary doesn't know about, or comments-adjacent structure. The fix: serialize the known struct to a `serde_yaml::Value`, then recursively copy back any key present in the *existing on-disk file* that the freshly-serialized value doesn't have — roughly 20 lines (read existing file → parse to `Value` → merge missing keys from old into new → write). This ships and is useful independent of the rest of the settings page and is ordered first in the implementation plan for that reason.

### Layering

None in v1: compiled defaults (`Default` impls on `EditorConfig`/`LspConfig`) plus the one user `config.yaml`, matching today's model exactly — no third layer, no override precedence to design.

Project-level `.token/config.yaml` is noted as a **future** third layer (mirroring the keymap's embedded-then-user structure, extended with a project tier). It is deliberately not built here because reset-to-default becomes a real problem the moment a third layer exists: with only "compiled default" and "user file", a value is either present in the user file (explicit) or absent (implicit default), and deleting the key *is* reset. With a project layer in between, "reset to default" needs to distinguish "never set" from "explicitly set to the default's value" — an `Option<Option<T>>`-shaped problem Zed also hit with its settings UI. Until a project layer exists, reset stays manual: delete the key by hand. No reset button is built in v1 (see Explicitly Out of Scope).

### LSP Section

- Master `lsp.enabled` toggle — one chip row (On/Off), maps directly to `LspConfig.enabled`.
- Per-server enabled chip, one row per entry in the compile-time server registry, reading/writing `LspConfig.servers.<id>.enabled` (default true when absent).
- Command override — read-only status row per server, showing the resolved command path and the YAML key (`lsp.servers.<id>.command`) to edit it by hand; no text input.
- Live server status — read-only status rows sourced from `LspUiState.servers: HashMap<LspServerId, ServerState>` (`src/model/mod.rs:437`), rendering `ServerState`'s `Starting | Indexing | Ready | Restarting{attempt} | Failed | Missing | ShuttingDown` as a dim status accessory next to each server's rows. This section reads live state; it does not restart or manage servers (no "restart server" action in v1).

A **Language Servers** picker modal, theme-picker-style, is being built separately as the interim surface for exactly this information (enable/disable, status). This LSP section is the eventual absorption point: once Settings ships, that standalone picker's rows fold into this section the same way the theme picker itself may eventually become a `theme` row's chip set plus an "open full picker" affordance. Not scoped to migrate in this doc's implementation plan — noted so the two aren't built as permanent parallel surfaces.

---

## Data Structures (sketch)

```rust
// One row per configurable setting. Table lives in one file; EditorConfig
// stays the persisted struct with no UI-facing annotations.
pub struct SettingDescriptor {
    pub id: &'static str,
    pub section: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub keywords: &'static [&'static str],
    pub choices: &'static [(&'static str, SettingValue)],
    pub read: fn(&EditorConfig) -> SettingValue,
    pub apply: fn(&mut EditorConfig, SettingValue),
}

// Small closed value type covering the preset shapes in use (u64 ms,
// f32 px, bool, String theme id, ...) — exact variants sized to what
// EditorConfig fields need, not a general-purpose config value type.
pub enum SettingValue {
    Bool(bool),
    U64(u64),
    F32(f32),
    Text(&'static str),
}

// Status-only rows (LSP command overrides, live server state) don't
// need a descriptor at all — they're rendered directly as Accessory::DimText
// rows in the LSP section, sourced from EditorConfig / LspUiState, no
// read/apply pair since they aren't editable from the UI.
```

`resolve_settings_rows(query: &str, table: &[SettingDescriptor], config: &EditorConfig) -> Vec<Section>` is the ordering authority: filters by fuzzy match over name+description+keywords, groups by `section`, and is the single function consumed by both the view's spec builder and the chip-select/commit handler — same discipline `overlay-surface.md` mandates for `resolve_palette_rows` (view-order == confirm-order, enforced by a unit test).

---

## Implementation Plan

### Phase 1: Unknown-key-preserving save (prerequisite, shippable alone)

**Effort:** S

- [x] `EditorConfig::save()`: serialize known settings and preserve unknown keys from the existing YAML. Comparing against the old typed configuration prevents restoring removed known options, server entries, and arbitrary LSP settings keys. Invalid/unreadable files fail before writing; comments and formatting are not retained.
- [x] Tests cover unknown top-level/nested keys, known-value changes, removed known options/map entries, malformed/unreadable files, and missing/empty files.
- [x] No UI changes in this phase; existing callers of `save()` (theme picker) are unaffected.

### Phase 2: Settings context on OverlaySurface

**Effort:** M

- [x] `SettingDescriptor` table covering the existing `EditorConfig` fields (theme, cursor_blink_ms, auto_surround, bracket_matching, show_scrollbar, status_bar_font_size) with 2-5 presets per editable row; theme opens the existing picker.
- [x] Segmented-chip `Accessory` rendering + Left/Right cycle and click-to-select input handling (new `ModalMsg` arms alongside the existing chip/row patterns).
- [x] `resolve_settings_rows` ordering authority; fuzzy search wired the same way as every other list context; sections hide when empty under a query.
- [x] Off-preset value handling: no chip lit when the config's current value doesn't match any choice.
- [x] Immediate-write-on-change wired to the now-safe `EditorConfig::save()`.
- [x] `Cmd+,` keybinding + "Open Settings" palette command.

### Phase 3: LSP section

**Effort:** S

- [x] `lsp.enabled` master toggle row and per-server enabled chip rows.
- [x] Read-only command-override rows (value + YAML key name).
- [x] Read-only live status rows sourced from `LspUiState.servers`.

### Phase 4 (Future): Keymap tab

**Effort:** M

- [ ] A Keymap tab alongside Settings' categories: merged binding list (embedded + user keymap.yaml, via the existing `merge_bindings`), rendered with `binding_chips`, same fuzzy search as the rest of Settings.
- [ ] Conflict detection between bindings.
- [ ] Chord-capture input mode for rebinding (record a keypress/chord in place of typing a binding string).
- [ ] Rebinding writes overrides to the user `keymap.yaml`, not `config.yaml`.
- [ ] Steal Zed's `base_keymap`-as-a-setting idea: a preset row for switching base keymap style, same chip mechanism as every other setting.

**Explicitly out of scope, forever or for now:** settings sync, import from other editors, JSON/YAML schema files, per-setting reset buttons, live file-watch config reload.

---

## Testing Strategy

- Unit test for the unknown-key-preserving `save()` (Phase 1) — the single highest-value test in this doc, since it's a data-loss guard.
- Unit test asserting `resolve_settings_rows`' view order matches the row the chip-commit handler mutates, for a query that filters multiple sections (same shape as `overlay-surface.md`'s view-order == confirm-order test).
- Unit test per descriptor: `apply(read(config))` round-trips to the same `SettingValue` for each of its own choices (catches read/apply mismatches at the table level, not just at runtime).
- Off-preset behavior test: a config value not present in any choice list renders with no chip active and is left untouched by opening/closing Settings without interacting with that row.
- Manual check: LSP section status rows reflect a live `LspUiState` transition (e.g. Starting → Ready) without requiring the modal to be closed and reopened.

---

## Acceptance Criteria

- [x] Opening Settings (`Cmd+,` or palette) shows all sections; typing filters across name/description/keywords with no dead sections shown.
- [x] Changing a chip writes `EditorConfig::save()` immediately; no save button exists.
- [x] A config value written by hand that isn't one of a setting's presets shows no active chip and is never overwritten by opening Settings.
- [x] Unknown YAML keys and their values survive settings-page saves. Comments and formatting are not retained.
- [x] LSP section shows `lsp.enabled`, per-server enabled state, command-override values (read-only), and live status per server.
- [x] No setting-value text input, numeric input, or validation UI exists; the header input is only for search.

---

## Design Decisions

| Decision | Rationale |
| --- | --- |
| Context on `OverlaySurface`, not a new surface | Reuses fuzzy search, sectioned lists, chip accessories, ordering-authority pattern — no new rendering code |
| Static descriptor table, not macro/registration annotations | Zed tried registration-with-UI-annotations and walked it back because UI concerns leaked into non-UI crates (zed.dev/blog/settings-ui); one table keeps `EditorConfig` UI-agnostic |
| Presets only, no free text | Eliminates validation and error-state UI entirely for v1; off-preset values still work via hand-edited YAML |
| Off-preset value = no chip lit, never overwritten | Matches sourcefour's asymmetry: the file is more expressive than the UI, and the UI must never clobber what it can't represent |
| Immediate write on change, no save button | Matches the existing theme picker's behavior; consistent mental model across all config-mutating modals |
| Unknown-key-preserving `save()` is Phase 1, not a footnote | Without it, the first settings click is a data-loss bug; it also stands alone as a fix worth shipping regardless of the rest of this doc |
| No project-level config layer in v1 | Reset-to-default needs "unset vs. explicitly set" the moment a third layer exists (Zed's lesson); deferred until that's designed |
| LSP command overrides are read-only status rows | Command paths are open-ended, not preset-shaped; a text input would reintroduce validation/error UI this doc explicitly avoids |
| Keymap tab is Phase 2/Future, not v1 | Rebinding needs chord-capture input mode and conflict detection — separable, larger scope than preset chip toggles |

---

## Open Questions

- Narrow-window layout is resolved for v1: chip slots shrink and labels truncate; chips remain individually clickable.
- Should "Open Settings" support a query-string entry point (`Cmd+,` then jump straight to a section, e.g. from a status-bar click on the LSP status segment) — deferred to whenever the LSP status segment itself needs a destination.
- Theme is resolved for v1: display the current theme and open the existing Theme Picker from its row.

---

## References

- [Zed: Configuring Zed](https://zed.dev/docs/configuring-zed)
- [Zed: Settings UI blog post](https://zed.dev/blog/settings-ui) — source of the registration-with-annotations rejection rationale
- [overlay-surface.md](../archived/overlay-surface.md) — the component this context is built on; spec/ordering-authority/behaviour patterns reused verbatim
- Theme picker (`docs/feature/overlay-surface.md` Contexts table; `src/view/modal.rs` theme picker rendering) — reference pattern for a config-mutating list modal
- `~/code/sourcefour` (`settings.rs`, `settings_ui.rs`, `persist.rs`) — studied prior art for the `Choice<T>` descriptor pattern and the preset/file asymmetry
