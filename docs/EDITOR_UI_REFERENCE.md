# Implementer's Guide to Modern Text Editor UI Geometry, Mathematics, Edge Cases, and Problems

A comprehensive reference for building scrollable text editing interfaces with tabs, split panels, soft wrapping,
autocomplete overlays, and all the coordinate transformations in between.

> **Target Audience:** Desktop software developers building code editors in Rust, Go, C++, or similar languages.
> This guide assumes monospace fonts and left-to-right text direction. Web-specific concerns (DOM, CSS) are
> mentioned only where relevant for conceptual understanding.

---

## Table of Contents

1. [Foundational Concepts: The Scrollable Region](#chapter-1-foundational-concepts-the-scrollable-region)
2. [Structural Hierarchy: Tabs, Splits, and Editor Groups](#chapter-2-structural-hierarchy-tabs-splits-and-editor-groups)
3. [The Gutter System](#chapter-3-the-gutter-system)
4. [Viewport Geometry and Line Calculations](#chapter-4-viewport-geometry-and-line-calculations)
5. [Cursor-Viewport Interaction Model](#chapter-5-cursor-viewport-interaction-model)
6. [Soft Wrapping: The Coordinate System Split](#chapter-6-soft-wrapping-the-coordinate-system-split)
7. [Autocomplete and Overlay Positioning](#chapter-7-autocomplete-and-overlay-positioning)
8. [Syntax Highlighting Integration](#chapter-8-syntax-highlighting-integration)
9. [The Complete Render Pipeline](#chapter-9-the-complete-render-pipeline)
10. [Edge Cases and Special Considerations](#chapter-10-edge-cases-and-special-considerations)
11. [Naming Conventions and Terminology](#chapter-11-naming-conventions-and-terminology)
12. [Cursor Styles: Pipe, Block, and Underline](#chapter-12-cursor-styles-pipe-block-and-underline)

**Appendices:**

- [A: Event Flow Example](#appendix-a-event-flow-example)
- [B: Coordinate Transformation Functions](#appendix-b-coordinate-transformation-functions)
- [C: Binary Search Helpers](#appendix-c-binary-search-helpers)
- [D: Useful Constants and Defaults](#appendix-d-useful-constants-and-defaults)
- [E: Unicode and UTF-8 Considerations for Code Editors](#appendix-e-unicode-and-utf-8-considerations-for-code-editors)
- [F: Right-to-Left and Bidirectional Text Theory](#appendix-f-right-to-left-and-bidirectional-text-theory)

---

## Chapter 1: Foundational Concepts: The Scrollable Region

### 1.1 Core Entities

At the most fundamental level, a scrollable region consists of these conceptual components:

#### Content (Document / Scrollable Content)

The full extent of what _could_ be displayed. Has its own coordinate system, often called **content coordinates** or
**document coordinates**. For a text editor, this might be 50,000 pixels tall even though only 800 pixels are visible at
any moment.

#### Viewport (View / Visible Region)

The rectangular "window" through which you see a portion of the content. Has a fixed size (width × height) and its own
coordinate system where (0,0) is typically the top-left corner of what's currently visible.

#### Scroll Offset (Scroll Position)

The translation between content coordinates and viewport coordinates. Typically expressed as `(scrollX, scrollY)` — how
far into the content the viewport's top-left corner is positioned.

#### Scrollable Extent (Scroll Range / Scroll Bounds)

The maximum distance the content can scroll. Always clamped to non-negative values:

```
scrollableHeight = max(0, contentHeight - viewportHeight)
scrollableWidth  = max(0, contentWidth - viewportWidth)
```

When `contentHeight <= viewportHeight`, there is no vertical scroll (scrollableHeight = 0).

### 1.2 The Scrollbar Anatomy

The **scrollbar** is the entire control assembly, typically containing:

| Part           | Alternative Names                | Purpose                                          |
| -------------- | -------------------------------- | ------------------------------------------------ |
| **Track**      | Gutter, Trough, Lane             | The full channel the thumb travels along         |
| **Thumb**      | Handle, Knob, Scrubber, Elevator | The draggable indicator of current position      |
| **Buttons**    | Arrows, Steppers                 | Optional increment/decrement buttons at ends     |
| **Page Zones** | Gutter regions                   | Clickable areas above/below thumb for page jumps |

### 1.3 Key Mathematical Relationships

#### Thumb Size (Proportional Scrollbars)

```
thumbLength / trackLength = viewportSize / contentSize
```

The thumb visually represents what fraction of content is visible.

**Edge case:** When `contentSize <= viewportSize`, either hide the scrollbar entirely or set `thumbLength = trackLength`
(thumb fills the track, indicating no scrolling possible).

#### Scroll Position ↔ Thumb Position

```
// Only valid when scrollableExtent > 0 and trackLength > thumbLength
scrollOffset / scrollableExtent = thumbPosition / (trackLength - thumbLength)
```

**Edge cases to handle:**

- `scrollableExtent == 0`: No scrolling possible, fix `scrollOffset = 0`, `thumbPosition = 0`
- `trackLength == thumbLength`: Thumb fills track, fix `thumbPosition = 0`

```
function scrollOffsetToThumbPosition(scrollOffset, scrollableExtent, trackLength, thumbLength) {
    if (scrollableExtent <= 0 || trackLength <= thumbLength) {
        return 0
    }
    return (scrollOffset / scrollableExtent) * (trackLength - thumbLength)
}

function thumbPositionToScrollOffset(thumbPosition, scrollableExtent, trackLength, thumbLength) {
    if (scrollableExtent <= 0 || trackLength <= thumbLength) {
        return 0
    }
    return (thumbPosition / (trackLength - thumbLength)) * scrollableExtent
}
```

#### Content-to-Viewport Transformation

```
viewportY = contentY - scrollOffsetY
contentY  = viewportY + scrollOffsetY
```

### 1.4 Coordinate Systems

When implementing a scrollable region, you're juggling multiple coordinate systems:

1. **Content/Document coordinates** — Position within the full scrollable content (pixels)
2. **Viewport/Client coordinates** — Position within the visible rectangle (pixels)
3. **Screen/Absolute coordinates** — Position on the physical display (pixels)
4. **Scrollbar-local coordinates** — For hit-testing thumb, track, and buttons (pixels)
5. **Document coordinates** — Position within text (line number, column number)

### 1.5 Visual Model

```
┌─────────────────────────────────────────┐
│            Content (Document)           │
│  ┌───────────────────────────────────┐  │
│  │                                   │  │
│  │     (content above viewport)      │  │
│  │                                   │  │
│  ├───────────────────────────────────┤◄─┼── scrollOffsetY
│  │ ┌───────────────────────────────┐ │  │
│  │ │                               │ │  │   ┌─────┐
│  │ │         VIEWPORT              │ │  │   │ ▲   │
│  │ │      (visible region)         │ │  │   ├─────┤
│  │ │                               │ │  │   │ ░░░ │◄─ thumb
│  │ │                               │ │  │   │ ░░░ │
│  │ └───────────────────────────────┘ │  │   ├─────┤
│  │     (content below viewport)      │  │   │     │◄─ track
│  │                                   │  │   │     │
│  └───────────────────────────────────┘  │   │ ▼   │
│                                         │   └─────┘
└─────────────────────────────────────────┘
```

---

## Chapter 2: Structural Hierarchy: Tabs, Splits, and Editor Groups

Modern editors like VS Code, JetBrains IDEs, and Fleet use a hierarchical layout system for managing multiple files and
views.

### 2.1 High-Level Structure

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Window                                                                  │
│ ┌─────────────────────────────────────────────────────────────────────┐ │
│ │ Editor Area                                                         │ │
│ │ ┌─────────────────────────────────┬───────────────────────────────┐ │ │
│ │ │ Editor Group (Left)             │ Editor Group (Right)          │ │ │
│ │ │ ┌─────────────────────────────┐ │ ┌───────────────────────────┐ │ │ │
│ │ │ │ Tab Bar                     │ │ │ Tab Bar                   │ │ │ │
│ │ │ │ [file1.ts][file2.ts]        │ │ │ [file3.ts]                │ │ │ │
│ │ │ └─────────────────────────────┘ │ └───────────────────────────┘ │ │ │
│ │ │ ┌─────────────────────────────┐ │ ┌───────────────────────────┐ │ │ │
│ │ │ │ Editor Pane                 │ │ │ Editor Pane               │ │ │ │
│ │ │ │ ┌──────┬──────────────────┐ │ │ │                           │ │ │ │
│ │ │ │ │Gutter│ Text Area        │ │ │ │                           │ │ │ │
│ │ │ │ │      │                  │ │ │ │                           │ │ │ │
│ │ │ │ │  1   │ const foo = ...  │▓│ │ │                           │ │ │ │
│ │ │ │ │  2   │ function bar()   │▓│ │ │                           │ │ │ │
│ │ │ │ │  3   │   return x;      │ │ │ │                           │ │ │ │
│ │ │ │ │      │                  │ │ │ │                           │ │ │ │
│ │ │ │ └──────┴──────────────────┘ │ │ │                           │ │ │ │
│ │ │ └─────────────────────────────┘ │ └───────────────────────────┘ │ │ │
│ │ └─────────────────────────────────┴───────────────────────────────┘ │ │
│ └─────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Entity Definitions

#### Document / Buffer

The underlying text content, independent of how it's displayed:

```
Document {
    content: Text               // rope, piece table, gap buffer, etc.
    uri: Identifier             // file path or unique identifier
    language: LanguageId        // for syntax highlighting
    version: Number             // increments on edit
    lineCount: Number

    // Computed/cached
    lineLengths: Array<Number>  // length of each line in columns
    lineOffsets: Array<Number>  // byte offset where each line starts (UTF-8 bytes)
}
```

> **Note on offsets:** `lineOffsets` stores byte positions in the UTF-8 encoded buffer. For column-based operations
> (cursor positioning, selection), use column indices. See [Appendix E](#appendix-e-unicode-and-utf-8-considerations-for-code-editors)
> for details on the distinction between bytes, codepoints, and columns.

A document knows nothing about viewports, cursors, or scroll positions. It is purely the data model.

#### Editor State / View State

Per-view state for a document. Multiple views can share one document:

```
EditorState {
    document: Document

    // Cursor & Selection
    cursors: Array<Cursor>          // multi-cursor support
    selections: Array<Selection>    // each cursor may have a selection

    // Scroll (in pixels)
    scrollOffset: { x: Number, y: Number }

    // View-specific
    foldedRanges: Array<Range>
    softWrapping: Boolean

    // Behavioral state
    scrollMode: 'cursor-locked' | 'free-browse'
    lastScrollSource: 'user' | 'programmatic'
}
```

#### Cursor

```
Cursor {
    line: Number                // 0-indexed line number
    column: Number              // 0-indexed column (character position)
    affinity: 'left' | 'right'  // which side of a character it's "attached" to
    preferredColumn: Number     // remembered column for vertical movement
}
```

The `preferredColumn` is crucial for intuitive vertical navigation. It stores the **column index** (not pixels) that
the cursor should return to when possible.

**Example:** You're at column 50, move down to a line with only 30 characters (cursor snaps to column 30), then move
down again to a line with 100 characters. The cursor should return to column 50 because that's the `preferredColumn`.

This is especially valuable when navigating through code with varying indentation levels or through blocks of code
with different line lengths.

```
function moveCursorDown(cursor, document) {
    const nextLine = cursor.line + 1
    if (nextLine >= document.lineCount) return cursor

    const nextLineLength = document.lineLengths[nextLine]
    const targetColumn = cursor.preferredColumn ?? cursor.column

    return {
        line: nextLine,
        column: min(targetColumn, nextLineLength),
        affinity: cursor.affinity,
        preferredColumn: targetColumn  // preserve the original target
    }
}

function moveCursorHorizontally(cursor, delta, document) {
    // Horizontal movement resets preferredColumn
    const newColumn = cursor.column + delta
    return {
        line: cursor.line,
        column: clamp(newColumn, 0, document.lineLengths[cursor.line]),
        affinity: cursor.affinity,
        preferredColumn: null  // reset on horizontal movement
    }
}
```

#### Selection

A selection is defined by two positions: where it started and where the cursor currently is:

```
Selection {
    start: Position     // the lesser position (earlier in document)
    end: Position       // the greater position (later in document)
    direction: 'forward' | 'backward'  // indicates which end has the cursor
}
```

- When `direction == 'forward'`: cursor is at `end`, anchor is at `start`
- When `direction == 'backward'`: cursor is at `start`, anchor is at `end`

Helper functions for working with selections:

```
function selectionHead(selection) {
    return selection.direction == 'forward' ? selection.end : selection.start
}

function selectionAnchor(selection) {
    return selection.direction == 'forward' ? selection.start : selection.end
}

function normalizeSelection(anchor, head) {
    if (positionLessThan(anchor, head)) {
        return { start: anchor, end: head, direction: 'forward' }
    } else {
        return { start: head, end: anchor, direction: 'backward' }
    }
}
```

#### Editor Group / Pane Container

A container that holds a tab bar and one editor pane. Can be split:

```
EditorGroup {
    id: Identifier
    tabs: Array<Tab>
    activeTabIndex: Number
    editorPane: EditorPane
}

Tab {
    id: Identifier
    label: String
    editorState: EditorState    // the view state for this tab
    isDirty: Boolean            // unsaved changes indicator
    isPinned: Boolean           // pinned tabs don't get replaced
    isPreview: Boolean          // preview mode - gets replaced on next open
}
```

### 2.3 Layout Tree

Splits create a tree structure:

```
LayoutNode = EditorGroup | SplitContainer

SplitContainer {
    direction: 'horizontal' | 'vertical'
    children: Array<LayoutNode>
    sizes: Array<Number>        // proportions or pixel sizes

    // Resize handles (splitter bars)
    splitters: Array<Splitter>
}

Splitter {
    position: Number
    minPosition: Number
    maxPosition: Number
}
```

This recursive structure allows for arbitrary nesting of splits. A horizontal split contains children arranged
left-to-right; a vertical split arranges children top-to-bottom.

### 2.4 Multi-View Document Synchronization

When the same document is open in multiple views (split or tabs):

```
Document changes    ──────►  All views notified
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
                 View A          View B          View C

Each view independently decides:
- How to adjust scroll (if edit was in another view)
- Whether to reveal cursor (if cursor moved)
- How to re-render affected lines
```

Key considerations:

- Edits in View A that affect lines visible in View B must trigger re-render in B
- If View A inserts lines above View B's viewport, should View B scroll to maintain visual stability? (This is the
  **scroll anchoring** problem)
- Cursor positions are per-view, but document content is shared

---

## Chapter 3: The Gutter System

The gutter is the vertical strip to the left of the text area, containing line numbers and various indicators.

### 3.1 Gutter Anatomy

The gutter is a composite of multiple **gutter lanes** (or **gutter columns**):

```
┌──────────────────────────────────────────────────────────────┐
│ Gutter                                  │ Text Area          │
│ ┌────────┬─────┬──────┬───────────────┐ │                    │
│ │Fold    │Break│ Line │ Git           │ │                    │
│ │Markers │point│ Nums │ Annotations   │ │                    │
│ ├────────┼─────┼──────┼───────────────┤ │                    │
│ │  ▶     │  ●  │  42  │ ┃ (modified)  │ │ function foo() {   │
│ │  ▼     │     │  43  │ ┃             │ │   // expanded...   │
│ │        │     │  44  │               │ │   return bar;      │
│ │  ▶     │  ●  │  55  │ ┃ (added)     │ │ }                  │
│ └────────┴─────┴──────┴───────────────┘ │                    │
└─────────────────────────────────────────┴────────────────────┘
```

### 3.2 Data Structures

```
Gutter {
    lanes: Array<GutterLane>
    totalWidth: Number  // sum of lane widths
}

GutterLane {
    id: String          // 'line-numbers', 'fold-markers', 'breakpoints', etc.
    width: Number       // fixed or dynamic (pixels)
    alignment: 'left' | 'right' | 'center'

    render(line: Number, state: EditorState): GutterDecoration
}

GutterDecoration {
    content: String | Icon      // what to display
    style: GutterStyle          // colors, fonts, etc.
    onClick: Function | null    // click handler (e.g., toggle breakpoint)
    onHover: Function | null    // hover handler (e.g., show tooltip)
}
```

### 3.3 Key Property

**The gutter scrolls vertically _with_ content, but is horizontally fixed.** It occupies a separate horizontal region
from the text area but shares the vertical scroll offset.

This means:

- When you scroll the text vertically, the gutter scrolls with it
- When you scroll the text horizontally, the gutter stays put
- The gutter's Y coordinate system is identical to the text area's
- The gutter's X coordinate system is independent

### 3.4 Dynamic Line Number Width

Line number width should adapt to the document size:

```
function computeLineNumberWidth(lineCount, charWidth, padding) {
    // Guard against empty documents
    const safeLineCount = max(1, lineCount)
    const digitCount = floor(log10(safeLineCount)) + 1
    const minDigits = 2  // always show at least 2 digits worth of space
    return max(digitCount, minDigits) * charWidth + padding
}
```

When the document grows from 99 to 100 lines, the gutter needs to widen. This can cause a layout shift, so some editors
pre-allocate extra space or animate the transition.

### 3.5 Gutter Hit Testing

For handling clicks on gutter elements (breakpoints, fold markers):

```
function gutterHitTest(clickY, scrollOffsetY, lineHeight, gutter) {
    // Convert click to document line
    const contentY = clickY + scrollOffsetY
    const line = floor(contentY / lineHeight)

    // Determine which lane was clicked (by X coordinate)
    let laneX = 0
    for (const lane of gutter.lanes) {
        if (clickX >= laneX && clickX < laneX + lane.width) {
            return { line, lane: lane.id }
        }
        laneX += lane.width
    }
    return null
}
```

---

## Chapter 4: Viewport Geometry and Line Calculations

### 4.1 Editor Pane Structure

```
EditorPane {
    // Outer bounds (allocated space in pixels)
    bounds: Rectangle { x, y, width, height }

    // Gutter region (fixed horizontal, scrolls vertical)
    gutterBounds: Rectangle

    // Text area (scrolls both axes)
    textAreaBounds: Rectangle

    // Scrollbars
    verticalScrollbar: Scrollbar
    horizontalScrollbar: Scrollbar

    // Computed metrics
    viewportLines: VisibleLineRange

    lineHeight: Number      // pixels per line
    charWidth: Number       // pixels per character (monospace)
}

VisibleLineRange {
    firstVisible: Number            // first line with any pixels showing (inclusive)
    lastVisibleExclusive: Number    // first line completely below viewport (exclusive)
    firstFullyVisible: Number       // first line entirely within viewport
    lastFullyVisible: Number        // last line entirely within viewport
    topClipPixels: Number           // pixels of first line clipped above viewport
    bottomClipPixels: Number        // pixels of last partial line showing
}
```

### 4.2 Viewport Line Calculation

For a simple (non-wrapped) editor with constant line height:

```
function computeVisibleLines(scrollOffsetY, viewportHeight, lineHeight, documentLineCount) {
    // First line with any pixels showing (inclusive, 0-indexed)
    const firstVisible = floor(scrollOffsetY / lineHeight)

    // First line completely below viewport (exclusive)
    // This is one past the last visible line
    const lastVisibleExclusive = min(
        ceil((scrollOffsetY + viewportHeight) / lineHeight),
        documentLineCount
    )

    // For iteration: for (let i = firstVisible; i < lastVisibleExclusive; i++)

    return { firstVisible, lastVisibleExclusive }
}
```

**Why exclusive upper bound?** Using `[firstVisible, lastVisibleExclusive)` (half-open interval) matches standard
iteration patterns and avoids off-by-one errors:

```
for (let line = visibleLines.firstVisible; line < visibleLines.lastVisibleExclusive; line++) {
    renderLine(line)
}
```

### 4.3 Fractional/Partial Visibility

```
function computePartialVisibility(scrollOffsetY, viewportHeight, lineHeight) {
    // How many pixels of the first visible line are clipped above the viewport
    const topClipPixels = scrollOffsetY % lineHeight

    // Fraction of first line that IS visible (1.0 = fully visible)
    const topLineVisibleFraction = 1.0 - (topClipPixels / lineHeight)

    // How many pixels of the last partial line are showing
    const bottomShowingPixels = (scrollOffsetY + viewportHeight) % lineHeight

    // Fraction of last partial line that is visible
    const bottomLineVisibleFraction = bottomShowingPixels / lineHeight

    return {
        topClipPixels,
        topLineVisibleFraction,
        bottomShowingPixels,
        bottomLineVisibleFraction
    }
}
```

### 4.4 Fully vs Partially Visible Lines

```
function computeFullyVisibleLines(scrollOffsetY, viewportHeight, lineHeight, documentLineCount) {
    // First line that is ENTIRELY visible (not clipped at top)
    const firstFullyVisible = ceil(scrollOffsetY / lineHeight)

    // Last line that is ENTIRELY visible (not clipped at bottom)
    // A line is fully visible if its bottom edge (lineTop + lineHeight) <= viewport bottom
    const lastFullyVisible = floor((scrollOffsetY + viewportHeight) / lineHeight) - 1

    // Clamp to valid range
    return {
        firstFullyVisible: min(firstFullyVisible, documentLineCount - 1),
        lastFullyVisible: max(lastFullyVisible, 0)
    }
}
```

The distinction matters for operations like "page down" which might want to keep the last fully visible line as the new
first line.

### 4.5 Line-to-Pixel Conversions

```
// Get the Y pixel coordinate of a line's top edge (in content coordinates)
function lineToContentY(line, lineHeight) {
    return line * lineHeight
}

// Get the Y pixel coordinate of a line's top edge (in viewport coordinates)
function lineToViewportY(line, lineHeight, scrollOffsetY) {
    return line * lineHeight - scrollOffsetY
}

// Get the line number from a Y pixel coordinate (content coordinates)
function contentYToLine(contentY, lineHeight) {
    return floor(contentY / lineHeight)
}

// Get the line number from a Y pixel coordinate (viewport coordinates)
function viewportYToLine(viewportY, lineHeight, scrollOffsetY) {
    return floor((viewportY + scrollOffsetY) / lineHeight)
}
```

### 4.6 Column-to-Pixel Conversions (Monospace)

With a monospace font, column-to-pixel conversion is straightforward:

```
// Get the X pixel coordinate of a column (in text area coordinates, excluding gutter)
function columnToX(column, charWidth) {
    return column * charWidth
}

// Get the column from an X pixel coordinate
function xToColumn(x, charWidth) {
    // Round to nearest column for better click targeting
    return round(x / charWidth)
}

// Get the X coordinate including gutter offset (in viewport coordinates)
function columnToViewportX(column, charWidth, gutterWidth, scrollOffsetX) {
    return gutterWidth + column * charWidth - scrollOffsetX
}
```

### 4.7 Complete Position Conversion

```
// Document position (line, column) to viewport pixel position
function documentPosToViewport(line, column, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY) {
    return {
        x: gutterWidth + column * charWidth - scrollOffsetX,
        y: line * lineHeight - scrollOffsetY
    }
}

// Viewport pixel position to document position (line, column)
function viewportToDocumentPos(viewportX, viewportY, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY, document) {
    const line = clamp(
        floor((viewportY + scrollOffsetY) / lineHeight),
        0,
        document.lineCount - 1
    )

    const textAreaX = viewportX - gutterWidth + scrollOffsetX
    const column = clamp(
        round(textAreaX / charWidth),
        0,
        document.lineLengths[line]
    )

    return { line, column }
}
```

---

## Chapter 5: Cursor-Viewport Interaction Model

This is where text editors become genuinely complex. The relationship between cursor position and scroll position has
many nuances.

### 5.1 Scroll Margins / Keep-in-View Zone

```
ScrollMargins {
    top: Number         // lines (vertical margin)
    bottom: Number      // lines (vertical margin)
    left: Number        // columns (horizontal margin)
    right: Number       // columns (horizontal margin)
}
```

> **Units:** Vertical margins are specified in **lines**, horizontal margins in **columns**. This makes configuration
> intuitive and independent of font metrics.

The **safe zone** is the viewport minus these margins. The cursor should stay within the safe zone during normal
editing:

```
┌─────────────────────────────────────────┐
│ ░░░░░░░░░░ top margin (3 lines) ░░░░░░░ │
├─────────────────────────────────────────┤
│                                         │
│           SAFE ZONE                     │
│     (cursor can be here freely)         │
│                                         │
├─────────────────────────────────────────┤
│ ░░░░░░░░ bottom margin (5 lines) ░░░░░░ │
└─────────────────────────────────────────┘
```

### 5.2 Scroll Modes / States

```
enum ScrollMode {
    CURSOR_LOCKED,      // viewport follows cursor
    FREE_BROWSE,        // user is exploring, cursor may be off-screen
    REVEAL_PENDING      // cursor is off-screen, next edit will reveal it
}
```

### 5.3 Scroll Triggers

Different actions have different scroll behaviors:

| Trigger                          | Typical Behavior                                                 |
| -------------------------------- | ---------------------------------------------------------------- |
| **Typing/editing**               | Reveal cursor, respect margins, snap to cursor-locked mode       |
| **Cursor movement (arrow keys)** | Keep cursor in safe zone, scroll minimally                       |
| **Mouse wheel**                  | Free scroll, enter free-browse mode                              |
| **Scrollbar drag**               | Free scroll, enter free-browse mode                              |
| **Page Up/Down**                 | Move cursor AND viewport together                                |
| **Ctrl+G (go to line)**          | Center target line, cursor-locked mode                           |
| **Click in text area**           | Move cursor, stay at current scroll (unless clicking off-screen) |
| **Search result jump**           | Center or reveal result, cursor-locked mode                      |

### 5.4 The Scroll Decision Algorithm

```
function ensureCursorVisible(cursor, viewport, scrollOffset, margins, lineHeight, charWidth) {
    let newScrollX = scrollOffset.x
    let newScrollY = scrollOffset.y

    // Convert margins from lines/columns to pixels
    const marginTopPx = margins.top * lineHeight
    const marginBottomPx = margins.bottom * lineHeight
    const marginLeftPx = margins.left * charWidth
    const marginRightPx = margins.right * charWidth

    // Cursor position in content coordinates
    const cursorY = cursor.line * lineHeight
    const cursorX = cursor.column * charWidth

    // Vertical adjustment
    const safeTop = scrollOffset.y + marginTopPx
    const safeBottom = scrollOffset.y + viewport.height - marginBottomPx - lineHeight

    if (cursorY < safeTop) {
        // Cursor above safe zone - scroll up
        newScrollY = cursorY - marginTopPx
    } else if (cursorY > safeBottom) {
        // Cursor below safe zone - scroll down
        newScrollY = cursorY - viewport.height + marginBottomPx + lineHeight
    }

    // Horizontal adjustment
    const safeLeft = scrollOffset.x + marginLeftPx
    const safeRight = scrollOffset.x + viewport.width - marginRightPx - charWidth

    if (cursorX < safeLeft) {
        newScrollX = cursorX - marginLeftPx
    } else if (cursorX > safeRight) {
        newScrollX = cursorX - viewport.width + marginRightPx + charWidth
    }

    // Clamp to valid scroll range
    newScrollY = clamp(newScrollY, 0, maxScrollY)
    newScrollX = clamp(newScrollX, 0, maxScrollX)

    return { x: newScrollX, y: newScrollY }
}
```

### 5.5 Scroll Reveal Strategies

#### Minimal Scroll

Move viewport just enough to bring cursor into the safe zone:

```
function computeMinimalScroll(cursorLine, viewport, scrollOffsetY, margins, lineHeight, documentHeight) {
    const cursorY = cursorLine * lineHeight
    const marginTopPx = margins.top * lineHeight
    const marginBottomPx = margins.bottom * lineHeight

    const safeTop = scrollOffsetY + marginTopPx
    const safeBottom = scrollOffsetY + viewport.height - marginBottomPx

    let newScrollY = scrollOffsetY

    if (cursorY < safeTop) {
        // Scroll up: put cursor at top margin
        newScrollY = cursorY - marginTopPx
    } else if (cursorY + lineHeight > safeBottom) {
        // Scroll down: put cursor at bottom margin
        newScrollY = cursorY + lineHeight - viewport.height + marginBottomPx
    }

    // Clamp to valid range
    const maxScroll = max(0, documentHeight - viewport.height)
    return clamp(newScrollY, 0, maxScroll)
}
```

#### Center Scroll

Put the cursor in the middle of the viewport (used for "go to line", search results):

```
function computeCenterScroll(cursorLine, viewport, lineHeight, documentHeight) {
    const cursorY = cursorLine * lineHeight
    const targetScroll = cursorY - (viewport.height / 2) + (lineHeight / 2)

    // Clamp to valid range
    const maxScroll = max(0, documentHeight - viewport.height)
    return clamp(targetScroll, 0, maxScroll)
}
```

### 5.6 Safe Zone Helpers

```
function isInSafeZone(cursorLine, scrollOffsetY, viewport, margins, lineHeight) {
    const cursorY = cursorLine * lineHeight
    const marginTopPx = margins.top * lineHeight
    const marginBottomPx = margins.bottom * lineHeight

    const safeTop = scrollOffsetY + marginTopPx
    const safeBottom = scrollOffsetY + viewport.height - marginBottomPx

    // Use >= and <= (inclusive) to avoid jitter at edges
    return cursorY >= safeTop && cursorY + lineHeight <= safeBottom
}

// Handle edge case: viewport too small for configured margins
function computeEffectiveMargins(configuredMargins, viewportLines) {
    // Each margin can be at most 1/4 of viewport
    const maxMargin = floor(viewportLines / 4)
    return {
        top: min(configuredMargins.top, maxMargin),
        bottom: min(configuredMargins.bottom, maxMargin)
    }
}
```

### 5.7 Multi-Cursor Scenarios

```
// With multiple cursors, which one should the viewport follow?
function determinePrimaryCursor(cursors, policy) {
    switch (policy) {
        case 'first':
            return cursors[0]
        case 'last':
            return cursors[cursors.length - 1]
        case 'most-recent':
            return cursors.reduce((a, b) =>
                a.lastModified > b.lastModified ? a : b
            )
        case 'any-visible':
            // Prefer a cursor already in view
            return cursors.find(c => isInViewport(c)) ?? cursors[0]
    }
}
```

### 5.8 Selection Extends Beyond Viewport

```
// Selection from line 10 to line 500, viewport shows 50-70
// Cursor (selection head) might be at line 500, anchor at line 10
// Follow the head, not the anchor

function ensureSelectionEndVisible(selection, viewport, scrollOffset) {
    const head = selectionHead(selection)

    // Always reveal the head (where cursor is)
    const newScroll = ensureCursorVisible(head, viewport, scrollOffset, margins)

    // Optionally: show indicator if anchor is off-screen
    const anchor = selectionAnchor(selection)
    if (!isInViewport(anchor, viewport, newScroll)) {
        showSelectionExtendsIndicator(
            anchor.line < viewport.firstVisible ? 'above' : 'below'
        )
    }

    return newScroll
}
```

---

## Chapter 6: Soft Wrapping: The Coordinate System Split

Soft wrapping (word wrap) is one of the most significant complications in text editor geometry. The moment you enable
it, you're managing two parallel realities.

### 6.1 The Two Realms

| Realm              | Unit                   | What it represents             |
| ------------------ | ---------------------- | ------------------------------ |
| **Document space** | Document line + column | Where text _is_ in the buffer  |
| **Visual space**   | Visual line + x-offset | Where text _appears_ on screen |

A single document line might span 3 visual lines when wrapped. Conversely, a visual line always maps to exactly one
portion of one document line.

### 6.2 Core Data Structures

```
DocumentLine {
    lineNumber: Number              // 0-indexed
    content: String
    length: Number                  // character count (columns)
}

VisualLineSegment {
    // Which slice of the document line this represents
    startColumn: Number             // inclusive
    endColumn: Number               // exclusive

    // Visual properties
    width: Number                   // in pixels

    // Indentation for continuation lines
    wrapIndent: Number              // extra left padding for wrapped portions (pixels)

    // For fast lookup
    visualLineIndex: Number         // global visual line number
}

VisualLineMapping {
    documentLine: Number
    segments: Array<VisualLineSegment>
    totalVisualLines: Number        // segments.length
}
```

### 6.3 The Visual Line Index

You need bidirectional lookup between document and visual coordinates:

```
VisualLineIndex {
    // Document line → visual lines
    documentToVisual: Array<{
        firstVisualLine: Number
        visualLineCount: Number
    }>

    // Visual line → document position
    visualToDocument: Array<{
        documentLine: Number
        segmentIndex: Number
    }>

    // Running totals for fast offset calculation
    visualLineOffsets: Array<Number>    // cumulative pixel Y offset per visual line

    totalVisualLines: Number
    totalVisualHeight: Number           // pixels
}
```

### 6.4 The Wrapping Algorithm (Monospace)

For monospace fonts, wrapping is straightforward—we can work entirely in columns:

```
function wrapLine(lineContent, maxColumns, wrapIndentColumns) {
    if (lineContent.length == 0) {
        // Empty lines still need one visual line entry
        return [{
            startColumn: 0,
            endColumn: 0,
            width: 0,
            wrapIndent: 0
        }]
    }

    const segments = []
    let currentStart = 0
    let isFirstSegment = true

    while (currentStart < lineContent.length) {
        // Available columns for this segment
        const indent = isFirstSegment ? 0 : wrapIndentColumns
        const availableColumns = maxColumns - indent

        if (availableColumns <= 0) {
            // Degenerate case: wrap indent >= viewport width
            // Fall back to no indent
            availableColumns = maxColumns
        }

        let segmentEnd = min(currentStart + availableColumns, lineContent.length)

        // Try to break at word boundary
        if (segmentEnd < lineContent.length) {
            const breakPoint = findBreakPoint(lineContent, currentStart, segmentEnd)
            if (breakPoint > currentStart) {
                segmentEnd = breakPoint
            }
            // else: no good break point, force break at column limit
        }

        segments.push({
            startColumn: currentStart,
            endColumn: segmentEnd,
            wrapIndent: indent
        })

        currentStart = segmentEnd
        // Skip whitespace at wrap point
        while (currentStart < lineContent.length && lineContent[currentStart] == ' ') {
            currentStart++
        }

        isFirstSegment = false
    }

    return segments
}

function findBreakPoint(line, start, end) {
    // Search backwards from end for a break opportunity
    for (let i = end; i > start; i--) {
        if (isBreakOpportunity(line[i - 1], line[i])) {
            return i
        }
    }
    return start  // no break found
}

function isBreakOpportunity(prevChar, char) {
    // Break after whitespace
    if (prevChar == ' ' || prevChar == '\t') return true

    // Break after certain punctuation
    if (prevChar == '-' || prevChar == '/' || prevChar == '\\') return true

    // Break before opening brackets
    if (char == '(' || char == '[' || char == '{') return true

    return false
}
```

### 6.5 Building the Visual Index

```
function buildVisualIndex(document, maxColumns, lineHeight, wrapIndentColumns) {
    const docToVis = []
    const visToDoc = []
    const visualLineOffsets = []
    let visualLineCounter = 0
    let yOffset = 0

    for (let docLine = 0; docLine < document.lineCount; docLine++) {
        const lineContent = document.getLine(docLine)
        const segments = wrapLine(lineContent, maxColumns, wrapIndentColumns)

        docToVis[docLine] = {
            firstVisualLine: visualLineCounter,
            visualLineCount: segments.length
        }

        for (let seg = 0; seg < segments.length; seg++) {
            visToDoc[visualLineCounter] = {
                documentLine: docLine,
                segmentIndex: seg
            }
            visualLineOffsets[visualLineCounter] = yOffset

            visualLineCounter++
            yOffset += lineHeight
        }
    }

    return {
        documentToVisual: docToVis,
        visualToDocument: visToDoc,
        visualLineOffsets: visualLineOffsets,
        totalVisualLines: visualLineCounter,
        totalVisualHeight: yOffset
    }
}
```

### 6.6 Coordinate Conversions with Wrapping

```
// Document position → visual position
function documentToVisual(docLine, docColumn, visualIndex, charWidth, lineHeight) {
    const mapping = visualIndex.documentToVisual[docLine]

    // Find which segment contains this column
    let segmentIndex = 0
    let segmentStart = 0
    // (In practice, store segment boundaries in the index for O(1) lookup)

    const visualLine = mapping.firstVisualLine + segmentIndex
    const columnWithinSegment = docColumn - segmentStart

    return {
        visualLine: visualLine,
        x: columnWithinSegment * charWidth,
        y: visualIndex.visualLineOffsets[visualLine]
    }
}

// Visual position → document position
function visualToDocument(visualLine, x, visualIndex, charWidth) {
    const { documentLine, segmentIndex } = visualIndex.visualToDocument[visualLine]
    const segment = getSegment(documentLine, segmentIndex)

    const columnWithinSegment = round(x / charWidth)
    const docColumn = segment.startColumn + clamp(columnWithinSegment, 0, segment.endColumn - segment.startColumn)

    return { line: documentLine, column: docColumn }
}
```

### 6.7 Visible Lines with Wrapping

```
function getVisibleDocumentLines(scrollOffsetY, viewportHeight, visualIndex, lineHeight) {
    // Find visual lines in view
    const firstVisualLine = binarySearchFloor(visualIndex.visualLineOffsets, scrollOffsetY)
    const lastVisualLineExclusive = binarySearchFloor(
        visualIndex.visualLineOffsets,
        scrollOffsetY + viewportHeight
    ) + 1

    // Map back to document lines
    const firstDocLine = visualIndex.visualToDocument[firstVisualLine].documentLine
    const lastDocLine = visualIndex.visualToDocument[
        min(lastVisualLineExclusive - 1, visualIndex.totalVisualLines - 1)
    ].documentLine

    return {
        firstDocumentLine: firstDocLine,
        lastDocumentLineExclusive: lastDocLine + 1,
        firstVisualLine: firstVisualLine,
        lastVisualLineExclusive: min(lastVisualLineExclusive, visualIndex.totalVisualLines)
    }
}
```

### 6.8 Cursor Movement with Wrapping

The `preferredColumn` concept becomes more nuanced with wrapping. We still store the **document column** as the
preferred position:

```
function moveCursorDown(cursor, document, visualIndex) {
    const mapping = visualIndex.documentToVisual[cursor.line]

    // Find current visual line
    let currentVisualLine = mapping.firstVisualLine
    let segmentIndex = 0
    // ... find segment containing cursor.column

    const nextVisualLine = currentVisualLine + 1

    if (nextVisualLine >= visualIndex.totalVisualLines) {
        return cursor  // at end of document
    }

    const { documentLine, segmentIndex: newSegIndex } =
        visualIndex.visualToDocument[nextVisualLine]

    // Use preferredColumn to determine target column
    const targetColumn = cursor.preferredColumn ?? cursor.column
    const newSegment = getSegment(documentLine, newSegIndex)

    // Map target column to this segment
    const newColumn = clamp(
        targetColumn,
        newSegment.startColumn,
        newSegment.endColumn
    )

    return {
        line: documentLine,
        column: newColumn,
        affinity: cursor.affinity,
        preferredColumn: targetColumn
    }
}
```

### 6.9 Scrollable Height with Wrapping

```
function computeScrollableHeight(visualIndex, viewportHeight) {
    const contentHeight = visualIndex.totalVisualHeight
    return max(0, contentHeight - viewportHeight)
}
```

### 6.10 When to Rebuild the Visual Index

The visual index must be rebuilt when:

1. **Document content changes** — lines added, removed, or modified
2. **Viewport width changes** — different wrap points
3. **Font size changes** — different character width → different columns per line
4. **Wrap settings change** — wrap enabled/disabled, wrap indent changed

For performance, consider:

- Incremental updates for single-line edits
- Lazy computation (only compute visible range + buffer)
- Background computation for large documents

---

## Chapter 7: Autocomplete and Overlay Positioning

Overlays (autocomplete, hover tooltips, parameter hints) must be positioned relative to text content while respecting
viewport boundaries.

### 7.1 Overlay Types

```
enum OverlayType {
    AUTOCOMPLETE,       // completion suggestions
    HOVER,              // documentation on hover
    SIGNATURE_HELP,     // function parameter hints
    DIAGNOSTIC,         // error/warning tooltips
    CONTEXT_MENU        // right-click menu
}
```

### 7.2 Anchor Points

Overlays are anchored to document positions:

```
OverlayAnchor {
    // Document position (stable across scrolling)
    documentPosition: { line: Number, column: Number }

    // Preferred placement relative to anchor
    preferredSide: 'above' | 'below' | 'left' | 'right'

    // Alignment
    alignment: 'start' | 'center' | 'end'
}
```

### 7.3 The Positioning Algorithm

```
function positionOverlay(anchor, overlaySize, viewport, gutterWidth, scrollOffset, lineHeight, charWidth) {
    // Convert document position to viewport coordinates
    const anchorViewportPos = documentPosToViewport(
        anchor.documentPosition.line,
        anchor.documentPosition.column,
        lineHeight, charWidth, gutterWidth,
        scrollOffset.x, scrollOffset.y
    )

    // Calculate candidate positions
    const candidates = []

    // Below the anchor (most common for autocomplete)
    candidates.push({
        side: 'below',
        x: anchorViewportPos.x,
        y: anchorViewportPos.y + lineHeight,
        score: anchor.preferredSide == 'below' ? 100 : 50
    })

    // Above the anchor
    candidates.push({
        side: 'above',
        x: anchorViewportPos.x,
        y: anchorViewportPos.y - overlaySize.height,
        score: anchor.preferredSide == 'above' ? 100 : 50
    })

    // Score each candidate based on how well it fits
    for (const candidate of candidates) {
        // Penalize if overlay would be clipped
        if (candidate.y < 0) {
            candidate.score -= 1000  // above viewport
        }
        if (candidate.y + overlaySize.height > viewport.height) {
            candidate.score -= 1000  // below viewport
        }
        if (candidate.x < gutterWidth) {
            candidate.score -= 500   // overlaps gutter
        }
        if (candidate.x + overlaySize.width > viewport.width) {
            candidate.score -= 200   // extends past right edge (less bad, can scroll)
        }
    }

    // Pick best candidate
    candidates.sort((a, b) => b.score - a.score)
    const best = candidates[0]

    // Clamp to viewport bounds
    return {
        x: clamp(best.x, gutterWidth, viewport.width - overlaySize.width),
        y: clamp(best.y, 0, viewport.height - overlaySize.height),
        side: best.side
    }
}
```

### 7.4 Autocomplete Specifics

```
AutocompleteWidget {
    anchor: OverlayAnchor
    items: Array<CompletionItem>
    selectedIndex: Number

    // Dimensions
    itemHeight: Number          // pixels per item
    maxVisibleItems: Number     // typically 8-12
    minWidth: Number
    maxWidth: Number

    // Computed
    visibleItems: Number
    scrollOffset: Number        // if list is scrollable
}

CompletionItem {
    label: String               // main text
    detail: String              // type info, etc.
    kind: CompletionKind        // function, variable, keyword, etc.
    insertText: String          // what to insert
    filterText: String          // for fuzzy matching
}

function computeAutocompleteSize(widget, charWidth) {
    const visibleItems = min(widget.items.length, widget.maxVisibleItems)
    const height = visibleItems * widget.itemHeight

    // Width based on longest item
    const maxLabelLength = widget.items.reduce(
        (max, item) => Math.max(max, item.label.length),
        0
    )
    const contentWidth = maxLabelLength * charWidth + padding
    const width = clamp(contentWidth, widget.minWidth, widget.maxWidth)

    return { width, height }
}
```

### 7.5 Hover Tooltip Positioning

Hover tooltips typically appear above the hovered word:

```
function positionHoverTooltip(wordRange, tooltipSize, viewport, lineHeight, charWidth, gutterWidth, scrollOffset) {
    // Anchor at start of word
    const anchor = {
        documentPosition: wordRange.start,
        preferredSide: 'above',
        alignment: 'start'
    }

    const pos = positionOverlay(anchor, tooltipSize, viewport, gutterWidth, scrollOffset, lineHeight, charWidth)

    // If tooltip is wide, center it over the word
    if (tooltipSize.width > (wordRange.end.column - wordRange.start.column) * charWidth) {
        const wordCenterX = gutterWidth +
            ((wordRange.start.column + wordRange.end.column) / 2) * charWidth -
            scrollOffset.x
        pos.x = wordCenterX - tooltipSize.width / 2
        pos.x = clamp(pos.x, gutterWidth, viewport.width - tooltipSize.width)
    }

    return pos
}
```

### 7.6 Overlay Lifecycle

```
enum OverlayState {
    HIDDEN,
    SHOWING,            // fade-in animation
    VISIBLE,
    HIDING              // fade-out animation
}

OverlayManager {
    overlays: Map<OverlayId, Overlay>

    show(overlay) {
        overlay.state = SHOWING
        overlay.position = positionOverlay(...)
        scheduleAnimation(overlay, VISIBLE, 150ms)
    }

    hide(overlay) {
        overlay.state = HIDING
        scheduleAnimation(overlay, HIDDEN, 100ms)
    }

    updatePositions(scrollOffset) {
        // Called on scroll - reposition all visible overlays
        for (const overlay of this.overlays.values()) {
            if (overlay.state == VISIBLE || overlay.state == SHOWING) {
                overlay.position = positionOverlay(overlay.anchor, ...)

                // Hide if anchor scrolled out of view
                if (!isAnchorVisible(overlay.anchor, scrollOffset)) {
                    this.hide(overlay)
                }
            }
        }
    }
}
```

### 7.7 Z-Order and Layering

```
// Overlay z-order (higher = on top)
const Z_ORDER = {
    SELECTION_HIGHLIGHT: 1,
    TEXT: 2,
    CURSOR: 3,
    DIAGNOSTIC_UNDERLINE: 4,
    HOVER_TOOLTIP: 100,
    AUTOCOMPLETE: 101,
    SIGNATURE_HELP: 102,
    CONTEXT_MENU: 200,
    MODAL_DIALOG: 300
}
```

---

## Chapter 8: Syntax Highlighting Integration

Syntax highlighting adds color and style information to the text rendering pipeline.

### 8.1 Token Model

```
Token {
    startColumn: Number     // inclusive
    endColumn: Number       // exclusive
    tokenType: TokenType    // keyword, string, comment, etc.
    modifiers: Array<Modifier>  // bold, italic, deprecated, etc.
}

TokenType = 'keyword' | 'string' | 'number' | 'comment' | 'operator' |
            'function' | 'variable' | 'type' | 'namespace' | 'property' | ...

Modifier = 'declaration' | 'definition' | 'readonly' | 'static' |
           'deprecated' | 'async' | 'modification' | 'documentation' | ...
```

### 8.2 Line Tokens

Each line has an array of tokens:

```
LineTokens {
    line: Number
    tokens: Array<Token>

    // Tokens must:
    // - Cover the entire line (no gaps)
    // - Not overlap
    // - Be sorted by startColumn
}

function getTokenAtColumn(lineTokens, column) {
    for (const token of lineTokens.tokens) {
        if (column >= token.startColumn && column < token.endColumn) {
            return token
        }
    }
    return null  // beyond end of line
}
```

### 8.3 Token Cache

```
TokenCache {
    // Map of line number → tokens
    lines: Map<Number, LineTokens>

    // Track which lines need re-tokenization
    dirtyLines: Set<Number>

    // Version for cache invalidation
    documentVersion: Number
}

function invalidateTokens(cache, editRange) {
    // Mark affected lines as dirty
    for (let line = editRange.startLine; line <= editRange.endLine; line++) {
        cache.dirtyLines.add(line)
    }

    // Multi-line constructs (strings, comments) may require
    // invalidating subsequent lines
    if (isMultiLineConstruct(editRange)) {
        invalidateToEndOfBlock(cache, editRange.endLine)
    }
}
```

### 8.4 Incremental Tokenization

For large files, tokenize lazily:

```
function ensureTokenized(cache, lineRange, tokenizer) {
    for (let line = lineRange.first; line < lineRange.lastExclusive; line++) {
        if (cache.dirtyLines.has(line) || !cache.lines.has(line)) {
            const lineContent = document.getLine(line)
            const prevState = line > 0 ? cache.lines.get(line - 1)?.endState : null

            const { tokens, endState } = tokenizer.tokenizeLine(lineContent, prevState)

            cache.lines.set(line, {
                line: line,
                tokens: tokens,
                endState: endState
            })
            cache.dirtyLines.delete(line)

            // Check if end state changed (affects subsequent lines)
            if (stateChanged(prevEndState, endState)) {
                cache.dirtyLines.add(line + 1)
            }
        }
    }
}
```

### 8.5 Theme Integration

```
Theme {
    // Map token type + modifiers → style
    tokenStyles: Map<TokenStyleKey, TokenStyle>

    // Editor chrome colors
    background: Color
    foreground: Color
    cursorColor: Color
    selectionBackground: Color
    lineNumberColor: Color
    gutterBackground: Color
}

TokenStyle {
    foreground: Color
    background: Color | null
    fontStyle: 'normal' | 'bold' | 'italic' | 'bold-italic'
    textDecoration: 'none' | 'underline' | 'strikethrough'
}

function resolveTokenStyle(token, theme) {
    // Try specific match first (type + modifiers)
    let key = token.tokenType + '.' + token.modifiers.join('.')
    if (theme.tokenStyles.has(key)) {
        return theme.tokenStyles.get(key)
    }

    // Fall back to just type
    if (theme.tokenStyles.has(token.tokenType)) {
        return theme.tokenStyles.get(token.tokenType)
    }

    // Default style
    return theme.defaultTokenStyle
}
```

### 8.6 Rendering Tokens (Monospace)

```
function renderLineWithTokens(line, lineContent, tokens, x, y, charWidth, lineHeight, theme, ctx) {
    for (const token of tokens) {
        const style = resolveTokenStyle(token, theme)
        const text = lineContent.substring(token.startColumn, token.endColumn)
        const tokenX = x + token.startColumn * charWidth
        const tokenWidth = (token.endColumn - token.startColumn) * charWidth

        // Background (if any)
        if (style.background) {
            ctx.fillStyle = style.background
            ctx.fillRect(tokenX, y, tokenWidth, lineHeight)
        }

        // Text
        ctx.fillStyle = style.foreground
        ctx.font = styleToFont(style)
        ctx.fillText(text, tokenX, y + baseline)

        // Decorations
        if (style.textDecoration == 'underline') {
            ctx.strokeStyle = style.foreground
            ctx.beginPath()
            ctx.moveTo(tokenX, y + lineHeight - 2)
            ctx.lineTo(tokenX + tokenWidth, y + lineHeight - 2)
            ctx.stroke()
        }
    }
}
```

### 8.7 Semantic Tokens

Beyond syntactic highlighting, semantic tokens provide richer information from language servers:

```
SemanticToken {
    line: Number
    startColumn: Number
    length: Number
    tokenType: SemanticTokenType
    modifiers: Array<SemanticModifier>
}

// Semantic tokens overlay/replace syntactic tokens
function mergeTokens(syntacticTokens, semanticTokens) {
    // Semantic tokens take precedence where they exist
    // This requires splitting syntactic tokens at semantic boundaries
    // ... complex merging logic
}
```

---

## Chapter 9: The Complete Render Pipeline

This chapter describes the full rendering process from state to pixels.

### 9.1 Rendering Approaches

#### Immediate Mode (GPU/Canvas)

Re-render everything each frame. Common in game engines and GPU-based editors:

```
function render(state, ctx) {
    ctx.clear()

    // 1. Background
    ctx.fillStyle = theme.background
    ctx.fillRect(0, 0, viewport.width, viewport.height)

    // 2. Gutter
    renderGutter(state, ctx)

    // 3. Text area
    ctx.save()
    ctx.translate(gutterWidth, 0)
    ctx.beginClip(0, 0, textAreaWidth, viewport.height)

    renderSelectionHighlights(state, ctx)
    renderText(state, ctx)
    renderCursor(state, ctx)
    renderDiagnosticUnderlines(state, ctx)

    ctx.endClip()
    ctx.restore()

    // 4. Scrollbars
    renderScrollbars(state, ctx)

    // 5. Overlays
    renderOverlays(state, ctx)
}
```

#### Retained Mode (DOM-like)

Maintain a render tree; update only what changed:

```
RenderTree {
    root: RenderNode
    dirtyNodes: Set<RenderNode>

    markDirty(node) {
        this.dirtyNodes.add(node)
        scheduleRepaint()
    }

    repaint() {
        for (const node of this.dirtyNodes) {
            node.paint()
        }
        this.dirtyNodes.clear()
    }
}
```

### 9.2 The Render Loop

```
Editor {
    state: EditorState
    renderScheduled: Boolean
    lastFrameTime: Number

    scheduleRender() {
        if (!this.renderScheduled) {
            this.renderScheduled = true
            requestAnimationFrame(() => this.doRender())
        }
    }

    doRender() {
        this.renderScheduled = false
        const now = performance.now()
        const deltaTime = now - this.lastFrameTime
        this.lastFrameTime = now

        // Update animations
        this.updateAnimations(deltaTime)

        // Render
        this.render()
    }
}
```

### 9.3 Render Order (Back to Front)

1. **Background** — editor background color
2. **Current line highlight** — subtle highlight on cursor's line
3. **Selection background** — selected text highlight
4. **Search match highlights** — find/replace matches
5. **Text content** — the actual characters with syntax highlighting
6. **Diagnostic underlines** — error/warning squiggles
7. **Cursor(s)** — the text cursor(s)
8. **Gutter** — line numbers, fold markers, breakpoints
9. **Scrollbars** — vertical and horizontal
10. **Overlays** — autocomplete, hover, tooltips

### 9.4 Rendering Text (Monospace, Immediate Mode)

```
function renderText(state, ctx, charWidth, lineHeight) {
    const visibleRange = computeVisibleLines(
        state.scrollOffset.y,
        viewport.height,
        lineHeight,
        state.document.lineCount
    )

    // Ensure tokens are ready for visible lines
    ensureTokenized(state.tokenCache, visibleRange, state.tokenizer)

    for (let line = visibleRange.firstVisible; line < visibleRange.lastVisibleExclusive; line++) {
        const y = line * lineHeight - state.scrollOffset.y
        const x = -state.scrollOffset.x

        const lineContent = state.document.getLine(line)
        const tokens = state.tokenCache.lines.get(line).tokens

        renderLineWithTokens(line, lineContent, tokens, x, y, charWidth, lineHeight, theme, ctx)
    }
}
```

### 9.5 Rendering Selections

```
function renderSelections(state, ctx, charWidth, lineHeight) {
    ctx.fillStyle = theme.selectionBackground

    for (const selection of state.selections) {
        if (selection.start.line == selection.end.line &&
            selection.start.column == selection.end.column) {
            continue  // empty selection
        }

        const startLine = selection.start.line
        const endLine = selection.end.line

        for (let line = startLine; line <= endLine; line++) {
            const y = line * lineHeight - state.scrollOffset.y

            // Skip if not visible
            if (y + lineHeight < 0 || y > viewport.height) continue

            let startCol, endCol

            if (line == startLine) {
                startCol = selection.start.column
            } else {
                startCol = 0
            }

            if (line == endLine) {
                endCol = selection.end.column
            } else {
                // Extend to end of line (or beyond for visual continuity)
                endCol = state.document.lineLengths[line]
                // Optionally extend slightly past EOL to show selection continues
            }

            const x = startCol * charWidth - state.scrollOffset.x
            const width = (endCol - startCol) * charWidth

            ctx.fillRect(x, y, width, lineHeight)
        }
    }
}
```

### 9.6 Rendering the Gutter

```
function renderGutter(state, ctx, lineHeight, gutterWidth) {
    // Gutter background
    ctx.fillStyle = theme.gutterBackground
    ctx.fillRect(0, 0, gutterWidth, viewport.height)

    const visibleRange = computeVisibleLines(...)

    for (let line = visibleRange.firstVisible; line < visibleRange.lastVisibleExclusive; line++) {
        const y = line * lineHeight - state.scrollOffset.y

        // Line number
        const lineNumber = (line + 1).toString()  // 1-indexed for display
        ctx.fillStyle = theme.lineNumberColor
        ctx.textAlign = 'right'
        ctx.fillText(lineNumber, gutterWidth - padding, y + baseline)

        // Other gutter decorations (breakpoints, fold markers, etc.)
        for (const lane of gutter.lanes) {
            const decoration = lane.render(line, state)
            if (decoration) {
                renderGutterDecoration(decoration, lane, y, ctx)
            }
        }
    }

    // Gutter/text separator line
    ctx.strokeStyle = theme.gutterBorder
    ctx.beginPath()
    ctx.moveTo(gutterWidth - 0.5, 0)
    ctx.lineTo(gutterWidth - 0.5, viewport.height)
    ctx.stroke()
}
```

### 9.7 Rendering Scrollbars

```
function renderVerticalScrollbar(state, ctx, scrollbarWidth, viewportHeight, contentHeight) {
    const scrollableHeight = max(0, contentHeight - viewportHeight)

    if (scrollableHeight == 0) {
        return  // no scrollbar needed
    }

    const trackX = viewport.width - scrollbarWidth
    const trackHeight = viewportHeight

    // Track
    ctx.fillStyle = theme.scrollbarTrack
    ctx.fillRect(trackX, 0, scrollbarWidth, trackHeight)

    // Thumb
    const thumbHeight = max(
        minThumbSize,
        (viewportHeight / contentHeight) * trackHeight
    )
    const thumbY = (state.scrollOffset.y / scrollableHeight) * (trackHeight - thumbHeight)

    ctx.fillStyle = theme.scrollbarThumb
    ctx.fillRect(trackX + 2, thumbY, scrollbarWidth - 4, thumbHeight)
}
```

### 9.8 Cursor Blinking

```
CursorBlinker {
    visible: Boolean
    lastToggle: Number
    blinkInterval: Number   // typically 500-600ms

    update(now) {
        if (now - this.lastToggle > this.blinkInterval) {
            this.visible = !this.visible
            this.lastToggle = now
            return true  // needs repaint
        }
        return false
    }

    reset() {
        // Show cursor immediately (e.g., after typing)
        this.visible = true
        this.lastToggle = performance.now()
    }
}

function renderCursor(state, ctx, blinker, charWidth, lineHeight) {
    if (!blinker.visible) return

    for (const cursor of state.cursors) {
        const x = cursor.column * charWidth - state.scrollOffset.x
        const y = cursor.line * lineHeight - state.scrollOffset.y

        // Skip if not visible
        if (x < 0 || x > textAreaWidth || y < -lineHeight || y > viewport.height) {
            continue
        }

        // See Chapter 12 for different cursor styles
        renderCursorStyle(cursor, x, y, charWidth, lineHeight, ctx, state.cursorStyle)
    }
}
```

### 9.9 Damage Tracking (Optimization)

For retained-mode or partial updates:

```
DamageTracker {
    dirtyRects: Array<Rectangle>

    markDirty(rect) {
        // Merge with existing rects if overlapping
        this.dirtyRects.push(rect)
        this.coalesce()
    }

    markLineDirty(line, lineHeight, scrollOffsetY) {
        const y = line * lineHeight - scrollOffsetY
        this.markDirty({ x: 0, y, width: viewport.width, height: lineHeight })
    }

    coalesce() {
        // Merge overlapping rectangles to reduce draw calls
        // ... implementation
    }

    forEachDirtyRect(callback) {
        for (const rect of this.dirtyRects) {
            callback(rect)
        }
    }

    clear() {
        this.dirtyRects = []
    }
}
```

---

## Chapter 10: Edge Cases and Special Considerations

### 10.1 Cursor Edge Cases

#### Cursor at End of Line

```
// Line "hello" has 5 characters (indices 0-4)
// Valid cursor columns are 0-5 (inclusive)
// Column 5 is "after the last character"

function isValidCursorColumn(column, lineLength) {
    return column >= 0 && column <= lineLength
}
```

#### Cursor in Empty Document

```
// An empty document still has one line (line 0)
// That line has length 0
// Valid cursor position: line=0, column=0

function handleEmptyDocument(document) {
    if (document.lineCount == 0) {
        // Treat as single empty line
        return { lineCount: 1, getLine: () => "", lineLengths: [0] }
    }
    return document
}
```

### 10.2 Viewport Edge Cases

#### Safe Zone Boundary

```
// Is line 10 in the safe zone if margin is 3 lines and viewport shows lines 7-20?
// Lines 7,8,9 are in top margin → line 10 is at edge
// Typically: treat "at edge" as "inside" to avoid jitter

function isInSafeZone(cursorY, scrollOffset, viewport, margins, lineHeight) {
    const safeTop = scrollOffset + margins.top * lineHeight
    const safeBottom = scrollOffset + viewport.height - margins.bottom * lineHeight

    // Use >= and <= (inclusive) to avoid jitter at edges
    return cursorY >= safeTop && cursorY + lineHeight <= safeBottom
}
```

#### Very Small Viewport

```
// Viewport is 5 lines, margins are 3 each → impossible!
// Solution: reduce margins dynamically when viewport is small

function computeEffectiveMargins(configuredMargins, viewportLines) {
    const maxMargin = floor(viewportLines / 4)
    return {
        top: min(configuredMargins.top, maxMargin),
        bottom: min(configuredMargins.bottom, maxMargin)
    }
}
```

#### Multi-Cursor Scenarios

```
// 3 cursors at lines 10, 500, 1000 → which one to follow?

function determinePrimaryCursor(cursors, policy) {
    switch (policy) {
        case 'first':
            return cursors[0]
        case 'last':
            return cursors[cursors.length - 1]
        case 'most-recent':
            return cursors.reduce((a, b) =>
                a.lastModified > b.lastModified ? a : b
            )
        case 'any-visible':
            // Prefer cursor already in view
            return cursors.find(c => this.isInViewport(c)) ?? cursors[0]
    }
}
```

#### Selection Extends Beyond Viewport

```
// Selection from line 10 to line 500, viewport shows 50-70
// Cursor (selection head) might be at line 500, anchor at line 10
// Follow the head, not the anchor

function ensureSelectionEndVisible(selection) {
    // Always reveal the head (where cursor is)
    const head = selectionHead(selection)
    ensureCursorVisible(head)

    // Optionally: if selection started outside view, show indicator
    const anchor = selectionAnchor(selection)
    if (!isInViewport(anchor)) {
        showSelectionExtendsIndicator(
            anchor.line < viewport.firstLine ? 'above' : 'below'
        )
    }
}
```

### 10.3 Wrapping Edge Cases

#### Empty Lines

Empty lines still need a visual line entry:

```
function wrapLine(lineContent) {
    if (lineContent.length == 0) {
        return [{
            startColumn: 0,
            endColumn: 0,
            width: 0
        }]
    }
    // ... normal wrapping
}
```

#### Very Long Words

When a single "word" exceeds the viewport width:

```
// Word: "supercalifragilisticexpialidocious_and_more_stuff"
// Viewport width: 40 columns
// Word length: 50 columns

// Options:
// 1. Force character wrap (break mid-word) ← most common for code
// 2. Allow horizontal overflow (show scrollbar)
// 3. Shrink text to fit (bad for editing)

// For code editors, option 1 is typical:
if (noBreakPointFound && currentWidth > maxColumns) {
    // Force break at column limit
    segments.push({
        startColumn: currentStart,
        endColumn: currentStart + maxColumns,
    })
    currentStart += maxColumns
}
```

#### Tab Characters

Tabs have variable width based on position:

```
function measureTab(column, tabSize) {
    const tabStop = floor(column / tabSize) + 1
    const nextTabColumn = tabStop * tabSize
    return nextTabColumn - column  // columns consumed by this tab
}

// Example: tabSize=4
// Tab at column 0 → width 4 (moves to column 4)
// Tab at column 1 → width 3 (moves to column 4)
// Tab at column 4 → width 4 (moves to column 8)
// Tab at column 5 → width 3 (moves to column 8)
```

### 10.4 Virtual Scrolling

For documents with 100k+ lines, rendering all lines is impractical:

```
VirtualScroller {
    // Only render visible lines + buffer
    bufferLines: Number = 5  // extra lines above/below viewport

    renderedRange: { first: Number, lastExclusive: Number }

    onScroll(scrollOffsetY) {
        const visible = computeVisibleLines(scrollOffsetY, viewport.height, lineHeight)

        const newFirst = max(0, visible.firstVisible - bufferLines)
        const newLastExclusive = min(
            document.lineCount,
            visible.lastVisibleExclusive + bufferLines
        )

        // Determine what changed
        const linesToRemove = []
        const linesToAdd = []

        // Lines that scrolled out
        for (let i = this.renderedRange.first; i < newFirst; i++) {
            linesToRemove.push(i)
        }
        for (let i = newLastExclusive; i < this.renderedRange.lastExclusive; i++) {
            linesToRemove.push(i)
        }

        // Lines that scrolled in
        for (let i = newFirst; i < this.renderedRange.first; i++) {
            linesToAdd.push(i)
        }
        for (let i = this.renderedRange.lastExclusive; i < newLastExclusive; i++) {
            linesToAdd.push(i)
        }

        // Update render state
        this.removeLines(linesToRemove)
        this.addLines(linesToAdd)
        this.renderedRange = { first: newFirst, lastExclusive: newLastExclusive }
    }
}
```

### 10.5 Scroll Anchoring

When content above the viewport changes, maintain visual stability:

```
function handleEditAboveViewport(edit, scrollOffset, lineHeight) {
    if (edit.range.end.line < firstVisibleLine(scrollOffset, lineHeight)) {
        // Edit is entirely above viewport
        const linesDelta = edit.newLineCount - edit.oldLineCount

        if (linesDelta != 0) {
            // Adjust scroll to keep same content visible
            scrollOffset.y += linesDelta * lineHeight
            scrollOffset.y = max(0, scrollOffset.y)
        }
    }
}
```

### 10.6 High-DPI / Retina Displays

```
function setupHighDpiCanvas(canvas, width, height) {
    const dpr = window.devicePixelRatio || 1

    // Set actual pixel dimensions
    canvas.width = width * dpr
    canvas.height = height * dpr

    // Set display size (CSS pixels)
    canvas.style.width = width + 'px'
    canvas.style.height = height + 'px'

    // Scale context so drawing operations use logical pixels
    const ctx = canvas.getContext('2d')
    ctx.scale(dpr, dpr)

    return ctx
}
```

### 10.7 Font Loading

Handle fonts that load asynchronously:

```
function waitForFont(fontFamily) {
    return document.fonts.ready.then(() => {
        // Recalculate metrics
        charWidth = measureCharWidth(fontFamily)
        lineHeight = measureLineHeight(fontFamily)

        // Rebuild visual index (wrapping depends on char width)
        rebuildVisualIndex()

        // Re-render
        scheduleRender()
    })
}
```

### 10.8 Undo/Redo and Cursor Position

```
UndoEntry {
    edit: Edit
    cursorsBefore: Array<Cursor>
    cursorsAfter: Array<Cursor>
    selectionsBefore: Array<Selection>
    selectionsAfter: Array<Selection>
}

function undo(state) {
    const entry = state.undoStack.pop()
    if (!entry) return

    // Reverse the edit
    applyEdit(state.document, inverseEdit(entry.edit))

    // Restore cursor positions from BEFORE the edit
    state.cursors = entry.cursorsBefore
    state.selections = entry.selectionsBefore

    // Push to redo stack
    state.redoStack.push(entry)

    // Reveal cursor
    ensureCursorVisible(state.cursors[0])
}
```

---

## Chapter 11: Naming Conventions and Terminology

A reference for the various names used across different editors and frameworks.

### Document & Content

| Concept                | Common Names                                  |
| ---------------------- | --------------------------------------------- |
| Full content           | Document, Buffer, Model, TextModel            |
| Content data structure | Rope, Piece Table, Gap Buffer, Array of Lines |
| Single line content    | Line, TextLine, LineContent                   |
| Position in document   | Position, Location, Point, Offset             |
| Range in document      | Range, Span, Region, Selection                |

### View & State

| Concept             | Common Names                                                   |
| ------------------- | -------------------------------------------------------------- |
| View state          | EditorState, ViewState, EditorViewModel, ViewConfiguration     |
| Visible area        | Viewport, View, VisibleRange, ViewRegion                       |
| Scroll distance     | ScrollOffset, ScrollPosition, ScrollTop/ScrollLeft             |
| Keep-in-view buffer | ScrollMargin, ScrollPadding, CursorSurroundingLines, ScrollOff |

### Layout

| Concept                | Common Names                                             |
| ---------------------- | -------------------------------------------------------- |
| Line number area       | Gutter, Margin, LineNumberGutter, LineDecorations        |
| Sub-regions of gutter  | GutterLane, GutterColumn, GutterDecoration, MarginColumn |
| Tab container          | EditorGroup, TabGroup, Pane, TabBar                      |
| Split container        | SplitView, SplitContainer, EditorGrid, PaneGroup         |
| Divider between splits | Splitter, Sash, ResizeHandle, Divider                    |

### Scrolling

| Concept              | Common Names                                             |
| -------------------- | -------------------------------------------------------- |
| Scroll follow mode   | ScrollMode, CursorFollow, RevealMode, AutoScroll         |
| Scroll reveal action | RevealCursor, EnsureVisible, ScrollIntoView, RevealRange |
| Scrollbar thumb      | Thumb, Handle, Knob, Scrubber, Elevator                  |
| Scrollbar track      | Track, Gutter, Trough, Lane, Channel                     |

### Wrapping

| Concept           | Common Names                                      |
| ----------------- | ------------------------------------------------- |
| Soft wrap         | Word Wrap, Line Wrap, Text Wrap                   |
| Wrapped line unit | VisualLine, ScreenLine, DisplayLine, PhysicalLine |
| Document line     | LogicalLine, BufferLine, DocumentLine, TextLine   |
| Wrap segments     | VisualLineSegment, WrapSegment, SubLine, ViewLine |

### Overlays

| Concept            | Common Names                                                |
| ------------------ | ----------------------------------------------------------- |
| Autocomplete popup | CompletionWidget, IntelliSense, Autocomplete, SuggestionBox |
| Hover info         | HoverWidget, Tooltip, QuickInfo, HoverCard                  |
| Parameter hints    | SignatureHelp, ParameterHints, ArgumentHints                |
| Error indicators   | Diagnostics, Squiggles, ErrorMarkers, LintMarkers           |

### Cursor & Selection

| Concept          | Common Names                                |
| ---------------- | ------------------------------------------- |
| Cursor           | Caret, Cursor, InsertionPoint, TextCursor   |
| Selection        | Selection, Range, Highlight, SelectedText   |
| Multi-cursor     | MultiCursor, MultipleSelections, MultiCaret |
| Selection anchor | Anchor, SelectionStart, SelectionOrigin     |
| Selection head   | Head, Cursor, ActiveEnd, SelectionEnd       |

### Units (Standardized in This Document)

| Unit            | Meaning                                        |
| --------------- | ---------------------------------------------- |
| **line**        | 0-indexed document line number                 |
| **column**      | 0-indexed character position within a line     |
| **pixel**       | Screen coordinate (may be logical or physical) |
| **visualLine**  | 0-indexed visual line (after wrapping)         |
| **byte offset** | Position in UTF-8 encoded buffer               |

---

## Chapter 12: Cursor Styles: Pipe, Block, and Underline

Text editors support different cursor (caret) styles. Each style has different geometry, rendering, and behavioral
implications.

### 12.1 The Three Common Styles

```
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   PIPE (Line/Bar)         BLOCK                 UNDERLINE          │
│                                                                    │
│      H e l l o           H e l l o             H e l l o           │
│      │                   █                     _                   │
│      ▲                   ▲                     ▲                   │
│   cursor between       cursor on             cursor under          │
│   'H' and 'e'          'e'                   'e'                   │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

### 12.2 Cursor Style Definitions

```
enum CursorStyle {
    PIPE,       // vertical line between characters (most common in modern editors)
    BLOCK,      // filled rectangle covering one character (vim normal mode)
    UNDERLINE,  // horizontal line under one character
    BLOCK_OUTLINE  // unfilled rectangle (vim for non-focused windows)
}

CursorGeometry {
    style: CursorStyle
    width: Number       // pixels (for PIPE: typically 1-3px)
    height: Number      // pixels (typically lineHeight)
}
```

### 12.3 Semantic Differences

The cursor styles have different **semantic meanings** for the cursor position:

#### Pipe Cursor (Inter-character)

The pipe cursor sits **between** characters. Column N means "between character N-1 and character N":

```
Text:    H  e  l  l  o
Index:   0  1  2  3  4
         ↑  ↑  ↑  ↑  ↑  ↑
Cursor:  0  1  2  3  4  5  (valid columns for a 5-char line)
```

- Column 0: before 'H'
- Column 1: between 'H' and 'e'
- Column 5: after 'o' (end of line)

**Typing inserts at the cursor position.** If cursor is at column 2 and you type 'X':

- Before: "Hello" with cursor at 2
- After: "HeXllo" with cursor at 3

#### Block Cursor (On-character)

The block cursor sits **on** a character. Column N means "character N is highlighted":

```
Text:    H  e  l  l  o
Index:   0  1  2  3  4
         █              (cursor at column 0 highlights 'H')
            █           (cursor at column 1 highlights 'e')
```

**The "current character" is the one under the block.** This affects:

- Delete key: deletes the character under cursor (not after)
- Movement: cursor moves to next/previous character

For vim-style editors, the block cursor represents the character that would be affected by operations like `x` (delete
character), `r` (replace character), etc.

**End-of-line handling:** When at end of line, the block cursor either:

1. Highlights a virtual space character, or
2. Appears as a half-width block after the last character

```
// Block cursor at end of line
Text:    H  e  l  l  o  │█│  ← virtual space or half-block
Index:   0  1  2  3  4   5
```

#### Underline Cursor

Same semantic as block cursor (sits on a character), but rendered as a line underneath:

```
Text:    H  e  l  l  o
         _              (cursor at column 0, under 'H')
```

### 12.4 Geometry Calculations

```
function computeCursorRect(cursor, cursorStyle, charWidth, lineHeight, cursorWidth) {
    const baseX = cursor.column * charWidth
    const baseY = cursor.line * lineHeight

    switch (cursorStyle) {
        case PIPE:
            // Thin vertical line at the left edge of the column position
            return {
                x: baseX - floor(cursorWidth / 2),  // center on column boundary
                y: baseY,
                width: cursorWidth,                  // typically 1-3 pixels
                height: lineHeight
            }

        case BLOCK:
            // Full character cell
            return {
                x: baseX,
                y: baseY,
                width: charWidth,
                height: lineHeight
            }

        case UNDERLINE:
            // Horizontal line at bottom of character cell
            const underlineHeight = max(2, floor(lineHeight * 0.15))
            return {
                x: baseX,
                y: baseY + lineHeight - underlineHeight,
                width: charWidth,
                height: underlineHeight
            }

        case BLOCK_OUTLINE:
            // Same size as BLOCK but will be stroked, not filled
            return {
                x: baseX + 0.5,  // offset for crisp 1px stroke
                y: baseY + 0.5,
                width: charWidth - 1,
                height: lineHeight - 1
            }
    }
}
```

### 12.5 Rendering

```
function renderCursor(cursor, style, x, y, charWidth, lineHeight, cursorColor, ctx) {
    const rect = computeCursorRect(cursor, style, charWidth, lineHeight, cursorWidth)

    // Apply scroll offset
    rect.x -= scrollOffsetX
    rect.y -= scrollOffsetY

    // Skip if off-screen
    if (rect.x + rect.width < 0 || rect.x > viewportWidth) return
    if (rect.y + rect.height < 0 || rect.y > viewportHeight) return

    switch (style) {
        case PIPE:
        case UNDERLINE:
            ctx.fillStyle = cursorColor
            ctx.fillRect(rect.x, rect.y, rect.width, rect.height)
            break

        case BLOCK:
            // Draw filled block
            ctx.fillStyle = cursorColor
            ctx.fillRect(rect.x, rect.y, rect.width, rect.height)

            // Draw the character in inverse color (so it's visible)
            const char = document.getCharAt(cursor.line, cursor.column)
            if (char) {
                ctx.fillStyle = theme.background  // inverse
                ctx.fillText(char, rect.x, rect.y + baseline)
            }
            break

        case BLOCK_OUTLINE:
            ctx.strokeStyle = cursorColor
            ctx.lineWidth = 1
            ctx.strokeRect(rect.x, rect.y, rect.width, rect.height)
            break
    }
}
```

### 12.6 Mode-Based Cursor Styles (Vim-like Editors)

```
function getCursorStyleForMode(mode) {
    switch (mode) {
        case 'normal':
            return BLOCK
        case 'insert':
            return PIPE
        case 'visual':
            return BLOCK
        case 'replace':
            return UNDERLINE
        case 'command':
            return BLOCK  // or PIPE in command line
        default:
            return PIPE
    }
}

// Cursor style can also vary by focus state
function getCursorStyle(mode, isFocused) {
    const baseStyle = getCursorStyleForMode(mode)

    if (!isFocused && baseStyle == BLOCK) {
        return BLOCK_OUTLINE  // unfilled block when not focused
    }

    return baseStyle
}
```

### 12.7 Cursor Width Configuration

```
// Common cursor width configurations
const CURSOR_WIDTHS = {
    thin: 1,        // single pixel line
    normal: 2,      // default for most editors
    thick: 3,       // high visibility
    block: 'auto'   // full character width (for BLOCK style)
}

function resolveCursorWidth(configured, charWidth, style) {
    if (style == BLOCK || style == BLOCK_OUTLINE) {
        return charWidth
    }
    if (configured == 'auto') {
        return 2  // default
    }
    return configured
}
```

### 12.8 Blinking Behavior by Style

Different cursor styles may have different blink behaviors:

```
CursorBlinkConfig {
    pipeBlinkEnabled: Boolean       // typically true
    blockBlinkEnabled: Boolean      // some editors don't blink block cursors
    underlineBlinkEnabled: Boolean  // typically true

    blinkOnDuration: Number         // ms cursor is visible
    blinkOffDuration: Number        // ms cursor is hidden
}

// Alternative: blink rate can differ by style
function getBlinkInterval(style) {
    switch (style) {
        case BLOCK:
            return 600   // slower blink for block
        case PIPE:
        case UNDERLINE:
            return 530   // standard blink
    }
}
```

### 12.9 Multi-Cursor Considerations

With multiple cursors, you may want to differentiate primary vs secondary:

```
function renderMultiCursors(cursors, primaryIndex, style, ctx) {
    for (let i = 0; i < cursors.length; i++) {
        const cursor = cursors[i]
        const isPrimary = (i == primaryIndex)

        // Secondary cursors might be slightly transparent
        const alpha = isPrimary ? 1.0 : 0.7

        renderCursor(cursor, style, cursorColorWithAlpha(alpha), ctx)
    }
}
```

### 12.10 Smooth Cursor Animation

Some editors animate cursor movement:

```
CursorAnimator {
    currentX: Number
    currentY: Number
    targetX: Number
    targetY: Number
    animating: Boolean
    animationDuration: Number = 100  // ms

    moveTo(targetLine, targetColumn, charWidth, lineHeight) {
        this.targetX = targetColumn * charWidth
        this.targetY = targetLine * lineHeight

        if (this.animationEnabled) {
            this.animating = true
            this.animationStart = performance.now()
        } else {
            this.currentX = this.targetX
            this.currentY = this.targetY
        }
    }

    update(now) {
        if (!this.animating) return false

        const elapsed = now - this.animationStart
        const t = min(1.0, elapsed / this.animationDuration)

        // Ease-out curve
        const eased = 1 - pow(1 - t, 3)

        this.currentX = lerp(this.startX, this.targetX, eased)
        this.currentY = lerp(this.startY, this.targetY, eased)

        if (t >= 1.0) {
            this.animating = false
        }

        return true  // needs repaint
    }
}
```

---

## Appendix A: Event Flow Example

User presses Down Arrow while cursor is at bottom margin:

```
1.  KeyDown event captured by editor
2.  Command resolver maps key to action: "cursorDown"
3.  Command dispatcher invokes cursor controller
4.  Cursor position updated: line 45 → line 46
5.  preferredColumn preserved (or set if not already set)
6.  EditorState emits 'cursor-changed' event
7.  Scroll controller receives event, checks cursor vs safe zone:
    - Current safe zone bottom = scrollY + viewportHeight - marginBottom
    - Cursor Y = line 46 * lineHeight
    - Cursor below safe zone? Yes → compute new scrollY
8.  If scroll needed:
    - scrollOffset.y updated to reveal cursor
    - Scrollbar controller notified
    - Scrollbar thumb position recalculated
9.  Viewport marks visible range dirty
10. Render scheduled
11. On next frame:
    - Layout phase computes new visible line range
    - Gutter re-renders (new lines visible, line numbers shift)
    - Text area re-renders (new lines visible)
    - Cursor caret re-renders at new position
    - Scrollbar thumb re-renders at new position
12. Any visible overlays repositioned (if anchor moved)
```

---

## Appendix B: Coordinate Transformation Functions

```
// Document position → viewport pixel position
function documentToViewport(docLine, docColumn, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY) {
    return {
        x: gutterWidth + docColumn * charWidth - scrollOffsetX,
        y: docLine * lineHeight - scrollOffsetY
    }
}

// Viewport pixel position → document position
function viewportToDocument(viewportX, viewportY, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY, document) {
    const contentY = viewportY + scrollOffsetY
    const contentX = viewportX + scrollOffsetX - gutterWidth

    const line = clamp(floor(contentY / lineHeight), 0, document.lineCount - 1)
    const column = clamp(round(contentX / charWidth), 0, document.lineLengths[line])

    return { line, column }
}

// Document position → screen/absolute position
function documentToScreen(docLine, docColumn, editorScreenRect, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY) {
    const viewportPos = documentToViewport(docLine, docColumn, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY)
    return {
        x: editorScreenRect.left + viewportPos.x,
        y: editorScreenRect.top + viewportPos.y
    }
}

// Screen/absolute position → document position
function screenToDocument(screenX, screenY, editorScreenRect, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY, document) {
    const viewportPos = {
        x: screenX - editorScreenRect.left,
        y: screenY - editorScreenRect.top
    }
    return viewportToDocument(viewportPos.x, viewportPos.y, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY, document)
}
```

### With Soft Wrapping

```
// Document position → viewport pixel position (with wrapping)
function documentToViewportWrapped(docLine, docColumn, visualIndex, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY) {
    const mapping = visualIndex.documentToVisual[docLine]

    // Find which segment contains this column
    let visualLine = mapping.firstVisualLine
    let segment = getFirstSegment(docLine)

    while (docColumn >= segment.endColumn && hasNextSegment(docLine, segment)) {
        segment = getNextSegment(docLine, segment)
        visualLine++
    }

    const columnInSegment = docColumn - segment.startColumn

    return {
        x: gutterWidth + segment.wrapIndent + columnInSegment * charWidth - scrollOffsetX,
        y: visualIndex.visualLineOffsets[visualLine] - scrollOffsetY
    }
}

// Viewport pixel position → document position (with wrapping)
function viewportToDocumentWrapped(viewportX, viewportY, visualIndex, lineHeight, charWidth, gutterWidth, scrollOffsetX, scrollOffsetY) {
    const contentY = viewportY + scrollOffsetY
    const contentX = viewportX + scrollOffsetX - gutterWidth

    // Find visual line
    const visualLine = binarySearchFloor(visualIndex.visualLineOffsets, contentY)
    const { documentLine, segmentIndex } = visualIndex.visualToDocument[visualLine]

    // Find column within segment
    const segment = getSegment(documentLine, segmentIndex)
    const localX = contentX - segment.wrapIndent
    const columnInSegment = clamp(round(localX / charWidth), 0, segment.endColumn - segment.startColumn)

    return {
        line: documentLine,
        column: segment.startColumn + columnInSegment
    }
}
```

---

## Appendix C: Binary Search Helpers

```
// Find largest index where array[index] <= target
// Returns 0 if target is below all values (clamps to start)
function binarySearchFloor(array, target) {
    if (array.length == 0) return 0

    let low = 0
    let high = array.length - 1
    let result = 0

    while (low <= high) {
        const mid = floor((low + high) / 2)

        if (array[mid] <= target) {
            result = mid
            low = mid + 1
        } else {
            high = mid - 1
        }
    }

    return result
}

// Find smallest index where array[index] >= target
// Returns array.length - 1 if target is above all values (clamps to end)
function binarySearchCeil(array, target) {
    if (array.length == 0) return 0

    let low = 0
    let high = array.length - 1
    let result = array.length - 1

    while (low <= high) {
        const mid = floor((low + high) / 2)

        if (array[mid] >= target) {
            result = mid
            high = mid - 1
        } else {
            low = mid + 1
        }
    }

    return result
}

// Find exact match or insertion point
function binarySearchExact(array, target) {
    let low = 0
    let high = array.length - 1

    while (low <= high) {
        const mid = floor((low + high) / 2)

        if (array[mid] == target) {
            return { found: true, index: mid }
        } else if (array[mid] < target) {
            low = mid + 1
        } else {
            high = mid - 1
        }
    }

    return { found: false, insertionPoint: low }
}
```

---

## Appendix D: Useful Constants and Defaults

```
// Typical default values (customize per editor)

const DEFAULTS = {
    // Scroll margins (in lines/columns)
    scrollMarginTop: 3,         // lines
    scrollMarginBottom: 3,      // lines
    scrollMarginLeft: 10,       // columns
    scrollMarginRight: 10,      // columns

    // Scrollbar
    scrollbarMinThumbSize: 30,  // pixels
    scrollbarWidth: 14,         // pixels
    scrollbarTrackMargin: 2,    // pixels

    // Cursor
    cursorBlinkRate: 530,       // milliseconds
    cursorWidth: 2,             // pixels (for pipe cursor)
    cursorStyle: 'pipe',        // 'pipe' | 'block' | 'underline'
    cursorSmoothAnimation: false,

    // Autocomplete
    autocompleteMaxItems: 12,
    autocompleteItemHeight: 22, // pixels
    autocompleteMinWidth: 200,  // pixels
    autocompleteMaxWidth: 400,  // pixels
    autocompleteGap: 2,         // pixels from cursor

    // Wrapping
    wrapEnabled: false,
    wrapIndent: 0,              // columns
    wrapAtWordBoundary: true,

    // Virtual scrolling
    virtualScrollBuffer: 5,     // lines above/below viewport

    // Gutter
    gutterMinWidth: 40,         // pixels
    gutterPadding: 8,           // pixels (each side)

    // Line height
    lineHeightMultiplier: 1.5,  // relative to font size

    // Font (monospace assumed)
    fontFamily: 'JetBrains Mono, Consolas, Monaco, monospace',
    fontSize: 14,               // pixels

    // Tab handling
    tabSize: 4,                 // columns
    insertSpaces: true,         // convert tabs to spaces on input
}
```

---

## Appendix E: Unicode and UTF-8 Considerations for Code Editors

This appendix covers the practical considerations for handling text encoding in a code editor focused on programming
languages and monospace fonts.

### E.1 The Three Units of Text

| Unit          | What it is                    | Example                   |
| ------------- | ----------------------------- | ------------------------- |
| **Byte**      | Raw storage unit (UTF-8)      | 'é' = 2 bytes (0xC3 0xA9) |
| **Codepoint** | Unicode scalar value (U+XXXX) | 'é' = U+00E9              |
| **Grapheme**  | User-perceived character      | 'é' = 1 grapheme          |

For **code editors with monospace fonts**, we typically care about:

- **Bytes** for file I/O and buffer offsets
- **Columns** for cursor positioning and display (≈ graphemes for most code)

### E.2 Practical Simplification for Code

For code editing (not prose), you can often treat **column = codepoint index** because:

1. Programming languages use ASCII identifiers (mostly)
2. Non-ASCII typically appears in strings/comments
3. Monospace fonts render each codepoint as one cell (with exceptions)

```
// Simplified column calculation (works for most code)
function columnToByteOffset(lineContent, column) {
    let byteOffset = 0
    let col = 0

    while (col < column && byteOffset < lineContent.byteLength) {
        const byte = lineContent[byteOffset]

        // UTF-8 byte length from first byte
        if ((byte & 0x80) == 0x00) {
            byteOffset += 1  // ASCII
        } else if ((byte & 0xE0) == 0xC0) {
            byteOffset += 2  // 2-byte sequence
        } else if ((byte & 0xF0) == 0xE0) {
            byteOffset += 3  // 3-byte sequence
        } else if ((byte & 0xF8) == 0xF0) {
            byteOffset += 4  // 4-byte sequence
        }

        col++
    }

    return byteOffset
}
```

### E.3 Known Problem Cases

These situations require special handling:

#### Combining Characters

```
// "é" can be represented two ways:
// 1. Single codepoint: U+00E9 (1 column)
// 2. Base + combining: U+0065 U+0301 ('e' + combining acute) - still 1 visual column!

// For code editors, normalize to NFC on load to avoid issues
text = unicodeNormalize(text, 'NFC')
```

#### Emoji in Comments

```
// 👨‍👩‍👧‍👦 (family emoji) = 7 codepoints but 1-2 visual columns
// Most code editors just accept that emojis may look "off" in monospace
// Alternative: measure actual rendered width and adjust

function isEmoji(codepoint) {
    // Simplified emoji detection
    return (codepoint >= 0x1F600 && codepoint <= 0x1F64F) ||  // emoticons
           (codepoint >= 0x1F300 && codepoint <= 0x1F5FF) ||  // misc symbols
           (codepoint >= 0x1F680 && codepoint <= 0x1F6FF) ||  // transport
           (codepoint >= 0x2600 && codepoint <= 0x26FF)       // misc symbols
}
```

#### Wide Characters (CJK)

```
// Chinese/Japanese/Korean characters are typically 2 columns wide
// Not needed if you're not supporting CJK, but here's the concept:

function charWidth(codepoint) {
    if (isFullWidth(codepoint)) {
        return 2
    }
    return 1
}

function isFullWidth(codepoint) {
    // CJK Unified Ideographs
    if (codepoint >= 0x4E00 && codepoint <= 0x9FFF) return true
    // Hiragana, Katakana
    if (codepoint >= 0x3040 && codepoint <= 0x30FF) return true
    // Fullwidth forms
    if (codepoint >= 0xFF00 && codepoint <= 0xFFEF) return true
    return false
}
```

### E.4 Recommended Approach for Code Editors

1. **Store text as UTF-8 bytes** (compact, compatible with files)
2. **Index by byte offset** for internal buffer operations
3. **Convert to codepoints** for cursor/selection (treat each codepoint as 1 column)
4. **Normalize to NFC** on file load to avoid combining character issues
5. **Don't worry about** grapheme clusters for most code editing use cases

```
Document {
    // Raw UTF-8 bytes
    bytes: ByteBuffer

    // Cached line info
    lineOffsets: Array<Number>      // byte offset of each line start
    lineCodepointCounts: Array<Number>  // codepoint count per line (for column validation)
}
```

### E.5 Testing Your Implementation

Test cases to verify correct UTF-8 handling:

```
// ASCII
"hello"                     // 5 bytes, 5 columns

// Accented characters
"café"                      // 5 bytes (c,a,f,é,null), 4 columns

// Multi-byte characters
"日本語"                    // 9 bytes, 3 columns (if treating as 1 col each)

// Mixed
"hello_世界"                // 5 + 1 + 6 = 12 bytes, 8 columns

// Edge cases
""                          // empty string
"\n"                        // just newline
"a\nb"                      // with embedded newline
```

---

## Appendix F: Right-to-Left and Bidirectional Text (Theory)

> **Note:** This appendix is provided for completeness. The main document assumes left-to-right (LTR) text only.
> Implementing full BiDi support is a substantial undertaking beyond the scope of a code editor focused on
> programming languages.

### F.1 The Problem

When mixing left-to-right (LTR) text with right-to-left (RTL) text (Arabic, Hebrew), the visual order of characters
differs from their logical (memory) order.

```
Logical order: "Hello שלום World"
               H e l l o   ש ל ו ם   W o r l d
               0 1 2 3 4 5 6 7 8 9 10 11 12 13 14

Visual order:  "Hello םולש World"
               H e l l o   ם ו ל ש   W o r l d
```

The Hebrew word "שלום" (shalom) is stored left-to-right in memory but displayed right-to-left.

### F.2 Key Concepts

**Logical Position:** Index in the character buffer (always left-to-right in memory)

**Visual Position:** Where the character appears on screen (may be reversed for RTL text)

**BiDi Level:** Unicode Bidirectional Algorithm assigns each character a level:

- Even levels (0, 2, 4...): left-to-right
- Odd levels (1, 3, 5...): right-to-left

### F.3 Impact on Editor Operations

#### Cursor Movement

```
// Arrow keys should move in VISUAL order, not logical order
function moveCursorRight(cursor, bidiLevels) {
    // Find current character's visual position
    const visualPos = logicalToVisual(cursor.column, bidiLevels)

    // Move one visual position right
    const newVisualPos = visualPos + 1

    // Convert back to logical
    return visualToLogical(newVisualPos, bidiLevels)
}
```

#### Hit Testing

```
// Click position must account for visual reordering
function hitTest(clickX, line, bidiLevels, charWidth) {
    // clickX is in visual coordinates
    const visualColumn = round(clickX / charWidth)

    // Convert to logical column for cursor position
    return visualToLogical(visualColumn, bidiLevels)
}
```

#### Selection Rendering

```
// Selection may be non-contiguous visually!
// "Hello שלום World" with logical selection [6,10] (the Hebrew word)
// Visual rendering: highlight appears in the middle but reads right-to-left
```

#### Cursor Rendering

```
// Cursor shape may indicate text direction at that position
function getCursorShape(column, bidiLevels) {
    const level = bidiLevels[column]
    if (level % 2 == 1) {
        // RTL: cursor might lean left or have different shape
        return CURSOR_RTL
    }
    return CURSOR_LTR
}
```

### F.4 Why Code Editors Often Skip BiDi

1. **Programming languages are LTR:** Keywords, identifiers, operators are all left-to-right
2. **Complexity:** Full BiDi support requires implementing UAX #9 (Unicode Bidirectional Algorithm)
3. **Limited use case:** RTL text in code is mostly in strings/comments
4. **Workaround exists:** Users can use separate tools for RTL content

### F.5 Minimal BiDi Support (Compromise)

If you want basic RTL support without full BiDi:

1. **Detect RTL strings/comments** and render them in a separate pass
2. **Disable cursor movement** within RTL segments (jump over them)
3. **Use a BiDi library** (like ICU, fribidi, or unicode-bidi crate) for the heavy lifting

```
// Example: Using a BiDi library
function renderLineWithBidi(line, lineContent) {
    const bidiResult = bidiLibrary.process(lineContent)

    for (const run of bidiResult.runs) {
        // Each run is a sequence of characters with same direction
        if (run.isRTL) {
            // Render reversed
            renderTextRTL(run.text, run.visualStart)
        } else {
            renderTextLTR(run.text, run.visualStart)
        }
    }
}
```

### F.6 Libraries for BiDi

If you decide to implement BiDi support:

- **Rust:** `unicode-bidi` crate
- **C/C++:** ICU (International Components for Unicode), fribidi
- **Go:** `golang.org/x/text/unicode/bidi`

---

_End of document_
