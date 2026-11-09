use crate::scenario::{Scenario, Step, client_size, key_input};
use flair_core::{
    command::Command,
    config::Config,
    geom::GridSize,
    ids::PaneId,
    key::InputEvent,
    link::{ClientLink, channel_pair},
    proto::{ClientKind, ClientMsg, PROTOCOL, ServerMsg, Target, WindowLayout},
    status::StatusLine,
};
use flair_gui::{
    compose::{compose, compose_frame, separator_glyphs},
    mirror::Mirror,
    status::{status_parts, status_text},
};
use flair_server::{ServerConfig, runtime::spawn_in_process};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{RecvTimeoutError, Sender},
    time::{Duration, Instant},
};

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

struct Capture {
    mirror: Mirror,
    pane: Option<PaneId>,
    layout: Option<WindowLayout>,
    status: StatusLine,
    size: GridSize,
    ended: bool,
    theme: flair_core::theme::Theme,
}
impl Capture {
    fn message(&mut self, message: ServerMsg, tx: &Sender<ClientMsg>) -> Result<(), String> {
        match message {
            ServerMsg::Welcome { .. }
            | ServerMsg::Title(_)
            | ServerMsg::Bell
            | ServerMsg::SetClipboard { .. }
            | ServerMsg::Sessionizer { .. }
            | ServerMsg::ChooseSession { .. }
            | ServerMsg::Sessions(_)
            | ServerMsg::ChooseWindow
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
            ServerMsg::Status(status) => self.status = status,
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

pub fn run(scenario: Scenario, output: &Path, timeout: Duration) -> Result<(), String> {
    fs::create_dir_all(output).map_err(|e| format!("cannot create artifacts: {e}"))?;
    let start = Instant::now();
    let deadline = start.checked_add(timeout).ok_or("timeout is too large")?;
    let home = TempHome::new()?;
    let mut config = Config::default();
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
        size: client_size(scenario.cols, scenario.rows)?,
        ended: false,
        theme: config.theme(),
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
                size: client_size(scenario.cols, scenario.rows)?,
                palette: config.theme(),
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
                } => Some(ClientMsg::Input(InputEvent::Key(key_input(
                    key, *ctrl, *shift, *alt,
                )?))),
                Step::Text { text } => Some(ClientMsg::Input(InputEvent::Text(text.clone()))),
                Step::Paste { text } => Some(ClientMsg::Input(InputEvent::Paste(text.clone()))),
                Step::Resize { cols, rows } => {
                    capture.size = client_size(*cols, *rows)?;
                    Some(ClientMsg::Resize(capture.size))
                }
                Step::Scroll { lines } => Some(ClientMsg::Command(Command::ScrollLines(*lines))),
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
