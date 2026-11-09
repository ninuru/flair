use std::{error::Error, fmt, sync::mpsc::Sender, time::Instant};

use flair_core::{
    command::{Command, WindowRef},
    config::Config,
    geom::{CellPoint, GridSize},
    ids::PaneId,
    key::{InputEvent, Key, KeyAction, KeyInput, MouseAction, MouseButton, MouseInput, NamedKey},
    link::ClientLink,
    proto::{ClientKind, ClientMsg, PROTOCOL, ServerMsg, Target, WindowLayout},
    status::StatusLine,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, Ime, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::ModifiersState,
    platform::wayland::WindowAttributesExtWayland,
    raw_window_handle::HasDisplayHandle,
    window::{Window, WindowId},
};

use crate::{
    Fonts,
    clipboard::Clipboard,
    compose::{compose, compose_frame},
    input::{self, InputState, LocalAction},
    mirror::Mirror,
    mouse::{self, WheelAccumulator},
    picker::{self, Picker},
    probe::Probe,
    prompt::{Outcome, Prompt},
    renderer::{Renderer, WindowContext},
    status::compose_status,
};

#[derive(Debug)]
pub struct GuiError(pub String);
impl fmt::Display for GuiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for GuiError {}

enum UserEvent {
    Message(ServerMsg),
    Disconnected,
}

struct Surface {
    renderer: Renderer,
    context: WindowContext,
    fonts: Fonts,
    clipboard: Clipboard,
    window: Window,
}

struct App {
    config: Config,
    tx: Sender<ClientMsg>,
    surface: Option<Surface>,
    mirror: Mirror,
    pane: Option<PaneId>,
    ack: Option<u64>,
    mods: ModifiersState,
    pointer: PhysicalPosition<f64>,
    button: MouseButton,
    wheel: WheelAccumulator,
    ime: bool,
    probe: Option<Probe>,
    error: Option<GuiError>,
    picker: Option<Picker>,
    prompt: Option<Prompt>,
    held: bool,
    input: InputState,
    layout: Option<WindowLayout>,
    status: StatusLine,
}

impl App {
    fn send(&self, message: ClientMsg) {
        if let Err(error) = self.tx.send(message) {
            log::debug!("server connection closed: {error}");
        }
    }
    fn fail(&mut self, error: impl fmt::Display, event_loop: &ActiveEventLoop) {
        self.error = Some(GuiError(error.to_string()));
        self.send(ClientMsg::Detach);
        event_loop.exit();
    }
    fn padding(&self, scale: f64) -> (f32, f32) {
        (
            f32::from(self.config.window.padding_x) * scale as f32,
            f32::from(self.config.window.padding_y) * scale as f32,
        )
    }
    fn size(&self) -> GridSize {
        let Some(surface) = &self.surface else {
            return GridSize { cols: 80, rows: 24 };
        };
        let px = surface.window.inner_size();
        let padding = self.padding(surface.window.scale_factor());
        let metrics = surface.fonts.metrics();
        GridSize {
            cols: ((px.width as f32 - padding.0 * 2.0).max(metrics.width) / metrics.width)
                .floor()
                .clamp(1.0, u16::MAX as f32) as u16,
            rows: ((px.height as f32 - padding.1 * 2.0).max(metrics.height) / metrics.height)
                .floor()
                .clamp(1.0, u16::MAX as f32) as u16,
        }
    }
    fn cell(&self) -> Option<CellPoint> {
        let surface = self.surface.as_ref()?;
        Some(mouse::to_cell(
            (self.pointer.x, self.pointer.y),
            &surface.fonts.metrics(),
            self.padding(surface.window.scale_factor()),
            self.size(),
        ))
    }
    fn mouse(&mut self, button: MouseButton, action: MouseAction) {
        let Some(point) = self.cell() else {
            return;
        };
        let route = route_mouse(point, button, action, self.held);
        self.held = held_after(self.held, button, action, route);
        match route {
            MouseRoute::Status => {
                if let Some(window) =
                    compose_status(&self.status, self.size().cols, &self.config.theme())
                        .hit_test(point)
                {
                    self.send(ClientMsg::Command(Command::SelectWindow(WindowRef::Id(
                        window,
                    ))));
                }
            }
            MouseRoute::Drop => {}
            MouseRoute::Server => {
                let mods = flair_core::key::Mods {
                    ctrl: self.mods.control_key(),
                    shift: self.mods.shift_key(),
                    alt: self.mods.alt_key(),
                    super_: self.mods.super_key(),
                };
                self.send(ClientMsg::Input(InputEvent::Mouse(MouseInput {
                    point,
                    button,
                    action,
                    mods,
                })));
            }
        }
    }
    fn request_redraw(&self) {
        if let Some(surface) = &self.surface {
            surface.window.request_redraw();
        }
    }
    fn picker_key(&mut self, key: &KeyInput) {
        let Some(picker) = &mut self.picker else {
            return;
        };
        let mut close = false;
        let mut target = None;
        match key.key {
            Key::Named(NamedKey::Escape) => close = true,
            Key::Named(NamedKey::Enter) => {
                target = picker.accept();
                close = target.is_some();
            }
            Key::Named(NamedKey::Backspace) => picker.backspace(),
            Key::Named(NamedKey::Up) => picker.move_selection(-1),
            Key::Named(NamedKey::Down) => picker.move_selection(1),
            Key::Char('u' | 'U') if key.mods.ctrl => picker.clear(),
            Key::Char(_) if !key.mods.ctrl && !key.mods.alt && !key.mods.super_ => {
                if let Some(text) = &key.text {
                    picker.insert(text);
                }
            }
            Key::Char(_) | Key::Named(_) => {}
        }
        if close {
            self.picker = None;
        }
        if let Some(target) = target {
            self.send(ClientMsg::Command(Command::Switch(target)));
        }
        self.request_redraw();
    }
    fn prompt_key(&mut self, key: &KeyInput) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        match prompt.key(key) {
            Outcome::Pending => {}
            Outcome::Close => self.prompt = None,
            Outcome::Run(command) => {
                self.prompt = None;
                self.send(ClientMsg::Command(command));
            }
        }
        self.request_redraw();
    }
    fn set_ime(&mut self, active: bool) {
        if self.ime != active {
            self.ime = active;
            self.send(ClientMsg::Input(InputEvent::Ime(active)));
        }
    }
    fn pane_at(&self, point: CellPoint) -> Option<PaneId> {
        self.layout
            .as_ref()
            .and_then(|layout| {
                layout
                    .panes
                    .iter()
                    .find(|placement| placement.rect.contains(point))
                    .map(|placement| placement.pane)
            })
            .or(self.pane)
    }
    fn reload(&mut self, config: Config, event_loop: &ActiveEventLoop) {
        let fonts_changed = config.font != self.config.font;
        self.config = config;
        if fonts_changed && let Some(surface) = &mut self.surface {
            match Fonts::load(&self.config.font, surface.window.scale_factor()) {
                Ok(fonts) => {
                    surface.renderer.set_metrics(fonts.metrics());
                    surface.fonts = fonts;
                }
                Err(error) => {
                    self.fail(error, event_loop);
                    return;
                }
            }
        }
        self.send(ClientMsg::Resize(self.size()));
        self.request_redraw();
    }
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let theme = self.config.theme();
        let mut grid = match (&self.layout, self.pane) {
            (Some(layout), _) => compose_frame(
                &self.mirror,
                layout,
                &self.status,
                self.size(),
                &theme,
                true,
            ),
            (None, Some(pane)) => compose(&self.mirror, pane, &theme, true),
            (None, None) => return,
        };
        if let Some(picker) = &self.picker {
            picker.draw(&mut grid, &theme);
        }
        if let Some(prompt) = &self.prompt {
            prompt.draw(&mut grid, &theme);
        }
        let padding = self.padding(
            self.surface
                .as_ref()
                .map_or(1.0, |s| s.window.scale_factor()),
        );
        let Some(surface) = &mut self.surface else {
            return;
        };
        if let Err(error) = surface
            .renderer
            .draw(&grid, &mut surface.fonts, padding, theme.background)
            .and_then(|()| surface.context.swap_buffers())
        {
            self.fail(error, event_loop);
            return;
        }
        if let Some(seq) = self.ack.take() {
            self.send(ClientMsg::FrameAck { seq });
        }
        if let Some(mut probe) = self.probe.take() {
            probe.mark("first_frame");
            probe.finish();
            self.send(ClientMsg::Detach);
            event_loop.exit();
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        let result = (|| -> Result<Surface, GuiError> {
            let attrs = Window::default_attributes()
                .with_title("flair")
                .with_name("flair", "flair")
                .with_decorations(false)
                .with_inner_size(LogicalSize::new(960.0, 600.0));
            let window = event_loop
                .create_window(attrs)
                .map_err(|e| GuiError(e.to_string()))?;
            if let Some(probe) = &mut self.probe {
                probe.mark("window");
            }
            window.set_ime_allowed(true);
            let font_config = self.config.font.clone();
            let scale = window.scale_factor();
            let font_worker = std::thread::Builder::new()
                .name("flair-fonts".into())
                .spawn(move || Fonts::load(&font_config, scale))
                .map_err(|e| GuiError(e.to_string()))?;
            let (context, gl) = WindowContext::new(&window).map_err(|e| GuiError(e.to_string()))?;
            let fonts = font_worker
                .join()
                .map_err(|_| GuiError("font loader panicked".into()))?
                .map_err(|e| GuiError(e.to_string()))?;
            if let Some(probe) = &mut self.probe {
                probe.mark("fonts");
            }
            let mut renderer =
                Renderer::new(gl, fonts.metrics()).map_err(|e| GuiError(e.to_string()))?;
            let px = window.inner_size();
            renderer.resize((px.width, px.height));
            if let Some(probe) = &mut self.probe {
                probe.mark("gl");
            }
            let display = window
                .display_handle()
                .map_err(|e| GuiError(e.to_string()))?;
            let clipboard = Clipboard::new(display.as_raw());
            Ok(Surface {
                renderer,
                context,
                fonts,
                clipboard,
                window,
            })
        })();
        match result {
            Ok(surface) => {
                self.surface = Some(surface);
                self.send(ClientMsg::Resize(self.size()));
            }
            Err(error) => self.fail(error, event_loop),
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Message(message) => match message {
                ServerMsg::Welcome { .. } => {
                    if let Some(probe) = &mut self.probe {
                        probe.mark("server");
                    }
                }
                ServerMsg::Frame { seq, panes } => {
                    self.pane = panes.first().map(|p| p.pane).or(self.pane);
                    self.mirror.apply(&panes);
                    self.ack = Some(seq);
                    if let Some(surface) = &self.surface {
                        surface.window.request_redraw();
                    }
                }
                ServerMsg::Title(title) => {
                    if let Some(surface) = &self.surface {
                        surface.window.set_title(&title);
                    }
                }
                ServerMsg::Bell => {}
                ServerMsg::SetClipboard { text, primary } => {
                    if let Some(surface) = &self.surface {
                        surface.clipboard.store(text, primary);
                    }
                }
                ServerMsg::Sessionizer {
                    sessions,
                    current,
                    last,
                } => {
                    self.picker =
                        Some(Picker::new(sessions, &current, last, picker::directories()));
                    self.request_redraw();
                }
                ServerMsg::Layout(layout) => {
                    self.mirror.retain_layout(&layout);
                    self.layout = Some(layout);
                    self.request_redraw();
                }
                ServerMsg::Status(status) => {
                    self.status = status;
                    self.request_redraw();
                }
                ServerMsg::ChooseWindow => {
                    self.prompt = Some(Prompt::choose(&self.status));
                    self.request_redraw();
                }
                ServerMsg::PromptRename { current } => {
                    let current = if current.is_empty() {
                        self.status
                            .windows
                            .iter()
                            .find(|window| window.active)
                            .map(|window| window.name.clone())
                            .unwrap_or_default()
                    } else {
                        current
                    };
                    self.prompt = Some(Prompt::rename(current));
                    self.request_redraw();
                }
                ServerMsg::PasteClipboard => {
                    if let Some(text) = self.surface.as_ref().and_then(|s| s.clipboard.load()) {
                        self.send(ClientMsg::Input(InputEvent::Paste(text)));
                    }
                }
                ServerMsg::ReloadConfig { toml } => match Config::from_toml(&toml) {
                    Ok(config) => self.reload(config, event_loop),
                    Err(error) => log::warn!("ignoring reloaded config: {error}"),
                },
                ServerMsg::Sessions(_) | ServerMsg::Done => {}
                ServerMsg::Error(error) => self.fail(error, event_loop),
                ServerMsg::Exit(_) => {
                    self.redraw(event_loop);
                    event_loop.exit();
                }
            },
            UserEvent::Disconnected => event_loop.exit(),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !self.surface.as_ref().is_some_and(|s| s.window.id() == id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.send(ClientMsg::Detach);
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => self.redraw(event_loop),
            WindowEvent::Resized(size) => {
                if size.width == 0 || size.height == 0 {
                    return;
                }
                if let Some(surface) = &mut self.surface {
                    surface.context.resize((size.width, size.height));
                    surface.renderer.resize((size.width, size.height));
                }
                self.send(ClientMsg::Resize(self.size()));
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                match Fonts::load(&self.config.font, scale_factor) {
                    Ok(fonts) => {
                        if let Some(surface) = &mut self.surface {
                            surface.renderer.set_metrics(fonts.metrics());
                            surface.fonts = fonts;
                        }
                        self.send(ClientMsg::Resize(self.size()));
                    }
                    Err(error) => self.fail(error, event_loop),
                }
            }
            WindowEvent::ModifiersChanged(mods) => self.mods = mods.state(),
            WindowEvent::Focused(focused) => self.send(ClientMsg::Focus(focused)),
            WindowEvent::Ime(Ime::Preedit(text, _)) => self.set_ime(!text.is_empty()),
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.set_ime(false);
                if let Some(picker) = &mut self.picker {
                    picker.insert(&text);
                    self.request_redraw();
                } else if let Some(prompt) = &mut self.prompt {
                    prompt.insert(&text);
                    self.request_redraw();
                } else {
                    self.send(ClientMsg::Input(InputEvent::Text(text)));
                }
            }
            WindowEvent::Ime(Ime::Disabled) => self.set_ime(false),
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if is_synthetic || self.ime {
                    return;
                }
                let Some(key) = self.input.translate(&event, self.mods) else {
                    return;
                };
                if self.picker.is_some() {
                    if key.action == KeyAction::Release {
                        return;
                    }
                    if input::local_action(&key) == Some(LocalAction::Paste) {
                        if let Some(text) = self.surface.as_ref().and_then(|s| s.clipboard.load()) {
                            if let Some(picker) = &mut self.picker {
                                picker.insert(&text);
                            }
                            self.request_redraw();
                        }
                    } else {
                        self.picker_key(&key);
                    }
                    return;
                }
                if self.prompt.is_some() {
                    self.prompt_key(&key);
                    return;
                }
                self.send(ClientMsg::Input(InputEvent::Key(key)));
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = position;
                if self.picker.is_some() || self.prompt.is_some() {
                    return;
                }
                self.mouse(self.button, MouseAction::Motion);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.picker.is_some() || self.prompt.is_some() {
                    return;
                }
                if button == winit::event::MouseButton::Middle
                    && let Some(point) = self.cell()
                    && route_mouse(point, MouseButton::Middle, MouseAction::Press, self.held)
                        == MouseRoute::Server
                    && (self.mods.shift_key()
                        || self
                            .pane_at(point)
                            .and_then(|pane| self.mirror.pane(pane))
                            .is_some_and(|pane| !pane.flags.mouse_reporting))
                {
                    if state == ElementState::Pressed {
                        if let Some(text) = self
                            .surface
                            .as_ref()
                            .and_then(|s| s.clipboard.load_primary())
                        {
                            self.mouse(MouseButton::Middle, MouseAction::Press);
                            self.send(ClientMsg::Input(InputEvent::Paste(text)));
                        }
                    } else {
                        self.mouse(MouseButton::Middle, MouseAction::Release);
                    }
                    return;
                }
                let button = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    _ => MouseButton::NoButton,
                };
                self.button = if state == ElementState::Pressed {
                    button
                } else {
                    MouseButton::NoButton
                };
                self.mouse(
                    button,
                    if state == ElementState::Pressed {
                        MouseAction::Press
                    } else {
                        MouseAction::Release
                    },
                );
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.picker.is_some() || self.prompt.is_some() {
                    return;
                }
                let line_px = self
                    .surface
                    .as_ref()
                    .map_or(16.0, |s| s.fonts.metrics().height);
                let lines = self.wheel.push(delta, line_px);
                let button = if lines >= 0 {
                    MouseButton::WheelUp
                } else {
                    MouseButton::WheelDown
                };
                for _ in 0..lines.unsigned_abs().min(100) {
                    self.mouse(button, MouseAction::Press);
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseRoute {
    Status,
    Server,
    Drop,
}

fn route_mouse(
    point: CellPoint,
    button: MouseButton,
    action: MouseAction,
    held: bool,
) -> MouseRoute {
    if held || point.row > 0 {
        MouseRoute::Server
    } else if button == MouseButton::Left && action == MouseAction::Press {
        MouseRoute::Status
    } else {
        MouseRoute::Drop
    }
}

fn held_after(held: bool, button: MouseButton, action: MouseAction, route: MouseRoute) -> bool {
    match action {
        MouseAction::Release => false,
        MouseAction::Press => {
            held || (route == MouseRoute::Server
                && matches!(
                    button,
                    MouseButton::Left | MouseButton::Middle | MouseButton::Right
                ))
        }
        MouseAction::Motion => held,
    }
}

pub fn run(config: Config, link: ClientLink, probe: Option<Instant>) -> Result<(), GuiError> {
    run_target(config, link, probe, Target::Main, Vec::new())
}

pub fn run_target(
    config: Config,
    link: ClientLink,
    probe: Option<Instant>,
    target: Target,
    env: Vec<(String, String)>,
) -> Result<(), GuiError> {
    link.tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 80, rows: 24 },
            palette: config.theme(),
            env,
            target,
        })
        .map_err(|e| GuiError(e.to_string()))?;
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| GuiError(e.to_string()))?;
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        while let Ok(message) = link.rx.recv() {
            if proxy.send_event(UserEvent::Message(message)).is_err() {
                return;
            }
        }
        let _ = proxy.send_event(UserEvent::Disconnected);
    });
    let mut app = App {
        config,
        tx: link.tx,
        surface: None,
        mirror: Mirror::default(),
        pane: None,
        ack: None,
        mods: ModifiersState::empty(),
        pointer: PhysicalPosition::new(0.0, 0.0),
        button: MouseButton::NoButton,
        wheel: WheelAccumulator::default(),
        ime: false,
        probe: probe.map(Probe::start),
        error: None,
        picker: None,
        prompt: None,
        held: false,
        input: InputState::default(),
        layout: None,
        status: StatusLine::default(),
    };
    if let Some(probe) = &mut app.probe {
        probe.mark("event_loop");
    }
    let result = event_loop
        .run_app(&mut app)
        .map_err(|e| GuiError(e.to_string()));
    app.send(ClientMsg::Detach);
    result?;
    match app.error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
