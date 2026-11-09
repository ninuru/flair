use crate::scenario::{Scenario, Step, client_size, key_input};
use flair_core::{
    command::Command,
    config::Config,
    geom::GridSize,
    ids::PaneId,
    key::{InputEvent, KeyInput},
    link::{ClientLink, channel_pair},
    proto::{ClientKind, ClientMsg, PROTOCOL, ServerMsg, Target, WindowLayout},
    status::StatusLine,
    theme::{Rgb, Theme},
};
use flair_gui::{
    Fonts,
    cellgrid::CellGrid,
    chrome_layout::{self, BarField, ChromeInput, ChromeLayout},
    compose::{compose, compose_frame, separator_glyphs},
    mirror::Mirror,
    notice::Notice,
    overlays::{OverlayEvent, Overlays},
    renderer::{Chrome, HeadlessContext, Renderer},
    status::{status_parts, status_text},
    theme_picker::ThemeOutcome,
};
use flair_server::{ServerConfig, runtime::spawn_in_process};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{RecvTimeoutError, Sender, TryRecvError},
    time::{Duration, Instant},
};

const QUIET: Duration = Duration::from_millis(50);

struct TempHome(PathBuf);
impl TempHome {
    fn new() -> Result<Self, String> {
        let base = std::env::temp_dir();
        for index in 0..100 {
            let path = base.join(format!("flair-test-{}-{index}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
        Err("cannot allocate test HOME".into())
    }
}
impl Drop for TempHome {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            log::warn!("cannot remove test HOME: {error}");
        }
    }
}

fn lines(grid: &flair_gui::cellgrid::CellGrid) -> String {
    grid.cells
        .chunks(usize::from(grid.size.cols).max(1))
        .map(|row| {
            row.iter()
                .filter(|cell| {
                    !cell
                        .flags
                        .contains(flair_core::proto::CellFlags::WIDE_SPACER)
                })
                .map(|cell| {
                    let mut text = cell.c.to_string();
                    if let Some(extra) = &cell.extra {
                        text.push_str(extra);
                    }
                    text
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct Pixels {
    px: (u32, u32),
    scale: f64,
    fonts: Fonts,
    gpu: Option<(Renderer, HeadlessContext, (u32, u32))>,
}
impl Pixels {
    fn grid(&self, config: &Config) -> Result<GridSize, String> {
        let size = chrome_layout::grid_size(
            self.px,
            padding(config, self.scale),
            self.scale as f32,
            self.fonts.metrics(),
        );
        client_size(size.cols, size.rows)
    }
    fn render(
        &mut self,
        grid: &CellGrid,
        chrome: &Chrome,
        background: Rgb,
    ) -> Result<Vec<u8>, String> {
        if self.gpu.as_ref().is_some_and(|(_, _, px)| *px != self.px) {
            self.gpu = None;
        }
        let (renderer, _, _) = match &mut self.gpu {
            Some(gpu) => gpu,
            None => {
                let (context, gl) = HeadlessContext::new(self.px)
                    .map_err(|e| format!("capture needs an EGL device: {e}"))?;
                let mut renderer =
                    Renderer::new(gl, self.fonts.metrics()).map_err(|e| e.to_string())?;
                renderer.resize(self.px);
                self.gpu.insert((renderer, context, self.px))
            }
        };
        renderer
            .draw_frame(grid, chrome, &mut self.fonts, background)
            .map_err(|e| e.to_string())?;
        Ok(ppm(self.px, &renderer.read_pixels()))
    }
}

fn padding(config: &Config, scale: f64) -> (f32, f32) {
    (
        (f32::from(config.window.padding_x) * scale as f32).round(),
        (f32::from(config.window.padding_y) * scale as f32).round(),
    )
}

fn ppm(px: (u32, u32), rgba: &[u8]) -> Vec<u8> {
    let (width, height) = px;
    let mut out = format!("P6\n{width} {height}\n255\n").into_bytes();
    for row in rgba.chunks(width as usize * 4).rev() {
        for pixel in row.chunks(4) {
            out.extend_from_slice(&pixel[..3]);
        }
    }
    out
}

struct Capture {
    mirror: Mirror,
    pane: Option<PaneId>,
    layout: Option<WindowLayout>,
    status: StatusLine,
    size: GridSize,
    ended: bool,
    theme: Theme,
    preview: Option<Theme>,
    config: Config,
    config_path: PathBuf,
    overlays: Overlays,
    pixels: Option<Pixels>,
}
impl Capture {
    fn message(&mut self, message: ServerMsg, tx: &Sender<ClientMsg>) -> Result<(), String> {
        if self
            .overlays
            .open(&message, &self.status, &self.config, &self.config_path)
        {
            return Ok(());
        }
        match message {
            ServerMsg::Welcome { .. }
            | ServerMsg::Title(_)
            | ServerMsg::Bell
            | ServerMsg::SetClipboard { .. }
            | ServerMsg::Sessionizer { .. }
            | ServerMsg::ChooseSession { .. }
            | ServerMsg::Sessions(_)
            | ServerMsg::ChooseWindow
            | ServerMsg::ChooseTheme
            | ServerMsg::PasteClipboard
            | ServerMsg::PromptRename { .. }
            | ServerMsg::PromptSearch { .. }
            | ServerMsg::ReloadConfig { .. }
            | ServerMsg::Done => {}
            ServerMsg::Layout(layout) => {
                self.mirror.retain_layout(&layout);
                self.pane = Some(layout.active);
                self.layout = Some(layout);
            }
            ServerMsg::Status(status) => {
                self.overlays.status(&self.status, &status);
                self.status = status;
            }
            ServerMsg::Frame { seq, panes } => {
                if self.layout.is_none() {
                    self.pane = panes.first().map(|pane| pane.pane).or(self.pane);
                }
                self.mirror.apply(&panes);
                tx.send(ClientMsg::FrameAck { seq })
                    .map_err(|e| e.to_string())?;
            }
            ServerMsg::Exit(_) => self.ended = true,
            ServerMsg::Error(error) => return Err(error),
        }
        Ok(())
    }
    fn screen(&self) -> String {
        let Some(pane) = self.pane else {
            return String::new();
        };
        lines(&compose(&self.mirror, pane, &self.theme, true))
    }

    fn frame(&self) -> String {
        match &self.layout {
            Some(layout) => {
                let mut grid = compose_frame(&self.mirror, layout, self.size, &self.theme, true);
                separator_glyphs(&mut grid, layout);
                let status = status_text(&status_parts(&self.status), self.size.cols);
                format!("{status}\n{}", lines(&grid))
            }
            None => self.screen(),
        }
    }

    fn key(&mut self, key: KeyInput) -> Option<ClientMsg> {
        self.place();
        let event = self.overlays.key(&key, || None);
        self.route(event, ClientMsg::Input(InputEvent::Key(key)))
    }

    fn text(&mut self, text: &str) -> Option<ClientMsg> {
        let event = self.overlays.text(text);
        self.route(event, ClientMsg::Input(InputEvent::Text(text.to_owned())))
    }

    fn paste(&mut self, text: &str) -> Option<ClientMsg> {
        let event = self.overlays.text(text);
        self.route(event, ClientMsg::Input(InputEvent::Paste(text.to_owned())))
    }

    fn route(&mut self, event: Option<OverlayEvent>, fallback: ClientMsg) -> Option<ClientMsg> {
        match event {
            None => Some(fallback),
            Some(OverlayEvent::Ignored | OverlayEvent::Redraw) => None,
            Some(OverlayEvent::Send(message)) => Some(message),
            Some(OverlayEvent::Theme(outcome)) => {
                match outcome {
                    ThemeOutcome::Pending => {}
                    ThemeOutcome::Preview(name) => {
                        let dir = Config::themes_dir(&self.config_path);
                        self.preview = Some(self.config.with_theme(&name).load_theme(&dir).0);
                    }
                    ThemeOutcome::Cancel => self.preview = None,
                    ThemeOutcome::Save(name) => {
                        let dir = Config::themes_dir(&self.config_path);
                        self.config = self.config.with_theme(&name);
                        self.overlays
                            .set_notice(Notice::info(format!("theme saved: {name}")));
                        let (theme, error) = self.config.load_theme(&dir);
                        if let Some(error) = error {
                            self.overlays.set_notice(Notice::theme_fallback(&error));
                        }
                        self.theme = theme;
                        self.preview = None;
                        return Some(ClientMsg::Palette(self.theme.clone()));
                    }
                }
                None
            }
        }
    }

    fn window(&mut self, px: (u32, u32)) -> Result<Option<ClientMsg>, String> {
        let Some(pixels) = &mut self.pixels else {
            return Ok(None);
        };
        pixels.px = px;
        self.size = pixels.grid(&self.config)?;
        Ok(Some(ClientMsg::Resize(self.size)))
    }

    fn place(&mut self) {
        let Some(pixels) = &mut self.pixels else {
            return;
        };
        if !self.overlays.is_open() {
            return;
        }
        let theme = self.preview.as_ref().unwrap_or(&self.theme);
        let parts = status_parts(&self.status);
        let prompt = self.overlays.bar_field();
        let input = ChromeInput {
            px: pixels.px,
            scale: pixels.scale as f32,
            padding: padding(&self.config, pixels.scale),
            metrics: pixels.fonts.metrics(),
            ui: pixels.fonts.ui_metrics(),
            theme,
            status: &parts,
            prompt: prompt.as_ref().map(|(text, cursor)| BarField {
                text,
                cursor: *cursor,
            }),
            layout: self.layout.as_ref(),
            hover: None,
            overlays: Some(&self.overlays),
            notice: self.overlays.notice(),
        };
        let ChromeLayout { overlay, .. } =
            chrome_layout::layout(&input, |text, bold| pixels.fonts.ui_width(text, bold));
        self.overlays.place(overlay);
    }

    fn capture(&mut self, output: &Path, name: &str) -> Result<(), String> {
        if self.pixels.is_none() {
            return fs::write(output.join(format!("{name}.txt")), self.frame())
                .map_err(|e| e.to_string());
        }
        let theme = self.preview.as_ref().unwrap_or(&self.theme).clone();
        let mut grid = match (&self.layout, self.pane) {
            (Some(layout), _) => compose_frame(&self.mirror, layout, self.size, &theme, true),
            (None, Some(pane)) => compose(&self.mirror, pane, &theme, true),
            (None, None) => return Err("capture before the first frame".into()),
        };
        let Some(pixels) = &mut self.pixels else {
            return Err("capture without --pixels".into());
        };
        let scale = pixels.scale as f32;
        let metrics = pixels.fonts.metrics();
        if self.overlays.is_open() {
            grid.cursor = None;
        }
        let parts = status_parts(&self.status);
        let prompt = self.overlays.bar_field();
        let input = ChromeInput {
            px: pixels.px,
            scale,
            padding: padding(&self.config, pixels.scale),
            metrics,
            ui: pixels.fonts.ui_metrics(),
            theme: &theme,
            status: &parts,
            prompt: prompt.as_ref().map(|(text, cursor)| BarField {
                text,
                cursor: *cursor,
            }),
            layout: self.layout.as_ref(),
            hover: None,
            overlays: Some(&self.overlays),
            notice: self.overlays.notice(),
        };
        let ChromeLayout {
            chrome, overlay, ..
        } = chrome_layout::layout(&input, |text, bold| pixels.fonts.ui_width(text, bold));
        let image = pixels.render(&grid, &chrome, theme.background)?;
        self.overlays.place(overlay);
        fs::write(output.join(format!("{name}.ppm")), image).map_err(|e| e.to_string())
    }

    fn drain(&mut self, link: &ClientLink) -> Result<(), String> {
        loop {
            match link.rx.try_recv() {
                Ok(message) => self.message(message, &link.tx)?,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
            }
        }
    }

    fn settle(&mut self, link: &ClientLink, deadline: Instant) -> Result<(), String> {
        while !self.ended {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("scenario deadline exceeded")?;
            match link.rx.recv_timeout(remaining.min(QUIET)) {
                Ok(message) => self.message(message, &link.tx)?,
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => break,
            }
        }
        Ok(())
    }

    fn wait(
        &mut self,
        link: &ClientLink,
        deadline: Instant,
        predicate: impl Fn(&Self) -> bool,
    ) -> Result<(), String> {
        loop {
            if predicate(self) {
                return Ok(());
            }
            if self.ended {
                return Err("shell exited before assertion passed".into());
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("scenario deadline exceeded")?;
            match link.rx.recv_timeout(remaining) {
                Ok(message) => self.message(message, &link.tx)?,
                Err(RecvTimeoutError::Timeout) => return Err("scenario deadline exceeded".into()),
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("server disconnected before assertion passed".into());
                }
            }
        }
    }
}

#[derive(Serialize)]
struct ResultFile {
    success: bool,
    error: Option<String>,
    failed_step: Option<usize>,
    cols: u16,
    rows: u16,
    cursor_col: Option<u16>,
    cursor_row: Option<u16>,
    elapsed_ms: u64,
}

pub fn run(
    scenario: Scenario,
    output: &Path,
    timeout: Duration,
    config_path: Option<&Path>,
    pixels: Option<(u32, u32)>,
    scale: f64,
) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|e| format!("cannot create artifacts: {e}"))?;
    let start = Instant::now();
    let deadline = start.checked_add(timeout).ok_or("timeout is too large")?;
    let home = TempHome::new()?;
    let config_path = match config_path {
        Some(path) if !path.is_file() => {
            return Err(format!("config not found: {}", path.display()));
        }
        Some(path) => path.to_owned(),
        None => home.0.join("config").join("flair").join("flair.toml"),
    };
    let (mut config, error) = Config::load(&config_path);
    if let Some(error) = error {
        return Err(error.to_string());
    }
    let (theme, error) = config.load_theme(&Config::themes_dir(&config_path));
    let mut overlays = Overlays::new(std::env::var_os("HOME").map(PathBuf::from));
    if let Some(error) = error {
        log::warn!("{error}; using defaults");
        overlays.set_notice(Notice::theme_fallback(&error));
    }
    let pixels = match pixels {
        Some(px) => Some(Pixels {
            px,
            scale,
            fonts: Fonts::load(&config.font, scale).map_err(|e| e.to_string())?,
            gpu: None,
        }),
        None => None,
    };
    let size = match &pixels {
        Some(pixels) => pixels.grid(&config)?,
        None => client_size(scenario.cols, scenario.rows)?,
    };
    config.shell.program = scenario.program.clone();
    config.shell.args = scenario.args.clone();
    let mut server_config = ServerConfig::from_config(&config);
    server_config.cwd = scenario.cwd.clone().unwrap_or_else(|| home.0.clone());
    server_config.env.extend([
        ("HOME".into(), home.0.to_string_lossy().into_owned()),
        (
            "XDG_CONFIG_HOME".into(),
            home.0.join("config").to_string_lossy().into_owned(),
        ),
        (
            "XDG_STATE_HOME".into(),
            home.0.join("state").to_string_lossy().into_owned(),
        ),
        ("ENV".into(), String::new()),
        ("ZDOTDIR".into(), home.0.to_string_lossy().into_owned()),
    ]);
    server_config.env.extend(scenario.env.clone());
    let mut isolated_args = vec![
        "-i".to_owned(),
        "PATH=/usr/bin:/bin".to_owned(),
        "LC_ALL=C.UTF-8".to_owned(),
    ];
    isolated_args.extend(
        server_config
            .env
            .iter()
            .map(|(key, value)| format!("{key}={value}")),
    );
    isolated_args.push(server_config.program.clone());
    isolated_args.extend(server_config.args.clone());
    server_config.program = "/usr/bin/env".into();
    server_config.args = isolated_args;
    let (link, server_link) = channel_pair();
    let mut capture = Capture {
        mirror: Mirror::default(),
        pane: None,
        layout: None,
        status: StatusLine::default(),
        size,
        ended: false,
        theme: theme.clone(),
        preview: None,
        config,
        config_path,
        overlays,
        pixels,
    };
    let mut failed_step = None;
    let mut worker = None;
    let result = (|| -> Result<(), String> {
        if !server_config.cwd.is_dir() {
            return Err(format!(
                "scenario cwd is not a directory: {}",
                server_config.cwd.display()
            ));
        }
        worker = Some(spawn_in_process(server_config, server_link).map_err(|e| e.to_string())?);
        link.tx
            .send(ClientMsg::Hello {
                protocol: PROTOCOL,
                kind: ClientKind::Headless,
                size,
                palette: theme,
                env: vec![],
                target: Target::Main,
            })
            .map_err(|e| e.to_string())?;
        for (index, step) in scenario.steps.iter().enumerate() {
            failed_step = Some(index + 1);
            if Instant::now() >= deadline {
                return Err("scenario deadline exceeded".into());
            }
            let message = match step {
                Step::Wait { text } => {
                    capture.wait(&link, deadline, |capture| capture.frame().contains(text))?;
                    None
                }
                Step::Cursor { col, row } => {
                    capture.wait(&link, deadline, |capture| {
                        capture
                            .pane
                            .and_then(|pane| capture.mirror.pane(pane))
                            .is_some_and(|pane| {
                                pane.cursor.point.col == *col && pane.cursor.point.row == *row
                            })
                    })?;
                    None
                }
                Step::Key {
                    key,
                    ctrl,
                    shift,
                    alt,
                } => {
                    capture.drain(&link)?;
                    capture.key(key_input(key, *ctrl, *shift, *alt)?)
                }
                Step::Text { text } => {
                    capture.drain(&link)?;
                    capture.text(text)
                }
                Step::Paste { text } => {
                    capture.drain(&link)?;
                    capture.paste(text)
                }
                Step::Resize { cols, rows } => {
                    capture.size = client_size(*cols, *rows)?;
                    Some(ClientMsg::Resize(capture.size))
                }
                Step::Scroll { lines } => Some(ClientMsg::Command(Command::ScrollLines(*lines))),
                Step::Sleep { ms } => {
                    std::thread::sleep(Duration::from_millis(*ms));
                    None
                }
                Step::Window { width, height } => capture.window((*width, *height))?,
                Step::Capture { name } => {
                    capture.settle(&link, deadline)?;
                    capture.capture(output, name)?;
                    None
                }
            };
            if let Some(message) = message {
                link.tx.send(message).map_err(|e| e.to_string())?;
            }
        }
        failed_step = None;
        Ok(())
    })();
    let _ = link.tx.send(ClientMsg::Detach);
    drop(link);
    let cleanup_deadline = Instant::now() + Duration::from_secs(2);
    let mut cleanup_error = None;
    if let Some(worker) = worker {
        while !worker.is_finished() && Instant::now() < cleanup_deadline {
            std::thread::park_timeout(Duration::from_millis(5));
        }
        if worker.is_finished() {
            if worker.join().is_err() {
                cleanup_error = Some("server thread panicked".to_owned());
            }
        } else {
            cleanup_error = Some("runtime cleanup exceeded 2 seconds".to_owned());
        }
    }
    let result = match (result, cleanup_error) {
        (Ok(()), None) => Ok(()),
        (Err(error), None) | (Ok(()), Some(error)) => Err(error),
        (Err(error), Some(cleanup)) => Err(format!("{error}; {cleanup}")),
    };
    let pane = capture.pane.and_then(|id| capture.mirror.pane(id));
    let size = pane.map_or(
        GridSize {
            cols: scenario.cols,
            rows: scenario.rows,
        },
        |pane| pane.size,
    );
    let report = ResultFile {
        success: result.is_ok(),
        error: result.as_ref().err().cloned(),
        failed_step,
        cols: size.cols,
        rows: size.rows,
        cursor_col: pane.map(|pane| pane.cursor.point.col),
        cursor_row: pane.map(|pane| pane.cursor.point.row),
        elapsed_ms: start.elapsed().as_millis() as u64,
    };
    fs::write(output.join("screen.txt"), capture.screen()).map_err(|e| e.to_string())?;
    fs::write(output.join("frame.txt"), capture.frame()).map_err(|e| e.to_string())?;
    fs::write(
        output.join("result.toml"),
        toml::to_string(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    result
}
