use super::*;
use flair_core::{
    command::{CopyCommand, PaneRef, WindowRef},
    geom::Axis,
    status::{ActivePane, StatusLine},
};
use flair_term::pty::SpawnSpec;
use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::PathBuf};

fn from(server: &mut Server, id: ClientId, now: u64, message: ClientMsg) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(Millis(now), Event::Client(id, message), &mut out);
    out
}

fn run(server: &mut Server, now: u64, command: Command) -> Vec<Effect> {
    from(server, ClientId(1), now, ClientMsg::Command(command))
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

fn status_for(out: &[Effect], id: ClientId) -> Option<&StatusLine> {
    out.iter().find_map(|effect| {
        if let Effect::Send(client, ServerMsg::Status(line)) = effect
            && *client == id
        {
            Some(line)
        } else {
            None
        }
    })
}

fn active(out: &[Effect]) -> Option<ActivePane> {
    status_for(out, ClientId(1)).and_then(|line| line.pane.clone())
}

fn at_prompt(out: &[Effect]) -> Option<bool> {
    active(out).map(|pane| pane.at_prompt)
}

fn spawns(out: &[Effect]) -> Vec<(PaneId, &SpawnSpec)> {
    out.iter()
        .filter_map(|effect| match effect {
            Effect::Spawn(pane, spec) | Effect::SpawnSplit(pane, _, spec) => Some((*pane, spec)),
            Effect::WritePty(..)
            | Effect::ResizePty(..)
            | Effect::KillPty(_)
            | Effect::Send(..)
            | Effect::SetTimer(..)
            | Effect::ReadConfig(_)
            | Effect::ReadGit { .. }
            | Effect::ReadContexts
            | Effect::ReadProcess(_)
            | Effect::PaneProcesses { .. }
            | Effect::Notify { .. }
            | Effect::Shutdown => None,
        })
        .collect()
}

fn errors(out: &[Effect]) -> usize {
    out.iter()
        .filter(|effect| matches!(effect, Effect::Send(_, ServerMsg::Error(_))))
        .count()
}

fn edit(file: &str, cwd: &str) -> Command {
    Command::SplitRun {
        argv: [
            "/bin/sh",
            "-c",
            "exec ${EDITOR:-vi} \"$1\"",
            "flair-edit",
            file,
        ]
        .map(str::to_owned)
        .to_vec(),
        cwd: PathBuf::from(cwd),
    }
}

#[test]
fn attach_publishes_the_active_pane_with_its_spawn_cwd_and_no_prompt() {
    let (server, out) = setup();
    assert_eq!(
        active(&out),
        Some(ActivePane {
            pane: PaneId(1),
            cwd: Some(server.panes[&PaneId(1)].cwd.clone()),
            at_prompt: false,
        })
    );
}

#[test]
fn osc_7_moves_the_published_cwd_even_without_prompt_chips() {
    let (mut server, _) = setup();
    assert_eq!(server.config.prompt.band(), 0);
    let out = output(
        &mut server,
        2,
        PaneId(1),
        b"\x1b]7;file://localhost/tmp/explorer%20cwd\x07",
    );
    assert_eq!(
        active(&out).unwrap().cwd,
        Some(PathBuf::from("/tmp/explorer cwd"))
    );
    let out = output(&mut server, 3, PaneId(1), b"more output");
    assert_eq!(status_for(&out, ClientId(1)), None);
}

#[test]
fn a_non_utf8_cwd_is_published_as_none() {
    let (mut server, _) = setup();
    server.panes.get_mut(&PaneId(1)).unwrap().cwd =
        PathBuf::from(OsStr::from_bytes(b"/tmp/caf\xe9"));
    let out = output(&mut server, 2, PaneId(1), b"\x1b]133;A\x07");
    assert_eq!(
        active(&out),
        Some(ActivePane {
            pane: PaneId(1),
            cwd: None,
            at_prompt: true,
        })
    );
}

#[test]
fn prompt_marks_a_and_b_set_at_prompt_and_c_and_d_clear_it() {
    let (mut server, _) = setup();
    assert_eq!(
        at_prompt(&output(&mut server, 2, PaneId(1), b"\x1b]133;A\x07")),
        Some(true)
    );
    assert_eq!(
        at_prompt(&output(&mut server, 3, PaneId(1), b"\x1b]133;C\x07")),
        Some(false)
    );
    assert_eq!(
        at_prompt(&output(&mut server, 4, PaneId(1), b"\x1b]133;B\x07")),
        Some(true)
    );
    assert_eq!(
        at_prompt(&output(&mut server, 5, PaneId(1), b"\x1b]133;D;0\x07")),
        Some(false)
    );
    assert!(!server.panes[&PaneId(1)].at_prompt);
}

#[test]
fn the_alt_screen_and_copy_mode_hide_the_prompt() {
    let (mut server, _) = setup();
    output(&mut server, 2, PaneId(1), b"\x1b]133;A\x07");
    assert_eq!(
        at_prompt(&output(&mut server, 3, PaneId(1), b"\x1b[?1049h")),
        Some(false)
    );
    assert!(server.panes[&PaneId(1)].at_prompt);
    assert_eq!(
        at_prompt(&output(&mut server, 4, PaneId(1), b"\x1b[?1049l")),
        Some(true)
    );
    assert_eq!(
        at_prompt(&run(&mut server, 5, Command::CopyMode)),
        Some(false)
    );
    assert_eq!(
        at_prompt(&run(&mut server, 6, Command::Copy(CopyCommand::Cancel))),
        Some(true)
    );
}

#[test]
fn pane_focus_and_window_switches_change_the_published_pane() {
    let (mut server, _) = setup();
    output(
        &mut server,
        2,
        PaneId(1),
        b"\x1b]7;file://localhost/tmp/first\x07\x1b]133;A\x07",
    );
    let out = run(&mut server, 3, Command::Split(Axis::Vertical));
    let split = server.tree.active_pane(WindowId(1)).unwrap();
    assert_ne!(split, PaneId(1));
    let published = active(&out).unwrap();
    assert_eq!(published.pane, split);
    assert!(!published.at_prompt);
    let out = run(&mut server, 4, Command::SelectPane(PaneRef::Id(PaneId(1))));
    assert_eq!(
        active(&out),
        Some(ActivePane {
            pane: PaneId(1),
            cwd: Some(PathBuf::from("/tmp/first")),
            at_prompt: true,
        })
    );
    let out = run(&mut server, 5, Command::NewWindow);
    let window = server.clients[&ClientId(1)].view.window;
    let fresh = server.tree.active_pane(window).unwrap();
    assert_eq!(active(&out).unwrap().pane, fresh);
    let out = run(&mut server, 6, Command::SelectWindow(WindowRef::Index(1)));
    assert_eq!(active(&out).unwrap().pane, PaneId(1));
}

#[test]
fn each_client_publishes_the_pane_it_is_viewing() {
    let (mut server, _) = setup();
    from(&mut server, ClientId(2), 1, hello());
    let out = run(&mut server, 2, Command::NewWindow);
    let own = active(&out).unwrap();
    assert_ne!(own.pane, PaneId(1));
    let out = output(
        &mut server,
        3,
        PaneId(1),
        b"\x1b]7;file://localhost/tmp/second\x07",
    );
    assert_eq!(active(&out), Some(own));
    assert_eq!(
        status_for(&out, ClientId(2))
            .and_then(|line| line.pane.clone())
            .unwrap(),
        ActivePane {
            pane: PaneId(1),
            cwd: Some(PathBuf::from("/tmp/second")),
            at_prompt: false,
        }
    );
}

#[test]
fn explorer_is_echoed_only_to_the_requesting_client() {
    let (mut server, _) = setup();
    from(&mut server, ClientId(2), 1, hello());
    let mut out = from(&mut server, ClientId(1), 2, key('a', true));
    out.extend(from(&mut server, ClientId(1), 3, key('e', false)));
    out.extend(from(
        &mut server,
        ClientId(2),
        4,
        ClientMsg::Command(Command::Explorer),
    ));
    let echoed: Vec<_> = out
        .iter()
        .filter_map(|effect| {
            if let Effect::Send(client, ServerMsg::Explorer) = effect {
                Some(*client)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(echoed, [ClientId(1), ClientId(2)]);
    assert!(!out.iter().any(|effect| matches!(
        effect,
        Effect::Send(_, ServerMsg::Layout(_)) | Effect::ResizePty(..) | Effect::WritePty(..)
    )));
}

#[test]
fn split_run_spawns_the_program_side_by_side_in_the_given_cwd() {
    let (mut server, _) = setup();
    let out = run(&mut server, 2, edit("/tmp/a b/notes.md", "/tmp/a b"));
    let spawned = spawns(&out);
    assert_eq!(spawned.len(), 1);
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::Spawn(pane, _) if *pane == spawned[0].0))
    );
    let (pane, spec) = spawned[0];
    assert_eq!(spec.program, "/bin/sh");
    assert_eq!(
        spec.args,
        [
            "-c",
            "exec ${EDITOR:-vi} \"$1\"",
            "flair-edit",
            "/tmp/a b/notes.md"
        ]
    );
    assert_eq!(spec.cwd, PathBuf::from("/tmp/a b"));
    assert!(
        spec.env
            .iter()
            .any(|(key, value)| key == "FLAIR_PANE" && *value == format!("%{}", pane.0))
    );
    assert_eq!(server.panes[&pane].cwd, PathBuf::from("/tmp/a b"));
    let window = server.tree.window(WindowId(1)).unwrap();
    assert_eq!(window.active_pane, pane);
    let placements = window.geometry(0).panes;
    let left = placements.iter().find(|p| p.pane == PaneId(1)).unwrap();
    let right = placements.iter().find(|p| p.pane == pane).unwrap();
    assert_eq!(left.rect.origin.row, right.rect.origin.row);
    assert!(right.rect.origin.col > left.rect.origin.col);
}

#[test]
fn invalid_split_run_requests_are_ignored_without_an_error() {
    let (mut server, _) = setup();
    let ok = |argv: Vec<String>| Command::SplitRun {
        argv,
        cwd: PathBuf::from("/tmp"),
    };
    let requests = [
        ok(Vec::new()),
        ok(vec!["vi".to_owned(); 65]),
        ok(vec!["vi".to_owned(), "a\0b".to_owned()]),
        ok(vec!["vi".to_owned(), "x".repeat(4097)]),
        Command::SplitRun {
            argv: vec!["vi".to_owned()],
            cwd: PathBuf::from("tmp"),
        },
    ];
    for (now, request) in (2..).zip(requests) {
        let out = run(&mut server, now, request);
        assert!(spawns(&out).is_empty());
        assert_eq!(errors(&out), 0);
    }
    assert_eq!(server.panes.len(), 1);
    let out = run(&mut server, 9, ok(vec!["x".repeat(4096); 64]));
    assert_eq!(spawns(&out).len(), 1);
}

#[test]
fn a_split_run_into_a_pane_too_small_never_sends_an_error() {
    let (mut server, _) = setup();
    let mut spawned = 0;
    for now in 2..12 {
        let out = run(&mut server, now, edit("/tmp/f", "/tmp"));
        assert_eq!(errors(&out), 0);
        spawned += spawns(&out).len();
    }
    assert!(spawned < 10);
    assert_eq!(server.panes.len(), spawned + 1);
}

#[test]
fn a_split_run_that_cannot_start_closes_its_pane_without_an_error() {
    let (mut server, _) = setup();
    let out = run(&mut server, 2, edit("/gone/f", "/gone"));
    let (pane, _) = spawns(&out)[0];
    let mut out = Vec::new();
    server.handle(Millis(3), Event::PtySpawnFailed(pane), &mut out);
    assert_eq!(errors(&out), 0);
    assert!(
        out.iter()
            .any(|effect| matches!(effect, Effect::KillPty(killed) if *killed == pane))
    );
    assert!(!server.panes.contains_key(&pane));
    assert!(!server.runs_program(pane));
    assert_eq!(
        server.tree.window(WindowId(1)).unwrap().panes,
        vec![PaneId(1)]
    );
    let mut out = Vec::new();
    server.handle(Millis(4), Event::PtySpawnFailed(PaneId(1)), &mut out);
    assert_eq!(errors(&out), 1);
}
