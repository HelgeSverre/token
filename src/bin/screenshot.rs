//! Screenshot generator for Token editor
//!
//! Renders headless screenshots from YAML scenario definitions.
//!
//! Usage:
//!   cargo run --bin screenshot -- --scenario screenshots/scenarios/basic.yaml
//!   cargo run --bin screenshot -- --all
//!   cargo run --bin screenshot -- --all --out-dir screenshots/output

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;

use token::csv::{detect_delimiter, parse_csv, CsvState, Delimiter};
use token::messages::{LayoutMsg, Msg};
use token::model::document::Document;
use token::model::editor::{EditorState, Position, Selection, ViewMode};
use token::model::editor_area::{EditorArea, Rect, SplitDirection};
use token::model::ui::{
    CommandPaletteState, FindReplaceState, GotoLineState, ModalState, ThemePickerState, UiState,
};
use token::model::workspace::Workspace;
use token::model::AppModel;
use token::model::ScaledMetrics;
use token::syntax::{LanguageId, ParserState};
use token::theme::Theme;
use token::update::update;
use token::view::{Frame, GlyphCache, PreviewRenderMode, Renderer, TextPainter};

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(name = "screenshot", about = "Generate screenshots of Token editor")]
struct Args {
    /// Path to a single scenario YAML file
    #[arg(long)]
    scenario: Option<PathBuf>,
    /// Run all scenarios in the scenarios directory
    #[arg(long)]
    all: bool,
    /// Directory containing scenario YAML files
    #[arg(long, default_value = "screenshots/scenarios")]
    scenarios_dir: PathBuf,
    /// Directory for output PNG files
    #[arg(long, default_value = "website/src/assets/screenshots")]
    out_dir: PathBuf,
    /// Override theme (file path or builtin id)
    #[arg(long)]
    theme: Option<String>,
    /// Override width in physical pixels
    #[arg(long)]
    width: Option<u32>,
    /// Override height in physical pixels
    #[arg(long)]
    height: Option<u32>,
}

// ---------------------------------------------------------------------------
// Scenario YAML schema
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct Scenario {
    name: String,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_scale")]
    scale: f64,
    #[serde(default)]
    theme: Option<String>,
    files: Vec<ScenarioFile>,
    #[serde(default)]
    split_direction: SplitDir,
    #[serde(default)]
    workspace: Option<WorkspaceConfig>,
    #[serde(default)]
    modal: Option<ModalConfig>,
}

#[derive(Deserialize, Debug)]
struct ScenarioFile {
    path: PathBuf,
    #[serde(default)]
    scroll_to: Option<usize>,
    #[serde(default)]
    cursor_line: Option<usize>,
    #[serde(default)]
    cursor_column: Option<usize>,
    #[serde(default)]
    extra_cursors: Vec<CursorPos>,
    #[serde(default)]
    selections: Vec<SelectionRange>,
    #[serde(default)]
    view_mode: Option<ScenarioViewMode>,
    /// Per-file split direction override (defaults to scenario-level split_direction)
    #[serde(default)]
    split: Option<SplitDir>,
    /// Ratio for this file's side of the split (0.0-1.0, default 0.5)
    #[serde(default)]
    split_ratio: Option<f32>,
    /// Open a preview pane (markdown/HTML) next to this file's editor
    #[serde(default)]
    preview: bool,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum ScenarioViewMode {
    Csv,
}

#[derive(Deserialize, Debug)]
struct CursorPos {
    line: usize,
    column: usize,
}

#[derive(Deserialize, Debug)]
struct SelectionRange {
    anchor_line: usize,
    anchor_column: usize,
    head_line: usize,
    head_column: usize,
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "lowercase")]
enum SplitDir {
    #[default]
    Horizontal,
    Vertical,
}

// ---------------------------------------------------------------------------
// Workspace config
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct WorkspaceConfig {
    root: PathBuf,
    #[serde(default = "default_sidebar_visible")]
    sidebar_visible: bool,
    #[serde(default)]
    sidebar_width: Option<f32>,
    #[serde(default)]
    expanded: Vec<PathBuf>,
    #[serde(default)]
    selected: Option<PathBuf>,
    #[serde(default)]
    scroll_offset: usize,
}

fn default_sidebar_visible() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Modal config
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct ModalConfig {
    id: ModalId,
    #[serde(default)]
    input: Option<String>,
    #[serde(default)]
    selected_index: Option<usize>,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    replacement: Option<String>,
    #[serde(default = "default_true")]
    replace_mode: bool,
    #[serde(default)]
    case_sensitive: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
enum ModalId {
    CommandPalette,
    GotoLine,
    FindReplace,
    ThemePicker,
}

fn default_width() -> u32 {
    2880
}
fn default_height() -> u32 {
    1800
}
fn default_scale() -> f64 {
    2.0
}

// ---------------------------------------------------------------------------
// Theme loading
// ---------------------------------------------------------------------------

fn load_theme_for_scenario(theme_override: Option<&str>, scenario_theme: Option<&str>) -> Theme {
    let theme_ref = theme_override.or(scenario_theme);
    match theme_ref {
        Some(t) => {
            // Try as file path first
            let path = PathBuf::from(t);
            if path.exists() {
                match token::theme::from_file(&path) {
                    Ok(theme) => return theme,
                    Err(e) => eprintln!("Warning: failed to load theme file {}: {}", t, e),
                }
            }
            // Try as builtin id
            match token::theme::load_theme(t) {
                Ok(theme) => theme,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to load theme '{}': {}, using default",
                        t, e
                    );
                    Theme::default()
                }
            }
        }
        None => Theme::default(),
    }
}

// ---------------------------------------------------------------------------
// Model creation
// ---------------------------------------------------------------------------

fn create_model_from_scenario(scenario: &Scenario, theme: Theme) -> Result<AppModel> {
    let scale = scenario.scale;
    let font = setup_font(scale);
    let line_height = font.line_height;
    let char_width = font.char_width;

    let status_bar_height = line_height;
    let visible_lines = (scenario.height as usize).saturating_sub(status_bar_height) / line_height;
    let visible_columns = ((scenario.width as f32 - 60.0) / char_width).floor() as usize;

    // Load first file
    let first = scenario
        .files
        .first()
        .context("scenario must have at least one file")?;
    let content = std::fs::read_to_string(&first.path)
        .with_context(|| format!("reading {}", first.path.display()))?;

    let mut document = Document::with_text(&content);
    document.file_path = Some(first.path.clone());
    document.language = LanguageId::from_path(&first.path);

    let mut editor = EditorState::with_viewport(visible_lines, visible_columns);
    apply_cursor_and_scroll(&mut editor, first);

    let editor_area = EditorArea::single_document(document, editor);

    let mut model = AppModel {
        editor_area,
        ui: UiState::new(),
        theme,
        config: token::config::EditorConfig::default(),
        window_size: (scenario.width, scenario.height),
        line_height,
        char_width,
        metrics: ScaledMetrics::new(scale),
        workspace: None,
        dock_layout: token::panel::DockLayout::default(),
        #[cfg(debug_assertions)]
        debug_overlay: None,
    };

    // Add additional files as splits
    let default_direction = match scenario.split_direction {
        SplitDir::Horizontal => SplitDirection::Horizontal,
        SplitDir::Vertical => SplitDirection::Vertical,
    };

    for file in scenario.files.iter().skip(1) {
        let file_content = std::fs::read_to_string(&file.path)
            .with_context(|| format!("reading {}", file.path.display()))?;

        // Use per-file split direction if specified, otherwise use scenario default
        let direction = file
            .split
            .as_ref()
            .map(|d| match d {
                SplitDir::Horizontal => SplitDirection::Horizontal,
                SplitDir::Vertical => SplitDirection::Vertical,
            })
            .unwrap_or(default_direction);

        // Split creates a new group with an editor pointing to the same document.
        update(&mut model, Msg::Layout(LayoutMsg::SplitFocused(direction)));

        // Adjust split ratio if specified (the new group is the second child)
        if let Some(ratio) = file.split_ratio {
            let ratio = ratio.clamp(0.05, 0.95);
            set_parent_ratio(
                &mut model.editor_area.layout,
                model.editor_area.focused_group_id,
                ratio,
            );
        }

        // Create a NEW document for this split (splits share by default).
        let new_doc_id = model.editor_area.next_document_id();
        let mut new_doc = Document::with_text(&file_content);
        new_doc.id = Some(new_doc_id);
        new_doc.file_path = Some(file.path.clone());
        new_doc.language = LanguageId::from_path(&file.path);
        model.editor_area.documents.insert(new_doc_id, new_doc);

        // Point the focused editor to the new document
        if let Some(editor) = model.editor_area.focused_editor_mut() {
            editor.document_id = Some(new_doc_id);
            apply_cursor_and_scroll(editor, file);
        }
    }

    // Apply syntax highlighting synchronously
    apply_syntax_highlighting(&mut model);

    // Apply view modes (e.g., CSV)
    apply_view_modes(&mut model, scenario);

    // Open preview panes for files that request them
    apply_previews(&mut model, scenario);

    // Set up workspace/sidebar if configured
    if let Some(ws_config) = &scenario.workspace {
        apply_workspace(&mut model, ws_config, scenario.scale);
    }

    // Set up modal if configured
    if let Some(modal_config) = &scenario.modal {
        apply_modal(&mut model, modal_config);
    }

    Ok(model)
}

/// Run tree-sitter syntax highlighting on all documents synchronously
fn apply_syntax_highlighting(model: &mut AppModel) {
    let mut parser = ParserState::new();
    for (doc_id, doc) in &mut model.editor_area.documents {
        if doc.language == LanguageId::PlainText {
            continue;
        }
        let source = doc.buffer.to_string();
        let highlights = parser.parse_and_highlight(&source, doc.language, *doc_id, doc.revision);
        doc.syntax_highlights = Some(highlights);
    }
}

/// Apply view modes (CSV grid, etc.) based on scenario file settings
fn apply_view_modes(model: &mut AppModel, scenario: &Scenario) {
    for scenario_file in &scenario.files {
        let wants_csv = matches!(scenario_file.view_mode, Some(ScenarioViewMode::Csv));
        if !wants_csv {
            continue;
        }

        // Find the editor pointing at this file and switch it to CSV mode
        let editor_ids: Vec<_> = model
            .editor_area
            .editors
            .iter()
            .filter_map(|(&eid, editor)| {
                let doc_id = editor.document_id?;
                let doc = model.editor_area.documents.get(&doc_id)?;
                if doc.file_path.as_ref() == Some(&scenario_file.path) {
                    Some((eid, doc_id))
                } else {
                    None
                }
            })
            .collect();

        for (editor_id, doc_id) in editor_ids {
            let content = model.editor_area.documents.get(&doc_id).map(|d| {
                let delimiter = d
                    .file_path
                    .as_ref()
                    .and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                    .map(Delimiter::from_extension)
                    .unwrap_or_else(|| detect_delimiter(&d.buffer.to_string()));
                (d.buffer.to_string(), delimiter)
            });

            if let Some((text, delimiter)) = content {
                if let Ok(data) = parse_csv(&text, delimiter) {
                    if !data.is_empty() && data.column_count() > 0 {
                        let line_height = model.line_height.max(1);
                        let tab_bar_height = model.metrics.tab_bar_height;
                        let status_bar_height = line_height;
                        let col_header_height = line_height;
                        let content_height = (model.window_size.1 as usize)
                            .saturating_sub(tab_bar_height)
                            .saturating_sub(status_bar_height)
                            .saturating_sub(col_header_height);
                        let visible_rows = content_height / line_height;
                        let mut csv_state = CsvState::new(data, delimiter);
                        csv_state.set_viewport_size(visible_rows.max(1), 10);

                        if let Some(editor) = model.editor_area.editors.get_mut(&editor_id) {
                            editor.view_mode = ViewMode::Csv(Box::new(csv_state));
                        }
                    }
                }
            }
        }
    }
}

/// Open preview panes for files that have `preview: true`
fn apply_previews(model: &mut AppModel, scenario: &Scenario) {
    for sf in &scenario.files {
        if !sf.preview {
            continue;
        }

        // Find groups whose active editor points to this file
        let group_ids: Vec<_> = model
            .editor_area
            .groups
            .iter()
            .filter_map(|(&gid, group)| {
                let editor_id = group.active_editor_id()?;
                let editor = model.editor_area.editors.get(&editor_id)?;
                let doc_id = editor.document_id?;
                let doc = model.editor_area.documents.get(&doc_id)?;
                (doc.file_path.as_ref() == Some(&sf.path)).then_some(gid)
            })
            .collect();

        for gid in group_ids {
            model.editor_area.open_preview_for_group(gid);
        }
    }
}

fn apply_workspace(model: &mut AppModel, config: &WorkspaceConfig, scale: f64) {
    let root = if config.root.is_absolute() {
        config.root.clone()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .join(&config.root)
    };

    match Workspace::new(root.clone(), &model.metrics) {
        Ok(mut ws) => {
            ws.sidebar_visible = config.sidebar_visible;
            if let Some(width) = config.sidebar_width {
                ws.set_sidebar_width(width * scale as f32, scale);
            }
            ws.scroll_offset = config.scroll_offset;

            // Expand requested folders
            let canon_root = std::fs::canonicalize(&root).unwrap_or(root);
            for rel in &config.expanded {
                ws.expand_folder(&canon_root.join(rel));
            }

            // Select item
            if let Some(sel) = &config.selected {
                ws.selected_item = Some(canon_root.join(sel));
            }

            model.workspace = Some(ws);
        }
        Err(e) => {
            eprintln!(
                "Warning: failed to set up workspace '{}': {}",
                config.root.display(),
                e
            );
        }
    }
}

fn apply_modal(model: &mut AppModel, config: &ModalConfig) {
    let modal_state = match config.id {
        ModalId::CommandPalette => {
            let mut state = CommandPaletteState::default();
            if let Some(ref input) = config.input {
                state.set_input(input);
            }
            if let Some(idx) = config.selected_index {
                state.selected_index = idx;
            }
            ModalState::CommandPalette(state)
        }
        ModalId::GotoLine => {
            let mut state = GotoLineState::default();
            if let Some(ref input) = config.input {
                state.set_input(input);
            }
            ModalState::GotoLine(state)
        }
        ModalId::FindReplace => {
            let mut state = FindReplaceState::default();
            if let Some(ref q) = config.query {
                state.set_query(q);
            }
            if let Some(ref r) = config.replacement {
                state.set_replacement(r);
            }
            state.replace_mode = config.replace_mode;
            state.case_sensitive = config.case_sensitive;
            ModalState::FindReplace(state)
        }
        ModalId::ThemePicker => {
            let current_id = model.config.theme.clone();
            let mut state = ThemePickerState::new(current_id);
            if let Some(idx) = config.selected_index {
                state.selected_index = idx;
                // Apply live preview: load the highlighted theme so the editor
                // background reflects the selected theme (matches runtime behavior)
                if let Some(theme_info) = state.themes.get(idx) {
                    if let Ok(preview_theme) = token::theme::load_theme(&theme_info.id) {
                        model.theme = preview_theme;
                    }
                }
            }
            ModalState::ThemePicker(state)
        }
    };
    model.ui.open_modal(modal_state);
}

/// Walk the layout tree to find the SplitContainer holding `group_id` and set
/// its ratio so the child containing that group gets `ratio` of the space.
fn set_parent_ratio(
    node: &mut token::model::editor_area::LayoutNode,
    group_id: token::model::editor_area::GroupId,
    ratio: f32,
) -> bool {
    use token::model::editor_area::LayoutNode;
    match node {
        LayoutNode::Split(container) => {
            for (i, child) in container.children.iter().enumerate() {
                let contains = match child {
                    LayoutNode::Group(gid) => *gid == group_id,
                    LayoutNode::Split(_) => false, // check recursively below
                    _ => false,
                };
                if contains {
                    // This child is the group — set ratios
                    let n = container.ratios.len();
                    let other = 1.0 - ratio;
                    for (j, r) in container.ratios.iter_mut().enumerate() {
                        *r = if j == i {
                            ratio
                        } else {
                            other / (n - 1) as f32
                        };
                    }
                    return true;
                }
            }
            // Recurse into children
            for child in &mut container.children {
                if set_parent_ratio(child, group_id, ratio) {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn apply_cursor_and_scroll(editor: &mut EditorState, file: &ScenarioFile) {
    use token::model::editor::Cursor;

    if let Some(line) = file.scroll_to {
        editor.viewport.top_line = line;
    }

    let line = file.cursor_line.unwrap_or(0);
    let column = file.cursor_column.unwrap_or(0);
    editor.cursors = vec![Cursor::at(line, column)];
    editor.selections = vec![Selection::new(Position::new(line, column))];

    for extra in &file.extra_cursors {
        editor.cursors.push(Cursor::at(extra.line, extra.column));
        editor
            .selections
            .push(Selection::new(Position::new(extra.line, extra.column)));
    }

    // Apply explicit selections (overrides default empty selections)
    if !file.selections.is_empty() {
        editor.selections.clear();
        editor.cursors.clear();
        for sel in &file.selections {
            let anchor = Position::new(sel.anchor_line, sel.anchor_column);
            let head = Position::new(sel.head_line, sel.head_column);
            editor
                .selections
                .push(Selection::from_anchor_head(anchor, head));
            editor.cursors.push(Cursor::at(head.line, head.column));
        }
    }
}

// ---------------------------------------------------------------------------
// Font setup
// ---------------------------------------------------------------------------

struct FontInfo {
    font: fontdue::Font,
    line_height: usize,
    char_width: f32,
    font_size: f32,
    ascent: f32,
}

fn setup_font(scale: f64) -> FontInfo {
    use fontdue::{Font, FontSettings};

    let font = Font::from_bytes(
        include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
        FontSettings::default(),
    )
    .expect("Failed to load font");

    let font_size = 14.0 * scale as f32;
    let line_metrics = font
        .horizontal_line_metrics(font_size)
        .expect("Font missing line metrics");

    let line_height = line_metrics.new_line_size.ceil() as usize;
    let (metrics, _) = font.rasterize('M', font_size);
    let char_width = metrics.advance_width;
    let ascent = line_metrics.ascent;

    FontInfo {
        font,
        line_height,
        char_width,
        font_size,
        ascent,
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_to_buffer(model: &mut AppModel, font_info: &FontInfo) -> Vec<u32> {
    let width = model.window_size.0 as usize;
    let height = model.window_size.1 as usize;

    let bg = model.theme.editor.background.to_argb_u32();
    let mut buffer: Vec<u32> = vec![bg; width * height];

    let mut glyph_cache: GlyphCache = HashMap::new();

    let status_bar_height = font_info.line_height;

    // Calculate sidebar width (matches real renderer pipeline)
    let sidebar_width = model
        .workspace
        .as_ref()
        .filter(|ws| ws.sidebar_visible)
        .map(|ws| ws.sidebar_width(model.metrics.scale_factor))
        .unwrap_or(0.0);

    let available_rect = Rect::new(
        sidebar_width,
        0.0,
        width as f32 - sidebar_width,
        (height - status_bar_height) as f32,
    );
    let splitters = model
        .editor_area
        .compute_layout_scaled(available_rect, model.metrics.splitter_width);

    // Sync viewports to actual group rects (critical for splits/previews)
    model
        .editor_area
        .sync_all_viewports(font_info.line_height, font_info.char_width, model.metrics.tab_bar_height);

    {
        let mut frame = Frame::new(&mut buffer, width, height);
        let mut painter = TextPainter::new(
            &font_info.font,
            &mut glyph_cache,
            font_info.font_size,
            font_info.ascent,
            font_info.char_width,
            font_info.line_height,
        );

        // 1. Editor area + splitters (render_editor_area_with_preview_mode includes splitters)
        Renderer::render_editor_area_with_preview_mode(
            &mut frame,
            &mut painter,
            model,
            &splitters,
            PreviewRenderMode::NativeMarkdown,
        );

        // 2. Sidebar (if workspace is open)
        if sidebar_width > 0.0 {
            Renderer::render_sidebar(
                &mut frame,
                &mut painter,
                model,
                sidebar_width as usize,
                height.saturating_sub(status_bar_height),
            );
        }

        // 3. Status bar
        Renderer::render_status_bar(&mut frame, &mut painter, model, width, height);

        // 4. Modals (on top of everything)
        if model.ui.active_modal.is_some() {
            Renderer::render_modals(&mut frame, &mut painter, model, width, height);
        }
    }

    buffer
}

// ---------------------------------------------------------------------------
// PNG output
// ---------------------------------------------------------------------------

fn save_png(buffer: &[u32], width: u32, height: u32, path: &std::path::Path) -> Result<()> {
    // Convert ARGB (0xAARRGGBB) to RGBA bytes
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for &pixel in buffer {
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        let a = ((pixel >> 24) & 0xFF) as u8;
        rgba.push(r);
        rgba.push(g);
        rgba.push(b);
        rgba.push(a);
    }

    let img = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width, height, rgba)
        .context("failed to create image buffer")?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    img.save(path)
        .with_context(|| format!("saving PNG to {}", path.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Scenario loading
// ---------------------------------------------------------------------------

fn load_scenario(path: &std::path::Path) -> Result<Scenario> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading scenario {}", path.display()))?;
    let scenario: Scenario = serde_yaml::from_str(&content)
        .with_context(|| format!("parsing scenario {}", path.display()))?;
    Ok(scenario)
}

fn collect_scenarios(args: &Args) -> Result<Vec<(PathBuf, Scenario)>> {
    let mut scenarios = Vec::new();

    if let Some(ref path) = args.scenario {
        let scenario = load_scenario(path)?;
        scenarios.push((path.clone(), scenario));
    } else if args.all {
        if !args.scenarios_dir.exists() {
            anyhow::bail!(
                "scenarios directory does not exist: {}",
                args.scenarios_dir.display()
            );
        }
        let mut entries: Vec<_> = std::fs::read_dir(&args.scenarios_dir)
            .with_context(|| format!("reading scenarios dir {}", args.scenarios_dir.display()))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "yaml" || ext == "yml")
                    .unwrap_or(false)
            })
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            match load_scenario(&path) {
                Ok(scenario) => scenarios.push((path, scenario)),
                Err(e) => eprintln!("Warning: skipping {}: {}", path.display(), e),
            }
        }

        if scenarios.is_empty() {
            anyhow::bail!(
                "no scenario files found in {}",
                args.scenarios_dir.display()
            );
        }
    } else {
        anyhow::bail!("specify --scenario <file> or --all");
    }

    Ok(scenarios)
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let args = Args::parse();
    let scenarios = collect_scenarios(&args)?;

    eprintln!(
        "Rendering {} scenario(s) → {}",
        scenarios.len(),
        args.out_dir.display()
    );

    for (_path, mut scenario) in scenarios {
        // Apply CLI overrides
        if let Some(w) = args.width {
            scenario.width = w;
        }
        if let Some(h) = args.height {
            scenario.height = h;
        }

        let theme = load_theme_for_scenario(args.theme.as_deref(), scenario.theme.as_deref());
        let font_info = setup_font(scenario.scale);

        eprint!("  {} ...", scenario.name);

        let mut model = create_model_from_scenario(&scenario, theme)?;
        let buffer = render_to_buffer(&mut model, &font_info);

        let out_path = args
            .out_dir
            .join(format!("screenshot-{}.png", scenario.name));

        save_png(&buffer, scenario.width, scenario.height, &out_path)?;
        let display_path = out_path.display().to_string();
        if !display_path.starts_with('/') && !display_path.starts_with('.') {
            eprintln!(" saved ./{}", display_path);
        } else {
            eprintln!(" saved {}", display_path);
        }
    }

    eprintln!("Done!");
    Ok(())
}
