/*
 * The component-first subjects used by the mockup gallery focus mode.
 * Selectors are evaluated inside #specimen .specimen-body by the gallery
 * runtime, so each deliberately names only the primitive and its documented
 * state variants—not the surrounding editor, form, or explanatory prose.
 */
window.TOKEN_MOCKUP_SUBJECTS = Object.freeze({
  'ACTIVITY-RAIL': {
    selectors: ['.rail', '.mini-rail'],
    description: 'The two edge rails and the compact selected/focused state study.'
  },
  'BADGE': {
    selectors: ['.kind-mark', '.badge-problems-head > .badge', '.ide-state .badge'],
    description: 'Completion-kind marks and the explicitly documented count-badge projections.'
  },
  'BREADCRUMBS': {
    selectors: ['.crumbbar', '.narrow-crumb'],
    description: 'The document-location bars, including the constrained-width overflow study.'
  },
  'BUTTON': {
    selectors: ['.ide-action .btn', '.ide-footer .btn', '.ide-state > .btn'],
    description: 'Action and commit buttons plus their labelled paint-state variants.'
  },
  'CHECKBOX': {
    selectors: ['input[type="checkbox"]'],
    description: 'Checkbox marks only; their setting-row labels remain contextual ownership.'
  },
  'COMBOBOX': {
    selectors: ['.combo-input', '.combo-popup', '.combo-mini'],
    description: 'The editable face, its open option surface, and compact field-state variants.'
  },
  'DIALOG': {
    selectors: ['.dialog-box'],
    description: 'The complete modal decision surface; surrounding decision notes remain context.'
  },
  'DOCKABLE-PANEL': {
    selectors: ['.floating-panel', '.docked-panel'],
    description: 'The same performance-panel identity in floating and docked placements.'
  },
  'DOCUMENTATION-CARD': {
    selectors: ['.docs-card'],
    description: 'The anchored reading card; completion rows and editor remain placement context.'
  },
  'EDITOR-SURFACE': {
    selectors: ['.editor-main'],
    description: 'The composite active editor surface, whose tab, path, gutter, text, overlays, and status layers solve as one region.'
  },
  'EMPTY-STATE': {
    selectors: ['.empty-message', '.empty-variant'],
    description: 'Feature-owned empty-message projections and their scoped variants.'
  },
  'FORM': {
    selectors: ['.field-pane', '.record-list'],
    description: 'The typed settings form with its record list, labels, draft fields, validation, and commit controls; the flow notes are explanatory context.'
  },
  'FOUNDATIONS': {
    selectors: ['.type-board', '.token-list', '.space-board', '.foundation-states'],
    description: 'The typography, semantic-token, spacing, and state reference boards that together define the foundation.'
  },
  'GROUP-HEADER': {
    selectors: ['.advanced-disclosure', '.header-sample'],
    description: 'The expanded Advanced disclosure and the standalone current, focused, and collapsed header variants.'
  },
  'ICON': {
    selectors: ['.icon-scene svg.icon', '.ide-state svg.icon'],
    description: 'The visible semantic glyph specimens in their editor, explorer, panel, and state contexts.'
  },
  'KEYCAP': {
    selectors: ['.keycap', '.ide-state .mono.dim'],
    description: 'Shortcut keycap chips and the specific dim-text overflow binding specimen, not command rows or their popup.'
  },
  'LABEL': {
    selectors: ['.ide-form-row > span:first-child:not(:empty)', '.ide-form-row .ide-hint', '.validation', '.metadata-row > span:not(.meta-detail)', '.metadata-row .meta-detail', '.label-sample'],
    description: 'Settings captions, help and validation text, list primary/detail labels, and compact label-role specimens—never their whole rows or icons.'
  },
  'LINK': {
    selectors: ['.term-link', '.link-button', '.link-state a'],
    description: 'Inline terminal, button-style, and state-variant links only.'
  },
  'LIST': {
    selectors: ['.row-viewport', '.compact-list', '.empty-row'],
    description: 'The uniform-row viewport, compact focus/disabled rows, and empty projection.'
  },
  'MENU': {
    selectors: ['.menu', '.compact-menu'],
    description: 'The open menu surfaces and compact contextual menu variant.'
  },
  'NOTIFICATION': {
    selectors: ['.notice-stack', '.notification-bottom'],
    description: 'The operation-owned notification stack and bottom notification projection.'
  },
  'PANE-CHROME': {
    selectors: ['.pane-header', '.pane-footer', '.mini-header'],
    description: 'Pane header and footer runs, including their compact header variants; pane bodies are context.'
  },
  'PANEL': {
    selectors: ['.dock'],
    description: 'The selected Problems panel in its dock host; the adjacent anatomy table is explanatory context.'
  },
  'POPUP': {
    selectors: ['.completion', '.mini-popup'],
    description: 'Completion popup surfaces and their smaller anchored variants.'
  },
  'PROGRESS': {
    selectors: ['.progress-query', '.progress-row', '.progress-status', '.proposed-meter'],
    description: 'The feature-owned query, indeterminate operation rows, status projection, and explicitly proposed determinate meter.'
  },
  'RADIO-GROUP': {
    selectors: ['input[type="radio"]'],
    description: 'Radio marks only; the chooser labels and form rows supply the decision context.'
  },
  'SCROLL-AREA': {
    selectors: ['.scroll-track', '.h-track', '.end-track'],
    description: 'Vertical, horizontal, and endpoint scrollbar tracks with their visible thumbs.'
  },
  'SEARCH-FIELD': {
    selectors: ['.findbar', '.mini-search'],
    description: 'The in-editor find field and its compact search-state counterparts.'
  },
  'SECTION-NAVIGATION': {
    selectors: ['.section-nav', '.proposed-nav', '.grid-nav'],
    description: 'The current section-navigation lists and separate proposed focus sample, without the settings content they select.'
  },
  'SEGMENTED-CONTROL': {
    selectors: ['.segments'],
    description: 'Segment strips and their labelled compact selected/focus state variants.'
  },
  'SELECT': {
    selectors: ['.select-face', '.select-popup', '.select-mini'],
    description: 'Select faces, their open option surface, and compact state variants.'
  },
  'SPLIT-BUTTON': {
    selectors: ['.split-main', '.split-disclosure', '.split-popup', '.split-mini'],
    description: 'Main action and disclosure halves, the alternatives menu, and joined state variants.'
  },
  'SPLITTER': {
    selectors: ['.split-bar', '.hit-demo'],
    description: 'The narrow divider strip and its focused hit-target demonstration; adjacent panes remain context.'
  },
  'STATUS-BAR': {
    selectors: ['.native-status'],
    description: 'The full-width bottom status run and its feature-owned segments.'
  },
  'TABLE': {
    selectors: ['.csv-grid', '.micro-grid'],
    description: 'The two-axis data grid and compact selected-cell reference grid.'
  },
  'TABS': {
    selectors: ['.doc-tabs', '.dock-tabbar', '.terminal-bar', '.overlay-tabs'],
    description: 'The four tab-strip families only—document, dock, terminal, and overlay—not their selected bodies.'
  },
  'TEXT-FIELD': {
    selectors: ['.field-sample'],
    description: 'Text-field faces, including focused, selected, invalid, and unavailable visual states.'
  },
  'TOGGLE-SWITCH': {
    selectors: ['input.switch[type="checkbox"]'],
    description: 'Switch tracks and thumbs only; live-panel rows remain their owner context.'
  },
  'TOOLBAR': {
    selectors: ['.ide-toolstrip', '.toolbar-narrow', '.toolbar-state-row', '.terminal-chrome'],
    description: 'Toolbar action runs, narrow overflow study, state row, and terminal action chrome—not outline content.'
  },
  'TOOLTIP': {
    selectors: ['.tooltip-bubble'],
    description: 'The anchored tooltip bubbles only; triggers and editor content remain placement context.'
  },
  'TREE': {
    selectors: ['.tree-content'],
    description: 'The three tree viewports: text-only Markdown headings, class inheritance with optional symbols, and structured keys and values. Host headers and explanatory captions remain context.'
  }
});
