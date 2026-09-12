# Dialog

## Purpose and naming

A **dialog** is an application-blocking decision or bounded workflow. In Token
it is called a _modal_ in code; retain “dialog” in UI vocabulary and reserve
“modal” for its input/capture behavior. IntelliJ describes dialogs as surfaces
requiring an action before proceeding ([UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html));
its `DialogWrapper` guidance also establishes Escape, preferred initial focus,
validation, and platform button ordering as useful reference behavior
([Dialogs](https://plugins.jetbrains.com/docs/intellij/dialog-wrapper.html)).

## Current implementation — high confidence

`UiState::active_modal: Option<ModalState>` is the single-modal invariant.
`ModalState` has Unsaved Changes, File Conflict, Settings, Command Palette,
Go To Line, Theme Picker, File Finder, Recent Files, Language Servers,
Set Language, and Rename Symbol variants. `FocusTarget::Modal` marks top-level
keyboard ownership. Modal-specific input uses `ModalMsg`; opening/closing and
transitions live in `update/ui.rs`; effects are returned as `Cmd`, preserving
the Message → Update → Command architecture.

`view::modal` maps each variant to a shared `OverlaySpec`; `OverlaySurface`
renders and lays out it. Centered modals dim the backdrop. Settings is a
deliberate exception: it uses `Anchor::Settings` and stays a preferences page
with category navigation, not a command palette. The current renderer supports
fields, lists, tabs, zones, footer, form choices and scrollbar geometry—not a
universal dialog button component.

| Model                          | Current transitions                                                          |
| ------------------------------ | ---------------------------------------------------------------------------- |
| `active_modal` + variant state | `UiMsg::ToggleModal`, `UiMsg::Modal(ModalMsg)`                               |
| editable/list fields           | text edit, clipboard, selection, rows, tabs, scroll, confirm, close          |
| effects                        | update validates/chooses; runtime performs file/LSP/configuration commands   |
| pointer state                  | hover row/choice, settings actions, close hover, scrollbar drag in `UiState` |

### Implemented data ownership (field-level)

| Variant/shared field                                                                                | Created and mutated by                                               | Rendering/commit authority                                                     |
| --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| [`active_modal: Option<ModalState>`](../../src/model/ui.rs)                                         | `update::ui::update_ui`; only one variant exists                     | `render_modals` and `hit_test_modal`; gates modal input before editor input    |
| `CommandPaletteState`                                                                               | palette update builds cached command/file/symbol rows and active tab | cached order drives spec, keyboard selection and confirmation                  |
| `GotoLineState`, `RenameSymbolState`                                                                | modal update; rename captures caret/request context before opening   | editable draft is read by confirmation, never mutated by painter               |
| `ThemePickerState`, `FileFinderState`, `RecentFilesState`, `LanguagePickerState`, `LspServersState` | feature updater constructs filter/results/selection                  | row action indexes the same variant state rendered in `OverlaySpec`            |
| `SettingsState`                                                                                     | settings update owns category/form draft/selections/async sessions   | `Anchor::Settings`/field layout borrow draft; apply remains feature validation |
| [`modal_hover_row`, `modal_hover_choice`, `modal_close_hovered`](../../src/model/ui.rs)             | pointer route                                                        | visual/hit feedback only; never replaces keyboard selection                    |

`ModalState::id()` is the identity bridge for `ToggleModal`, not a generic
factory. Unsaved Changes/File Conflict are constructed only by file-change
paths because their payload represents a concrete pending operation.

### Input, dismissal, lifecycle

Opening replaces a different active modal; toggling the same id closes it.
Modal input supports cursor/word/selection movement, clipboard operations,
list and page navigation, Enter confirmation, tab navigation, pointer row
activation and scroll. Escape maps to close where the active handler permits.
The modal captures input before editor editing; its state is discarded or
saved-as-last-state according to the feature (for example palette/find reuse).
Unsaved-change and conflict dialogs are opened only for concrete file intents,
not by `ToggleModal`. Source tests in `view/modal.rs`, `update/ui.rs`, and
`runtime/app_tests.rs` cover key routing and selected-row consistency.

| Lifecycle transition (implemented) | Owner                                                          | Required invariant                                                   |
| ---------------------------------- | -------------------------------------------------------------- | -------------------------------------------------------------------- |
| open a `ModalId`                   | `update_ui` constructs its variant state                       | only one `active_modal`; Settings retains its page design            |
| edit/navigate/scroll               | `ModalMsg` handler and variant draft                           | view and confirm index the same cached order/field state             |
| pointer row/tab/choice             | hit test uses `OverlayLayout`; update receives indexed message | no separately derived geometry or order                              |
| confirm                            | feature-specific update                                        | validate/derive effect before returning `Cmd`                        |
| close/cancel or replacement        | `UiState::close_modal`/toggle path                             | clear temporary capture and set focus to `Editor` (current behavior) |

Token has no native screen-reader semantic tree, focus trap enumeration,
announced validation error, remembered dialog size, or OS-standard button
ordering. Those are **not implemented**; IntelliJ's corresponding behavior is
reference guidance, not evidence that Token has it.

### Geometry and appearance

Centered dimensions use `WidthRule` and `layout::anchor`, with clamping to
window margins; backdrop dim alpha is carried by the anchor. Overlay constants
are logical pixels scaled and rounded: 10 radius, 16 horizontal header padding,
12 vertical panel padding, 30-height rows/footer. `OverlayTheme` owns chrome,
text, selection, accents, borders and panel tones. `TextPainter` switches UI
and code font roles; all drawing is clipped through `Frame`.

#### Measured-layout and modality invariants — implemented

- [`with_modal_overlay_layout`](../../src/view/modal.rs) creates the same
  shape-only spec/layout used by [`hit_test_modal`](../../src/view/hit_test.rs);
  renderer and pointer routing must not derive separate row/form geometry.
- `OverlayLayout` holds visible rows (including unselectable headers), fields,
  tabs, choices, scrollbars and docs. `FlatIndex` excludes headers, so keyboard
  navigation and click activation cannot use display-row offsets.
- Centered width resolves through `WidthRule` then clamps by window margins;
  centered Y and dim alpha come from shared `Anchor::Centered`. Settings uses
  `Anchor::Settings`, not palette geometry.
- Rendering measures with glyph cache and clips panel/scroll viewport. List and
  form scroll are update state; documentation expansion clears a stale scrollbar
  capture before geometry changes.

## Proposed contract

Keep the existing sum type rather than adding a general `Dialog` trait:

```text
DialogState = specific workflow draft + immutable opening context
DialogEvent = Edit | Navigate | Choose | Confirm | Cancel | Scroll
update(DialogEvent) -> (new state, Cmd)
```

Each new variant must state: initial focus; whether cancel is safe; validation
and inline error state; default/primary action; which data is committed only on
confirm; late-result cancellation; and focus restoration. Required actions use
a dialog; contextual optional actions use a popup; settings remain Settings.

## Gallery and acceptance

There is no full dialog specimen. Gallery coverage is indirect: fields,
validation, search field, list/menu rows, overlay tabs, scrollbar, and panel
surfaces ([catalog](../../src/model/gallery.rs)). Add a fixture for a centered
confirmation and a long form only when modifying shared dialog behavior.

Acceptance: one active dialog; no application effects in painter/hit test;
Enter/Escape/click semantics and focus restoration tested; fields and lists
share layout with hit testing; validation stays visible at 1x/HiDPI/light/dark;
small-window clipping/scrolling works; opening data and stale async replies
cannot commit to another document/workflow.

Concrete workflow acceptance: filter Search Everywhere until one row remains,
switch tabs with unavailable tabs present, scroll without changing selection,
click an active row, reopen and confirm with Enter; every route must resolve the
same command identity. Separately open Settings with a long form at 200%, drag
its scrollbar, edit, close/cancel, and verify a pending form reply cannot alter
a later Settings session. For file conflict/rename, confirmation must still
target the originally captured document/caret context.

## Evidence

- [Modal and focus state](../../src/model/ui.rs), [events](../../src/messages.rs), [update](../../src/update/ui.rs), [renderer/spec mapping](../../src/view/modal.rs)
- [Shared overlay surface](../../src/view/overlay_surface.rs), [gallery](../../src/model/gallery.rs)
- [Local IntelliJ SDK UI/dialog reference](../../temporary-docs/intellij-platform-sdk/references/ui-settings-and-toolwindows.md) (secondary; terminology cross-checked against official docs)
- [IntelliJ dialogs](https://plugins.jetbrains.com/docs/intellij/dialog-wrapper.html) and [UI overview](https://plugins.jetbrains.com/docs/intellij/ui-overview.html)
