use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;
use flair_core::{
    config::PromptConfig,
    isles::{AheadBehind, Changes, GitInfo, Head, LastCommand, PromptInfo, ShellFacts},
    time::Millis,
};

fn all_on(icons: bool) -> PromptConfig {
    PromptConfig {
        icons,
        cwd: true,
        git_branch: true,
        git_ahead_behind: true,
        git_changes: true,
        exit: true,
        duration_ms: 2000,
        time: true,
        user: true,
        host: true,
        venv: true,
        docker: true,
        kube: true,
        ..PromptConfig::default()
    }
}

fn without(chip: Chip) -> PromptConfig {
    let config = all_on(false);
    match chip {
        Chip::User => PromptConfig {
            user: false,
            ..config
        },
        Chip::Host => PromptConfig {
            host: false,
            ..config
        },
        Chip::Cwd => PromptConfig {
            cwd: false,
            ..config
        },
        Chip::GitBranch => PromptConfig {
            git_branch: false,
            ..config
        },
        Chip::GitAheadBehind => PromptConfig {
            git_ahead_behind: false,
            ..config
        },
        Chip::GitChanges => PromptConfig {
            git_changes: false,
            ..config
        },
        Chip::Venv => PromptConfig {
            venv: false,
            ..config
        },
        Chip::Docker => PromptConfig {
            docker: false,
            ..config
        },
        Chip::Kube => PromptConfig {
            kube: false,
            ..config
        },
        Chip::Exit => PromptConfig {
            exit: false,
            ..config
        },
        Chip::Time => PromptConfig {
            time: false,
            ..config
        },
    }
}

fn full() -> PromptInfo {
    PromptInfo {
        cwd: Some(PathBuf::from("/home/ada/src/flair")),
        git: Some(GitInfo {
            head: Head::Branch("main".into()),
            upstream: Some(AheadBehind {
                ahead: 1,
                behind: 2,
            }),
            changes: Some(Changes {
                files: 3,
                added: 10,
                removed: 2,
            }),
        }),
        last: Some(LastCommand {
            exit: Some(1),
            duration: Millis(420),
        }),
        shell: Some(ShellFacts {
            user: "ada".into(),
            host: "box".into(),
            venv: Some("env".into()),
        }),
        docker: Some("colima".into()),
        kube: Some("prod".into()),
    }
}

fn home() -> Option<&'static Path> {
    Some(Path::new("/home/ada"))
}

fn texts(labels: &[Label]) -> Vec<&str> {
    labels.iter().map(|label| label.text.as_str()).collect()
}

fn chips(labels: &[Label]) -> Vec<Chip> {
    labels.iter().map(|label| label.chip).collect()
}

fn text_of(info: &PromptInfo, config: &PromptConfig, chip: Chip) -> Option<String> {
    labels(info, config, Some((15, 48)), home())
        .into_iter()
        .find(|label| label.chip == chip)
        .map(|label| label.text)
}

#[test]
fn every_chip_on_gives_labels_in_band_order() {
    let labels = labels(&full(), &all_on(false), Some((15, 48)), home());
    assert_eq!(chips(&labels), Chip::ALL);
    assert_eq!(
        texts(&labels),
        [
            "ada",
            "box",
            "~/src/flair",
            "main",
            "↑1 ↓2",
            "3 · +10 −2",
            "venv env",
            "docker colima",
            "kube prod",
            "1",
            "15:48",
        ]
    );
}

#[test]
fn a_disabled_chip_is_absent_and_the_others_stay() {
    for chip in Chip::ALL {
        let labels = labels(&full(), &without(chip), Some((15, 48)), home());
        let expected: Vec<Chip> = Chip::ALL.into_iter().filter(|c| *c != chip).collect();
        assert_eq!(chips(&labels), expected, "{chip:?}");
    }
}

#[test]
fn icons_on_put_a_terminal_glyph_before_every_chip_but_ahead_behind_and_exit() {
    let labels = labels(&full(), &all_on(true), Some((15, 48)), home());
    let icons: Vec<(Chip, Option<char>)> = labels
        .iter()
        .map(|label| (label.chip, label.icon))
        .collect();
    assert_eq!(
        icons,
        [
            (Chip::User, Some('\u{f2c0}')),
            (Chip::Host, Some('\u{f109}')),
            (Chip::Cwd, Some('\u{f114}')),
            (Chip::GitBranch, Some('\u{e725}')),
            (Chip::GitAheadBehind, None),
            (Chip::GitChanges, Some('\u{f0f6}')),
            (Chip::Venv, Some('\u{e73c}')),
            (Chip::Docker, Some('\u{e7b0}')),
            (Chip::Kube, Some('\u{f10fe}')),
            (Chip::Exit, None),
            (Chip::Time, Some('\u{f017}')),
        ]
    );
    assert_eq!(texts(&labels)[6..9], ["env", "colima", "prod"]);
}

#[test]
fn icons_off_give_no_glyphs() {
    let labels = labels(&full(), &all_on(false), Some((15, 48)), home());
    assert!(labels.iter().all(|label| label.icon.is_none()));
}

#[test]
fn cwd_abbreviates_the_home_directory() {
    let config = all_on(false);
    let at = |path: &str, home: Option<&Path>| {
        let info = PromptInfo {
            cwd: Some(PathBuf::from(path)),
            ..PromptInfo::default()
        };
        labels(&info, &config, None, home)
            .into_iter()
            .find(|label| label.chip == Chip::Cwd)
            .map(|label| label.text)
    };
    assert_eq!(at("/home/ada", home()).as_deref(), Some("~"));
    assert_eq!(at("/home/ada/src", home()).as_deref(), Some("~/src"));
    assert_eq!(
        at("/home/adam/src", home()).as_deref(),
        Some("/home/adam/src")
    );
    assert_eq!(at("/etc", home()).as_deref(), Some("/etc"));
    assert_eq!(at("/home/ada/src", None).as_deref(), Some("/home/ada/src"));
    assert!(labels(&PromptInfo::default(), &config, None, home()).is_empty());
}

#[test]
fn branch_shows_the_name_or_the_short_sha() {
    let config = all_on(false);
    let mut info = full();
    assert_eq!(
        text_of(&info, &config, Chip::GitBranch).as_deref(),
        Some("main")
    );
    info.git = Some(GitInfo {
        head: Head::Detached("a1b2c3d".into()),
        upstream: None,
        changes: None,
    });
    assert_eq!(
        text_of(&info, &config, Chip::GitBranch).as_deref(),
        Some("a1b2c3d")
    );
    assert_eq!(text_of(&info, &config, Chip::GitAheadBehind), None);
    assert_eq!(text_of(&info, &config, Chip::GitChanges), None);
    info.git = None;
    assert_eq!(text_of(&info, &config, Chip::GitBranch), None);
}

#[test]
fn ahead_behind_omits_zero_sides_and_caps_at_999() {
    let config = all_on(false);
    let with = |ahead, behind| {
        let mut info = full();
        if let Some(git) = &mut info.git {
            git.upstream = Some(AheadBehind { ahead, behind });
        }
        text_of(&info, &config, Chip::GitAheadBehind)
    };
    assert_eq!(with(1, 2).as_deref(), Some("↑1 ↓2"));
    assert_eq!(with(3, 0).as_deref(), Some("↑3"));
    assert_eq!(with(0, 4).as_deref(), Some("↓4"));
    assert_eq!(with(0, 0), None);
    assert_eq!(with(1000, 999).as_deref(), Some("↑999+ ↓999"));
}

#[test]
fn changes_show_files_then_line_counts() {
    let config = all_on(false);
    let with = |changes| {
        let mut info = full();
        if let Some(git) = &mut info.git {
            git.changes = changes;
        }
        text_of(&info, &config, Chip::GitChanges)
    };
    let changes = |files, added, removed| {
        Some(Changes {
            files,
            added,
            removed,
        })
    };
    assert_eq!(with(changes(3, 10, 2)).as_deref(), Some("3 · +10 −2"));
    assert_eq!(with(changes(1, 5, 0)).as_deref(), Some("1 · +5"));
    assert_eq!(with(changes(1, 0, 4)).as_deref(), Some("1 · −4"));
    assert_eq!(with(changes(2, 0, 0)).as_deref(), Some("2"));
    assert_eq!(with(changes(0, 0, 0)), None);
    assert_eq!(with(None), None);
}

fn exit(code: Option<i32>, ms: u64, threshold: u64) -> Option<(String, Tone)> {
    let info = PromptInfo {
        last: Some(LastCommand {
            exit: code,
            duration: Millis(ms),
        }),
        ..PromptInfo::default()
    };
    let config = PromptConfig {
        duration_ms: threshold,
        ..all_on(false)
    };
    labels(&info, &config, None, None)
        .into_iter()
        .find(|label| label.chip == Chip::Exit)
        .map(|label| (label.text, label.tone))
}

#[test]
fn exit_shows_code_and_duration_only_above_the_threshold() {
    assert_eq!(exit(Some(0), 1999, 2000), None);
    assert_eq!(exit(None, 100, 2000), None);
    assert_eq!(exit(Some(0), 2000, 2000), Some(("2.0s".into(), Tone::Ok)));
    assert_eq!(exit(None, 3000, 2000), Some(("3.0s".into(), Tone::Ok)));
    assert_eq!(exit(Some(1), 420, 2000), Some(("1".into(), Tone::Failed)));
    assert_eq!(
        exit(Some(1), 420, 400),
        Some(("1 · 0.42s".into(), Tone::Failed))
    );
    assert_eq!(exit(Some(0), 420, 0), Some(("0.42s".into(), Tone::Ok)));
    assert_eq!(
        exit(Some(130), 12_400, 2000),
        Some(("130 · 12.4s".into(), Tone::Failed))
    );
    let info = PromptInfo {
        last: None,
        ..PromptInfo::default()
    };
    assert!(labels(&info, &all_on(false), None, None).is_empty());
}

#[test]
fn durations_read_in_the_largest_useful_unit() {
    let text = |ms| exit(Some(0), ms, 0).map(|(text, _)| text);
    assert_eq!(text(420).as_deref(), Some("0.42s"));
    assert_eq!(text(999).as_deref(), Some("0.99s"));
    assert_eq!(text(3000).as_deref(), Some("3.0s"));
    assert_eq!(text(12_400).as_deref(), Some("12.4s"));
    assert_eq!(text(59_999).as_deref(), Some("59.9s"));
    assert_eq!(text(60_000).as_deref(), Some("1m 0s"));
    assert_eq!(text(252_000).as_deref(), Some("4m 12s"));
    assert_eq!(text(3_780_000).as_deref(), Some("1h 3m"));
}

#[test]
fn time_is_zero_padded_and_absent_without_a_clock() {
    let config = all_on(false);
    let at = |clock| {
        labels(&full(), &config, clock, home())
            .into_iter()
            .find(|label| label.chip == Chip::Time)
            .map(|label| label.text)
    };
    assert_eq!(at(Some((9, 5))).as_deref(), Some("09:05"));
    assert_eq!(at(Some((15, 48))).as_deref(), Some("15:48"));
    assert_eq!(at(None), None);
}

#[test]
fn context_chips_need_their_facts() {
    let config = all_on(false);
    let info = PromptInfo {
        shell: Some(ShellFacts {
            user: "ada".into(),
            host: "box".into(),
            venv: None,
        }),
        ..PromptInfo::default()
    };
    assert_eq!(
        chips(&labels(&info, &config, None, None)),
        [Chip::User, Chip::Host]
    );
}

#[test]
fn line_joins_labels_with_two_spaces() {
    let config = PromptConfig {
        user: false,
        host: false,
        venv: false,
        docker: false,
        kube: false,
        time: false,
        ..all_on(true)
    };
    let labels = labels(&full(), &config, None, home());
    assert_eq!(line(&labels), "~/src/flair  main  ↑1 ↓2  3 · +10 −2  1");
    assert_eq!(line(&[]), "");
}

fn widths() -> Vec<(Chip, f32)> {
    let width = |text: &str| text.chars().count() as f32 * 7.0 + 12.0;
    labels(&full(), &all_on(false), Some((15, 48)), home())
        .iter()
        .map(|label| (label.chip, width(&label.text)))
        .collect()
}

fn span(widths: &[(Chip, f32)], kept: &[Chip], gap: f32) -> f32 {
    let total: f32 = widths
        .iter()
        .filter(|(chip, _)| kept.contains(chip))
        .map(|(_, width)| width)
        .sum();
    total + gap * kept.len().saturating_sub(1) as f32
}

const DROP_ORDER: [Chip; 10] = [
    Chip::Time,
    Chip::Host,
    Chip::User,
    Chip::Kube,
    Chip::Docker,
    Chip::Venv,
    Chip::GitAheadBehind,
    Chip::GitChanges,
    Chip::Exit,
    Chip::GitBranch,
];

fn kept_after(dropped: usize) -> Vec<Chip> {
    Chip::ALL
        .into_iter()
        .filter(|chip| !DROP_ORDER[..dropped].contains(chip))
        .collect()
}

#[test]
fn fit_keeps_everything_that_fits() {
    let widths = widths();
    let room = span(&widths, &Chip::ALL, 6.0);
    assert_eq!(fit(&widths, room, 6.0), Chip::ALL);
    assert_eq!(fit(&widths, room - 1.0, 6.0), kept_after(1));
}

#[test]
fn fit_drops_chips_in_order_as_the_room_shrinks() {
    let widths = widths();
    for dropped in 1..DROP_ORDER.len() {
        let expected = kept_after(dropped);
        let room = span(&widths, &expected, 6.0);
        assert_eq!(fit(&widths, room, 6.0), expected, "room {room}");
        assert_eq!(
            fit(&widths, room - 1.0, 6.0),
            kept_after(dropped + 1),
            "room {}",
            room - 1.0
        );
    }
}

#[test]
fn fit_never_drops_the_cwd() {
    let widths = widths();
    assert_eq!(fit(&widths, 10.0, 6.0), [Chip::Cwd]);
    assert_eq!(fit(&widths, 0.0, 6.0), [Chip::Cwd]);
    let without_cwd: Vec<(Chip, f32)> = widths
        .into_iter()
        .filter(|(chip, _)| *chip != Chip::Cwd)
        .collect();
    assert_eq!(fit(&without_cwd, 10.0, 6.0), []);
    assert_eq!(fit(&[], 100.0, 6.0), []);
}

#[test]
fn the_shortest_cwd_keeps_an_ellipsis_and_the_last_component() {
    assert_eq!(shortest("/srv/projects/flair"), "…flair");
    assert_eq!(shortest("~/src/flair"), "…flair");
    assert_eq!(shortest("~/a"), "…a");
    assert_eq!(shortest("~"), "~");
    assert_eq!(shortest("/"), "/");
    assert_eq!(shortest("/a"), "/a");
    assert_eq!(shortest("/srv/日本"), "…日本");
    assert_eq!(shortest(""), "");
}

#[test]
fn the_next_minute_is_the_rest_of_the_current_one() {
    assert_eq!(
        next_minute(Duration::from_secs(120)),
        Duration::from_secs(60)
    );
    assert_eq!(
        next_minute(Duration::from_millis(125_500)),
        Duration::from_millis(54_500)
    );
    assert_eq!(
        next_minute(Duration::from_secs(179)),
        Duration::from_secs(1)
    );
}

#[test]
fn the_local_clock_reads_a_valid_time() {
    let (hour, minute) = local_clock().expect("localtime");
    assert!(hour < 24 && minute < 60);
}
