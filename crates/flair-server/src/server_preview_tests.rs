use super::*;
use flair_core::{
    command::Command,
    config::Config,
    proto::{ClientKind, Color, Ink},
};

const ME: ClientId = ClientId(1);
const MAIN: SessionId = SessionId(1);

fn hello() -> ClientMsg {
    ClientMsg::Hello {
        protocol: PROTOCOL,
        kind: ClientKind::Gui,
        size: GridSize { cols: 20, rows: 4 },
        palette: Config::default().theme(),
        env: Vec::new(),
        target: Target::Main,
    }
}

fn handle(server: &mut Server, at: u64, event: Event) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(Millis(at), event, &mut out);
    out
}

fn command(server: &mut Server, at: u64, command: Command) -> Vec<Effect> {
    handle(server, at, Event::Client(ME, ClientMsg::Command(command)))
}

fn request(server: &mut Server, at: u64, session: Option<SessionId>) -> Vec<Effect> {
    handle(server, at, Event::Client(ME, ClientMsg::Preview(session)))
}

fn output(server: &mut Server, at: u64, pane: PaneId, text: &str) -> Vec<Effect> {
    handle(server, at, Event::PtyOutput(pane, text.as_bytes().to_vec()))
}

fn attached(windows: usize) -> Server {
    let mut server = Server::new(ServerConfig::from_config(&Config::default()));
    handle(&mut server, 0, Event::Client(ME, hello()));
    for at in 1..windows {
        command(&mut server, at as u64, Command::NewWindow);
    }
    server
}

fn first_pane(server: &Server) -> PaneId {
    server.tree.sessions[0].windows[0].panes[0]
}

fn previews(out: &[Effect]) -> Vec<SessionPreview> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::Send(ClientId(1), ServerMsg::Preview(preview)) => Some(preview.clone()),
            Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::KillPty(_)
            | Effect::SetTimer(_, _)
            | Effect::ReadConfig(_)
            | Effect::ReadGit { .. }
            | Effect::ReadContexts
            | Effect::ReadProcess(_)
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn preview_timers(out: &[Effect]) -> Vec<Millis> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::SetTimer(Timer::Preview(ClientId(1)), at) => Some(*at),
            Effect::SetTimer(_, _)
            | Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::KillPty(_)
            | Effect::ReadConfig(_)
            | Effect::ReadGit { .. }
            | Effect::ReadContexts
            | Effect::ReadProcess(_)
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn first_row(preview: &SessionPreview) -> Vec<Ink> {
    preview.windows[0].panes[0].rows[0].clone()
}

fn ink(len: u16) -> Vec<Ink> {
    vec![Ink {
        col: 0,
        len,
        fg: Some(Color::Foreground),
        bg: Color::Background,
    }]
}

#[test]
fn choose_session_carries_the_preview_of_the_current_session_and_watches_it() {
    let mut server = attached(2);
    let pane = first_pane(&server);
    output(&mut server, 3, pane, "hello");
    let out = command(&mut server, 4, Command::ChooseSession);
    let preview = out
        .iter()
        .find_map(|effect| match effect {
            Effect::Send(ME, ServerMsg::ChooseSession { preview, .. }) => Some(preview.clone()),
            Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::KillPty(_)
            | Effect::SetTimer(_, _)
            | Effect::ReadConfig(_)
            | Effect::ReadGit { .. }
            | Effect::ReadContexts
            | Effect::ReadProcess(_)
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .expect("the chooser is sent");
    assert_eq!(server.tree.sessions[0].id, MAIN);
    assert_eq!(preview.session, MAIN);
    assert_eq!(preview.windows.len(), 2);
    assert_eq!(first_row(&preview), ink(5));
    let layout = server.tree.sessions[0].windows[0].geometry(0);
    assert_eq!(preview.windows[0].layout, layout);
    assert_eq!(
        server.clients[&ME].preview.map(|watch| watch.session),
        Some(MAIN)
    );
}

#[test]
fn a_preview_request_replies_with_at_most_nine_windows() {
    let mut server = attached(11);
    let out = request(&mut server, 20, Some(MAIN));
    let [preview] = previews(&out).try_into().expect("one preview");
    let expected: Vec<_> = server.tree.sessions[0]
        .windows
        .iter()
        .take(9)
        .map(|window| window.id)
        .collect();
    let ids: Vec<_> = preview.windows.iter().map(|window| window.window).collect();
    assert_eq!(ids, expected);
    assert!(
        preview
            .windows
            .iter()
            .all(|window| window.panes.len() == window.layout.panes.len())
    );
}

#[test]
fn an_unknown_session_gets_no_preview_and_no_watch() {
    let mut server = attached(1);
    request(&mut server, 1, Some(MAIN));
    let out = request(&mut server, 2, Some(SessionId(999)));
    assert!(previews(&out).is_empty());
    assert_eq!(server.clients[&ME].preview, None);
}

#[test]
fn changes_in_the_watched_session_are_pushed_at_most_every_250_ms() {
    let mut server = attached(2);
    let pane = first_pane(&server);
    request(&mut server, 1000, Some(MAIN));
    let early = output(&mut server, 1100, pane, "ab");
    assert!(previews(&early).is_empty());
    assert_eq!(preview_timers(&early), [Millis(1250)]);
    let again = output(&mut server, 1200, pane, "cd");
    assert!(previews(&again).is_empty());
    assert!(preview_timers(&again).is_empty());
    let fired = handle(&mut server, 1250, Event::Timer(Timer::Preview(ME)));
    let [preview] = previews(&fired).try_into().expect("one push");
    assert_eq!(first_row(&preview), ink(4));
    let quiet = handle(&mut server, 1600, Event::Timer(Timer::Prompt));
    assert!(previews(&quiet).is_empty());
    let late = output(&mut server, 1600, pane, "e");
    let [preview] = previews(&late).try_into().expect("one push");
    assert_eq!(first_row(&preview), ink(5));
}

#[test]
fn the_throttle_does_not_wait_for_frame_acks() {
    let mut server = attached(1);
    let pane = first_pane(&server);
    request(&mut server, 1000, Some(MAIN));
    server.clients.get_mut(&ME).unwrap().awaiting_ack = Some(99);
    let out = output(&mut server, 1300, pane, "x");
    assert_eq!(previews(&out).len(), 1);
}

#[test]
fn changes_in_other_sessions_are_not_pushed() {
    let mut server = attached(1);
    request(&mut server, 1000, Some(MAIN));
    handle(
        &mut server,
        1001,
        Event::Client(
            ClientId(9),
            ClientMsg::Control(Control::Create {
                target: Target::Path("/tmp/other".into()),
                env: Vec::new(),
            }),
        ),
    );
    let other = server.tree.sessions[1].windows[0].panes[0];
    let out = output(&mut server, 2000, other, "busy");
    assert!(previews(&out).is_empty());
    assert!(preview_timers(&out).is_empty());
}

#[test]
fn unwatching_switching_and_detaching_end_the_watch() {
    let mut server = attached(1);
    let pane = first_pane(&server);
    request(&mut server, 1000, Some(MAIN));
    request(&mut server, 1001, None);
    assert_eq!(server.clients[&ME].preview, None);
    assert!(previews(&output(&mut server, 2000, pane, "a")).is_empty());
    request(&mut server, 2001, Some(MAIN));
    command(
        &mut server,
        2002,
        Command::Switch(Target::Path("/tmp/project".into())),
    );
    assert_eq!(server.clients[&ME].preview, None);
    assert!(previews(&output(&mut server, 3000, pane, "b")).is_empty());
    request(&mut server, 3001, Some(MAIN));
    handle(&mut server, 3002, Event::Client(ME, ClientMsg::Detach));
    assert!(!server.clients.contains_key(&ME));
    assert!(previews(&output(&mut server, 4000, pane, "c")).is_empty());
}

fn create(server: &mut Server, at: u64, root: &str) -> SessionId {
    handle(
        server,
        at,
        Event::Client(
            ClientId(9),
            ClientMsg::Control(Control::Create {
                target: Target::Path(root.into()),
                env: Vec::new(),
            }),
        ),
    );
    server.tree.sessions.last().unwrap().id
}

#[test]
fn subscribes_within_250_ms_send_the_last_selection_when_the_window_elapses() {
    let mut server = attached(1);
    let first = create(&mut server, 1, "/tmp/first");
    let second = create(&mut server, 2, "/tmp/second");
    let now = request(&mut server, 1000, Some(MAIN));
    let [preview] = previews(&now).try_into().expect("one immediate preview");
    assert_eq!(preview.session, MAIN);
    let held = request(&mut server, 1100, Some(first));
    assert!(previews(&held).is_empty());
    assert_eq!(preview_timers(&held), [Millis(1250)]);
    let again = request(&mut server, 1200, Some(second));
    assert!(previews(&again).is_empty());
    assert!(preview_timers(&again).is_empty());
    let fired = handle(&mut server, 1250, Event::Timer(Timer::Preview(ME)));
    let [preview] = previews(&fired).try_into().expect("one delayed preview");
    assert_eq!(preview.session, second);
    assert_eq!(
        server.clients[&ME].preview.map(|watch| watch.session),
        Some(second)
    );
}

#[test]
fn a_destroyed_watched_session_sends_an_empty_preview_and_ends_the_watch() {
    let mut server = attached(1);
    let other = create(&mut server, 1, "/tmp/other");
    request(&mut server, 1000, Some(other));
    let pane = server.tree.session(other).unwrap().windows[0].panes[0];
    let out = handle(&mut server, 2000, Event::PtyExited(pane));
    assert!(server.tree.session(other).is_none());
    assert_eq!(
        previews(&out),
        [SessionPreview {
            session: other,
            windows: Vec::new(),
        }]
    );
    assert_eq!(server.clients[&ME].preview, None);
}
