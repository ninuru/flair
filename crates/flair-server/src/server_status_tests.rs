use super::*;
use flair_core::{
    command::Command,
    geom::{CellPoint, GridSize},
    key::{Key, KeyAction, Locks, Mods, NamedKey},
    proto::ClientKind,
};

fn hello() -> ClientMsg {
    ClientMsg::Hello {
        protocol: PROTOCOL,
        kind: ClientKind::Gui,
        size: GridSize { cols: 20, rows: 5 },
        palette: Config::default().theme(),
        env: Vec::new(),
        target: Target::Main,
    }
}

fn setup() -> Server {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    let mut out = Vec::new();
    server.handle(Millis(0), Event::Client(ClientId(1), hello()), &mut out);
    server
}

fn press(key: Key, ctrl: bool) -> KeyInput {
    KeyInput {
        key,
        mods: Mods {
            ctrl,
            ..Mods::default()
        },
        text: None,
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

fn send(server: &mut Server, now: u64, message: ClientMsg) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(Millis(now), Event::Client(ClientId(1), message), &mut out);
    out
}

fn key(server: &mut Server, now: u64, input: KeyInput) -> Vec<Effect> {
    send(server, now, ClientMsg::Input(InputEvent::Key(input)))
}

fn status(out: &[Effect]) -> Option<&StatusLine> {
    out.iter().find_map(|effect| {
        if let Effect::Send(ClientId(1), ServerMsg::Status(line)) = effect {
            Some(line)
        } else {
            None
        }
    })
}

fn writes(out: &[Effect]) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|effect| {
            if let Effect::WritePty(_, bytes) = effect {
                Some(bytes.as_slice())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn attach_sends_status_with_session_and_window() {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    let mut out = Vec::new();
    server.handle(Millis(0), Event::Client(ClientId(1), hello()), &mut out);
    let line = status(&out).unwrap();
    assert_eq!(line.session, "main");
    assert_eq!(line.windows.len(), 1);
    assert_eq!(line.windows[0].index, 1);
    assert_eq!(line.windows[0].name, "zsh");
    assert!(line.windows[0].active);
    assert_eq!(line.pending, None);
}

#[test]
fn status_follows_windows_titles_renames_and_last_window() {
    let mut server = setup();
    let out = send(&mut server, 1, ClientMsg::Command(Command::NewWindow));
    let line = status(&out).unwrap();
    assert_eq!(line.windows.len(), 2);
    assert!(!line.windows[0].active && line.windows[0].last);
    assert!(line.windows[1].active && !line.windows[1].last);
    let first_pane = server.tree.sessions[0].windows[0].active_pane;
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::PtyOutput(first_pane, b"\x1b]2;nvim\x07".to_vec()),
        &mut out,
    );
    assert_eq!(status(&out).unwrap().windows[0].name, "nvim");
    let out = send(
        &mut server,
        3,
        ClientMsg::Command(Command::RenameWindow(Some("edit".into()))),
    );
    assert_eq!(status(&out).unwrap().windows[1].name, "edit");
    let mut out = Vec::new();
    server.handle(
        Millis(4),
        Event::PtyOutput(first_pane, b"x".to_vec()),
        &mut out,
    );
    assert_eq!(status(&out), None);
}

#[test]
fn prefix_shows_pending_and_runs_bound_command() {
    let mut server = setup();
    let out = key(&mut server, 1, press(Key::Char('a'), true));
    assert_eq!(status(&out).unwrap().pending.as_deref(), Some("^A"));
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Title(title)) if title.ends_with(" ^A"))));
    assert!(writes(&out).is_empty());
    let out = key(&mut server, 2, press(Key::Char('c'), false));
    assert!(writes(&out).is_empty());
    assert_eq!(server.tree.sessions[0].windows.len(), 2);
    let line = status(&out).unwrap();
    assert_eq!(line.pending, None);
    assert!(line.windows[1].active);
}

#[test]
fn double_prefix_sends_literal_and_unbound_prefix_key_is_swallowed() {
    let mut server = setup();
    key(&mut server, 1, press(Key::Char('a'), true));
    let out = key(&mut server, 2, press(Key::Char('a'), true));
    assert_eq!(writes(&out), vec![&b"\x01"[..]]);
    key(&mut server, 3, press(Key::Char('a'), true));
    let out = key(&mut server, 4, press(Key::Char('y'), false));
    assert!(writes(&out).is_empty());
    assert!(!server.clients[&ClientId(1)].prefix.pending());
}

#[test]
fn text_focus_loss_and_ime_cancel_a_pending_prefix() {
    let mut server = setup();
    key(&mut server, 1, press(Key::Char('a'), true));
    let out = send(
        &mut server,
        2,
        ClientMsg::Input(InputEvent::Text("é".into())),
    );
    assert_eq!(writes(&out), vec!["é".as_bytes()]);
    assert_eq!(status(&out).unwrap().pending, None);
    key(&mut server, 3, press(Key::Char('a'), true));
    let out = send(&mut server, 4, ClientMsg::Focus(false));
    assert_eq!(status(&out).unwrap().pending, None);
    key(&mut server, 5, press(Key::Char('a'), true));
    send(&mut server, 6, ClientMsg::Input(InputEvent::Ime(true)));
    assert!(!server.clients[&ClientId(1)].prefix.pending());
    let out = key(&mut server, 7, press(Key::Char('a'), true));
    assert!(!server.clients[&ClientId(1)].prefix.pending());
    assert!(writes(&out).is_empty());
}

#[test]
fn stale_prefix_timer_is_ignored_and_current_one_expires() {
    let mut server = setup();
    let first = key(&mut server, 1, press(Key::Char('a'), true));
    let stale = first
        .iter()
        .find_map(|effect| {
            if let Effect::SetTimer(token, _) = effect {
                Some(*token)
            } else {
                None
            }
        })
        .unwrap();
    key(&mut server, 2, press(Key::Char('y'), false));
    let second = key(&mut server, 3, press(Key::Char('a'), true));
    let (token, until) = second
        .iter()
        .find_map(|effect| {
            if let Effect::SetTimer(token, until) = effect {
                Some((*token, *until))
            } else {
                None
            }
        })
        .unwrap();
    assert_ne!(stale, token);
    let mut out = Vec::new();
    server.handle(until, Event::Timer(stale), &mut out);
    assert!(server.clients[&ClientId(1)].prefix.pending());
    server.handle(until, Event::Timer(token), &mut out);
    assert!(!server.clients[&ClientId(1)].prefix.pending());
    assert_eq!(status(&out).unwrap().pending, None);
}

#[test]
fn modifier_only_keys_and_releases_keep_the_mouse_selection() {
    let mut server = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::PtyOutput(PaneId(1), b"hello".to_vec()),
        &mut out,
    );
    let pane = server.panes.get_mut(&PaneId(1)).unwrap();
    pane.terminal.select_start(CellPoint { col: 0, row: 0 }, 1);
    pane.terminal.select_update(CellPoint { col: 4, row: 0 });
    let mut shift = press(Key::Named(NamedKey::ShiftLeft), false);
    let out = key(&mut server, 2, shift.clone());
    assert!(writes(&out).is_empty());
    shift.action = KeyAction::Release;
    key(&mut server, 3, shift);
    assert_eq!(
        server.panes[&PaneId(1)]
            .terminal
            .selection_text()
            .as_deref(),
        Some("hello")
    );
    key(&mut server, 4, press(Key::Char('x'), false));
    assert_eq!(server.panes[&PaneId(1)].terminal.selection_text(), None);
}

#[test]
fn root_clipboard_bindings_go_through_the_key_table() {
    let mut server = setup();
    let mut paste = press(Key::Char('V'), true);
    paste.mods.shift = true;
    let out = key(&mut server, 1, paste);
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::PasteClipboard)))
    );
    assert!(writes(&out).is_empty());
}

#[test]
fn reload_reads_the_config_file_and_reaches_every_client() {
    let mut server = setup();
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), hello()), &mut out);
    key(&mut server, 2, press(Key::Char('a'), true));
    let out = key(&mut server, 3, press(Key::Char('r'), false));
    assert!(
        out.iter().any(
            |effect| matches!(effect, Effect::ReadConfig(path) if *path == server.config.path)
        )
    );
    let toml = "[keys]\nkey = \"C-b\"\n[colors]\nbackground = \"#101010\"\n".to_owned();
    let mut out = Vec::new();
    server.handle(Millis(4), Event::ConfigRead(Ok(toml.clone())), &mut out);
    for id in [ClientId(1), ClientId(2)] {
        assert!(out.iter().any(|effect| matches!(effect, Effect::Send(client, ServerMsg::ReloadConfig { toml: sent }) if *client == id && *sent == toml)));
        assert_eq!(server.clients[&id].palette, Config::default().theme());
    }
    let out = key(&mut server, 5, press(Key::Char('b'), true));
    assert_eq!(status(&out).unwrap().pending.as_deref(), Some("^B"));
}

#[test]
fn choose_theme_asks_only_that_client_to_open_the_picker() {
    let mut server = setup();
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(2), hello()), &mut out);
    key(&mut server, 2, press(Key::Char('a'), true));
    let out = key(&mut server, 3, press(Key::Char('t'), false));
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::ChooseTheme)))
    );
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::Send(ClientId(2), ServerMsg::ChooseTheme)))
    );
    assert!(writes(&out).is_empty());
}

#[test]
fn palette_answers_background_queries_in_every_pane() {
    let mut server = setup();
    send(&mut server, 1, ClientMsg::Command(Command::NewWindow));
    let mut palette = Config::default().theme();
    palette.background = Config::from_toml("[colors]\nbackground = \"#101010\"\n")
        .unwrap()
        .theme()
        .background;
    send(&mut server, 2, ClientMsg::Palette(palette.clone()));
    assert_eq!(server.clients[&ClientId(1)].palette, palette);
    let panes: Vec<PaneId> = server.panes.keys().copied().collect();
    assert_eq!(panes.len(), 2);
    for pane in panes {
        let mut out = Vec::new();
        server.handle(
            Millis(3),
            Event::PtyOutput(pane, b"\x1b]11;?\x07".to_vec()),
            &mut out,
        );
        assert!(out.iter().any(|effect| matches!(effect, Effect::WritePty(target, bytes) if *target == pane && String::from_utf8_lossy(bytes).contains("1010/1010/1010"))));
    }
}

#[test]
fn broken_reload_keeps_the_running_config() {
    let mut server = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::ConfigRead(Ok("[keys]\nkey = 3\n".into())),
        &mut out,
    );
    server.handle(
        Millis(2),
        Event::ConfigRead(Err("cannot read".into())),
        &mut out,
    );
    assert!(!out.iter().any(|effect| matches!(
        effect,
        Effect::Send(_, ServerMsg::ReloadConfig { .. } | ServerMsg::Error(_))
    )));
    assert_eq!(server.config.keys.key, "C-a");
}

#[test]
fn reload_keeps_the_socket_variable_and_working_directory() {
    let mut config = ServerConfig::from_config(&Config::default());
    config
        .env
        .push(("FLAIR".into(), "/run/flair.sock,1".into()));
    config.cwd = "/srv".into();
    let next = config.reloaded(&Config::default());
    assert!(
        next.env
            .iter()
            .any(|(key, value)| key == "FLAIR" && value == "/run/flair.sock,1")
    );
    assert_eq!(next.cwd, std::path::Path::new("/srv"));
}

#[test]
fn paste_cancels_a_pending_prefix() {
    let mut server = setup();
    key(&mut server, 1, press(Key::Char('a'), true));
    let out = send(
        &mut server,
        2,
        ClientMsg::Input(InputEvent::Paste("x".into())),
    );
    assert!(!server.clients[&ClientId(1)].prefix.pending());
    assert_eq!(status(&out).unwrap().pending, None);
    let out = key(&mut server, 3, press(Key::Char('c'), false));
    assert_eq!(server.tree.sessions[0].windows.len(), 1);
    assert!(!writes(&out).is_empty());
}

fn set_title(server: &mut Server, now: u64, title: &str) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(now),
        Event::PtyOutput(PaneId(1), format!("\x1b]0;{title}\x07").into_bytes()),
        &mut out,
    );
    out
}

#[test]
fn prompt_title_names_the_window_after_its_directory() {
    let mut server = setup();
    let out = set_title(&mut server, 1, "user@host:~/IdeaProjects/flair");
    assert_eq!(status(&out).unwrap().windows[0].name, "flair");
    let out = set_title(&mut server, 2, "user@host: ~");
    assert_eq!(status(&out).unwrap().windows[0].name, "~");
    let out = set_title(&mut server, 3, "user@host:/");
    assert_eq!(status(&out).unwrap().windows[0].name, "/");
    let out = set_title(&mut server, 4, "/srv/www/");
    assert_eq!(status(&out).unwrap().windows[0].name, "www");
}

#[test]
fn command_title_names_the_window_after_the_program() {
    let mut server = setup();
    let out = set_title(&mut server, 1, "/usr/bin/htop -d 5");
    assert_eq!(status(&out).unwrap().windows[0].name, "htop");
    let out = set_title(&mut server, 2, "cargo build");
    assert_eq!(status(&out).unwrap().windows[0].name, "cargo");
}

#[test]
fn rename_wins_over_a_verbose_title() {
    let mut server = setup();
    set_title(&mut server, 1, "user@host:~/IdeaProjects/flair");
    let out = send(
        &mut server,
        2,
        ClientMsg::Command(Command::RenameWindow(Some("edit".into()))),
    );
    assert_eq!(status(&out).unwrap().windows[0].name, "edit");
    let out = set_title(&mut server, 3, "user@host:~/other");
    assert_eq!(status(&out), None);
}

#[test]
fn blank_title_falls_back_to_the_program_name() {
    let mut server = setup();
    set_title(&mut server, 1, "nvim");
    let out = set_title(&mut server, 2, "   ");
    assert_eq!(status(&out).unwrap().windows[0].name, "zsh");
}

#[test]
fn os_title_keeps_the_full_pane_title() {
    let mut server = setup();
    let out = set_title(&mut server, 1, "user@host:~/IdeaProjects/flair");
    assert!(out.iter().any(|effect| matches!(effect, Effect::Send(ClientId(1), ServerMsg::Title(title)) if title == "user@host:~/IdeaProjects/flair")));
    assert_eq!(status(&out).unwrap().windows[0].name, "flair");
}

fn status_for(out: &[Effect], id: ClientId) -> Option<&StatusLine> {
    out.iter().find_map(|effect| match effect {
        Effect::Send(client, ServerMsg::Status(line)) if *client == id => Some(line),
        _ => None,
    })
}

fn create_work(server: &mut Server) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::Client(
            ClientId(9),
            ClientMsg::Control(Control::Create {
                target: Target::Create {
                    name: Some("work".into()),
                    root: "/".into(),
                },
                env: Vec::new(),
            }),
        ),
        &mut out,
    );
    out
}

fn output(server: &mut Server, now: u64, pane: PaneId, bytes: &[u8]) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(now),
        Event::PtyOutput(pane, bytes.to_vec()),
        &mut out,
    );
    out
}

#[test]
fn other_sessions_count_as_hidden() {
    let mut server = setup();
    let out = create_work(&mut server);
    let line = status(&out).unwrap();
    assert_eq!(line.session, "main");
    assert_eq!(line.hidden, 1);
    assert!(!line.running && !line.finished);
}

#[test]
fn a_command_in_another_session_runs_then_finishes_until_shown() {
    let mut server = setup();
    create_work(&mut server);
    let out = output(&mut server, 2, PaneId(2), b"\x1b]133;C\x07");
    let line = status(&out).unwrap();
    assert!(line.running && !line.finished);
    let out = output(&mut server, 3, PaneId(2), b"\x1b]133;D;0\x07");
    let line = status(&out).unwrap();
    assert!(!line.running && line.finished);
    let out = send(
        &mut server,
        4,
        ClientMsg::Command(Command::Switch(Target::Name("work".into()))),
    );
    let line = status(&out).unwrap();
    assert_eq!(line.session, "work");
    assert!(!line.running && !line.finished);
    let out = send(
        &mut server,
        5,
        ClientMsg::Command(Command::Switch(Target::Name("main".into()))),
    );
    let line = status(&out).unwrap();
    assert_eq!(line.session, "main");
    assert!(!line.running && !line.finished);
}

#[test]
fn prompt_start_ends_a_command_and_prompt_end_keeps_it_running() {
    let mut server = setup();
    create_work(&mut server);
    output(&mut server, 2, PaneId(2), b"\x1b]133;C\x07");
    let out = output(&mut server, 3, PaneId(2), b"\x1b]133;B\x07");
    assert_eq!(status(&out), None);
    let out = output(&mut server, 4, PaneId(2), b"\x1b]133;A\x07");
    let line = status(&out).unwrap();
    assert!(!line.running && line.finished);
}

#[test]
fn a_client_showing_the_session_never_gets_finished() {
    let mut server = setup();
    create_work(&mut server);
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::Client(
            ClientId(2),
            ClientMsg::Hello {
                protocol: PROTOCOL,
                kind: ClientKind::Gui,
                size: GridSize { cols: 20, rows: 5 },
                palette: Config::default().theme(),
                env: Vec::new(),
                target: Target::Name("work".into()),
            },
        ),
        &mut out,
    );
    assert_eq!(status_for(&out, ClientId(2)).unwrap().session, "work");
    let mut lines = Vec::new();
    for (now, bytes) in [(3, &b"\x1b]133;C\x07"[..]), (4, b"\x1b]133;D;0\x07")] {
        let out = output(&mut server, now, PaneId(2), bytes);
        lines.extend(status_for(&out, ClientId(2)).cloned());
        if now == 4 {
            assert!(status_for(&out, ClientId(1)).unwrap().finished);
        }
    }
    assert!(lines.iter().all(|line| !line.finished));
    assert!(
        !server.clients[&ClientId(2)]
            .finished
            .contains(&SessionId(2))
    );
}

#[test]
fn a_pane_that_exits_while_running_finishes_its_session() {
    let mut server = setup();
    create_work(&mut server);
    let mut out = Vec::new();
    server.handle(
        Millis(2),
        Event::Client(
            ClientId(2),
            ClientMsg::Hello {
                protocol: PROTOCOL,
                kind: ClientKind::Gui,
                size: GridSize { cols: 20, rows: 5 },
                palette: Config::default().theme(),
                env: Vec::new(),
                target: Target::Name("work".into()),
            },
        ),
        &mut out,
    );
    let mut out = Vec::new();
    server.handle(
        Millis(3),
        Event::Client(ClientId(2), ClientMsg::Command(Command::NewWindow)),
        &mut out,
    );
    output(&mut server, 4, PaneId(2), b"\x1b]133;C\x07");
    let mut out = Vec::new();
    server.handle(Millis(5), Event::PtyExited(PaneId(2)), &mut out);
    let line = status(&out).unwrap();
    assert!(!line.running && line.finished);
}

fn attach_work(server: &mut Server, now: u64) {
    let mut out = Vec::new();
    server.handle(
        Millis(now),
        Event::Client(
            ClientId(2),
            ClientMsg::Hello {
                protocol: PROTOCOL,
                kind: ClientKind::Gui,
                size: GridSize { cols: 20, rows: 5 },
                palette: Config::default().theme(),
                env: Vec::new(),
                target: Target::Name("work".into()),
            },
        ),
        &mut out,
    );
}

fn command_from_work(server: &mut Server, now: u64, command: Command) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(now),
        Event::Client(ClientId(2), ClientMsg::Command(command)),
        &mut out,
    );
    out
}

#[test]
fn killing_a_running_pane_finishes_its_session() {
    let mut server = setup();
    create_work(&mut server);
    attach_work(&mut server, 2);
    command_from_work(&mut server, 3, Command::NewWindow);
    command_from_work(&mut server, 4, Command::LastWindow);
    let out = output(&mut server, 5, PaneId(2), b"\x1b]133;C\x07");
    assert!(status(&out).unwrap().running);
    let out = command_from_work(&mut server, 6, Command::ClosePane);
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::KillPty(PaneId(2))))
    );
    let line = status(&out).unwrap();
    assert!(!line.running && line.finished);
}

#[test]
fn killing_the_last_running_pane_forgets_the_reaped_session() {
    let mut server = setup();
    create_work(&mut server);
    attach_work(&mut server, 2);
    output(&mut server, 3, PaneId(2), b"\x1b]133;C\x07");
    let out = command_from_work(&mut server, 4, Command::CloseWindow);
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::KillPty(PaneId(2))))
    );
    assert!(server.tree.session(SessionId(2)).is_none());
    let line = status(&out).unwrap();
    assert!(!line.running && !line.finished);
    assert!(server.clients[&ClientId(1)].finished.is_empty());
}
