use super::{request, spawn_in_process};
use crate::{
    ServerConfig,
    event::Event,
    facts::{Contexts, Job},
    git::tests::{commit, git_available, temp},
};
use flair_core::{
    config::{Config, PromptStyle},
    geom::GridSize,
    isles::Head,
    key::{InputEvent, Key, KeyAction, KeyInput, Locks, Mods, NamedKey},
    link::channel_pair,
    proto::{ClientKind, ClientMsg, ExitReason, PROTOCOL, ServerMsg, Target},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::{self, RecvTimeoutError},
    time::{Duration, Instant},
};

fn start(script: &str) -> Option<(flair_core::link::ClientLink, std::thread::JoinHandle<()>)> {
    if !Path::new("/bin/sh").exists() {
        eprintln!("skipping real PTY test: /bin/sh unavailable");
        return None;
    }
    let mut prompt_free = Config::default();
    prompt_free.prompt.style = PromptStyle::Ps1;
    let mut config = ServerConfig::from_config(&prompt_free);
    config.program = "/bin/sh".into();
    config.args = vec!["-c".into(), script.into()];
    let (client, server) = channel_pair();
    let handle = spawn_in_process(config, server).unwrap();
    client
        .tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 40, rows: 6 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Main,
        })
        .unwrap();
    Some((client, handle))
}

fn wait_text(client: &flair_core::link::ClientLink, text: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let message = client
            .rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        match message {
            ServerMsg::Frame { seq, panes } => {
                let screen: String = panes
                    .iter()
                    .flat_map(|pane| pane.rows.iter())
                    .flat_map(|(_, row)| row.iter().map(|cell| cell.c))
                    .collect();
                client.tx.send(ClientMsg::FrameAck { seq }).unwrap();
                if screen.contains(text) {
                    return;
                }
            }
            ServerMsg::Error(error) => panic!("server error: {error}"),
            ServerMsg::Exit(reason) => panic!("shell exited before expected text: {reason:?}"),
            ServerMsg::Welcome { .. }
            | ServerMsg::Title(_)
            | ServerMsg::Bell
            | ServerMsg::SetClipboard { .. }
            | ServerMsg::Sessionizer { .. }
            | ServerMsg::ChooseSession { .. }
            | ServerMsg::Preview(_)
            | ServerMsg::PaneProcesses(_)
            | ServerMsg::Sessions(_)
            | ServerMsg::Layout(_)
            | ServerMsg::Status(_)
            | ServerMsg::ChooseWindow
            | ServerMsg::ChooseTheme
            | ServerMsg::Settings
            | ServerMsg::ToggleZen
            | ServerMsg::PasteClipboard
            | ServerMsg::PromptRename { .. }
            | ServerMsg::PromptSearch { .. }
            | ServerMsg::PromptInfo { .. }
            | ServerMsg::PromptHistory { .. }
            | ServerMsg::ReloadConfig { .. }
            | ServerMsg::Done => {}
        }
    }
}

#[test]
fn real_pty_round_trip_delivers_final_output_before_exit() {
    let Some((client, handle)) = start(r#"printf ready; read x; printf 'answer:%s' "$x""#) else {
        return;
    };
    wait_text(&client, "ready");
    for key in [Key::Char('o'), Key::Char('k'), Key::Named(NamedKey::Enter)] {
        client
            .tx
            .send(ClientMsg::Input(InputEvent::Key(KeyInput {
                key,
                mods: Mods::default(),
                text: None,
                repeat: false,
                keypad: false,
                action: KeyAction::Press,
                unshifted: None,
                shifted: None,
                base: None,
                locks: Locks::default(),
            })))
            .unwrap();
    }
    wait_text(&client, "answer:ok");
    assert!(matches!(
        client.rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ServerMsg::Exit(ExitReason::SessionEnded)
    ));
    handle.join().unwrap();
}

#[test]
fn owned_runtime_stops_after_detached_link_is_dropped() {
    let Some((client, handle)) = start("sleep 100 & printf ready; wait") else {
        return;
    };
    wait_text(&client, "ready");
    client.tx.send(ClientMsg::Detach).unwrap();
    assert!(matches!(
        client.rx.recv_timeout(Duration::from_secs(5)).unwrap(),
        ServerMsg::Exit(ExitReason::Detached)
    ));
    drop(client);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !handle.is_finished() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(handle.is_finished(), "runtime must finish bounded shutdown");
    handle.join().unwrap();
}

#[test]
fn dropping_the_client_shuts_down_the_owned_runtime() {
    let Some((client, handle)) = start("printf ready; read x") else {
        return;
    };
    wait_text(&client, "ready");
    drop(client);
    let deadline = Instant::now() + Duration::from_secs(2);
    while !handle.is_finished() && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(handle.is_finished());
    handle.join().unwrap();
}

#[test]
fn spawn_failure_is_reported_to_the_client() {
    let mut config = ServerConfig::from_config(&Config::default());
    config.program = "/flair-missing-shell".into();
    let (client, server) = channel_pair();
    let handle = spawn_in_process(config, server).unwrap();
    client
        .tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 40, rows: 6 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Main,
        })
        .unwrap();
    loop {
        match client.rx.recv_timeout(Duration::from_secs(5)) {
            Ok(ServerMsg::Error(_)) => break,
            Ok(_) => {}
            Err(RecvTimeoutError::Timeout) => panic!("spawn failure was not reported"),
            Err(RecvTimeoutError::Disconnected) => panic!("server disconnected without an error"),
        }
    }
    handle.join().unwrap();
}

#[path = "runtime_split_tests.rs"]
mod splits;

#[test]
fn reload_reads_the_config_file_and_reaches_the_client() {
    if !Path::new("/bin/sh").exists() {
        eprintln!("skipping real PTY test: /bin/sh unavailable");
        return;
    }
    let dir = std::env::temp_dir().join(format!("flair-reload-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let toml = "[keys]\nkey = \"C-b\"\n";
    std::fs::write(dir.join("flair.toml"), toml).unwrap();
    let mut config = ServerConfig::from_config(&Config::default());
    config.program = "/bin/sh".into();
    config.args = vec!["-c".into(), "printf ready; read x".into()];
    config.path = dir.join("flair.toml");
    let (client, server) = channel_pair();
    let handle = spawn_in_process(config, server).unwrap();
    client
        .tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 40, rows: 6 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Main,
        })
        .unwrap();
    wait_text(&client, "ready");
    client
        .tx
        .send(ClientMsg::Command(
            flair_core::command::Command::ReloadConfig,
        ))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let reloaded = loop {
        let message = client
            .rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|error| panic!("no reload before deadline: {error}"));
        if let ServerMsg::ReloadConfig { toml } = message {
            break toml;
        }
        if let ServerMsg::Frame { seq, .. } = message {
            client.tx.send(ClientMsg::FrameAck { seq }).unwrap();
        }
    };
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(reloaded, toml);
    drop(client);
    handle.join().unwrap();
}

#[test]
fn a_prompt_in_a_repository_reports_its_branch() {
    if !git_available() || !Path::new("/bin/sh").exists() {
        eprintln!("skipping prompt facts test: git or /bin/sh unavailable");
        return;
    }
    let dir = temp("runtime");
    let repo = dir.join("repo");
    commit(&repo, "a.txt", "one\n");
    let mut chips = Config::default();
    chips.prompt.style = PromptStyle::Chips;
    let mut config = ServerConfig::from_config(&chips);
    config.program = "/bin/sh".into();
    config.cwd = dir.clone();
    config.args = vec![
        "-c".into(),
        "cd \"$1\" && printf '\\033]7;file://%s\\007' \"$1\"; exec sleep 10".into(),
        "sh".into(),
        repo.display().to_string(),
    ];
    let (client, server) = channel_pair();
    let handle = spawn_in_process(config, server).unwrap();
    client
        .tx
        .send(ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: GridSize { cols: 40, rows: 6 },
            palette: Config::default().theme(),
            env: vec![],
            target: Target::Main,
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let info = loop {
        let message = client
            .rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("no prompt facts for the repository");
        match message {
            ServerMsg::Frame { seq, .. } => client.tx.send(ClientMsg::FrameAck { seq }).unwrap(),
            ServerMsg::PromptInfo { info, .. } => {
                if info.git.is_some() {
                    break info;
                }
            }
            ServerMsg::Error(error) => panic!("server error: {error}"),
            ServerMsg::Exit(reason) => panic!("shell exited early: {reason:?}"),
            ServerMsg::Welcome { .. }
            | ServerMsg::Title(_)
            | ServerMsg::Bell
            | ServerMsg::SetClipboard { .. }
            | ServerMsg::Sessionizer { .. }
            | ServerMsg::ChooseSession { .. }
            | ServerMsg::Preview(_)
            | ServerMsg::PaneProcesses(_)
            | ServerMsg::Sessions(_)
            | ServerMsg::Layout(_)
            | ServerMsg::Status(_)
            | ServerMsg::ChooseWindow
            | ServerMsg::ChooseTheme
            | ServerMsg::Settings
            | ServerMsg::ToggleZen
            | ServerMsg::PasteClipboard
            | ServerMsg::PromptRename { .. }
            | ServerMsg::PromptSearch { .. }
            | ServerMsg::ReloadConfig { .. }
            | ServerMsg::PromptHistory { .. }
            | ServerMsg::Done => {}
        }
    };
    assert_eq!(info.cwd, Some(repo));
    assert_eq!(info.git.unwrap().head, Head::Branch("main".into()));
    drop(client);
    handle.join().unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_gone_facts_worker_answers_requests_with_nothing() {
    let (jobs, queue) = mpsc::channel();
    let cwd = PathBuf::from("/tmp/flair-gone-worker");
    let git = || Job::Git {
        cwd: cwd.clone(),
        changes: true,
        shells: vec![1],
    };
    assert!(request(&jobs, git()).is_none());
    assert!(request(&jobs, Job::Contexts).is_none());
    drop(queue);
    let answer = request(&jobs, git());
    assert!(
        matches!(&answer, Some(Event::GitRead(path, None)) if *path == cwd),
        "{answer:?}"
    );
    let answer = request(&jobs, Job::Contexts);
    assert!(
        matches!(&answer, Some(Event::ContextsRead(contexts)) if *contexts == Contexts::default()),
        "{answer:?}"
    );
}
