import type { ImageMetadata } from 'astro';

import imgHero from '../assets/screenshots/screenshot-hero.png';
import imgMultiCursor from '../assets/screenshots/screenshot-multi-cursor.png';
import imgSplits from '../assets/screenshots/screenshot-splits.png';
import imgCsv from '../assets/screenshots/screenshot-csv.png';
import imgMinimal from '../assets/screenshots/screenshot-minimal.png';
import imgDracula from '../assets/screenshots/screenshot-showcase-dracula.png';
import imgNord from '../assets/screenshots/screenshot-showcase-nord.png';
import imgGruvbox from '../assets/screenshots/screenshot-showcase-gruvbox.png';
import imgTokyoNight from '../assets/screenshots/screenshot-showcase-tokyo-night.png';
import imgMocha from '../assets/screenshots/screenshot-showcase-mocha.png';
import imgGithubLight from '../assets/screenshots/screenshot-showcase-github-light.png';
import imgGithubDark from '../assets/screenshots/screenshot-showcase-github-dark.png';
import imgVerticalSplits from '../assets/screenshots/screenshot-showcase-vertical-splits.png';
import imgPolyglot from '../assets/screenshots/screenshot-showcase-polyglot.png';
import imgMixedSplits from '../assets/screenshots/screenshot-showcase-mixed-splits.png';
import imgMulticursorTs from '../assets/screenshots/screenshot-showcase-multicursor-ts.png';
import imgSidebar from '../assets/screenshots/screenshot-showcase-sidebar.png';
import imgSidebarSplits from '../assets/screenshots/screenshot-showcase-sidebar-splits.png';
import imgCommandPalette from '../assets/screenshots/screenshot-showcase-command-palette.png';
import imgFindReplace from '../assets/screenshots/screenshot-showcase-find-replace.png';
import imgGotoLine from '../assets/screenshots/screenshot-showcase-goto-line.png';
import imgThemePicker from '../assets/screenshots/screenshot-showcase-theme-picker.png';
import imgSelection from '../assets/screenshots/screenshot-showcase-selection.png';
import imgCsvSplit from '../assets/screenshots/screenshot-showcase-csv-split.png';
import imgMarkdownPreview from '../assets/screenshots/screenshot-showcase-markdown-preview.png';
import imgHtmlPreview from '../assets/screenshots/screenshot-showcase-html-preview.png';
import imgFractalSplits from '../assets/screenshots/screenshot-showcase-fractal-splits.png';

export interface ShowcaseItem {
  id: string;
  src: ImageMetadata;
  title: string;
  description: string;
  tags: string[];
  feature?: string;
}

export const showcase: ShowcaseItem[] = [
  {
    id: 'hero',
    src: imgHero,
    title: 'Split Editing — Rust + YAML',
    description:
      'Side-by-side split view editing Rust source code alongside a YAML configuration file. Fleet Dark theme with Tree-sitter syntax highlighting.',
    tags: ['Splits'],
    feature: '⌘+\\ to split',
  },
  {
    id: 'multi-cursor',
    src: imgMultiCursor,
    title: 'Multi-Cursor Editing',
    description:
      'Three cursors editing Rust struct fields simultaneously. All cursors move and edit in sync — overlapping cursors merge automatically.',
    tags: ['Multi-cursor'],
    feature: '⌥+Click to add cursor',
  },
  {
    id: 'splits',
    src: imgSplits,
    title: 'Three-Way Split',
    description:
      'Three horizontal split panes showing Rust, TypeScript, and Python side by side. Each pane has its own viewport and scroll position.',
    tags: ['Splits', 'Languages'],
  },
  {
    id: 'csv',
    src: imgCsv,
    title: 'CSV Spreadsheet View',
    description:
      'CSV file rendered as a proper data table with column headers, cell selection, and inline editing. Navigate with arrow keys like a spreadsheet.',
    tags: ['CSV'],
    feature: 'Arrow keys to navigate cells',
  },
  {
    id: 'minimal',
    src: imgMinimal,
    title: 'Minimal Rust Editing',
    description:
      'Clean single-file editing experience with line numbers, syntax highlighting, and zero distractions. Fleet Dark theme.',
    tags: ['Languages'],
  },
  {
    id: 'showcase-dracula',
    src: imgDracula,
    title: 'Dracula Theme — Go',
    description:
      'Go source code highlighted with the Dracula color theme. Rich purples, pinks, and greens on a dark background.',
    tags: ['Themes', 'Languages'],
  },
  {
    id: 'showcase-nord',
    src: imgNord,
    title: 'Nord Theme — Python',
    description:
      'Python code with the Nord color palette. Cool arctic blues and muted pastels inspired by the polar night.',
    tags: ['Themes', 'Languages'],
  },
  {
    id: 'showcase-gruvbox',
    src: imgGruvbox,
    title: 'Gruvbox Dark — Java',
    description:
      'Java code in the Gruvbox Dark color scheme. Warm, retro-inspired earthy tones with excellent readability.',
    tags: ['Themes', 'Languages'],
  },
  {
    id: 'showcase-tokyo-night',
    src: imgTokyoNight,
    title: 'Tokyo Night — TypeScript',
    description:
      'TypeScript highlighted with Tokyo Night. Neon blues, pinks, and greens inspired by the lights of downtown Tokyo.',
    tags: ['Themes', 'Languages'],
  },
  {
    id: 'showcase-mocha',
    src: imgMocha,
    title: 'Catppuccin Mocha — Rust',
    description:
      'Rust code in the Catppuccin Mocha palette. Soothing pastel colors on a warm dark background.',
    tags: ['Themes', 'Languages'],
  },
  {
    id: 'showcase-github-light',
    src: imgGithubLight,
    title: 'GitHub Light — JavaScript',
    description:
      'JavaScript with the GitHub Light theme. Clean white background with familiar GitHub-style syntax colors for daytime coding.',
    tags: ['Themes', 'Languages'],
  },
  {
    id: 'showcase-github-dark',
    src: imgGithubDark,
    title: 'GitHub Dark — HTML + CSS Split',
    description:
      'Vertical split with HTML and CSS files side by side in GitHub Dark theme. Great for web development workflows.',
    tags: ['Themes', 'Splits', 'Languages'],
  },
  {
    id: 'showcase-vertical-splits',
    src: imgVerticalSplits,
    title: 'Vertical Split — Rust + Python',
    description:
      'Vertical split layout with Rust on top and Python on the bottom. Useful for comparing implementations across languages.',
    tags: ['Splits', 'Languages'],
  },
  {
    id: 'showcase-polyglot',
    src: imgPolyglot,
    title: 'Polyglot — Go, Java, C',
    description:
      'Three-way split with Go, Java, and C source files. Token handles syntax highlighting for each language independently via Tree-sitter.',
    tags: ['Splits', 'Languages'],
  },
  {
    id: 'showcase-mixed-splits',
    src: imgMixedSplits,
    title: 'Mixed Splits — Rust, TypeScript, YAML',
    description:
      'Nested split layout with Rust on the left, TypeScript top-right, and YAML bottom-right. Horizontal and vertical splits can be freely combined.',
    tags: ['Splits', 'Languages'],
    feature: '⌘+\\ horizontal, ⌘+⇧+\\ vertical',
  },
  {
    id: 'showcase-multicursor-ts',
    src: imgMulticursorTs,
    title: 'Five Cursors — TypeScript',
    description:
      'Five simultaneous cursors editing TypeScript code. Select next match with ⌘+J, undo last cursor with ⌘+⇧+J.',
    tags: ['Multi-cursor', 'Languages'],
    feature: '⌘+J select next match',
  },
  {
    id: 'showcase-sidebar',
    src: imgSidebar,
    title: 'Workspace File Tree',
    description:
      'Full workspace view with the sidebar file tree expanded, showing project structure alongside the editor. Toggle with ⌘+1.',
    tags: ['Workspace'],
    feature: '⌘+1 toggle sidebar',
  },
  {
    id: 'showcase-sidebar-splits',
    src: imgSidebarSplits,
    title: 'Sidebar + Split Views',
    description:
      'Workspace sidebar combined with split editing — Rust and TypeScript side by side with the full project tree visible.',
    tags: ['Workspace', 'Splits'],
  },
  {
    id: 'showcase-command-palette',
    src: imgCommandPalette,
    title: 'Theme Picker',
    description:
      'The theme picker modal showing all 9 built-in themes. Browse and preview themes instantly with arrow keys.',
    tags: ['UI'],
    feature: '⌘+⇧+A → "theme"',
  },
  {
    id: 'showcase-find-replace',
    src: imgFindReplace,
    title: 'Find & Replace',
    description:
      'Find and replace dialog with case-sensitive search. Supports regex, whole word matching, and replace-all.',
    tags: ['UI'],
    feature: '⌘+F find & replace',
  },
  {
    id: 'showcase-goto-line',
    src: imgGotoLine,
    title: 'Go to Line',
    description:
      'Quick navigation to any line number. Type a number and press Enter to jump instantly.',
    tags: ['UI'],
    feature: '⌘+L go to line',
  },
  {
    id: 'showcase-theme-picker',
    src: imgThemePicker,
    title: 'Theme Selection',
    description:
      'Browsing built-in themes with live preview. The current theme is marked with a checkmark while you explore alternatives.',
    tags: ['Themes', 'UI'],
  },
  {
    id: 'showcase-selection',
    src: imgSelection,
    title: 'Select Next Match',
    description:
      'Five occurrences of "name" selected simultaneously across the file using Select Next Match. Each selection highlights the exact word.',
    tags: ['Multi-cursor'],
    feature: '⌘+J select next match',
  },
  {
    id: 'showcase-csv-split',
    src: imgCsvSplit,
    title: 'CSV + Code Split',
    description:
      'CSV spreadsheet view side by side with Rust source code. View data and implementation together in a single workspace.',
    tags: ['CSV', 'Splits'],
  },
  {
    id: 'showcase-markdown-preview',
    src: imgMarkdownPreview,
    title: 'Markdown Preview',
    description:
      'Live markdown preview alongside the source file. Headings, lists, and code blocks render in real time as you type.',
    tags: ['Preview'],
    feature: '⌘+⇧+P toggle preview',
  },
  {
    id: 'showcase-html-preview',
    src: imgHtmlPreview,
    title: 'HTML Preview + JavaScript',
    description:
      'HTML source with live preview on the right, JavaScript below. Nested horizontal and vertical splits for a full web development layout.',
    tags: ['Preview', 'Splits', 'Languages'],
  },
  {
    id: 'showcase-fractal-splits',
    src: imgFractalSplits,
    title: 'Fractal Splits — 11 Languages',
    description:
      'Ten levels of recursive splits alternating horizontal and vertical, creating a fractal spiral with Rust, TypeScript, Python, Go, Java, C, HTML, CSS, JSON, YAML, and Bash — because we can.',
    tags: ['Splits', 'Languages'],
  },
];
