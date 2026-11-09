use std::{env, error::Error, fmt, fs, path::PathBuf, sync::mpsc::Sender, thread, time::Instant};

use flair_core::{
    command::Command,
    config::Config,
    geom::{CellPoint, GridSize},
    ids::PaneId,
    key::{InputEvent, KeyInput, MouseAction, MouseButton, MouseInput},
    link::ClientLink,
    proto::{ClientKind, ClientMsg, PROTOCOL, ServerMsg, Target, WindowLayout},
    status::StatusLine,
    theme::Theme,
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
    keyboard::ModifiersState,
    platform::wayland::WindowAttributesExtWayland,
    raw_window_handle::HasDisplayHandle,
    window::{Window, WindowId},
};

use crate::{
    Fonts,
    chrome_layout::{self, ChromeInput, ChromeLayout, Hit},
    clipboard::Clipboard,
    compose::{compose, compose_frame},
    drive::Drive,
    input::InputState,
    mirror::Mirror,
    mouse::{self, WheelAccumulator},
    mouse_route::{Capture, MouseRoute, bar_command, capture_after, route_mouse},
    notice::Notice,
    overlays::{OverlayEvent, Overlays},
    probe::Probe,
    renderer::{Renderer, WindowContext},
    status::status_parts,
    theme_file::{resolve_theme, save_theme},
    theme_picker::ThemeOutcome,
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
    Drive(Drive),
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
    config_path: PathBuf,
    theme: Theme,
    preview: Option<Theme>,
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
    overlays: Overlays,
    capture: Capture,
    input: InputState,
    layout: Option<WindowLayout>,
    status: StatusLine,
    hits: Vec<Hit>,
    drive: Option<(Vec<Drive>, EventLoopProxy<UserEvent>)>,
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
            (f32::from(self.config.window.padding_x) * scale as f32).round(),
            (f32::from(self.config.window.padding_y) * scale as f32).round(),
        )
    }
    fn size(&self) -> GridSize {
        let Some(surface) = &self.surface else {
            return GridSize { cols: 80, rows: 24 };
        };
        let px = surface.window.inner_size();
        let scale = surface.window.scale_factor();
        chrome_layout::grid_size(
            (px.width, px.height),
            self.padding(scale),
            scale as f32,
            surface.fonts.metrics(),
        )
    }
    fn cell(&self) -> Option<CellPoint> {
        let surface = self.surface.as_ref()?;
        let scale = surface.window.scale_factor();
        let px = surface.window.inner_size();
        Some(mouse::to_cell(
            (self.pointer.x, self.pointer.y),
            &surface.fonts.metrics(),
            chrome_layout::grid_origin(
                (px.width, px.height),
                self.padding(scale),
                scale as f32,
                surface.fonts.metrics(),
            ),
            self.size(),
        ))
    }
    fn in_bar(&self) -> bool {
        self.surface.as_ref().is_some_and(|surface| {
            self.pointer.y
                < f64::from(chrome_layout::bar_height(
                    surface.window.scale_factor() as f32
                ))
        })
    }
    fn mouse(&mut self, button: MouseButton, action: MouseAction) {
        let Some(point) = self.cell() else {
            return;
        };
        let route = route_mouse(self.in_bar(), button, action, self.capture);
        self.capture = capture_after(self.capture, button, action, route);
        match route {
            MouseRoute::Bar => {
                if let Some(target) =
                    chrome_layout::hit_test(&self.hits, (self.pointer.x, self.pointer.y))
                {
                    self.send(ClientMsg::Command(bar_command(target)));
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
    fn theme_outcome(&mut self, outcome: ThemeOutcome, event_loop: &ActiveEventLoop) {
        match outcome {
            ThemeOutcome::Pending => {}
            ThemeOutcome::Preview(name) => {
                let dir = Config::themes_dir(&self.config_path);
                self.preview = Some(self.config.with_theme(&name).load_theme(&dir).0);
            }
            ThemeOutcome::Cancel => self.preview = None,
            ThemeOutcome::Save(name) => {
                self.preview = None;
                match save_theme(&self.config_path, &name) {
                    Ok(config) => {
                        self.overlays
                            .set_notice(Notice::info(format!("theme saved: {name}")));
                        self.reload(config, event_loop);
                        self.send(ClientMsg::Command(Command::ReloadConfig));
                    }
                    Err(error) => {
                        log::warn!(
                            "could not save theme {name:?} to {}: {error}",
                            self.config_path.display()
                        );
                        self.overlays
                            .set_notice(Notice::error(format!("could not save theme: {error}")));
                    }
                }
            }
        }
        self.request_redraw();
    }
    fn overlay_event(&mut self, event: OverlayEvent, event_loop: &ActiveEventLoop) {
        match event {
            OverlayEvent::Ignored => {}
            OverlayEvent::Redraw => self.request_redraw(),
            OverlayEvent::Send(message) => {
                self.send(message);
                self.request_redraw();
            }
            OverlayEvent::Theme(outcome) => self.theme_outcome(outcome, event_loop),
        }
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
        let (theme, notice) = resolve_theme(&self.config, &self.config_path);
        if let Some(notice) = notice {
            self.overlays.set_notice(notice);
        }
        if theme != self.theme {
            self.theme = theme;
            self.send(ClientMsg::Palette(self.theme.clone()));
        }
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
        let theme = self.preview.as_ref().unwrap_or(&self.theme).clone();
        let mut grid = match (&self.layout, self.pane) {
            (Some(layout), _) => compose_frame(&self.mirror, layout, self.size(), &theme, true),
            (None, Some(pane)) => compose(&self.mirror, pane, &theme, true),
            (None, None) => return,
        };
        let Some(scale) = self.surface.as_ref().map(|s| s.window.scale_factor()) else {
            return;
        };
        let padding = self.padding(scale);
        let Some(surface) = &mut self.surface else {
            return;
        };
        let metrics = surface.fonts.metrics();
        if self.overlays.is_open() {
            grid.cursor = None;
        }
        let parts = status_parts(&self.status);
        let px = surface.window.inner_size();
        let input = ChromeInput {
            px: (px.width, px.height),
            scale: scale as f32,
            padding,
            metrics,
            ui: surface.fonts.ui_metrics(),
            theme: &theme,
            status: &parts,
            prompt: self.overlays.bar_field(),
            layout: self.layout.as_ref(),
            hover: Some((self.pointer.x as f32, self.pointer.y as f32)),
            overlays: Some(&self.overlays),
            notice: self.overlays.notice(),
        };
        let ChromeLayout {
            chrome,
            hits,
            overlay,
        } = chrome_layout::layout(&input, |text, bold| surface.fonts.ui_width(text, bold));
        self.hits = hits;
        self.overlays.place(overlay);
        if let Err(error) = surface
            .renderer
            .draw_frame(&grid, &chrome, &mut surface.fonts, theme.background)
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
        if let Some((steps, proxy)) = self.drive.take() {
            thread::spawn(move || {
                for step in steps {
                    match step {
                        Drive::Sleep(duration) => thread::sleep(duration),
                        Drive::Key(_) | Drive::Text(_) | Drive::Resize { .. } => {
                            if proxy.send_event(UserEvent::Drive(step)).is_err() {
                                return;
                            }
                        }
                    }
                }
            });
        }
    }
    fn text(&mut self, text: String, event_loop: &ActiveEventLoop) {
        self.set_ime(false);
        match self.overlays.text(&text) {
            Some(event) => self.overlay_event(event, event_loop),
            None => self.send(ClientMsg::Input(InputEvent::Text(text))),
        }
    }
    fn key(&mut self, key: KeyInput, event_loop: &ActiveEventLoop) {
        if self.overlays.notice().is_some() {
            self.request_redraw();
        }
        let surface = &self.surface;
        match self
            .overlays
            .key(&key, || surface.as_ref().and_then(|s| s.clipboard.load()))
        {
            Some(event) => self.overlay_event(event, event_loop),
            None => self.send(ClientMsg::Input(InputEvent::Key(key))),
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
            UserEvent::Message(message)
                if self
                    .overlays
                    .open(&message, &self.status, &self.config, &self.config_path) =>
            {
                self.request_redraw();
            }
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
                ServerMsg::Layout(layout) => {
                    self.mirror.retain_layout(&layout);
                    self.layout = Some(layout);
                    self.request_redraw();
                }
                ServerMsg::Status(status) => {
                    self.status = status;
                    self.request_redraw();
                }
                ServerMsg::PasteClipboard => {
                    if let Some(text) = self.surface.as_ref().and_then(|s| s.clipboard.load()) {
                        self.send(ClientMsg::Input(InputEvent::Paste(text)));
                    }
                }
                ServerMsg::ReloadConfig { toml } => {
                    let text = fs::read_to_string(&self.config_path).unwrap_or(toml);
                    match Config::from_toml(&text) {
                        Ok(config) => {
                            if self.overlays.notice().is_none_or(|notice| notice.error) {
                                self.overlays.set_notice(Notice::info("config reloaded"));
                            }
                            self.reload(config, event_loop);
                        }
                        Err(error) => {
                            log::warn!("ignoring reloaded config: {error}");
                            self.overlays
                                .set_notice(Notice::error(format!("config not reloaded: {error}")));
                            self.request_redraw();
                        }
                    }
                    if let Some(picker) = self.overlays.theme_picker_mut() {
                        let outcome = picker.refresh(&self.config.theme);
                        self.theme_outcome(outcome, event_loop);
                    }
                }
                ServerMsg::Sessionizer { .. }
                | ServerMsg::ChooseSession { .. }
                | ServerMsg::ChooseWindow
                | ServerMsg::PromptRename { .. }
                | ServerMsg::PromptSearch { .. }
                | ServerMsg::ChooseTheme
                | ServerMsg::Settings
                | ServerMsg::Sessions(_)
                | ServerMsg::Done => {}
                ServerMsg::Error(error) => self.fail(error, event_loop),
                ServerMsg::Exit(_) => {
                    self.redraw(event_loop);
                    event_loop.exit();
                }
            },
            UserEvent::Disconnected => event_loop.exit(),
            UserEvent::Drive(Drive::Key(key)) => self.key(key, event_loop),
            UserEvent::Drive(Drive::Text(text)) => self.text(text, event_loop),
            UserEvent::Drive(Drive::Resize { width, height }) => {
                if let Some(surface) = &self.surface {
                    let _ = surface
                        .window
                        .request_inner_size(PhysicalSize::new(width, height));
                }
            }
            UserEvent::Drive(Drive::Sleep(duration)) => thread::sleep(duration),
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
            WindowEvent::Ime(Ime::Commit(text)) => self.text(text, event_loop),
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
                self.key(key, event_loop);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let target = |app: &App, pos: (f64, f64)| {
                    (
                        chrome_layout::hit_test(&app.hits, pos).filter(|_| !app.overlays.is_open()),
                        app.overlays.hit_test(pos),
                    )
                };
                let before = target(self, (self.pointer.x, self.pointer.y));
                self.pointer = position;
                if target(self, (position.x, position.y)) != before {
                    self.request_redraw();
                }
                if self.overlays.is_open() || self.capture == Capture::Overlay {
                    return;
                }
                self.mouse(self.button, MouseAction::Motion);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.overlays.is_open() || self.capture == Capture::Overlay {
                    if state == ElementState::Released {
                        self.capture = Capture::Idle;
                        self.button = MouseButton::NoButton;
                        return;
                    }
                    let open = self.overlays.is_open();
                    self.capture = Capture::Overlay;
                    if open
                        && button == winit::event::MouseButton::Left
                        && let Some(hit) = self.overlays.hit_test((self.pointer.x, self.pointer.y))
                    {
                        let event = self.overlays.click(hit);
                        self.overlay_event(event, event_loop);
                    }
                    return;
                }
                if button == winit::event::MouseButton::Middle
                    && let Some(point) = self.cell()
                    && route_mouse(
                        self.in_bar(),
                        MouseButton::Middle,
                        MouseAction::Press,
                        self.capture,
                    ) == MouseRoute::Server
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
                let line_px = self
                    .surface
                    .as_ref()
                    .map_or(16.0, |s| s.fonts.metrics().height);
                let lines = self.wheel.push(delta, line_px);
                if self.capture == Capture::Overlay {
                    return;
                }
                if self.overlays.is_open() {
                    let event = self.overlays.wheel(-lines);
                    self.overlay_event(event, event_loop);
                    return;
                }
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

pub fn run(
    config: Config,
    config_path: PathBuf,
    link: ClientLink,
    probe: Option<Instant>,
) -> Result<(), GuiError> {
    run_target(
        config,
        config_path,
        link,
        probe,
        Target::Main,
        Vec::new(),
        Vec::new(),
    )
}

pub fn run_target(
    config: Config,
    config_path: PathBuf,
    link: ClientLink,
    probe: Option<Instant>,
    target: Target,
    env: Vec<(String, String)>,
    drive: Vec<Drive>,
) -> Result<(), GuiError> {
    let (theme, notice) = resolve_theme(&config, &config_path);
    link.tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 80, rows: 24 },
            palette: theme.clone(),
            env,
            target,
        })
        .map_err(|e| GuiError(e.to_string()))?;
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| GuiError(e.to_string()))?;
    let proxy = event_loop.create_proxy();
    let drive = (!drive.is_empty()).then(|| (drive, proxy.clone()));
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
        config_path,
        theme,
        preview: None,
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
        overlays: Overlays::new(env::var_os("HOME").map(PathBuf::from)),
        capture: Capture::Idle,
        input: InputState::default(),
        layout: None,
        status: StatusLine::default(),
        hits: Vec::new(),
        drive,
    };
    if let Some(notice) = notice {
        app.overlays.set_notice(notice);
    }
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
