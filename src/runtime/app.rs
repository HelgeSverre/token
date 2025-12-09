use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use anyhow::Result;
use softbuffer::Context;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
#[cfg(debug_assertions)]
use winit::keyboard::{Key, NamedKey};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorIcon, Window};

use token::commands::{filter_commands, Cmd};
use token::messages::{AppMsg, EditorMsg, LayoutMsg, ModalMsg, Msg, UiMsg};
use token::model::editor::Position;
use token::model::editor_area::{Rect, SplitDirection};
use token::model::{AppModel, ModalState};
use token::update::update;

use crate::view::geometry::point_in_modal;

use super::input::handle_key;
use crate::view::Renderer;

use super::perf::PerfStats;

use std::path::PathBuf;
use winit::keyboard::ModifiersState;

pub struct App {
    model: AppModel,
    renderer: Option<Renderer>,
    window: Option<Rc<Window>>,
    context: Option<Context<Rc<Window>>>,
    last_tick: Instant,
    modifiers: ModifiersState,
    mouse_position: Option<(f64, f64)>,
    last_click_time: Instant,
    last_click_position: Option<(usize, usize)>,
    click_count: u32,
    last_option_press: Option<Instant>,
    option_double_tapped: bool,
    left_mouse_down: bool,
    last_auto_scroll: Option<Instant>,
    drag_start_position: Option<(f64, f64)>,
    drag_active: bool,
    msg_tx: Sender<Msg>,
    msg_rx: Receiver<Msg>,
    perf: PerfStats,
}

impl App {
    pub fn new(window_width: u32, window_height: u32, file_paths: Vec<PathBuf>) -> Self {
        let (msg_tx, msg_rx) = mpsc::channel();
        Self {
            model: AppModel::new(window_width, window_height, file_paths),
            renderer: None,
            window: None,
            context: None,
            last_tick: Instant::now(),
            modifiers: ModifiersState::empty(),
            mouse_position: None,
            last_click_time: Instant::now(),
            last_click_position: None,
            click_count: 0,
            last_option_press: None,
            option_double_tapped: false,
            left_mouse_down: false,
            last_auto_scroll: None,
            drag_start_position: None,
            drag_active: false,
            msg_tx,
            msg_rx,
            perf: PerfStats::default(),
        }
    }

    fn init_renderer(&mut self, window: Rc<Window>, context: &Context<Rc<Window>>) -> Result<()> {
        let renderer = Renderer::new(window, context)?;

        self.model.set_char_width(renderer.char_width());

        self.renderer = Some(renderer);
        Ok(())
    }

    fn try_auto_scroll_for_drag(&mut self, y: f64) -> Option<Cmd> {
        const AUTO_SCROLL_INTERVAL_MS: u64 = 80;

        let line_height = self.model.line_height as f64;
        let window_height = self.model.window_size.1 as f64;
        let status_bar_top = window_height - line_height;

        let scroll_direction = if y < 0.0 {
            Some(-1)
        } else if y >= status_bar_top {
            Some(1)
        } else {
            None
        };

        let direction = scroll_direction?;

        let now = Instant::now();
        if let Some(last) = self.last_auto_scroll {
            if now.duration_since(last) < Duration::from_millis(AUTO_SCROLL_INTERVAL_MS) {
                return None;
            }
        }

        self.last_auto_scroll = Some(now);
        update(&mut self.model, Msg::Editor(EditorMsg::Scroll(direction)))
    }

    fn update_cursor_icon(&mut self, x: f64, y: f64) {
        let Some(window) = &self.window else { return };
        let Some(renderer) = &self.renderer else {
            return;
        };

        // Compute layout to get splitters
        let status_bar_height = renderer.line_height();
        let (width, height) = renderer.dimensions();
        let available_rect = Rect::new(
            0.0,
            0.0,
            width as f32,
            (height as usize).saturating_sub(status_bar_height) as f32,
        );
        let splitters = self.model.editor_area.compute_layout(available_rect);

        // Check splitter bars first
        if let Some(idx) = self
            .model
            .editor_area
            .splitter_at_point(&splitters, x as f32, y as f32)
        {
            let icon = match splitters[idx].direction {
                SplitDirection::Horizontal => CursorIcon::ColResize,
                SplitDirection::Vertical => CursorIcon::RowResize,
            };
            window.set_cursor(icon);
            return;
        }

        // Status bar → Default
        if renderer.is_in_status_bar(y) {
            window.set_cursor(CursorIcon::Default);
            return;
        }

        // Tab bar → Default
        if renderer.is_in_tab_bar(y) {
            window.set_cursor(CursorIcon::Default);
            return;
        }

        // Editor area → Text (I-beam)
        window.set_cursor(CursorIcon::Text);
    }

    fn handle_event(&mut self, event: &WindowEvent) -> Option<Cmd> {
        match event {
            WindowEvent::Resized(size) => update(
                &mut self.model,
                Msg::App(AppMsg::Resize(size.width, size.height)),
            ),
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
                None
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let is_option_key = matches!(
                    event.physical_key,
                    PhysicalKey::Code(KeyCode::AltLeft) | PhysicalKey::Code(KeyCode::AltRight)
                );

                if is_option_key {
                    if event.state == ElementState::Pressed && !event.repeat {
                        let now = Instant::now();
                        if let Some(last) = self.last_option_press {
                            if now.duration_since(last) < Duration::from_millis(300) {
                                self.option_double_tapped = true;
                            }
                        }
                        self.last_option_press = Some(now);
                    } else if event.state == ElementState::Released {
                        self.option_double_tapped = false;
                    }
                }

                if event.state == ElementState::Pressed {
                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F2) {
                        self.perf.show_overlay = !self.perf.show_overlay;
                        return Some(Cmd::Redraw);
                    }

                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F7) {
                        let dump = crate::debug_dump::StateDump::from_model(&self.model);
                        match dump.save_to_file() {
                            Ok(filename) => eprintln!("[DEBUG] State dumped to: {}", filename),
                            Err(e) => eprintln!("[DEBUG] Failed to dump state: {}", e),
                        }
                        return Some(Cmd::Redraw);
                    }

                    #[cfg(debug_assertions)]
                    if event.logical_key == Key::Named(NamedKey::F8) {
                        if let Some(ref mut overlay) = self.model.debug_overlay {
                            overlay.toggle();
                        }
                        return Some(Cmd::Redraw);
                    }

                    let ctrl = self.modifiers.control_key();
                    let shift = self.modifiers.shift_key();
                    let alt = self.modifiers.alt_key();
                    let logo = self.modifiers.super_key();
                    handle_key(
                        &mut self.model,
                        event.logical_key.clone(),
                        event.physical_key,
                        ctrl,
                        shift,
                        alt,
                        logo,
                        self.option_double_tapped,
                    )
                } else {
                    None
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render() {
                    eprintln!("Render error: {}", e);
                }
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_position = Some((position.x, position.y));
                self.update_cursor_icon(position.x, position.y);

                if self.model.editor().rectangle_selection.active {
                    if let Some(renderer) = &mut self.renderer {
                        let (line, column) =
                            renderer.pixel_to_cursor(position.x, position.y, &self.model);
                        return update(
                            &mut self.model,
                            Msg::Editor(EditorMsg::UpdateRectangleSelection { line, column }),
                        );
                    }
                } else if self.left_mouse_down {
                    const DRAG_THRESHOLD_PIXELS: f64 = 4.0;

                    if let Some(renderer) = &mut self.renderer {
                        if !self.drag_active {
                            if let Some((start_x, start_y)) = self.drag_start_position {
                                let dx = position.x - start_x;
                                let dy = position.y - start_y;
                                let distance = (dx * dx + dy * dy).sqrt();

                                if distance >= DRAG_THRESHOLD_PIXELS {
                                    self.drag_active = true;
                                    let (start_line, start_col) =
                                        renderer.pixel_to_cursor(start_x, start_y, &self.model);
                                    self.model.editor_mut().primary_selection_mut().anchor =
                                        Position::new(start_line, start_col);
                                }
                            }
                        }

                        if self.drag_active {
                            let (line, column) =
                                renderer.pixel_to_cursor(position.x, position.y, &self.model);

                            self.model.editor_mut().primary_cursor_mut().line = line;
                            self.model.editor_mut().primary_cursor_mut().column = column;
                            self.model.editor_mut().primary_selection_mut().head =
                                Position::new(line, column);

                            self.try_auto_scroll_for_drag(position.y);

                            return Some(Cmd::Redraw);
                        }
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some((x, y)) = self.mouse_position {
                    // Modal mouse blocking - click outside closes, click inside is consumed
                    if self.model.ui.has_modal() {
                        let (has_list, list_items) = match &self.model.ui.active_modal {
                            Some(ModalState::CommandPalette(state)) => {
                                (true, filter_commands(&state.input).len())
                            }
                            _ => (false, 0),
                        };
                        let in_modal = point_in_modal(
                            x,
                            y,
                            self.model.window_size.0 as usize,
                            self.model.window_size.1 as usize,
                            self.model.line_height,
                            has_list,
                            list_items,
                        );

                        if in_modal {
                            // Future: could handle clicking on list items here
                            return Some(Cmd::Redraw);
                        } else {
                            // Click outside modal closes it
                            return update(
                                &mut self.model,
                                Msg::Ui(UiMsg::Modal(ModalMsg::Close)),
                            );
                        }
                    }

                    if let Some(renderer) = &mut self.renderer {
                        if renderer.is_in_status_bar(y) {
                            return None;
                        }

                        if renderer.is_in_tab_bar(y) {
                            // Find which group this tab bar belongs to
                            // Use y + 50 to hit the editor area below the tab bar
                            if let Some(group_id) = self
                                .model
                                .editor_area
                                .group_at_point(x as f32, y as f32 + 50.0)
                            {
                                // Focus the group if not already focused
                                if group_id != self.model.editor_area.focused_group_id {
                                    update(
                                        &mut self.model,
                                        Msg::Layout(LayoutMsg::FocusGroup(group_id)),
                                    );
                                }

                                // Find which tab was clicked
                                if let Some(group) = self.model.editor_area.groups.get(&group_id) {
                                    // Adjust x for the group's rect offset
                                    let local_x = x - group.rect.x as f64;
                                    if let Some(tab_index) =
                                        renderer.tab_at_position(local_x, &self.model, group)
                                    {
                                        return update(
                                            &mut self.model,
                                            Msg::Layout(LayoutMsg::SwitchToTab(tab_index)),
                                        );
                                    }
                                }
                            }
                            return None;
                        }

                        self.left_mouse_down = true;
                        self.drag_start_position = Some((x, y));
                        self.drag_active = false;

                        if let Some(group_id) =
                            self.model.editor_area.group_at_point(x as f32, y as f32)
                        {
                            if group_id != self.model.editor_area.focused_group_id {
                                update(
                                    &mut self.model,
                                    Msg::Layout(LayoutMsg::FocusGroup(group_id)),
                                );
                            }
                        }

                        let (line, column) = renderer.pixel_to_cursor(x, y, &self.model);
                        let now = Instant::now();
                        let double_click_time = Duration::from_millis(300);

                        let is_rapid_click =
                            now.duration_since(self.last_click_time) < double_click_time;
                        let is_same_position = self.last_click_position == Some((line, column));

                        if is_rapid_click && is_same_position {
                            self.click_count += 1;
                            if self.click_count > 3 {
                                self.click_count = 1;
                            }
                        } else {
                            self.click_count = 1;
                        }

                        self.last_click_time = now;
                        self.last_click_position = Some((line, column));

                        if self.modifiers.shift_key() {
                            return update(
                                &mut self.model,
                                Msg::Editor(EditorMsg::ExtendSelectionToPosition { line, column }),
                            );
                        }

                        if self.modifiers.alt_key() {
                            return update(
                                &mut self.model,
                                Msg::Editor(EditorMsg::ToggleCursorAtPosition { line, column }),
                            );
                        }

                        match self.click_count {
                            2 => {
                                update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                                return update(&mut self.model, Msg::Editor(EditorMsg::SelectWord));
                            }
                            3 => {
                                update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                                return update(&mut self.model, Msg::Editor(EditorMsg::SelectLine));
                            }
                            _ => {
                                self.model.editor_mut().clear_selection();
                                return update(
                                    &mut self.model,
                                    Msg::Editor(EditorMsg::SetCursorPosition { line, column }),
                                );
                            }
                        }
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.left_mouse_down = false;
                self.last_auto_scroll = None;
                self.drag_start_position = None;
                self.drag_active = false;
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Middle,
                ..
            } => {
                if let Some((x, y)) = self.mouse_position {
                    if let Some(renderer) = &mut self.renderer {
                        if renderer.is_in_status_bar(y) {
                            return None;
                        }

                        if renderer.is_in_tab_bar(y) {
                            return None;
                        }

                        let (line, column) = renderer.pixel_to_cursor(x, y, &self.model);
                        return update(
                            &mut self.model,
                            Msg::Editor(EditorMsg::StartRectangleSelection { line, column }),
                        );
                    }
                }
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Middle,
                ..
            } => {
                if self.model.editor().rectangle_selection.active {
                    return update(
                        &mut self.model,
                        Msg::Editor(EditorMsg::FinishRectangleSelection),
                    );
                }
                None
            }
            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                let (h_delta, v_delta) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => ((x * 3.0) as i32, (-y * 3.0) as i32),
                    MouseScrollDelta::PixelDelta(pos) => {
                        let line_height = self.model.line_height as f64;
                        let char_width = self.model.char_width as f64;
                        ((pos.x / char_width) as i32, (-pos.y / line_height) as i32)
                    }
                };

                let v_cmd = if v_delta != 0 {
                    update(&mut self.model, Msg::Editor(EditorMsg::Scroll(v_delta)))
                } else {
                    None
                };

                let h_cmd = if h_delta != 0 {
                    update(
                        &mut self.model,
                        Msg::Editor(EditorMsg::ScrollHorizontal(h_delta)),
                    )
                } else {
                    None
                };

                v_cmd.or(h_cmd)
            }
            WindowEvent::DroppedFile(path) => update(
                &mut self.model,
                Msg::Layout(LayoutMsg::OpenFileInNewTab(path.clone())),
            ),
            _ => None,
        }
    }

    fn render(&mut self) -> Result<()> {
        self.perf.start_frame();

        if let Some(renderer) = &mut self.renderer {
            renderer.render(&mut self.model, &mut self.perf)?;
        }

        self.perf.record_frame_time();
        self.perf.record_render_history();
        Ok(())
    }

    fn tick(&mut self) -> Option<Cmd> {
        update(&mut self.model, Msg::Ui(UiMsg::BlinkCursor))
    }

    fn process_cmd(&self, cmd: Cmd) {
        match cmd {
            Cmd::None => {}
            Cmd::Redraw => {}
            Cmd::SaveFile { path, content } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::write(&path, content).map_err(|e| e.to_string());
                    let _ = tx.send(Msg::App(AppMsg::SaveCompleted(result)));
                });
            }
            Cmd::LoadFile { path } => {
                let tx = self.msg_tx.clone();
                std::thread::spawn(move || {
                    let result = std::fs::read_to_string(&path).map_err(|e| e.to_string());
                    let _ = tx.send(Msg::App(AppMsg::FileLoaded { path, result }));
                });
            }
            Cmd::Batch(cmds) => {
                for cmd in cmds {
                    self.process_cmd(cmd);
                }
            }
        }
    }

    fn process_async_messages(&mut self) -> bool {
        let mut needs_redraw = false;
        while let Ok(msg) = self.msg_rx.try_recv() {
            if let Some(cmd) = update(&mut self.model, msg) {
                if cmd.needs_redraw() {
                    needs_redraw = true;
                }
                self.process_cmd(cmd);
            }
        }
        needs_redraw
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("Token")
                .with_inner_size(LogicalSize::new(800, 600)); // TODO: Persist window size/position/monitor on exit/boot

            let window = Rc::new(event_loop.create_window(window_attributes).unwrap());
            let context = Context::new(Rc::clone(&window)).unwrap();

            self.init_renderer(Rc::clone(&window), &context).unwrap();
            self.window = Some(window);
            self.context = Some(context);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let should_exit = matches!(event, WindowEvent::CloseRequested);
        let should_redraw = if let Some(window) = &self.window {
            if window_id == window.id() && !should_exit {
                if let Some(cmd) = self.handle_event(&event) {
                    let needs_redraw = cmd.needs_redraw();
                    self.process_cmd(cmd);
                    needs_redraw
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_exit {
            event_loop.exit();
        } else if should_redraw {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);

        if self.process_async_messages() {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        let now = Instant::now();
        if now.duration_since(self.last_tick) > Duration::from_millis(500) {
            self.last_tick = now;
            if self.tick().is_some() {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }
}
