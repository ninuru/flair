use flair_core::{
    config::Config,
    ids::PaneId,
    link::ClientLink,
    proto::{ClientMsg, Control, ServerMsg, SessionInfo},
};
use flair_server::socket;
use std::{
    env, fs, io,
    os::unix::{net::UnixStream, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn connect(config: Option<&Path>, start: bool) -> io::Result<ClientLink> {
    let path = socket::socket_path()?;
    match socket::connect(&path) {
        Ok(link) => return Ok(link),
        Err(error)
            if start
                && matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) => {}
        Err(error) => return Err(error),
    }
    let _lock = socket::lock(&path)?;
    match socket::connect(&path) {
        Ok(link) => return Ok(link),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) => {}
        Err(error) => return Err(error),
    }
    let mut child = Command::new(env::current_exe()?);
    child
        .arg("server")
        .env("FLAIR_SOCKET", &path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(config) = config {
        child.arg("--config").arg(config);
    }
    unsafe {
        child.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = child.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match socket::connect(&path) {
            Ok(link) => return Ok(link),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                ) => {}
            Err(error) => return Err(error),
        }
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!("daemon exited: {status}")));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "daemon did not start",
            ));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub fn control(link: &ClientLink, command: Control) -> io::Result<Vec<SessionInfo>> {
    link.tx
        .send(ClientMsg::Control(command))
        .map_err(io::Error::other)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut sessions = Vec::new();
    loop {
        match link
            .rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(io::Error::other)?
        {
            ServerMsg::Sessions(value) => sessions = value,
            ServerMsg::Done => return Ok(sessions),
            ServerMsg::Error(error) => return Err(io::Error::other(error)),
            ServerMsg::Exit(_) => return Ok(sessions),
            other => {
                return Err(io::Error::other(format!(
                    "unexpected control reply: {other:?}"
                )));
            }
        }
    }
}

pub fn kill() -> io::Result<()> {
    let path = socket::socket_path()?;
    let stream = UnixStream::connect(path)?;
    let pid = socket::peer_pid(&stream)?;
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn pane_for_socket(socket: &Path) -> Option<PaneId> {
    matching_pane(
        socket,
        env::var("FLAIR").ok().as_deref(),
        env::var("FLAIR_PANE").ok().as_deref(),
    )
}

fn matching_pane(socket: &Path, inherited: Option<&str>, pane: Option<&str>) -> Option<PaneId> {
    let (owner, _) = inherited?.rsplit_once(',')?;
    if Path::new(owner) != socket {
        return None;
    }
    let pane = pane?;
    pane.strip_prefix('%')
        .unwrap_or(pane)
        .parse()
        .ok()
        .map(PaneId)
}

pub fn environment() -> Vec<(String, String)> {
    env::vars()
        .filter(|(key, _)| {
            matches!(
                key.as_str(),
                "WAYLAND_DISPLAY" | "DISPLAY" | "SSH_AUTH_SOCK" | "HYPRLAND_INSTANCE_SIGNATURE"
            ) || key.starts_with("XDG_")
        })
        .collect()
}

pub fn server_config(config: &Config) -> io::Result<flair_server::ServerConfig> {
    let mut value = flair_server::ServerConfig::from_config(config);
    let socket = socket::socket_path()?;
    value.env.push((
        "FLAIR".into(),
        format!("{},{}", socket.display(), std::process::id()),
    ));
    Ok(value)
}

pub fn log_file() -> io::Result<fs::File> {
    let root = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|path| PathBuf::from(path).join(".local/state")))
        .ok_or_else(|| io::Error::other("HOME or XDG_STATE_HOME required for server log"))?
        .join("flair");
    fs::create_dir_all(&root)?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("server.log"))
}

#[cfg(test)]
#[path = "daemon_tests.rs"]
mod tests;
