use super::*;
use crate::{event::Timer, facts::Contexts};
use flair_core::{
    command::{Command, WindowRef},
    config::{PromptConfig, PromptStyle},
    isles::{AheadBehind, Changes, GitInfo, Head, LastCommand, PromptInfo, ShellFacts},
    proto::ClientKind,
};
use std::{ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf};

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

fn chips(prompt: impl FnOnce(&mut PromptConfig)) -> (Server, Vec<Effect>) {
    let mut config = Config::default();
    config.prompt.style = PromptStyle::Chips;
    prompt(&mut config.prompt);
    let mut server = Server::new(ServerConfig::from_config(&config));
    let mut out = Vec::new();
    server.handle(Millis(0), Event::Client(ClientId(1), hello()), &mut out);
    (server, out)
}

fn handle(server: &mut Server, at: u64, event: Event) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(Millis(at), event, &mut out);
    out
}

fn output(server: &mut Server, at: u64, bytes: &[u8]) -> Vec<Effect> {
    handle(server, at, Event::PtyOutput(PaneId(1), bytes.to_vec()))
}

fn command(server: &mut Server, at: u64, command: Command) -> Vec<Effect> {
    handle(
        server,
        at,
        Event::Client(ClientId(1), ClientMsg::Command(command)),
    )
}

fn infos(out: &[Effect]) -> Vec<(ClientId, PaneId, PromptInfo)> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::Send(client, ServerMsg::PromptInfo { pane, info }) => {
                Some((*client, *pane, info.clone()))
            }
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
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn git_jobs(out: &[Effect]) -> Vec<(PathBuf, bool)> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::ReadGit { cwd, changes, .. } => Some((cwd.clone(), *changes)),
            Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::KillPty(_)
            | Effect::SetTimer(_, _)
            | Effect::ReadConfig(_)
            | Effect::ReadContexts
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn git_panes(out: &[Effect]) -> Vec<Vec<PaneId>> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::ReadGit { panes, .. } => Some(panes.clone()),
            Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::KillPty(_)
            | Effect::SetTimer(_, _)
            | Effect::ReadConfig(_)
            | Effect::ReadContexts
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn prompt_timers(out: &[Effect]) -> Vec<Millis> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::SetTimer(Timer::Prompt, until) => Some(*until),
            Effect::SetTimer(Timer::Prefix { .. }, _)
            | Effect::Send(_, _)
            | Effect::Spawn(_, _)
            | Effect::SpawnSplit(_, _, _)
            | Effect::WritePty(_, _)
            | Effect::ResizePty(_, _)
            | Effect::KillPty(_)
            | Effect::ReadConfig(_)
            | Effect::ReadGit { .. }
            | Effect::ReadContexts
            | Effect::PaneProcesses { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn context_reads(out: &[Effect]) -> usize {
    out.iter()
        .filter(|effect| matches!(effect, Effect::ReadContexts))
        .count()
}

fn root(server: &Server) -> PathBuf {
    server.panes[&PaneId(1)].cwd.clone()
}

fn osc7(path: &str) -> Vec<u8> {
    format!("\x1b]7;file://{path}\x07").into_bytes()
}

fn repo() -> GitInfo {
    let head = Head::Branch("main".into());
    let upstream = Some(AheadBehind {
        ahead: 2,
        behind: 1,
    });
    let changes = Some(Changes {
        files: 3,
        added: 4,
        removed: 5,
    });
    GitInfo {
        head,
        upstream,
        changes,
    }
}

#[test]
fn exit_and_duration_come_from_the_command_marks() {
    let (mut server, _) = chips(|_| {});
    output(&mut server, 100, b"\x1b]133;C\x07");
    let out = output(&mut server, 2600, b"\x1b]133;D;1\x07\x1b]133;A\x07");
    let sent = infos(&out);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].2.last,
        Some(LastCommand {
            exit: Some(1),
            duration: Millis(2500),
        })
    );
}

#[test]
fn an_empty_enter_clears_the_last_command() {
    let (mut server, _) = chips(|_| {});
    output(&mut server, 100, b"\x1b]133;C\x07");
    output(&mut server, 300, b"\x1b]133;D;127\x07\x1b]133;A\x07");
    assert_eq!(
        server.panes[&PaneId(1)].last,
        Some(LastCommand {
            exit: Some(127),
            duration: Millis(200),
        })
    );
    output(&mut server, 900, b"\x1b]133;D;127\x07\x1b]133;A\x07");
    assert_eq!(server.panes[&PaneId(1)].last, None);
}

#[test]
fn a_new_directory_requests_git_once_and_the_latest_request_wins() {
    let (mut server, initial) = chips(|_| {});
    assert_eq!(git_jobs(&initial), vec![(root(&server), true)]);
    let isle = PathBuf::from("/tmp/flair-isle");
    let mut moved = b"\x1b]133;A\x07".to_vec();
    moved.extend(osc7("/tmp/flair-isle"));
    let out = output(&mut server, 1, &moved);
    assert_eq!(git_jobs(&out), vec![(isle.clone(), true)]);
    let again = output(&mut server, 2, b"\x1b]133;A\x07");
    assert!(git_jobs(&again).is_empty());
    let read = handle(&mut server, 3, Event::GitRead(isle.clone(), Some(repo())));
    assert_eq!(git_jobs(&read), vec![(isle.clone(), true)]);
    let sent = infos(&read);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].2.git, Some(repo()));
    assert_eq!(sent[0].2.cwd, Some(isle.clone()));
    let last = handle(&mut server, 4, Event::GitRead(isle, Some(repo())));
    assert!(git_jobs(&last).is_empty());
    assert!(infos(&last).is_empty());
}

#[test]
fn a_git_result_for_an_unknown_directory_is_dropped() {
    let (mut server, _) = chips(|_| {});
    let nowhere = PathBuf::from("/nowhere");
    let out = handle(
        &mut server,
        1,
        Event::GitRead(nowhere.clone(), Some(repo())),
    );
    assert!(git_jobs(&out).is_empty());
    assert!(infos(&out).is_empty());
    assert!(!server.git.contains_key(&nowhere));
}

#[test]
fn disabled_git_chips_request_nothing_and_changes_follow_their_chip() {
    let (mut server, initial) = chips(|prompt| {
        prompt.git_branch = false;
        prompt.git_ahead_behind = false;
        prompt.git_changes = false;
    });
    assert!(git_jobs(&initial).is_empty());
    assert!(prompt_timers(&initial).is_empty());
    let out = output(&mut server, 1, &osc7("/tmp/flair-off"));
    assert!(git_jobs(&out).is_empty());
    assert!(infos(&out).iter().all(|(_, _, info)| info.git.is_none()));
    let (server, initial) = chips(|prompt| prompt.git_changes = false);
    assert_eq!(git_jobs(&initial), vec![(root(&server), false)]);
}

#[test]
fn git_facts_are_trimmed_to_the_enabled_chips() {
    let (mut server, _) = chips(|prompt| {
        prompt.git_ahead_behind = false;
        prompt.git_changes = false;
    });
    let cwd = root(&server);
    let out = handle(&mut server, 1, Event::GitRead(cwd, Some(repo())));
    let head = Head::Branch("main".into());
    let trimmed = GitInfo {
        head,
        upstream: None,
        changes: None,
    };
    assert_eq!(infos(&out)[0].2.git, Some(trimmed));
}

#[test]
fn the_refresh_timer_runs_only_while_a_client_is_attached() {
    let mut idle = Server::new(ServerConfig::from_config(&Config::default()));
    let connected = handle(&mut idle, 0, Event::ClientConnected(ClientId(1)));
    assert!(prompt_timers(&connected).is_empty());
    let (mut server, initial) = chips(|_| {});
    assert_eq!(prompt_timers(&initial), vec![Millis(5000)]);
    let cwd = root(&server);
    let read = handle(&mut server, 10, Event::GitRead(cwd.clone(), None));
    assert!(prompt_timers(&read).is_empty());
    let fired = handle(&mut server, 5000, Event::Timer(Timer::Prompt));
    assert_eq!(git_jobs(&fired), vec![(cwd.clone(), true)]);
    assert_eq!(prompt_timers(&fired), vec![Millis(10000)]);
    handle(&mut server, 5001, Event::GitRead(cwd, None));
    handle(
        &mut server,
        5002,
        Event::Client(ClientId(1), ClientMsg::Detach),
    );
    let alone = handle(&mut server, 10000, Event::Timer(Timer::Prompt));
    assert!(git_jobs(&alone).is_empty());
    assert!(prompt_timers(&alone).is_empty());
}

#[test]
fn the_refresh_timer_forgets_directories_no_pane_uses() {
    let (mut server, _) = chips(|_| {});
    let gone = PathBuf::from("/tmp/flair-gone");
    let here = PathBuf::from("/tmp/flair-here");
    let cwd = root(&server);
    handle(&mut server, 1, Event::GitRead(cwd, None));
    output(&mut server, 2, &osc7("/tmp/flair-gone"));
    handle(&mut server, 3, Event::GitRead(gone.clone(), None));
    output(&mut server, 4, &osc7("/tmp/flair-here"));
    handle(&mut server, 5, Event::GitRead(here.clone(), None));
    let fired = handle(&mut server, 5000, Event::Timer(Timer::Prompt));
    assert_eq!(git_jobs(&fired), vec![(here, true)]);
    assert!(!server.git.contains_key(&gone));
}

#[test]
fn shell_facts_are_sent_only_when_their_chips_are_on() {
    let facts = ShellFacts {
        user: "ada".into(),
        host: "box".into(),
        venv: Some("env".into()),
    };
    let osc = b"\x1b]6973;flair;u=ada;h=box;v=env\x07";
    let (mut server, _) = chips(|prompt| prompt.user = true);
    let out = output(&mut server, 1, osc);
    assert_eq!(infos(&out)[0].2.shell, Some(facts.clone()));
    let (mut server, _) = chips(|_| {});
    let out = output(&mut server, 1, osc);
    assert!(infos(&out).is_empty());
    assert_eq!(server.panes[&PaneId(1)].shell, Some(facts));
}

#[test]
fn a_cwd_that_is_not_utf8_is_not_published() {
    let (mut server, _) = chips(|_| {});
    let raw = PathBuf::from(OsString::from_vec(b"/tmp/flair-\xff".to_vec()));
    server.panes.get_mut(&PaneId(1)).unwrap().cwd = raw.clone();
    let out = handle(&mut server, 1, Event::Timer(Timer::Prompt));
    let sent = infos(&out);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].2.cwd, None);
    assert_eq!(git_jobs(&out), vec![(raw, true)]);
}

#[test]
fn git_requests_name_every_pane_whose_shell_claims_the_directory() {
    let (mut server, initial) = chips(|_| {});
    assert_eq!(git_panes(&initial), vec![vec![PaneId(1)]]);
    let cwd = root(&server);
    handle(&mut server, 1, Event::GitRead(cwd.clone(), None));
    command(&mut server, 2, Command::NewWindow);
    output(&mut server, 3, &osc7("/tmp/flair-elsewhere"));
    let out = handle(&mut server, 5000, Event::Timer(Timer::Prompt));
    assert_eq!(git_jobs(&out), vec![(cwd, true)]);
    assert_eq!(git_panes(&out), vec![vec![PaneId(2)]]);
}

#[test]
fn a_failed_read_returns_the_directory_to_idle() {
    let (mut server, initial) = chips(|_| {});
    let cwd = root(&server);
    assert_eq!(git_jobs(&initial), vec![(cwd.clone(), true)]);
    assert!(git_jobs(&output(&mut server, 1, b"\x1b]133;A\x07")).is_empty());
    let failed = handle(&mut server, 2, Event::GitRead(cwd.clone(), None));
    assert_eq!(git_jobs(&failed), vec![(cwd.clone(), true)]);
    handle(&mut server, 3, Event::GitRead(cwd.clone(), None));
    let again = output(&mut server, 4, b"\x1b]133;A\x07");
    assert_eq!(git_jobs(&again), vec![(cwd, true)]);
}

#[test]
fn plain_output_keeps_prompt_facts_and_command_marks_update_them() {
    let (mut server, _) = chips(|_| {});
    assert!(infos(&output(&mut server, 1, b"plain text")).is_empty());
    let marked = output(&mut server, 2, b"\x1b]133;C\x07\x1b]133;D;3\x07");
    assert_eq!(infos(&marked)[0].2.last.unwrap().exit, Some(3));
}

#[test]
fn the_shell_facts_osc_reaches_the_band() {
    let (mut server, _) = chips(|prompt| {
        prompt.user = true;
        prompt.host = true;
        prompt.venv = true;
    });
    let out = output(
        &mut server,
        1,
        b"\x1b]6973;flair;u=ada;h=box%3B1;v=my%3Denv\x07",
    );
    let sent = infos(&out);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, PaneId(1));
    assert_eq!(
        sent[0].2.shell,
        Some(ShellFacts {
            user: "ada".into(),
            host: "box;1".into(),
            venv: Some("my=env".into()),
        })
    );
    let (mut server, _) = chips(|_| {});
    let out = output(&mut server, 1, b"\x1b]6973;flair;u=ada;h=box;v=\x07");
    assert!(infos(&out).is_empty());
}

#[test]
fn contexts_are_read_once_at_a_time_and_reach_the_band() {
    let (mut server, initial) = chips(|prompt| prompt.docker = true);
    assert_eq!(context_reads(&initial), 0);
    let first = output(&mut server, 1, b"\x1b]133;A\x07");
    assert_eq!(context_reads(&first), 1);
    let second = output(&mut server, 2, b"\x1b]133;A\x07");
    assert_eq!(context_reads(&second), 0);
    let contexts = Contexts {
        docker: Some("prod".into()),
        kube: Some("cluster".into()),
    };
    let read = handle(&mut server, 3, Event::ContextsRead(contexts));
    assert_eq!(context_reads(&read), 1);
    let sent = infos(&read);
    assert_eq!(sent[0].2.docker, Some("prod".into()));
    assert_eq!(sent[0].2.kube, None);
    let (mut server, _) = chips(|_| {});
    let out = output(&mut server, 1, b"\x1b]133;A\x07");
    assert_eq!(context_reads(&out), 0);
}

#[test]
fn each_client_gets_its_own_copy_once_per_change() {
    let (mut server, initial) = chips(|_| {});
    let first = infos(&initial);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].0, ClientId(1));
    assert!(infos(&output(&mut server, 1, b"x")).is_empty());
    let second = handle(&mut server, 2, Event::Client(ClientId(2), hello()));
    let sent = infos(&second);
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].0, sent[0].1), (ClientId(2), PaneId(1)));
    let changed = output(&mut server, 3, b"\x1b]133;C\x07\x1b]133;D;2\x07");
    let mut clients: Vec<_> = infos(&changed)
        .into_iter()
        .map(|(client, _, _)| client)
        .collect();
    clients.sort();
    assert_eq!(clients, vec![ClientId(1), ClientId(2)]);
}

#[test]
fn a_window_switch_sends_info_for_the_newly_visible_panes() {
    let (mut server, _) = chips(|_| {});
    let created = command(&mut server, 1, Command::NewWindow);
    let sent = infos(&created);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, PaneId(2));
    let back = command(&mut server, 2, Command::SelectWindow(WindowRef::Index(1)));
    let sent = infos(&back);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].1, PaneId(1));
}

#[test]
fn new_windows_use_the_full_pane_size_for_the_shell_prompt() {
    let (mut server, initial) = chips(|_| {});
    let created = command(&mut server, 1, Command::NewWindow);
    for (out, pane) in [(initial, PaneId(1)), (created, PaneId(2))] {
        let size = out
            .iter()
            .find_map(|effect| match effect {
                Effect::Spawn(id, spec) if *id == pane => Some(spec.size),
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
                | Effect::PaneProcesses { .. }
                | Effect::Shutdown => None,
            })
            .unwrap();
        assert_eq!(size, GridSize { cols: 20, rows: 4 });
        assert!(
            !out.iter()
                .any(|effect| matches!(effect, Effect::ResizePty(id, _) if *id == pane))
        );
    }
}

#[test]
fn reloading_chips_sends_facts_without_reserving_a_pane_row() {
    let (mut server, initial) = chips(|prompt| prompt.style = PromptStyle::Ps1);
    assert!(infos(&initial).is_empty());
    assert!(git_jobs(&initial).is_empty());
    assert!(prompt_timers(&initial).is_empty());
    let geometry = server.tree.window(WindowId(1)).unwrap().geometry(0);
    assert_eq!(geometry.panes[0].band, 0);
    for effect in &initial {
        if let Effect::Send(client, ServerMsg::Frame { seq, .. }) = effect {
            handle(
                &mut server,
                1,
                Event::Client(*client, ClientMsg::FrameAck { seq: *seq }),
            );
        }
    }
    let toml = "[prompt]\nstyle = \"chips\"\n";
    let out = handle(&mut server, 2, Event::ConfigRead(Ok(toml.into())));
    assert!(
        !out.iter()
            .any(|effect| matches!(effect, Effect::ResizePty(_, _)))
    );
    assert_eq!(infos(&out).len(), 1);
    assert_eq!(prompt_timers(&out), vec![Millis(5002)]);
}

#[test]
fn prompt_history_keeps_each_directory_when_one_read_contains_multiple_prompts() {
    let (mut server, initial) = chips(|_| {});
    for effect in initial {
        if let Effect::Send(client, ServerMsg::Frame { seq, .. }) = effect {
            handle(
                &mut server,
                0,
                Event::Client(client, ClientMsg::FrameAck { seq }),
            );
        }
    }
    let out = output(&mut server, 1, b"\x1b]7;file:///first\x07\x1b]6973;prompt\x07\r\nfirst\r\n\x1b]7;file:///second\x07\x1b]6973;prompt\x07\r\nsecond");
    let history = out
        .iter()
        .find_map(|effect| {
            if let Effect::Send(
                ClientId(1),
                ServerMsg::PromptHistory {
                    pane: PaneId(1),
                    prompts,
                },
            ) = effect
            {
                Some(prompts)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].info.cwd.as_deref(), Some(Path::new("/first")));
    assert_eq!(history[1].info.cwd.as_deref(), Some(Path::new("/second")));
    assert!(history.iter().all(|prompt| prompt.timestamp > 0));
    let unchanged = output(&mut server, 2, b"x");
    assert!(
        !unchanged
            .iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptHistory { .. })))
    );
    let scrolled = output(&mut server, 3, b"\r\nmore");
    assert!(
        !scrolled
            .iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptHistory { .. })))
    );
    let seq = out
        .iter()
        .find_map(|effect| {
            if let Effect::Send(ClientId(1), ServerMsg::Frame { seq, .. }) = effect {
                Some(*seq)
            } else {
                None
            }
        })
        .unwrap();
    let acked = handle(
        &mut server,
        4,
        Event::Client(ClientId(1), ClientMsg::FrameAck { seq }),
    );
    let history = acked
        .iter()
        .find_map(|effect| {
            if let Effect::Send(
                ClientId(1),
                ServerMsg::PromptHistory {
                    pane: PaneId(1),
                    prompts,
                },
            ) = effect
            {
                Some(prompts)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].row, 1);
    assert_eq!(history[0].info.cwd.as_deref(), Some(Path::new("/second")));
}

#[test]
fn prompt_history_waits_for_frame_credit_when_alt_screen_changes() {
    let (mut server, _) = chips(|_| {});
    let seq = server.clients[&ClientId(1)].awaiting_ack.unwrap();
    handle(
        &mut server,
        0,
        Event::Client(ClientId(1), ClientMsg::FrameAck { seq }),
    );
    let shown = output(&mut server, 1, b"\x1b]6973;prompt\x07\r\ninput");
    assert!(shown.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptHistory { prompts, .. }) if prompts.len() == 1)));
    let hidden = output(&mut server, 2, b"\x1b[?1049h");
    assert!(
        !hidden
            .iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptHistory { .. })))
    );
    let seq = server.clients[&ClientId(1)].awaiting_ack.unwrap();
    let hidden = handle(
        &mut server,
        3,
        Event::Client(ClientId(1), ClientMsg::FrameAck { seq }),
    );
    assert!(hidden.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptHistory { prompts, .. }) if prompts.is_empty())));
    let restored = output(&mut server, 4, b"\x1b[?1049l");
    assert!(
        !restored
            .iter()
            .any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptHistory { .. })))
    );
    let seq = server.clients[&ClientId(1)].awaiting_ack.unwrap();
    let restored = handle(
        &mut server,
        5,
        Event::Client(ClientId(1), ClientMsg::FrameAck { seq }),
    );
    assert!(restored.iter().any(|effect| matches!(effect, Effect::Send(_, ServerMsg::PromptHistory { prompts, .. }) if prompts.len() == 1)));
}
