use flair_core::{
    config::Config,
    geom::GridSize,
    ids::PaneId,
    key::InputEvent,
    link::ClientLink,
    proto::{ClientKind, ClientMsg, Control, PROTOCOL, ServerMsg, Target},
};
use flair_gui::{compose::compose, mirror::Mirror};
use flair_server::socket;
use std::{
    fs,
    path::PathBuf,
    process::{Child, Command},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Daemon {
    root: PathBuf,
    child: Child,
}
impl Daemon {
    fn new() -> Self {
        Self::with_config(
            r#"[shell]
program = "/bin/sh"
args = ["-c", "printf 'READY:%s:%s:%s\\r\\n' $$ ${HISTFILE-absent} $FLAIR_PANE; printf 'ROUTE:%s:%s\\r\\n' ${FLAIR_SOCKET-absent} ${FLAIR_SOCKET_NAME-absent}; while IFS= read -r line; do printf 'GOT:%s\\r\\n' \"$line\"; done"]
"#,
        )
    }
    fn with_config(config_text: &str) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "flair-daemon-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let config = root.join("config.toml");
        fs::write(&config, config_text).unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_flair"))
            .arg("server")
            .arg("--config")
            .arg(&config)
            .env("FLAIR_SOCKET", root.join("test.sock"))
            .env("XDG_STATE_HOME", &root)
            .env("XDG_RUNTIME_DIR", &root)
            .env("HISTFILE", "inherited-history")
            .env("FLAIR_SOCKET_NAME", "test-internal")
            .spawn()
            .unwrap();
        let mut value = Self { root, child };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !value.path().exists() {
            assert!(
                value.child.try_wait().unwrap().is_none(),
                "daemon exited before binding"
            );
            assert!(Instant::now() < deadline, "daemon startup timeout");
            std::thread::sleep(Duration::from_millis(2));
        }
        value
    }
    fn path(&self) -> PathBuf {
        self.root.join("test.sock")
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_flair"))
            .args(args)
            .env("FLAIR_SOCKET", self.path())
            .env("XDG_STATE_HOME", &self.root)
            .env("XDG_RUNTIME_DIR", &self.root)
            .output()
            .unwrap()
    }
    fn attach(&self, name: &str, cols: u16) -> ClientLink {
        let link = socket::connect(&self.path()).unwrap();
        link.tx
            .send(ClientMsg::Hello {
                protocol: PROTOCOL,
                kind: ClientKind::Headless,
                size: GridSize { cols, rows: 8 },
                palette: Config::default().theme(),
                env: vec![],
                target: Target::Name(name.into()),
            })
            .unwrap();
        link
    }
}
impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.cli(&["kill-server"]);
        let deadline = Instant::now() + Duration::from_secs(3);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn screen(link: &ClientLink, expected: &str) -> (String, PaneId) {
    let mut mirror = Mirror::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match link
            .rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap()
        {
            ServerMsg::Frame { seq, panes } => {
                let updates: Vec<_> = panes
                    .iter()
                    .map(|update| {
                        let mut update = update.clone();
                        if mirror.pane(update.pane).is_none() {
                            update.full = true;
                        }
                        update
                    })
                    .collect();
                mirror.apply(&updates);
                let pane = panes[0].pane;
                let text = compose(&mirror, pane, &Config::default().theme(), true)
                    .to_lines()
                    .join("\n");
                link.tx.send(ClientMsg::FrameAck { seq }).unwrap();
                if text.contains(expected) {
                    return (text, pane);
                }
            }
            ServerMsg::Error(error) => panic!("{error}"),
            _ => {}
        }
    }
}

#[test]
fn detached_sessions_keep_real_shells_and_independent_clients() {
    let daemon = Daemon::new();
    for name in ["main", "H"] {
        let output = daemon.cli(&["new", "-d", "-s", name, "-c", "/tmp"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let a = daemon.attach("main", 60);
    let (first, pane) = screen(&a, "ROUTE:absent:absent");
    assert!(first.contains("inherited-history"));
    a.tx.send(ClientMsg::Input(InputEvent::Text("survives\n".into())))
        .unwrap();
    screen(&a, "GOT:survives");
    a.tx.send(ClientMsg::Detach).unwrap();
    drop(a);
    let a = daemon.attach("main", 60);
    let (reattached, same_pane) = screen(&a, "GOT:survives");
    assert_eq!(pane, same_pane);
    assert!(
        reattached.contains(
            first
                .lines()
                .find(|line| line.starts_with("READY:"))
                .unwrap()
        )
    );
    let b = daemon.attach("H", 50);
    let (private, other_pane) = screen(&b, ":absent:");
    assert_ne!(pane, other_pane);
    assert!(!private.contains("inherited-history"));
    let list = daemon.cli(&["ls"]);
    assert!(list.status.success());
    let list = String::from_utf8(list.stdout).unwrap();
    assert!(list.contains("main:") && list.contains("H:"));
    drop(a);
    drop(b);
    let a = daemon.attach("main", 60);
    screen(&a, "GOT:survives");
    let same_session = daemon.attach("main", 40);
    screen(&same_session, "GOT:survives");
    a.tx.send(ClientMsg::Input(InputEvent::Text("two-clients\n".into())))
        .unwrap();
    screen(&a, "GOT:two-clients");
    screen(&same_session, "GOT:two-clients");
}

#[test]
fn kill_server_ends_shells_and_removes_socket() {
    let mut daemon = Daemon::new();
    assert!(
        daemon
            .cli(&["new", "-d", "-s", "main", "-c", "/tmp"])
            .status
            .success()
    );
    let link = daemon.attach("main", 60);
    let (text, _) = screen(&link, "READY:");
    let pid = text
        .lines()
        .find_map(|line| line.strip_prefix("READY:"))
        .unwrap()
        .split(':')
        .next()
        .unwrap()
        .parse::<u32>()
        .unwrap();
    assert!(daemon.cli(&["kill-server"]).status.success());
    let deadline = Instant::now() + Duration::from_secs(5);
    while daemon.child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!PathBuf::from(format!("/proc/{pid}")).exists());
    assert!(!daemon.path().exists());
}

#[test]
fn switch_without_gui_returns_target_for_attach_and_keeps_original_session() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cli(&["new", "-d", "-s", "main", "-c", "/tmp"])
            .status
            .success()
    );
    let link = socket::connect(&daemon.path()).unwrap();
    link.tx
        .send(ClientMsg::Control(Control::Switch {
            target: Target::Path(daemon.root.clone()),
            pane: None,
            env: vec![],
        }))
        .unwrap();
    let message = link.rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(
        matches!(message, ServerMsg::Sessions(sessions) if sessions.len() == 1 && sessions[0].root == daemon.root)
    );
    assert_eq!(
        link.rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ServerMsg::Done
    );
    assert!(
        String::from_utf8(daemon.cli(&["ls"]).stdout)
            .unwrap()
            .contains("main:")
    );
}

#[test]
fn daemon_attach_helper() {
    let Some(socket_path) = std::env::var_os("FLAIR_CRASH_TEST_SOCKET") else {
        return;
    };
    let link = socket::connect(&PathBuf::from(socket_path)).unwrap();
    link.tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Headless,
            size: GridSize { cols: 60, rows: 8 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Name("main".into()),
        })
        .unwrap();
    screen(&link, "READY:");
    fs::write(std::env::var_os("FLAIR_CRASH_READY").unwrap(), "ready").unwrap();
    loop {
        std::thread::park();
    }
}

#[test]
fn sigkill_of_client_preserves_shell_pid_and_scrollback() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cli(&["new", "-d", "-s", "main", "-c", "/tmp"])
            .status
            .success()
    );
    let first = daemon.attach("main", 60);
    let (original, pane) = screen(&first, "READY:");
    drop(first);
    let ready = daemon.root.join("client-ready");
    let mut client = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "daemon_attach_helper", "--nocapture"])
        .env("FLAIR_CRASH_TEST_SOCKET", daemon.path())
        .env("FLAIR_CRASH_READY", &ready)
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready.exists() {
        if Instant::now() >= deadline {
            let _ = client.kill();
            let _ = client.wait();
            panic!("client startup timed out");
        }
        assert!(client.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(2));
    }
    client.kill().unwrap();
    client.wait().unwrap();
    let second = daemon.attach("main", 60);
    let (retained, retained_pane) = screen(&second, "READY:");
    assert_eq!(pane, retained_pane);
    assert_eq!(original, retained);
    second
        .tx
        .send(ClientMsg::Input(InputEvent::Text("after-crash\n".into())))
        .unwrap();
    screen(&second, "GOT:after-crash");
}

#[test]
fn concurrent_autostart_reclaims_stale_socket_once() {
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};
    let root = std::env::temp_dir().join(format!("flair-autostart-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let socket_path = root.join("test.sock");
    drop(UnixListener::bind(&socket_path).unwrap());
    let config = root.join("config.toml");
    fs::write(
        &config,
        "[shell]\nprogram = '/bin/sh'\nargs = ['-c', 'read hold']\n",
    )
    .unwrap();
    let mut clients: Vec<_> = (0..3)
        .map(|index| {
            Command::new(env!("CARGO_BIN_EXE_flair"))
                .args([
                    "new",
                    "-d",
                    "-s",
                    &format!("client{index}"),
                    "-c",
                    "/tmp",
                    "--config",
                ])
                .arg(&config)
                .env("FLAIR_SOCKET", &socket_path)
                .env("XDG_STATE_HOME", &root)
                .spawn()
                .unwrap()
        })
        .collect();
    let success = clients
        .iter_mut()
        .all(|client| client.wait().unwrap().success());
    let list = Command::new(env!("CARGO_BIN_EXE_flair"))
        .arg("ls")
        .env("FLAIR_SOCKET", &socket_path)
        .output()
        .unwrap();
    let cleanup = Command::new(env!("CARGO_BIN_EXE_flair"))
        .arg("kill-server")
        .env("FLAIR_SOCKET", &socket_path)
        .status()
        .unwrap();
    assert!(success && cleanup.success(), "autostart or cleanup failed");
    let text = String::from_utf8(list.stdout).unwrap();
    for index in 0..3 {
        assert!(text.contains(&format!("client{index}:")), "{text}");
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while socket_path.exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn protocol_mismatch_leaves_existing_sessions_alive() {
    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
    };
    let daemon = Daemon::new();
    assert!(
        daemon
            .cli(&["new", "-d", "-s", "main", "-c", "/tmp"])
            .status
            .success()
    );
    let mut stream = UnixStream::connect(daemon.path()).unwrap();
    stream.write_all(&(PROTOCOL + 1).to_le_bytes()).unwrap();
    let mut version = [0; 4];
    stream.read_exact(&mut version).unwrap();
    assert_eq!(u32::from_le_bytes(version), PROTOCOL);
    drop(stream);
    let link = daemon.attach("main", 60);
    screen(&link, "READY:");
}

#[test]
fn failed_session_spawn_does_not_disconnect_existing_clients() {
    let daemon = Daemon::new();
    assert!(
        daemon
            .cli(&["new", "-d", "-s", "main", "-c", "/tmp"])
            .status
            .success()
    );
    let main = daemon.attach("main", 60);
    screen(&main, "READY:");
    let control = socket::connect(&daemon.path()).unwrap();
    control
        .tx
        .send(ClientMsg::Control(Control::Create {
            target: Target::Create {
                name: Some("broken".into()),
                root: daemon.root.join("missing-directory"),
            },
            env: vec![],
        }))
        .unwrap();
    assert!(matches!(
        control.rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ServerMsg::Error(_)
    ));
    main.tx
        .send(ClientMsg::Input(InputEvent::Text("still-alive\n".into())))
        .unwrap();
    screen(&main, "GOT:still-alive");
}

#[test]
fn cli_socket_override_switches_existing_gui_without_foreign_pane_filter() {
    let daemon = Daemon::new();
    for name in ["main", "other"] {
        assert!(
            daemon
                .cli(&["new", "-d", "-s", name, "-c", "/tmp"])
                .status
                .success()
        );
    }
    let gui = socket::connect(&daemon.path()).unwrap();
    gui.tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 60, rows: 8 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Name("main".into()),
        })
        .unwrap();
    let (_, original_pane) = screen(&gui, "READY:");
    let run = Command::new(env!("CARGO_BIN_EXE_flair"))
        .args(["switch", "other"])
        .env("FLAIR_SOCKET", daemon.path())
        .env("FLAIR", "/tmp/foreign-daemon.sock,1234")
        .env("FLAIR_PANE", "%103")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let (_, switched_pane) = screen(&gui, "READY:");
    assert_ne!(switched_pane, original_pane);
    let run = Command::new(env!("CARGO_BIN_EXE_flair"))
        .args(["switch", "main"])
        .env("FLAIR_SOCKET", daemon.path())
        .env("FLAIR", format!("{},1234", daemon.path().display()))
        .env("FLAIR_PANE", format!("%{}", switched_pane.0))
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(screen(&gui, "READY:").1, original_pane);
}

fn drain(link: &ClientLink) {
    while let Ok(message) = link.rx.try_recv() {
        if let ServerMsg::Frame { seq, .. } = message {
            link.tx.send(ClientMsg::FrameAck { seq }).unwrap();
        }
    }
}

#[test]
fn open_drops_the_file_into_the_nvim_of_the_active_gui_pane() {
    let nvim_found = Command::new("nvim")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success());
    if !nvim_found {
        eprintln!("skipping flair open test: nvim is not on PATH");
        return;
    }
    let daemon = Daemon::with_config(
        "[shell]\nprogram = \"/bin/sh\"\nargs = [\"-c\", \"printf 'READY\\\\r\\\\n'; nvim --headless --clean; printf 'DONE\\\\r\\\\n'\"]\n",
    );
    let file = daemon.root.join("open me.txt");
    fs::write(&file, "hello\n").unwrap();
    let file_arg = file.to_str().unwrap();
    let output = daemon.cli(&["open", file_arg]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "flair open: no flair window is open\n"
    );
    let link = socket::connect(&daemon.path()).unwrap();
    link.tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 60, rows: 8 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Main,
        })
        .unwrap();
    screen(&link, "READY");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        drain(&link);
        let output = daemon.cli(&["open", file_arg]);
        if output.status.success() {
            break;
        }
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "flair open: no nvim runs in the active pane\n"
        );
        assert!(Instant::now() < deadline, "nvim never became reachable");
        std::thread::sleep(Duration::from_millis(50));
    }
    let server = fs::read_dir(&daemon.root)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("nvim."))
        })
        .unwrap();
    let current = Command::new("nvim")
        .arg("--server")
        .arg(&server)
        .args(["--remote-expr", "expand('%:p')"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&current.stdout).trim_end(),
        file_arg
    );
}
