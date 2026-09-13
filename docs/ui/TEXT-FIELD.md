# Text field — implementation reference

Token text fields are deliberately split rather than a monolithic event widget.
EditableState<StringBuffer> owns value, cursor, selection, constraints and
history; the consuming feature owns focus, drag capture and submit policy;
TextFieldRenderer projects a borrowed snapshot using caller-derived geometry.
This lets Settings, Find, modal prompts and CSV editing share text mechanics
without letting a renderer mutate configuration or decide what Enter means.

## Current representation

**Current excerpt** — [editable/state.rs](../../src/editable/state.rs#L13):

```rust
pub struct EditableState<B: TextBuffer> {
    pub buffer: B,
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub active_cursor: usize,
    pub constraints: EditConstraints,
    history: EditHistory,
}
```

StringBuffer is the durable mutable field value. Cursor and selection positions
are logical Position { line, column }: column is a Unicode-character column in
this renderer, not a byte position or a pixel. Cursor and selection vectors are
parallel; active_cursor indexes both. Private history prevents callers from
forging undo state. EditConstraints owns multiline, selection, multi-cursor,
undo, maximum-character and character-filter policy. Most fields use
single-line constraints; open-ended Settings fields opt into multiline.

**Current excerpt** — [text_field.rs](../../src/view/text_field.rs#L16):

```rust
pub struct TextFieldOptions {
    pub x: usize, pub y: usize, pub width: usize, pub height: usize,
    pub char_width: f32,
    pub text_color: u32, pub cursor_color: u32, pub selection_color: u32,
    pub cursor_visible: bool,
    pub scroll_x: usize, pub scroll_y: usize, pub rows: usize,
}
```

Options are an owned, Cloneable derived presentation value built by the caller;
TextFieldRenderer borrows it for a single paint/caret calculation. It is never
durable editable state. Position, width and height are physical pixels; height
is one rendered text row rather than outer field height. Scroll x is character
columns, scroll y logical lines. Colors and blink visibility arrive from
Settings, Find, or a modal because the renderer has no focus state.

The renderer borrows only this content interface:

```rust
pub trait TextFieldContent {
    fn text(&self) -> &str;
    fn is_multiline(&self) -> bool { false }
    fn cursors(&self) -> &[Cursor];
    fn selections(&self) -> &[Selection];
    fn active_cursor_index(&self) -> usize;
}
```

EditableState lends text and slices—rendering does not clone field state. The
trait enables test fixtures, not a new command framework.

### Ownership and repair invariants

| Data                                | Owner         | Invariant and repair                                             |
| ----------------------------------- | ------------- | ---------------------------------------------------------------- |
| buffer, constraints, history        | EditableState | edit operations enforce constraints/history                      |
| cursors/selections                  | EditableState | at least one; active index valid; setter clamps line/column      |
| focus, drag, submit/cancel          | feature owner | Settings/Find end capture; renderer never guesses                |
| options/colors/blink                | render caller | owned Cloneable projection; recompute from layout/current cursor |
| label, surface, placeholder, errors | caller        | renderer intentionally paints none                               |
| field and parent clips              | caller        | required before rendering long/scrolled content                  |

Construct state through EditableState::new, not a struct literal. Movement
remembers desired column vertically and clamps to buffer line length;
set_cursor_position clamps both line and column. A feature replacing text must
use its state API so active positions cannot survive beyond a shortened buffer.
The renderer relies on these conditions when indexing active cursor data.

## Geometry derivation

### Single-line projection

for_text_box computes:

```text
visible_chars = ceil(rect.w / char_width) + 1
cursor_col    = active_cursor.column, or 0
scroll_x      = calculate_scroll(cursor_col, 0, visible_chars)
x             = rect.x
y             = rect.y + (rect.h - line_height) / 2
height        = line_height
rows          = 1
```

for_modal first insets horizontally using ModalSpacing::input_pad_x(scale).
Callers whose rectangle already represents text use for_text_box, avoiding a
second inset. For a 96 px inner rect, 8 px grid, and cursor column 20,
visible_chars = ceil(96/8)+1 = 13. With two-column margin, scroll becomes
20 - (13 - 3) = 10; caret x is x + round((20-10)*8) = x+80. At column 1,
saturating subtraction returns zero rather than underflowing.

**Current algorithm** — [text_field.rs](../../src/view/text_field.rs#L368):

```rust
fn calculate_scroll(cursor_col: usize, scroll_x: usize, visible_chars: usize) -> usize {
    let margin = 2;
    if cursor_col < scroll_x + margin {
        cursor_col.saturating_sub(margin)
    } else if cursor_col >= scroll_x + visible_chars.saturating_sub(margin) {
        cursor_col.saturating_sub(visible_chars.saturating_sub(margin + 1))
    } else {
        scroll_x
    }
}
```

The current caller passes zero as previous scroll. A future persistent
horizontal viewport must explicitly supply its old offset; it cannot assume
renderer state. Measured Code-font char width must be positive: position_at
defensively uses max(1.0), while for_text_box divides by supplied width.

### Multiline and clipped parent projection

for_text_area handles a row that has scrolled partly above its parent:

```text
line_height = max(line_height, 1)
rows        = max(floor(max(rect.height,0) / line_height), 1)
skipped     = ceil(max(-rect.y / line_height, 0))
y           = max(rect.y + skipped*line_height, 0)
scroll_y    = cursor_line.saturating_sub(rows - 1) + skipped
rows        = rows.saturating_sub(skipped)
```

Example: y=-18, height=72, line height=20, cursor line=7. Nominal rows are
3; skipped=1; paint starts at y=2; scroll_y=7-(3-1)+1=6; two rows remain.
Cursor line 7 appears at y=22 inside the visible region. If skipped consumes
all rows, rows becomes zero and paint is empty; the parent clip remains required.

Pointer mapping and caret calculation are inverse views of the same options:

```text
line = scroll_y + floor(max(pointer_y-y,0) / max(height,1))
col  = scroll_x + round(max(pointer_x-x,0) / max(char_width,1))
caret_x = clamp(x + round((cursor_col-scroll_x)*char_width), x, x+width-caret_w)
```

With x=100, scroll_x=10, char width 7.5 and pointer x=126, the pre-clamp
column is 10 + round(26/7.5) = 13. State clamps it to actual line length.
The right-edge caret clamp means the final caret remains visible despite
rounding. Never duplicate these calculations in input code.

This is character-grid projection, not shaped typography. Tabs render as one
space; columns use chars, so emoji width, combining marks, tab stops and bidi
are documented limits. Current Code-font input use makes that trade-off
explicitly scoped.

## Paint algorithm, clipping, and transitions

**Current algorithm sketch** — [text_field.rs](../../src/view/text_field.rs#L226):

```text
split text into one line or logical lines
skip scroll_y; take rows
for each displayed line:
  paint each intersecting nonempty selection before glyphs
  subtract scroll_x; clamp selection width to field width
  take visible characters, map tab to space, paint in Code font
paint each visible cursor afterwards; dim non-active cursors
```

A selection crossing lines begins at zero on interior lines and reaches
line.chars().count()+1, allowing the wash through the newline region. The
renderer clips selection width but glyphs require caller Frame::push_clip; a
long line otherwise can overpaint neighbouring UI.

Settings paints render_field_surface, derives field options from the same
settings row, supplies overlay colors/blink, clips input and calls renderer.
Find uses render_modal_input within FindBarLayout's field rectangle. Labels and
placeholders use UI font; editable content uses Code font.

| Event                       | Current action                            | Result                       |
| --------------------------- | ----------------------------------------- | ---------------------------- |
| focus                       | owner changes focused setting/Find field  | caller enables blink         |
| press                       | owner maps point through the same options | set/clamp cursor             |
| Shift press or drag         | owner passes extend=true                  | anchor retained, head moves  |
| Find double/triple click    | update selects word/all                   | next frame paints selection  |
| typed text or edit key      | runtime classifies; owner dispatches edit | buffer/history mutate        |
| undo/redo                   | owner calls state; Settings marks dirty   | next frame                   |
| release, Escape, focus loss | owner ends capture                        | renderer holds no capture    |
| Enter, Tab, Escape          | container policy                          | no renderer-specific command |

Settings uses SettingsMsg::FieldPointer with logical Position. Find passes a
computed column. Update is consequently deterministic and tests need not depend
on device scale.

## Invalidation and cost

Renderer allocates one visible String per displayed line after character
skip/take. Cost is proportional to displayed text, visible lines, and
selection/cursor intersections. Splitting/skipping is not a rope viewport
algorithm; ordinary form and Find fields are one line. No frame-time claim
follows.

Rebuild options on outer rect, scale padding, line/character metrics, active
cursor, multiline text/parent scroll, colors, or blink. Repaint surface for
focus/theme changes. Renderer caches nothing: it has no owner identity with
which to invalidate safely. A parent layout cache must invalidate on its
viewport clipping change, not merely text mutation.

## Proposed semantic adapter (not implemented)

```rust
// Proposed names shared by FORM.md and COMBOBOX.md only.
use std::sync::Arc;

use crate::editable::{EditableState, MoveTarget, Position, StringBuffer};
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldId(pub Arc<str>);
pub enum FieldMode { Editable, ReadOnly, Disabled }
pub enum TextOperation {
    Insert(String),
    DeleteBackward,
    Move { target: MoveTarget, extend: bool },
    SelectAll,
}
pub enum CompositionEvent {
    Start { replacement: std::ops::Range<Position> },
    Update { text: String },
    Commit { text: String },
    Cancel,
}
pub struct TextFieldView<'a> {
    id: FieldId,
    label: &'a str,
    description: Option<&'a str>,
    editable: &'a EditableState<StringBuffer>,
    mode: FieldMode,
    placeholder: Option<&'a str>,
    invalid_message: Option<&'a str>,
}
enum TextFieldIntent {
    Focus { id: FieldId, select_all: bool },
    Pointer { id: FieldId, position: Position, extend: bool },
    Edit { id: FieldId, operation: TextOperation },
    Composition { id: FieldId, event: CompositionEvent },
    Submit { id: FieldId },
    Cancel { id: FieldId },
}
```

It emits intent only; no command/config/async validation enters rendering.
Disabled is unfocusable/inert; read-only remains focusable/selectable/copyable
but rejects editing; invalid supplies explicit visual and accessible description.
Token currently has neither these semantic states nor platform accessibility
tree support.

IME needs transient preedit text/range and a candidate anchor from caret_rect.
Only committed text enters editable history; focus loss/session replacement
cancels preedit so a late platform callback cannot edit a different field.

## Verification vectors

Existing scroll unit tests are in [text_field.rs](../../src/view/text_field.rs#L424).
Retain/add the following:

| Setup                              | Input            | Expected                                   |
| ---------------------------------- | ---------------- | ------------------------------------------ |
| width 96, char 8, cursor 20        | for_text_box     | visible 13, scroll 10, caret x+80          |
| cursor 1, scroll 0, visible 13     | calculate scroll | zero, no underflow                         |
| x 100, scroll 10, char 7.5         | pointer x 126    | logical column 13 before clamp             |
| y -18, h 72, line 20, cursor 7     | text area        | y=2, scroll_y=6, rows=2                    |
| selection (0,2)..(0,7), scroll_x 4 | paint            | wash starts x, is three columns            |
| selection across ab/newline/cd     | paint            | second line starts at column zero          |
| zero-width field                   | caret            | safe clamped caret; no panic               |
| single-line a/newline/b            | render           | first line only; insertion rejects newline |
| future preedit then focus loss     | late commit      | original/history unchanged                 |

Repeat 1×, 1.25× and 2× fractional character widths; test combining/emoji as
known limits until grapheme-aware shaping replaces this grid.

## Sources

- [Editable state](../../src/editable/state.rs), [constraints](../../src/editable/constraints.rs)
- [Options and renderer](../../src/view/text_field.rs)
- [Field surfaces](../../src/view/controls.rs)
- [Settings composition](../../src/view/settings_page.rs)
- [Find composition](../../src/view/find_bar.rs)
