//! Profiling binary for render performance analysis
//!
//! This binary opens multiple files in a split layout and renders frames
//! for profiling with samply or other profilers.
//!
//! Usage:
//!   cargo build --profile profiling --bin profile_render
//!   samply record ./target/profiling/profile_render
//!
//! Or to profile with a specific scenario:
//!   samply record ./target/profiling/profile_render --frames 1000 --splits 3

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;

const SCALE_FACTOR: f64 = 2.0;

#[derive(Parser, Debug)]
#[command(name = "profile_render")]
#[command(about = "Profile rendering performance with multiple splits")]
struct Args {
    /// Number of frames to render
    #[arg(long, default_value = "500")]
    frames: usize,

    /// Number of editor splits
    #[arg(long, default_value = "3")]
    splits: usize,

    /// UTF-8 text/CSV files (cycle through independent copies if fewer than splits)
    #[arg(long)]
    files: Vec<PathBuf>,

    /// Generate synthetic content if no files provided
    #[arg(long, default_value = "10000")]
    lines: usize,

    /// Use a synthetic CSV grid in the final split (without --files)
    #[arg(long)]
    include_csv: bool,

    /// Window width
    #[arg(long, default_value = "1920")]
    width: u32,

    /// Window height  
    #[arg(long, default_value = "1080")]
    height: u32,

    /// Simulate scrolling during render
    #[arg(long)]
    scroll: bool,

    /// Print timing statistics
    #[arg(long)]
    stats: bool,

    /// Disable indentation guides for a same-workload rendering comparison.
    #[arg(long)]
    no_indent_guides: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        args.frames > 0 && args.splits > 0,
        "frames and splits must be nonzero"
    );

    eprintln!("Profile Render - Multi-Split Performance Test");
    eprintln!("==============================================");
    eprintln!("Frames: {}", args.frames);
    eprintln!("Splits: {}", args.splits);
    eprintln!("Window: {}x{}", args.width, args.height);
    eprintln!();

    // Create the application model
    let mut model = create_model(&args)?;
    model.config.indent_guides = !args.no_indent_guides;
    eprintln!("Indent guides: {}", model.config.indent_guides);

    eprintln!(
        "Model created with {} splits",
        model.editor_area.groups.len()
    );
    for (id, group) in &model.editor_area.groups {
        if let Some(editor_id) = group.active_editor_id() {
            if let Some(editor) = model.editor_area.editors.get(&editor_id) {
                if let Some(doc_id) = editor.document_id {
                    if let Some(doc) = model.editor_area.documents.get(&doc_id) {
                        let mode = if editor.view_mode.is_csv() {
                            "CSV grid"
                        } else {
                            "text"
                        };
                        eprintln!(
                            "  Group {:?}: document {:?}, {} ({mode}, {:?}), {} lines",
                            id,
                            doc_id,
                            doc.display_name(),
                            doc.language,
                            doc.line_count()
                        );
                    }
                }
            }
        }
    }
    eprintln!();

    // Set up rendering infrastructure (headless)
    let (font, line_height, char_width, font_size, ascent) = setup_font(args.height);
    model.line_height = line_height;
    model.status_bar_height = line_height;
    model.set_char_width(char_width);

    let width = args.width as usize;
    let height = args.height as usize;
    let mut buffer: Vec<u32> = vec![0xFF1E1E2E; width * height];
    let mut glyph_cache = std::collections::HashMap::new();

    // Pre-warm the glyph cache with ASCII characters
    for ch in ' '..='~' {
        let (metrics, bitmap) = font.rasterize(ch, font_size);
        glyph_cache.insert((ch, font_size.to_bits()), (metrics, bitmap));
    }

    eprintln!(
        "Glyph cache pre-warmed with {} characters",
        glyph_cache.len()
    );
    eprintln!();

    // Compute layout once from the same solved shell as the real renderer.
    let available_rect = token::layout::chrome::shell(&model)
        .rect(token::layout::UiKey::EditorArea)
        .expect("window shell always declares the editor area");
    let splitters = model
        .editor_area
        .compute_layout_scaled(available_rect, model.metrics.splitter_width);
    model.resync_viewports();

    eprintln!("Starting render loop ({} frames)...", args.frames);
    eprintln!();

    let mut frame_times: Vec<Duration> = Vec::with_capacity(args.frames);
    let start_time = Instant::now();

    for frame in 0..args.frames {
        let frame_start = Instant::now();

        // Simulate scrolling to exercise different code paths
        if args.scroll && frame % 10 == 0 {
            scroll_model(&mut model, (frame / 10) % 100);
        }

        // Clear the buffer (as real renderer does)
        buffer.fill(0xFF1E1E2E);

        let mut render_frame = token::view::Frame::new(&mut buffer, width, height);
        let mut painter = token::view::TextPainter::new(
            &font,
            &mut glyph_cache,
            font_size,
            ascent,
            char_width,
            line_height,
        );
        let mut perf = token::perf::PerfStats::default();
        token::view::Renderer::render_editor_area_with_preview_mode(
            &mut render_frame,
            &mut painter,
            &model,
            &splitters,
            token::view::PreviewRenderMode::NativeMarkdown,
            &mut perf,
        );

        frame_times.push(frame_start.elapsed());

        // Progress indicator
        if (frame + 1) % 100 == 0 {
            eprintln!("  Rendered {} frames...", frame + 1);
        }
    }

    let total_time = start_time.elapsed();

    eprintln!();
    eprintln!("Render complete!");
    eprintln!();

    if args.stats {
        print_stats(&frame_times, total_time, args.frames);
    } else {
        let avg_ms = total_time.as_secs_f64() * 1000.0 / args.frames as f64;
        let fps = args.frames as f64 / total_time.as_secs_f64();
        eprintln!(
            "Average CPU editor-area render: {:.2}ms ({:.1} iterations/s; excludes window/present)",
            avg_ms, fps
        );
    }

    // Prevent the buffer from being optimized away
    std::hint::black_box(&buffer);

    Ok(())
}

fn create_model(args: &Args) -> Result<token::model::AppModel> {
    use token::config::EditorConfig;
    use token::messages::{LayoutMsg, Msg};
    use token::model::editor::EditorState;
    use token::model::editor_area::EditorArea;
    use token::model::ui::UiState;
    use token::model::AppModel;
    use token::theme::Theme;
    use token::update::update;

    let line_height = 20usize;
    let char_width = 10.0f32;

    anyhow::ensure!(args.splits > 0, "splits must be nonzero");
    anyhow::ensure!(
        args.files.is_empty() || !args.include_csv,
        "--include-csv is for synthetic fixtures; pass a .csv or .tsv in --files instead"
    );
    let fixtures = load_fixtures(args)?;

    // Create initial model with first document
    let document = fixture_document(&fixtures[0]);
    let mut editor = EditorState::with_viewport(1, 1);
    editor.view_mode = fixture_view_mode(&document)?;
    let editor_area = EditorArea::single_document(document, editor);

    let mut model = AppModel {
        editor_area,
        ui: UiState::new(),
        theme: Theme::default(),
        config: EditorConfig::default(),
        window_size: (args.width, args.height),
        line_height,
        status_bar_height: line_height,
        char_width,
        metrics: token::model::ScaledMetrics::new(SCALE_FACTOR),
        workspace: None,
        dock_layout: token::panel::DockLayout::default(),
        terminal: token::terminal::TerminalState::default(),
        outline_panel: token::model::OutlinePanelState::default(),
        problems_panel: token::model::ProblemsPanelState::default(),
        usages_panel: Default::default(),
        recent_files: token::recent_files::RecentFiles::default(),
        command_history: token::command_history::CommandHistory::default(),
        #[cfg(debug_assertions)]
        debug_overlay: None,
        lsp: token::model::LspUiState::default(),
        jump_history: Vec::new(),
        forward_history: Vec::new(),
    };

    // Add more splits using the layout system
    for index in 1..args.splits {
        // Split the current focused group horizontally (side by side)
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(
                token::model::editor_area::SplitDirection::Horizontal,
            )),
        );

        // SplitFocused intentionally shares its source document. Give this
        // fixture its own document identity before attaching any new content.
        let mut doc = fixture_document(&fixtures[index % fixtures.len()]);
        let view_mode = fixture_view_mode(&doc)?;
        let document_id = model.editor_area.next_document_id();
        doc.id = Some(document_id);
        model.editor_area.documents.insert(document_id, doc);
        let editor = model
            .editor_area
            .focused_editor_mut()
            .context("split has no editor")?;
        editor.document_id = Some(document_id);
        editor.view_mode = view_mode;
    }

    // Parsing is setup, not measured rendering work. Use real highlight spans
    // for code files, as the running editor does once its worker has finished.
    let mut parser = token::syntax::ParserState::new();
    for (&id, doc) in &mut model.editor_area.documents {
        if doc.language.has_highlighting() {
            doc.syntax_highlights = Some(parser.parse_and_highlight(
                &doc.buffer.to_string(),
                doc.language,
                id,
                doc.revision,
            ));
        }
    }
    model.resize(args.width, args.height);

    Ok(model)
}

fn load_fixtures(args: &Args) -> Result<Vec<(PathBuf, String)>> {
    if !args.files.is_empty() {
        return args
            .files
            .iter()
            .map(|path| {
                let content = std::fs::read_to_string(path).with_context(|| {
                    format!(
                        "cannot load profiling input {} as UTF-8 text",
                        path.display()
                    )
                })?;
                Ok((path.clone(), content))
            })
            .collect();
    }
    Ok((0..args.splits)
        .map(|index| {
            if args.include_csv && index + 1 == args.splits {
                (
                    PathBuf::from("profile-data.csv"),
                    generate_csv_content(args.lines),
                )
            } else {
                let text = if index % 2 == 0 {
                    generate_code_content(args.lines)
                } else {
                    generate_rust_content(args.lines)
                };
                (PathBuf::from(format!("profile-code-{index}.rs")), text)
            }
        })
        .collect())
}

fn fixture_document((path, content): &(PathBuf, String)) -> token::model::Document {
    let mut doc = token::model::Document::with_text(content);
    doc.language = token::syntax::LanguageId::from_path(path);
    doc.file_path = Some(path.clone());
    doc
}

fn fixture_view_mode(doc: &token::model::Document) -> Result<token::model::editor::ViewMode> {
    use token::csv::{parse_csv, CsvState, Delimiter};
    use token::model::editor::ViewMode;
    let extension = doc
        .file_path
        .as_ref()
        .and_then(|path| path.extension())
        .and_then(|ext| ext.to_str())
        .unwrap_or("");
    if extension.eq_ignore_ascii_case("csv") || extension.eq_ignore_ascii_case("tsv") {
        let delimiter = Delimiter::from_extension(&extension.to_ascii_lowercase());
        let data = parse_csv(&doc.buffer.to_string(), delimiter)
            .with_context(|| format!("cannot parse CSV profiling input {}", doc.display_name()))?;
        anyhow::ensure!(
            !data.is_empty() && data.column_count() > 0,
            "CSV profiling input {} has no cells",
            doc.display_name()
        );
        Ok(ViewMode::Csv(Box::new(CsvState::new(data, delimiter))))
    } else {
        Ok(ViewMode::Text)
    }
}

fn scroll_model(model: &mut token::model::AppModel, target: usize) {
    for editor in model.editor_area.editors.values_mut() {
        if let Some(csv) = editor.view_mode.as_csv_mut() {
            csv.scroll_vertical(target as i32 - csv.viewport.top_row as i32);
        } else if editor.is_plain_text_mode() {
            if let Some(doc) = editor
                .document_id
                .and_then(|id| model.editor_area.documents.get(&id))
            {
                editor.set_top_line_clamped(doc, target);
            }
        }
    }
}

fn setup_font(_window_height: u32) -> (fontdue::Font, usize, f32, f32, f32) {
    use fontdue::{Font, FontSettings};

    let font = Font::from_bytes(
        include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
        FontSettings::default(),
    )
    .expect("Failed to load font");

    let font_size = 14.0 * SCALE_FACTOR as f32;

    let line_metrics = font
        .horizontal_line_metrics(font_size)
        .expect("Font missing line metrics");

    let line_height = line_metrics.new_line_size.ceil() as usize;
    let (metrics, _) = font.rasterize('M', font_size);
    let char_width = metrics.advance_width;
    let ascent = line_metrics.ascent;

    (font, line_height, char_width, font_size, ascent)
}

fn generate_code_content(lines: usize) -> String {
    let mut content = String::with_capacity(token::util::ByteSize::bytes(80).as_usize() * lines);
    for i in 0..lines {
        content.push_str(&format!(
            "fn process_document_{}(doc: &Document) -> Result<(), Error> {{ Ok(()) }}\n",
            i
        ));
    }
    content
}

fn generate_rust_content(lines: usize) -> String {
    let mut content = String::with_capacity(token::util::ByteSize::bytes(80).as_usize() * lines);
    content.push_str("use std::collections::HashMap;\n\n");
    for i in 0..lines {
        match i % 5 {
            0 => content.push_str(&format!("pub struct Handler{} {{\n", i / 5)),
            1 => content.push_str(&format!("    field_{}: String,\n", i)),
            2 => content.push_str(&format!("    data_{}: Vec<u8>,\n", i)),
            3 => content.push_str("}\n"),
            _ => content.push_str(&format!(
                "impl Handler{} {{ fn name(&self) -> &str {{ &self.field_{} }} }}\n",
                i / 5,
                i - 3
            )),
        }
    }
    content
}

fn generate_csv_content(rows: usize) -> String {
    let mut content = String::with_capacity(token::util::ByteSize::bytes(150).as_usize() * rows);
    content.push_str(
        "id,first_name,last_name,email,company,department,job_title,salary,hire_date,country\n",
    );
    for i in 0..rows {
        content.push_str(&format!(
            "{},John{},Smith{},john{}@company.com,Company{},Engineering,Developer,{},{}-01-15,USA\n",
            i, i % 100, i % 50, i, i % 10, 50000 + (i % 100) * 1000, 2020 + (i % 5)
        ));
    }
    content
}

fn print_stats(frame_times: &[Duration], total_time: Duration, frame_count: usize) {
    let mut sorted: Vec<_> = frame_times.to_vec();
    sorted.sort();

    let min = sorted.first().unwrap();
    let max = sorted.last().unwrap();
    let median = sorted[sorted.len() / 2];
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
    let p99 = sorted[(sorted.len() as f64 * 0.99) as usize];
    let avg = total_time / frame_count as u32;
    let fps = frame_count as f64 / total_time.as_secs_f64();

    eprintln!("CPU editor-area timings (excludes window/present):");
    eprintln!("  Min:    {:>8.2}ms", min.as_secs_f64() * 1000.0);
    eprintln!("  Max:    {:>8.2}ms", max.as_secs_f64() * 1000.0);
    eprintln!("  Avg:    {:>8.2}ms", avg.as_secs_f64() * 1000.0);
    eprintln!("  Median: {:>8.2}ms", median.as_secs_f64() * 1000.0);
    eprintln!("  P95:    {:>8.2}ms", p95.as_secs_f64() * 1000.0);
    eprintln!("  P99:    {:>8.2}ms", p99.as_secs_f64() * 1000.0);
    eprintln!();
    eprintln!("  Iterations/s: {:>8.1}", fps);
    eprintln!("  Total:  {:>8.2}s", total_time.as_secs_f64());
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::model::DocumentId;

    fn args(extra: &[&str]) -> Args {
        Args::parse_from(
            ["profile_render", "--lines", "20"]
                .into_iter()
                .chain(extra.iter().copied()),
        )
    }

    #[test]
    fn profiler_splits_have_independent_documents_and_real_csv_mode() {
        let mut model = create_model(&args(&["--splits", "4", "--include-csv"])).unwrap();
        assert_eq!(model.editor_area.groups.len(), 4);
        assert_eq!(model.editor_area.editors.len(), 4);
        assert_eq!(model.editor_area.documents.len(), 4);
        let ids: std::collections::HashSet<_> = model
            .editor_area
            .editors
            .values()
            .map(|editor| editor.document_id.unwrap())
            .collect();
        assert_eq!(ids.len(), 4);
        for editor in model.editor_area.editors.values() {
            let doc = &model.editor_area.documents[&editor.document_id.unwrap()];
            if doc.file_path.as_ref().unwrap().extension().unwrap() == "csv" {
                let csv = editor.view_mode.as_csv().expect("CSV grid, not text");
                assert_eq!(csv.data.row_count(), 21); // header plus 20 generated rows
                assert_eq!(csv.data.column_count(), 10);
                assert!(csv.viewport.visible_rows > 1);
                assert!(!editor.is_plain_text_mode());
            } else {
                assert!(editor.is_plain_text_mode());
                assert_eq!(doc.language, token::syntax::LanguageId::Rust);
                assert!(!doc.syntax_highlights.as_ref().unwrap().lines.is_empty());
            }
        }
        let first = model.editor_area.documents[&DocumentId(1)]
            .buffer
            .to_string();
        model
            .editor_area
            .documents
            .get_mut(&DocumentId(4))
            .unwrap()
            .buffer
            .insert(0, "changed");
        assert_eq!(
            model.editor_area.documents[&DocumentId(1)]
                .buffer
                .to_string(),
            first
        );
        model.editor_area.assert_invariants();
    }

    #[test]
    fn profiler_file_inputs_cycle_in_order_without_sharing_identity() {
        let dir = tempfile::tempdir().unwrap();
        let rust = dir.path().join("one.rs");
        let csv = dir.path().join("two.tsv");
        std::fs::write(&rust, "fn first() {}\n").unwrap();
        std::fs::write(&csv, "name\tvalue\nsecond\t2\n").unwrap();
        let mut args = args(&["--splits", "3"]);
        args.files = vec![rust.clone(), csv.clone()];
        let mut model = create_model(&args).unwrap();
        let docs = &model.editor_area.documents;
        for (index, path) in [rust.clone(), csv, rust].into_iter().enumerate() {
            assert_eq!(
                docs[&DocumentId(index as u64 + 1)].file_path.as_ref(),
                Some(&path)
            );
        }
        let csv_editor = model
            .editor_area
            .editors
            .values()
            .find(|editor| editor.document_id == Some(DocumentId(2)))
            .unwrap();
        assert_eq!(
            csv_editor.view_mode.as_csv().unwrap().data.column_count(),
            2
        );
        model
            .editor_area
            .documents
            .get_mut(&DocumentId(3))
            .unwrap()
            .buffer
            .insert(0, "other");
        assert_eq!(
            model.editor_area.documents[&DocumentId(1)]
                .buffer
                .to_string(),
            "fn first() {}\n"
        );
    }

    #[test]
    fn profiler_missing_or_non_utf8_inputs_fail_instead_of_profiling_error_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.rs");
        let mut args = args(&[]);
        args.files = vec![path.clone()];
        assert!(create_model(&args)
            .unwrap_err()
            .to_string()
            .contains("cannot load profiling input"));
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(create_model(&args).is_err());
    }

    #[test]
    fn profiler_csv_flag_is_explicit_and_works_with_one_split() {
        let model = create_model(&args(&["--splits", "1", "--include-csv"])).unwrap();
        assert!(model.editor().view_mode.is_csv());
        let model = create_model(&args(&["--splits", "3"])).unwrap();
        assert!(model
            .editor_area
            .editors
            .values()
            .all(|editor| editor.is_plain_text_mode()));
        assert!(create_model(&args(&["--splits", "0"])).is_err());
        assert!(create_model(&args(&["--files", "unused.csv", "--include-csv"])).is_err());
    }

    #[test]
    fn profiler_scrolls_the_active_mode_and_clamps_short_documents() {
        let mut args = args(&["--splits", "2", "--include-csv"]);
        args.lines = 200;
        let mut model = create_model(&args).unwrap();
        scroll_model(&mut model, 40);
        for editor in model.editor_area.editors.values_mut() {
            if let Some(csv) = editor.view_mode.as_csv_mut() {
                assert_eq!(csv.viewport.top_row, 40);
                assert_eq!(editor.viewport.top_line, 0);
                csv.viewport.visible_rows = 1000;
            } else {
                assert_eq!(editor.viewport.top_line, 40);
                editor.viewport.visible_lines = 1000;
            }
        }
        scroll_model(&mut model, 99);
        for editor in model.editor_area.editors.values() {
            assert_eq!(editor.viewport.top_line, 0);
            if let Some(csv) = editor.view_mode.as_csv() {
                assert_eq!(csv.viewport.top_row, 0);
            }
        }
    }
}
