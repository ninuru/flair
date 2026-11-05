use std::{error::Error, fmt, sync::mpsc::Sender, time::Instant};

use flair_core::{
    command::Command,
    config::Config,
    geom::GridSize,
    ids::PaneId,
    key::{InputEvent, MouseAction, MouseButton, MouseInput},
    link::ClientLink,
    proto::{ClientKind, ClientMsg, PROTOCOL, ServerMsg, Target},
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
    compose::compose,
    input::{self, LocalAction},
    mirror::Mirror,
    mouse::{self, WheelAccumulator},
    probe::Probe,
    renderer::{Renderer, WindowContext},
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
    fn mouse(&self, button: MouseButton, action: MouseAction) {
        let Some(surface) = &self.surface else {
            return;
        };
        let point = mouse::to_cell(
            (self.pointer.x, self.pointer.y),
            &surface.fonts.metrics(),
            self.padding(surface.window.scale_factor()),
            self.size(),
        );
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
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        let Some(pane) = self.pane else {
            return;
        };
        let theme = self.config.theme();
        let grid = compose(&self.mirror, pane, &theme, true);
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
            WindowEvent::Ime(Ime::Preedit(text, _)) => self.ime = !text.is_empty(),
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.ime = false;
                self.send(ClientMsg::Input(InputEvent::Text(text)));
            }
            WindowEvent::Ime(Ime::Disabled) => self.ime = false,
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if is_synthetic || event.state != ElementState::Pressed || self.ime {
                    return;
                }
                if let Some(key) = input::translate(
                    &event.logical_key,
                    event.location,
                    event.text.as_deref(),
                    self.mods,
                    event.repeat,
                ) {
                    match input::local_action(&key) {
                        Some(LocalAction::Paste) => {
                            if let Some(text) =
                                self.surface.as_ref().and_then(|s| s.clipboard.load())
                            {
                                self.send(ClientMsg::Input(InputEvent::Paste(text)));
                            }
                        }
                        Some(LocalAction::ScrollPageUp) => {
                            self.send(ClientMsg::Command(Command::ScrollPages(1)))
                        }
                        Some(LocalAction::ScrollPageDown) => {
                            self.send(ClientMsg::Command(Command::ScrollPages(-1)))
                        }
                        None => self.send(ClientMsg::Input(InputEvent::Key(key))),
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = position;
                self.mouse(self.button, MouseAction::Motion);
            }
            WindowEvent::MouseInput { state, button, .. } => {
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

pub fn run(config: Config, link: ClientLink, probe: Option<Instant>) -> Result<(), GuiError> {
    link.tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 80, rows: 24 },
            palette: config.theme(),
            env: vec![],
            target: Target::Main,
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
