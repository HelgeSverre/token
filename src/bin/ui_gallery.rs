//! Development-only native gallery. Run `just ui-gallery --help`.
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use token::model::gallery::{GalleryFocus, GalleryState};
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
    /// Include the open theme dropdown in headless screenshots.
    #[arg(long)]
    theme_menu: bool,
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
    theme_drag: Option<f32>,
    theme_wheel: f64,
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
        if let Some(popup) = &layout.theme_popup {
            self.state.theme_select.scroll = popup.first;
        }
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
        if self.state.theme_select.open {
            if let Some(popup) = &layout.theme_popup {
                if let Some(bar) = &popup.scrollbar {
                    if bar.hits_thumb(x, y) {
                        self.theme_drag = Some(y - bar.thumb_rect.y);
                        return;
                    }
                    if bar.hits_track(x, y) {
                        self.state.theme_select.scroll_to(
                            bar.position_from_track_click(y),
                            popup.rows.len(),
                            self.themes.len(),
                        );
                        return;
                    }
                }
                let panel = popup.panel;
                if token::model::Rect::new(
                    panel.x as f32,
                    panel.y as f32,
                    panel.w as f32,
                    panel.h as f32,
                )
                .contains(x, y)
                    && popup.option_at(x, y).is_none()
                {
                    return;
                }
            }
            let option = layout
                .theme_popup
                .as_ref()
                .and_then(|popup| popup.option_at(x, y));
            if let Some(index) = option {
                self.apply_theme(index);
            } else {
                self.state.theme_select.open = false;
            }
            return;
        }
        if layout.scrollbar.needed && layout.scrollbar.hits_thumb(x, y) {
            self.drag = Some(y - layout.scrollbar.thumb_rect.y);
        } else if layout.scrollbar.needed && layout.scrollbar.hits_track(x, y) {
            self.state.scroll = layout.scrollbar.position_from_track_click(y) as f64;
        } else if let Some(category) =
            token::view::section_navigation::section_at(&layout.categories, x, y)
        {
            self.state.category = category;
            self.state.scroll = 0.0;
            self.state.focus = GalleryFocus::Filter;
        } else if let Some(index) =
            token::view::section_navigation::section_at(&layout.width_segments, x, y)
        {
            self.state.compact = index == 0;
            self.state.focus = GalleryFocus::Width;
        } else if layout.theme.contains(x, y) {
            self.open_themes();
        } else if layout.search.contains(x, y) {
            self.state.focus = GalleryFocus::Filter;
        }
    }

    fn open_themes(&mut self) {
        self.state.focus = GalleryFocus::Theme;
        self.theme_wheel = 0.0;
        self.state.theme_select.open(self.state.selected_theme);
    }

    fn apply_theme(&mut self, index: usize) {
        if let Some(info) = self.themes.get(index) {
            match theme::load_theme(&info.id) {
                Ok(theme) => {
                    self.theme = theme;
                    self.state.selected_theme = index;
                }
                Err(error) => eprintln!("Cannot load theme {}: {error}", info.id),
            }
        }
        self.state.theme_select.open = false;
    }

    fn key(&mut self, key: Key, text: Option<&str>) {
        let extend = self.modifiers.shift_key();
        if key == Key::Named(NamedKey::Tab) {
            self.state.theme_select.open = false;
            self.state.focus = match (self.state.focus, extend) {
                (GalleryFocus::Filter, false) | (GalleryFocus::Width, true) => GalleryFocus::Theme,
                (GalleryFocus::Theme, false) | (GalleryFocus::Filter, true) => GalleryFocus::Width,
                _ => GalleryFocus::Filter,
            };
            return;
        }
        if self.state.theme_select.open {
            match key {
                Key::Named(NamedKey::Escape) => self.state.theme_select.open = false,
                Key::Named(NamedKey::Enter | NamedKey::Space) => {
                    self.apply_theme(self.state.theme_select.active)
                }
                Key::Named(NamedKey::ArrowDown) => {
                    self.state.theme_select.move_by(1, self.themes.len())
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.state.theme_select.move_by(-1, self.themes.len())
                }
                Key::Named(NamedKey::Home) => self.state.theme_select.active = 0,
                Key::Named(NamedKey::End) => {
                    self.state.theme_select.active = self.themes.len().saturating_sub(1)
                }
                _ => {}
            }
            return;
        }
        if self.state.focus == GalleryFocus::Theme {
            if matches!(
                key,
                Key::Named(
                    NamedKey::Enter | NamedKey::Space | NamedKey::ArrowDown | NamedKey::ArrowUp
                )
            ) {
                self.open_themes();
            }
            return;
        }
        if self.state.focus == GalleryFocus::Width {
            match key {
                Key::Named(NamedKey::ArrowLeft | NamedKey::Home) => self.state.compact = true,
                Key::Named(NamedKey::ArrowRight | NamedKey::End) => self.state.compact = false,
                _ => {}
            }
            return;
        }
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
                if let (Some(grab), Some(popup)) = (
                    self.theme_drag,
                    self.layout
                        .as_ref()
                        .and_then(|layout| layout.theme_popup.as_ref()),
                ) {
                    if let Some(bar) = &popup.scrollbar {
                        self.state.theme_select.scroll_to(
                            bar.position_from_drag(grab, self.mouse.1),
                            popup.rows.len(),
                            self.themes.len(),
                        );
                    }
                } else if let (Some(grab), Some(layout)) = (self.drag, &self.layout) {
                    self.state.scroll =
                        layout.scrollbar.position_from_drag(grab, self.mouse.1) as f64;
                } else if self.state.theme_select.open {
                    if let Some(index) = self
                        .layout
                        .as_ref()
                        .and_then(|layout| layout.theme_popup.as_ref())
                        .and_then(|popup| popup.option_at(self.mouse.0, self.mouse.1))
                    {
                        self.state.theme_select.active = index;
                    }
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
                    self.theme_drag = None;
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
                    if self.state.theme_select.open {
                        if let Some(popup) = &layout.theme_popup {
                            let row_height =
                                popup.rows.first().map_or(24, |row| row.h).max(1) as f64;
                            self.theme_wheel -= dy / row_height;
                            let steps = self.theme_wheel.trunc() as isize;
                            self.theme_wheel -= steps as f64;
                            self.state.theme_select.scroll_to(
                                self.state.theme_select.scroll.saturating_add_signed(steps),
                                popup.rows.len(),
                                self.themes.len(),
                            );
                        }
                    } else {
                        self.state.scroll = (self.state.scroll - dy)
                            .clamp(0.0, layout.scrollbar.state.max_position() as f64);
                    }
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
                self.state.theme_select.open = false;
                self.theme_drag = None;
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
    let mut state = GalleryState {
        theme_names: themes.iter().map(|info| info.name.clone()).collect(),
        selected_theme: themes
            .iter()
            .position(|info| {
                args.theme
                    .as_ref()
                    .map_or(info.name == theme.name, |id| info.id == *id)
            })
            .unwrap_or(0),
        ..Default::default()
    };
    if args.theme_menu {
        state.theme_select.open(state.selected_theme);
        state.focus = GalleryFocus::Theme;
    }
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
        theme_drag: None,
        theme_wheel: 0.0,
        modifiers: ModifiersState::empty(),
        error: None,
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn theme_navigation_commits_only_on_accept_and_width_selects_directly() {
        let themes: Vec<_> = [
            ("default-dark", "Default Dark"),
            ("github-light", "GitHub Light"),
        ]
        .into_iter()
        .map(|(id, name)| ThemeInfo {
            id: id.into(),
            name: name.into(),
            source: theme::ThemeSource::Builtin,
        })
        .collect();
        let state = GalleryState {
            theme_names: themes.iter().map(|t| t.name.clone()).collect(),
            ..Default::default()
        };
        let mut app = App {
            args: Args::parse_from(["ui-gallery"]),
            state,
            painter: GalleryRenderer::new().unwrap(),
            theme: Theme::default_dark(),
            themes,
            window: None,
            surface: None,
            layout: None,
            mouse: (0.0, 0.0),
            drag: None,
            theme_drag: None,
            theme_wheel: 0.0,
            modifiers: ModifiersState::empty(),
            error: None,
        };
        app.state.query.insert_text("field");
        app.key(Key::Named(NamedKey::Tab), None);
        app.key(Key::Named(NamedKey::ArrowDown), None);
        app.key(Key::Named(NamedKey::ArrowDown), None);
        assert_eq!(app.state.selected_theme, 0);
        app.key(Key::Named(NamedKey::Escape), None);
        assert_eq!(app.state.query.text(), "field");
        assert!(!app.state.theme_select.open);
        app.key(Key::Named(NamedKey::Enter), None);
        app.key(Key::Named(NamedKey::End), None);
        app.key(Key::Named(NamedKey::Enter), None);
        assert_eq!(app.state.selected_theme, 1);
        assert_eq!(app.theme.name, "GitHub Light");
        app.layout = Some(GalleryLayout::new(1100, 820, 1.0, &app.state));
        let narrow = app.layout.as_ref().unwrap().width_segments[0];
        app.mouse = ((narrow.x + 4) as f32, (narrow.y + 4) as f32);
        app.click();
        app.click();
        assert!(app.state.compact);
        app.key(Key::Named(NamedKey::ArrowRight), None);
        assert!(!app.state.compact);
    }
}
