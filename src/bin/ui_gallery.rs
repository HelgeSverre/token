//! Development-only native gallery. Run `just ui-gallery --help`.
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use token::model::gallery::GalleryState;
use token::theme::{self, Theme, ThemeInfo};
use token::view::gallery::{GalleryLayout, GalleryRenderer};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Window, WindowId};

#[derive(Parser)]
#[command(about = "Native Token component gallery (does not change editor settings)")]
struct Args {
    /// Render a PNG without opening a window. Same painter as the native gallery.
    #[arg(long)]
    screenshot: Option<PathBuf>,
    #[arg(long, default_value_t = 1100)]
    width: u32,
    #[arg(long, default_value_t = 820)]
    height: u32,
    /// Screenshot pixel scale. Native windows follow the display scale.
    #[arg(long, default_value_t = 1.0)]
    scale: f64,
    #[arg(long)]
    theme: Option<String>,
    /// Filter by stable specimen name, painter, or palette role.
    #[arg(long, default_value = "")]
    filter: String,
}

struct App {
    args: Args,
    state: GalleryState,
    painter: GalleryRenderer,
    theme: Theme,
    themes: Vec<ThemeInfo>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    layout: Option<GalleryLayout>,
    mouse: (f32, f32),
    drag: Option<f32>,
    modifiers: ModifiersState,
    error: Option<anyhow::Error>,
}

impl App {
    fn draw(&mut self) -> Result<()> {
        let window = self.window.as_ref().context("gallery window")?;
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(());
        };
        let surface = self.surface.as_mut().context("gallery surface")?;
        surface
            .resize(w, h)
            .map_err(|e| anyhow!("resize gallery: {e}"))?;
        let mut buffer = surface
            .buffer_mut()
            .map_err(|e| anyhow!("acquire gallery: {e}"))?;
        let layout = self.painter.render(
            &mut buffer,
            (size.width as usize, size.height as usize),
            window.scale_factor(),
            &self.state,
            &self.theme,
        );
        self.state.scroll = self
            .state
            .scroll
            .clamp(0.0, layout.scrollbar.state.max_position() as f64);
        self.layout = Some(layout);
        buffer
            .present()
            .map_err(|e| anyhow!("present gallery: {e}"))
    }

    fn click(&mut self) {
        let Some(layout) = &self.layout else {
            return;
        };
        let (x, y) = self.mouse;
        if layout.scrollbar.needed && layout.scrollbar.hits_thumb(x, y) {
            self.drag = Some(y - layout.scrollbar.thumb_rect.y);
        } else if layout.scrollbar.needed && layout.scrollbar.hits_track(x, y) {
            self.state.scroll = layout.scrollbar.position_from_track_click(y) as f64;
        } else if let Some(category) = layout.categories.iter().position(|r| r.contains(x, y)) {
            self.state.category = category;
            self.state.scroll = 0.0;
        } else if layout.width_toggle.contains(x, y) {
            self.state.compact = !self.state.compact;
        } else if layout.theme.contains(x, y) {
            let current = self
                .themes
                .iter()
                .position(|t| t.name == self.theme.name)
                .unwrap_or(0);
            if let Some(next) = self.themes.get((current + 1) % self.themes.len().max(1)) {
                match theme::load_theme(&next.id) {
                    Ok(theme) => self.theme = theme,
                    Err(error) => eprintln!("Cannot load theme {}: {error}", next.id),
                }
            }
        }
    }

    fn key(&mut self, key: Key, text: Option<&str>) {
        let extend = self.modifiers.shift_key();
        match key {
            Key::Named(NamedKey::Escape) => {
                self.state.query.select_all();
                self.state.query.delete_backward();
            }
            Key::Named(NamedKey::Backspace) => {
                self.state.query.delete_backward();
            }
            Key::Named(NamedKey::Delete) => {
                self.state.query.delete_forward();
            }
            Key::Named(NamedKey::ArrowLeft) => self.state.query.move_left(extend),
            Key::Named(NamedKey::ArrowRight) => self.state.query.move_right(extend),
            Key::Named(NamedKey::Home) => self.state.query.move_line_start(extend),
            Key::Named(NamedKey::End) => self.state.query.move_line_end(extend),
            Key::Character(ref c)
                if (self.modifiers.super_key() || self.modifiers.control_key())
                    && c.eq_ignore_ascii_case("a") =>
            {
                self.state.query.select_all()
            }
            _ if !self.modifiers.super_key() && !self.modifiers.control_key() => {
                if let Some(text) = text.filter(|t| !t.chars().any(char::is_control)) {
                    self.state.query.insert_text(text);
                }
            }
            _ => {}
        }
        self.state.scroll = 0.0;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<()> {
            let window = Rc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("Token — UI Gallery")
                        .with_inner_size(LogicalSize::new(self.args.width, self.args.height))
                        .with_min_inner_size(LogicalSize::new(900, 440)),
                )?,
            );
            let context = softbuffer::Context::new(window.clone())
                .map_err(|e| anyhow!("gallery context: {e}"))?;
            self.surface = Some(
                softbuffer::Surface::new(&context, window.clone())
                    .map_err(|e| anyhow!("gallery surface: {e}"))?,
            );
            window.request_redraw();
            self.window = Some(window);
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
                return;
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.draw() {
                    self.error = Some(error);
                    event_loop.exit();
                }
                return;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = (position.x as f32, position.y as f32);
                if let (Some(grab), Some(layout)) = (self.drag, &self.layout) {
                    self.state.scroll =
                        layout.scrollbar.position_from_drag(grab, self.mouse.1) as f64;
                } else {
                    return;
                }
            }
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                if state == ElementState::Pressed {
                    self.click();
                } else {
                    self.drag = None;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(layout) = &self.layout {
                    let dy = match delta {
                        MouseScrollDelta::PixelDelta(p) => p.y,
                        MouseScrollDelta::LineDelta(_, y) => {
                            y as f64 * 40.0 * self.window.as_ref().map_or(1.0, |w| w.scale_factor())
                        }
                    };
                    self.state.scroll = (self.state.scroll - dy)
                        .clamp(0.0, layout.scrollbar.state.max_position() as f64);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.key(event.logical_key, event.text.as_deref())
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
                return;
            }
            WindowEvent::Focused(false) => {
                self.drag = None;
                self.modifiers = ModifiersState::empty();
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {}
            _ => return,
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(
        (900..=3840).contains(&args.width) && (440..=2160).contains(&args.height),
        "logical size must be 900–3840 × 440–2160"
    );
    anyhow::ensure!(
        args.scale.is_finite() && (0.75..=3.0).contains(&args.scale),
        "scale must be 0.75–3"
    );
    let themes = theme::list_available_themes();
    let theme = match args.theme.as_deref() {
        Some(id) => theme::load_theme(id).map_err(|e| anyhow!(e))?,
        None => Theme::default_dark(),
    };
    let mut state = GalleryState::default();
    state.query.insert_text(&args.filter);
    let mut painter = GalleryRenderer::new()?;
    if let Some(path) = &args.screenshot {
        let w = (args.width as f64 * args.scale).round() as usize;
        let h = (args.height as f64 * args.scale).round() as usize;
        let mut buffer = vec![0; w * h];
        painter.render(&mut buffer, (w, h), args.scale, &state, &theme);
        let rgb: Vec<u8> = buffer
            .iter()
            .flat_map(|pixel| [(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])
            .collect();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        image::save_buffer(path, &rgb, w as u32, h as u32, image::ColorType::Rgb8)?;
        println!("{}", path.display());
        return Ok(());
    }
    let mut app = App {
        args,
        state,
        painter,
        theme,
        themes,
        window: None,
        surface: None,
        layout: None,
        mouse: (0.0, 0.0),
        drag: None,
        modifiers: ModifiersState::empty(),
        error: None,
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}
