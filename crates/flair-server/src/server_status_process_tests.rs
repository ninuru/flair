use super::*;

fn timer(server: &mut Server, now: u64, timer: Timer) -> Vec<Effect> {
    let mut out = Vec::new();
    server.handle(Millis(now), Event::Timer(timer), &mut out);
    out
}

fn process_timers(out: &[Effect]) -> Vec<(PaneId, Millis)> {
    out.iter()
        .filter_map(|effect| {
            if let Effect::SetTimer(Timer::Process(pane), due) = effect {
                Some((*pane, *due))
            } else {
                None
            }
        })
        .collect()
}

fn process_reads(out: &[Effect]) -> Vec<PaneId> {
    out.iter()
        .filter_map(|effect| {
            if let Effect::ReadProcess(pane) = effect {
                Some(*pane)
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn command_start_and_finish_rename_the_window_after_the_foreground_process() {
    let mut server = setup();
    let mut out = Vec::new();
    server.handle(
        Millis(10),
        Event::PtyOutput(PaneId(1), b"\x1b]133;C\x07".to_vec()),
        &mut out,
    );
    assert_eq!(process_timers(&out), [(PaneId(1), Millis(160))]);
    assert!(process_reads(&out).is_empty());
    let out = timer(&mut server, 160, Timer::Process(PaneId(1)));
    assert_eq!(process_reads(&out), [PaneId(1)]);
    let out = process_read(&mut server, 161, PaneId(1), &["/usr/bin/sleep", "60"]);
    let line = status(&out).unwrap();
    assert_eq!(line.windows[0].name, "sleep");
    assert_eq!(line.windows[0].program, "sleep");
    let mut out = Vec::new();
    server.handle(
        Millis(5000),
        Event::PtyOutput(PaneId(1), b"\x1b]133;D;0\x07".to_vec()),
        &mut out,
    );
    assert_eq!(process_timers(&out), [(PaneId(1), Millis(5150))]);
    let out = timer(&mut server, 5150, Timer::Process(PaneId(1)));
    assert_eq!(process_reads(&out), [PaneId(1)]);
    let out = process_read(&mut server, 5151, PaneId(1), &["-zsh"]);
    let line = status(&out).unwrap();
    assert_eq!(line.windows[0].name, "zsh");
    assert_eq!(line.windows[0].program, "zsh");
    let out = process_read(&mut server, 5152, PaneId(1), &["zsh", "-l"]);
    assert_eq!(status(&out), None);
}

#[test]
fn output_rechecks_the_process_at_most_once_per_second() {
    let mut server = setup();
    let mut out = Vec::new();
    for now in [1, 2, 3] {
        server.handle(
            Millis(now),
            Event::PtyOutput(PaneId(1), b"x".to_vec()),
            &mut out,
        );
    }
    assert_eq!(process_timers(&out), [(PaneId(1), Millis(151))]);
    assert_eq!(
        process_reads(&timer(&mut server, 151, Timer::Process(PaneId(1)))),
        [PaneId(1)]
    );
    assert!(process_reads(&timer(&mut server, 152, Timer::Process(PaneId(1)))).is_empty());
    let mut out = Vec::new();
    for now in [400, 900] {
        server.handle(
            Millis(now),
            Event::PtyOutput(PaneId(1), b"\x1b]0;title\x07".to_vec()),
            &mut out,
        );
    }
    assert_eq!(process_timers(&out), [(PaneId(1), Millis(1151))]);
    assert_eq!(
        process_reads(&timer(&mut server, 1151, Timer::Process(PaneId(1)))),
        [PaneId(1)]
    );
}

#[test]
fn exited_panes_are_not_probed() {
    let mut server = setup();
    let mut out = Vec::new();
    server.handle(Millis(1), Event::PtyExited(PaneId(1)), &mut out);
    server.handle(
        Millis(2),
        Event::PtyOutput(PaneId(1), b"x".to_vec()),
        &mut out,
    );
    assert!(process_timers(&out).is_empty());
}

#[test]
fn unknown_process_falls_back_to_the_program_name() {
    let mut server = setup();
    process_read(&mut server, 1, PaneId(1), &["htop"]);
    let out = process_read(&mut server, 2, PaneId(1), &[]);
    let line = status(&out).unwrap();
    assert_eq!(line.windows[0].name, "zsh");
    assert_eq!(line.windows[0].program, "zsh");
}
