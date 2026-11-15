use super::*;
use flair_core::{command::Command, config::Config, proto::ClientKind};

fn hello(kind: ClientKind, target: Target) -> ClientMsg {
    ClientMsg::Hello {
        protocol: PROTOCOL,
        kind,
        size: GridSize { cols: 20, rows: 4 },
        palette: Config::default().theme(),
        env: Vec::new(),
        target,
    }
}

fn server_with(config: &Config, kind: ClientKind) -> Server {
    let mut server = Server::new(ServerConfig::from_config(config));
    let mut out = Vec::new();
    server.handle(
        Millis(0),
        Event::Client(ClientId(1), hello(kind, Target::Main)),
        &mut out,
    );
    server
}

fn setup() -> Server {
    server_with(&Config::default(), ClientKind::Gui)
}

fn send(server: &mut Server, id: u32, message: ClientMsg) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(Millis(1), Event::Client(ClientId(id), message), &mut out);
    out
}

fn output(server: &mut Server, pane: u32, text: &str) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(1),
        Event::PtyOutput(PaneId(pane), text.as_bytes().to_vec()),
        &mut out,
    );
    out
}

fn agent(server: &mut Server, pane: u32, report: &str) -> Vec<Effect> {
    output(server, pane, &format!("\x1b]7501;{report}\x07"))
}

fn process(server: &mut Server, pane: u32, argv: &[&str]) -> Vec<Effect> {
    let mut out = Vec::new();
    let argv = argv.iter().map(|arg| (*arg).to_owned()).collect();
    server.handle(Millis(1), Event::ProcessRead(PaneId(pane), argv), &mut out);
    out
}

fn rows(server: &Server) -> Vec<AgentRow> {
    server.agent_rows()
}

fn notifications(out: &[Effect]) -> Vec<(String, String)> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::Notify { title, body } => Some((title.clone(), body.clone())),
            Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::KillPty(_)
            | Effect::Send(_, _)
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

fn record(state: AgentState, app: Option<&str>) -> AgentRecord {
    AgentRecord {
        state,
        kind: None,
        app: app.map(str::to_owned),
        title: None,
        msg: None,
    }
}

fn set(records: &mut BTreeMap<String, AgentRecord>, id: &str, record: AgentRecord) {
    apply(
        records,
        AgentReport::Set {
            id: id.to_owned(),
            record,
        },
    );
}

#[test]
fn a_report_shows_up_in_the_status_of_every_client_across_sessions() {
    let mut server = setup();
    send(&mut server, 2, hello(ClientKind::Gui, Target::New));
    let session = server.tree.sessions[1].name.clone();
    let window = server.tree.sessions[1].windows[0].id;
    let out = agent(
        &mut server,
        2,
        "state=blocked:kind=permission:app=claude:title=Rml4IGxvZ2luIGJ1Zw==",
    );
    let expected = vec![AgentRow {
        session,
        window,
        pane: PaneId(2),
        app: "claude".to_owned(),
        state: AgentState::Blocked,
        kind: Some(AgentKind::Permission),
        title: "Fix login bug".to_owned(),
        unseen: true,
    }];
    for id in [ClientId(1), ClientId(2)] {
        let line = out
            .iter()
            .find_map(|effect| {
                if let Effect::Send(client, ServerMsg::Status(line)) = effect
                    && *client == id
                {
                    Some(line)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(line.agents, expected, "{id:?}");
    }
}

#[test]
fn the_summary_is_the_highest_rank_and_the_first_key_on_ties() {
    let mut records = BTreeMap::new();
    set(&mut records, "b", record(AgentState::Done, Some("b")));
    set(&mut records, "", record(AgentState::Done, Some("root")));
    assert_eq!(summary(&records).unwrap().0, "");
    set(&mut records, "a", record(AgentState::Working, None));
    set(&mut records, "c", record(AgentState::Error, None));
    assert_eq!(summary(&records).unwrap().0, "c");
    set(&mut records, "d", record(AgentState::Blocked, None));
    assert_eq!(summary(&records).unwrap().0, "d");
    assert!(summary(&BTreeMap::new()).is_none());
}

#[test]
fn a_child_inherits_the_app_of_its_nearest_ancestor() {
    let mut records = BTreeMap::new();
    set(
        &mut records,
        "",
        record(AgentState::Working, Some("claude")),
    );
    set(&mut records, "a", record(AgentState::Working, Some("sub")));
    set(&mut records, "a/b", record(AgentState::Working, None));
    set(&mut records, "c/d", record(AgentState::Working, None));
    set(&mut records, "e", record(AgentState::Working, Some("own")));
    assert_eq!(records["a/b"].app.as_deref(), Some("sub"));
    assert_eq!(records["c/d"].app.as_deref(), Some("claude"));
    assert_eq!(records["e"].app.as_deref(), Some("own"));
}

#[test]
fn new_records_past_sixty_four_are_ignored() {
    let mut records = BTreeMap::new();
    for index in 0..64 {
        set(
            &mut records,
            &format!("{index}"),
            record(AgentState::Working, None),
        );
    }
    set(&mut records, "late", record(AgentState::Blocked, None));
    assert_eq!(records.len(), 64);
    assert!(!records.contains_key("late"));
    set(&mut records, "7", record(AgentState::Done, None));
    assert_eq!(records["7"].state, AgentState::Done);
}

#[test]
fn clear_removes_the_subtree_or_everything() {
    let mut records = BTreeMap::new();
    for id in ["", "a", "a/b", "a/b/c", "ab", "b"] {
        set(&mut records, id, record(AgentState::Working, None));
    }
    apply(&mut records, AgentReport::Clear(Some("a".to_owned())));
    assert_eq!(
        records.keys().map(String::as_str).collect::<Vec<_>>(),
        ["", "ab", "b"]
    );
    apply(&mut records, AgentReport::Clear(None));
    assert!(records.is_empty());
}

#[test]
fn the_title_falls_back_to_the_root_then_the_window_name_then_the_process() {
    let mut server = setup();
    agent(&mut server, 1, "state=working");
    let (program, label) = server.process(PaneId(1));
    assert_eq!(rows(&server)[0].title, label);
    assert_eq!(rows(&server)[0].app, program);
    send(
        &mut server,
        1,
        ClientMsg::Command(Command::RenameWindow(Some("build".to_owned()))),
    );
    assert_eq!(rows(&server)[0].title, "build");
    agent(&mut server, 1, "state=working:app=codex:title=cm9vdA");
    agent(&mut server, 1, "state=blocked:id=a");
    assert_eq!(rows(&server)[0].title, "root");
    assert_eq!(rows(&server)[0].app, "codex");
    assert_eq!(rows(&server)[0].state, AgentState::Blocked);
    agent(
        &mut server,
        1,
        "state=blocked:id=a:app=claude:title=Y2hpbGQ=",
    );
    assert_eq!(rows(&server)[0].title, "child");
    assert_eq!(rows(&server)[0].app, "claude");
}

#[test]
fn a_prompt_keeps_an_unseen_done_and_drops_seen_and_running_records() {
    let mut server = setup();
    agent(&mut server, 1, "state=done");
    assert!(rows(&server)[0].unseen);
    output(&mut server, 1, "\x1b]133;A\x07");
    assert_eq!(rows(&server)[0].state, AgentState::Done);
    send(&mut server, 1, ClientMsg::Focus(true));
    assert!(!rows(&server)[0].unseen);
    output(&mut server, 1, "\x1b]133;A\x07");
    assert!(rows(&server).is_empty());
    agent(&mut server, 1, "state=working");
    output(&mut server, 1, "\x1b]133;A\x07");
    assert!(rows(&server).is_empty());
}

#[test]
fn process_exit_drops_running_records() {
    let mut server = setup();
    agent(&mut server, 1, "state=working");
    agent(&mut server, 1, "state=blocked:id=a");
    let mut out = Vec::new();
    server.handle(Millis(2), Event::PtyExited(PaneId(1)), &mut out);
    assert!(rows(&server).is_empty());
}

#[test]
fn a_program_change_drops_records_but_the_first_probe_does_not() {
    let mut server = setup();
    process(&mut server, 1, &["zsh"]);
    agent(&mut server, 1, "state=working");
    process(&mut server, 1, &["claude"]);
    process(&mut server, 1, &["claude"]);
    assert_eq!(rows(&server).len(), 1);
    process(&mut server, 1, &["zsh"]);
    assert!(rows(&server).is_empty());
    agent(&mut server, 1, "state=working");
    process(&mut server, 1, &["zsh"]);
    assert_eq!(rows(&server).len(), 1);
}

#[test]
fn unseen_holds_until_a_focused_gui_client_views_the_window() {
    let mut server = setup();
    agent(&mut server, 1, "state=error");
    assert!(rows(&server)[0].unseen);
    send(&mut server, 1, ClientMsg::Focus(true));
    assert!(!rows(&server)[0].unseen);
    agent(&mut server, 1, "state=blocked");
    assert!(!rows(&server)[0].unseen);
    send(&mut server, 1, ClientMsg::Command(Command::NewWindow));
    agent(&mut server, 1, "state=done");
    assert!(rows(&server)[0].unseen);
    agent(&mut server, 1, "state=working");
    assert!(!rows(&server)[0].unseen);
}

#[test]
fn notify_needs_an_unviewed_change_a_gui_client_and_the_setting() {
    let mut server = setup();
    let out = agent(
        &mut server,
        1,
        "state=blocked:kind=permission:app=claude:title=Rml4IDxiPiAmIGM=",
    );
    let session = server.tree.sessions[0].name.clone();
    assert_eq!(
        notifications(&out),
        [(
            "claude needs approval".to_owned(),
            format!("{session}: Fix &lt;b&gt; &amp; c")
        )]
    );
    let out = agent(
        &mut server,
        1,
        "state=blocked:kind=permission:app=claude:title=YWdhaW4=",
    );
    assert!(notifications(&out).is_empty());
    let out = agent_at(&mut server, 3001, "state=done:app=claude:msg=YWxsIGdvb2Q=");
    assert_eq!(
        notifications(&out),
        [("claude done".to_owned(), format!("{session}: all good"))]
    );
    send(&mut server, 1, ClientMsg::Focus(true));
    let out = agent(&mut server, 1, "state=error:app=claude");
    assert!(notifications(&out).is_empty());

    let mut headless = server_with(&Config::default(), ClientKind::Headless);
    let out = agent(&mut headless, 1, "state=error");
    assert!(notifications(&out).is_empty());
    assert!(rows(&headless)[0].unseen);

    let mut config = Config::default();
    config.notifications.enabled = false;
    let mut quiet = server_with(&config, ClientKind::Gui);
    let out = agent(&mut quiet, 1, "state=error");
    assert!(notifications(&out).is_empty());
    assert!(rows(&quiet)[0].unseen);
}

#[test]
fn the_agents_command_echoes_only_to_the_sender() {
    let mut server = setup();
    send(&mut server, 2, hello(ClientKind::Gui, Target::Main));
    let out = send(&mut server, 1, ClientMsg::Command(Command::Agents));
    let echoes: Vec<_> = out
        .iter()
        .filter_map(|effect| {
            if let Effect::Send(id, ServerMsg::Agents) = effect {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(echoes, [ClientId(1)]);
}

fn agent_at(server: &mut Server, now: u64, report: &str) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(
        Millis(now),
        Event::PtyOutput(PaneId(1), format!("\x1b]7501;{report}\x07").into_bytes()),
        &mut out,
    );
    out
}

#[test]
fn a_pane_notifies_at_most_once_every_three_seconds() {
    let mut server = setup();
    assert_eq!(
        notifications(&agent_at(&mut server, 1, "state=blocked")).len(),
        1
    );
    agent_at(&mut server, 500, "state=working");
    assert!(notifications(&agent_at(&mut server, 1000, "state=blocked")).is_empty());
    agent_at(&mut server, 2000, "state=working");
    assert_eq!(
        notifications(&agent_at(&mut server, 3001, "state=blocked")).len(),
        1
    );
}

#[test]
fn dropping_records_never_notifies() {
    let mut server = setup();
    agent(&mut server, 1, "state=done:id=a");
    agent(&mut server, 1, "state=blocked");
    let out = output(&mut server, 1, "\x1b]133;A\x07");
    assert_eq!(rows(&server)[0].state, AgentState::Done);
    assert!(notifications(&out).is_empty());

    let mut server = setup();
    agent(&mut server, 1, "state=done:id=a");
    agent(&mut server, 1, "state=blocked");
    let mut out = Vec::new();
    server.handle(Millis(2), Event::PtyExited(PaneId(1)), &mut out);
    assert_eq!(rows(&server)[0].state, AgentState::Done);
    assert!(notifications(&out).is_empty());

    let mut server = setup();
    process(&mut server, 1, &["claude"]);
    agent(&mut server, 1, "state=done:id=a");
    agent(&mut server, 1, "state=blocked");
    process(&mut server, 1, &["claude"]);
    let out = process(&mut server, 1, &["zsh"]);
    assert_eq!(rows(&server)[0].state, AgentState::Done);
    assert!(notifications(&out).is_empty());
}
