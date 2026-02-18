export interface Keybinding {
  action: string;
  keys: string;
  command: string;
  when?: string[];
  platform?: string;
  notes?: string;
}

export interface KeybindingCategory {
  id: string;
  label: string;
  icon: string;
  items: Keybinding[];
}

export const keybindings: KeybindingCategory[] = [
  {
    id: "files",
    label: "Files",
    icon: "📁",
    items: [
      { action: "Save", keys: "⌘S", command: "SaveFile" },
      { action: "Save As", keys: "⌘⇧S", command: "SaveFileAs" },
      { action: "Open File", keys: "⌘O", command: "OpenFile" },
      { action: "Go to File", keys: "⌘⇧O", command: "FuzzyFileFinder" },
      { action: "New Tab", keys: "⌘⇧N", command: "NewTab" },
      { action: "Close Tab", keys: "⌘W", command: "CloseTab" },
    ],
  },
  {
    id: "editing",
    label: "Editing",
    icon: "✏️",
    items: [
      { action: "Undo", keys: "⌘Z", command: "Undo" },
      { action: "Redo", keys: "⌘⇧Z", command: "Redo" },
      { action: "Copy", keys: "⌘C", command: "Copy" },
      { action: "Cut", keys: "⌘X", command: "Cut" },
      { action: "Paste", keys: "⌘V", command: "Paste" },
      { action: "Select All", keys: "⌘A", command: "SelectAll" },
      { action: "Duplicate", keys: "⌘D", command: "Duplicate" },
      { action: "Delete Line", keys: "⌘⌫", command: "DeleteLine" },
      { action: "Delete Word Backward", keys: "⌥⌫", command: "DeleteWordBackward" },
      { action: "Delete Word Forward", keys: "⌥⌦", command: "DeleteWordForward" },
      { action: "Indent", keys: "⇥", command: "IndentLines", when: ["has_selection"] },
      { action: "Unindent", keys: "⇧⇥", command: "UnindentLines" },
      { action: "Insert Newline", keys: "↩", command: "InsertNewline" },
    ],
  },
  {
    id: "selection",
    label: "Selection",
    icon: "🔤",
    items: [
      { action: "Add Cursor", keys: "⌥+Click", command: "AddCursor", notes: "Click while holding Option to add a cursor" },
      { action: "Select Next Occurrence", keys: "⌘J", command: "SelectNextOccurrence" },
      { action: "Unselect Last", keys: "⌘⇧J", command: "UnselectOccurrence" },
      { action: "Expand Selection", keys: "⌥↑", command: "ExpandSelection" },
      { action: "Shrink Selection", keys: "⌥↓", command: "ShrinkSelection" },
      { action: "Collapse to Single Cursor", keys: "⎋", command: "CollapseToSingleCursor", when: ["has_multiple_cursors"] },
      { action: "Clear Selection", keys: "⎋", command: "ClearSelection", when: ["has_selection", "single_cursor"] },
    ],
  },
  {
    id: "navigation",
    label: "Navigation",
    icon: "🧭",
    items: [
      { action: "Command Palette", keys: "⌘⇧A", command: "ToggleCommandPalette" },
      { action: "Go to Line", keys: "⌘L", command: "ToggleGotoLine" },
      { action: "Find / Replace", keys: "⌘F", command: "ToggleFindReplace" },
      { action: "Word Left", keys: "⌥←", command: "MoveCursorWordLeft" },
      { action: "Word Right", keys: "⌥→", command: "MoveCursorWordRight" },
      { action: "Line Start", keys: "Home", command: "MoveCursorLineStart" },
      { action: "Line End", keys: "End", command: "MoveCursorLineEnd" },
      { action: "Document Start", keys: "⌃Home", command: "MoveCursorDocumentStart" },
      { action: "Document End", keys: "⌃End", command: "MoveCursorDocumentEnd" },
      { action: "Page Up", keys: "Page Up", command: "PageUp" },
      { action: "Page Down", keys: "Page Down", command: "PageDown" },
    ],
  },
  {
    id: "workspace",
    label: "Workspace",
    icon: "🪟",
    items: [
      { action: "Toggle File Explorer", keys: "⌘1", command: "ToggleFileExplorer" },
      { action: "Reveal in Sidebar", keys: "⌘⇧R", command: "RevealInSidebar" },
      { action: "Split Horizontal", keys: "⌘⇧⌥H", command: "SplitHorizontal" },
      { action: "Split Vertical", keys: "⌘⇧⌥V", command: "SplitVertical" },
      { action: "Next Tab", keys: "⌘⌥→", command: "NextTab" },
      { action: "Prev Tab", keys: "⌘⌥←", command: "PrevTab" },
      { action: "Focus Next Group", keys: "⌃⇥", command: "FocusNextGroup" },
      { action: "Focus Group 1", keys: "⌘⇧1", command: "FocusGroup1" },
      { action: "Focus Group 2", keys: "⌘⇧2", command: "FocusGroup2" },
      { action: "Focus Group 3", keys: "⌘⇧3", command: "FocusGroup3" },
      { action: "Focus Group 4", keys: "⌘⇧4", command: "FocusGroup4" },
      { action: "Toggle Terminal", keys: "⌘2", command: "ToggleTerminal" },
      { action: "Toggle Outline", keys: "⌘7", command: "ToggleOutline" },
    ],
  },
];
