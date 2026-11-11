use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

use super::*;
use crate::{
    chrome_layout::layout,
    font::{CellMetrics, UiMetrics},
    status::{ChipParts, StatusParts},
};
use flair_core::{
    config::{Config, PromptConfig},
    geom::CellPoint,
    ids::WindowId,
    isles::{AheadBehind, Changes, GitInfo, Head, LastCommand, PromptInfo, ShellFacts},
    proto::PanePlacement,
    time::Millis,
};

fn all_on(icons: bool) -> PromptConfig {
    PromptConfig {
        icons,
        date: true,
        path: flair_core::config::PromptPath::Abbreviated,
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
        Chip::Date => PromptConfig {
            date: false,
            ..config
        },
    }
}

fn labels(
    info: &PromptInfo,
    config: &PromptConfig,
    clock: Option<(u8, u8)>,
    home: Option<&Path>,
) -> Vec<Label> {
    labels_at(info, config, clock, clock.map(|_| (2026, 11, 10)), home)
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
            "2026-11-10",
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
            (Chip::User, Some('\u{f415}')),
            (Chip::Host, Some('\u{ea7a}')),
            (Chip::Cwd, Some('\u{f114}')),
            (Chip::GitBranch, Some('\u{f418}')),
            (Chip::GitAheadBehind, None),
            (Chip::GitChanges, Some('\u{f440}')),
            (Chip::Venv, Some('\u{f487}')),
            (Chip::Docker, Some('\u{f308}')),
            (Chip::Kube, Some('\u{f10fe}')),
            (Chip::Exit, None),
            (Chip::Time, Some('\u{f017}')),
            (Chip::Date, Some('\u{f073}')),
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
        date: false,
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

const DROP_ORDER: [Chip; 11] = [
    Chip::Date,
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
fn the_shortest_cwd_keeps_an_ellipsis_and_the_last_folder() {
    assert_eq!(shortest("/srv/projects/flair"), "…/flair");
    assert_eq!(shortest("~/src/flair"), "…/flair");
    assert_eq!(shortest("~/a"), "~/a");
    assert_eq!(shortest("~/ab"), "~/ab");
    assert_eq!(shortest("~/abc/d"), "…/d");
    assert_eq!(shortest("~"), "~");
    assert_eq!(shortest("/"), "/");
    assert_eq!(shortest("/a"), "/a");
    assert_eq!(shortest("/srv/日本"), "…/日本");
    assert_eq!(shortest(""), "");
}

fn cut_at(path: &str, chars: usize) -> String {
    let width = chars as f32 * 7.0;
    cut(path, |text| text.chars().count() as f32 * 7.0 <= width)
}

#[test]
fn the_cwd_drops_whole_leading_folders_before_cutting_characters() {
    let path = "/tmp/fm7-tFkw/repo";
    for (chars, expected) in [
        (40, "/tmp/fm7-tFkw/repo"),
        (18, "/tmp/fm7-tFkw/repo"),
        (17, "…/fm7-tFkw/repo"),
        (15, "…/fm7-tFkw/repo"),
        (14, "…/repo"),
        (6, "…/repo"),
        (5, "…repo"),
        (4, "…epo"),
        (3, "…po"),
        (2, "…o"),
        (1, ""),
        (0, ""),
    ] {
        assert_eq!(cut_at(path, chars), expected, "{chars}");
    }
}

#[test]
fn the_cwd_keeps_the_home_prefix_until_it_has_to_go() {
    let path = "~/src/flair/crates";
    for (chars, expected) in [
        (18, "~/src/flair/crates"),
        (17, "…/flair/crates"),
        (14, "…/flair/crates"),
        (13, "…/crates"),
        (8, "…/crates"),
        (7, "…crates"),
        (3, "…es"),
    ] {
        assert_eq!(cut_at(path, chars), expected, "{chars}");
    }
    assert_eq!(cut_at("~", 1), "~");
    assert_eq!(cut_at("~/a", 3), "~/a");
    assert_eq!(cut_at("~/abc", 4), "…abc");
    assert_eq!(cut_at("/srv/日本", 4), "…/日本");
    assert_eq!(cut_at("/srv/日本", 3), "…日本");
}

#[test]
fn the_cwd_cut_is_stable_while_the_width_stays_between_folder_boundaries() {
    let path = "/home/ada/IdeaProjects/flair/.claude/worktrees/repo";
    let cuts: Vec<String> = (16..=60).map(|chars| cut_at(path, chars)).collect();
    let mut distinct = cuts.clone();
    distinct.dedup();
    assert_eq!(
        distinct,
        [
            "…/worktrees/repo",
            "…/.claude/worktrees/repo",
            "…/flair/.claude/worktrees/repo",
            "…/IdeaProjects/flair/.claude/worktrees/repo",
            "…/ada/IdeaProjects/flair/.claude/worktrees/repo",
            "/home/ada/IdeaProjects/flair/.claude/worktrees/repo",
        ]
    );
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

#[test]
fn control_characters_never_reach_a_chip() {
    let mut info = full();
    info.shell = Some(ShellFacts {
        user: "a\u{1b}da\u{7}".into(),
        host: "box\n".into(),
        venv: None,
    });
    let config = all_on(false);
    assert_eq!(text_of(&info, &config, Chip::User).as_deref(), Some("ada"));
    assert_eq!(text_of(&info, &config, Chip::Host).as_deref(), Some("box"));
}

fn icon_ui() -> UiMetrics {
    UiMetrics {
        height: 14.0,
        ascent: 11.0,
        small: 0.5,
        cap: 8.0,
        icon: CellMetrics {
            width: 7.0,
            height: 12.0,
            baseline: 9.0,
        },
    }
}

fn prompt_scene(
    info: PromptInfo,
    cols: u16,
    row: u16,
    occupied: bool,
) -> crate::chrome_layout::ChromeLayout {
    let theme = Config::default().theme();
    let status = StatusParts {
        chip: ChipParts {
            session: "main".into(),
            hidden: 0,
            running: false,
            finished: false,
        },
        tabs: Vec::new(),
        badges: Vec::new(),
    };
    let window = WindowLayout {
        window: WindowId(1),
        area: CellRect {
            origin: CellPoint { col: 0, row: 0 },
            size: GridSize { cols, rows: 34 },
        },
        panes: vec![PanePlacement {
            pane: PaneId(1),
            rect: CellRect {
                origin: CellPoint { col: 0, row: 1 },
                size: GridSize { cols, rows: 33 },
            },
            band: 1,
        }],
        separators: Vec::new(),
        active: PaneId(1),
        zoomed: false,
    };
    let info = BTreeMap::from([(PaneId(1), info)]);
    let config = PromptConfig::default();
    let mut mirror = crate::mirror::Mirror::default();
    mirror.apply(&[flair_core::proto::PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols, rows: 33 },
        full: true,
        rows: if occupied && row > 0 {
            vec![(
                row - 1,
                vec![flair_core::proto::Cell {
                    c: 'X',
                    ..Default::default()
                }],
            )]
        } else {
            Vec::new()
        },
        cursor: flair_core::proto::Cursor {
            point: CellPoint { col: 0, row: 1 },
            shape: flair_core::proto::CursorShape::Block,
            visible: true,
        },
        flags: flair_core::proto::PaneFlags {
            prompt_row: Some(row),
            ..Default::default()
        },
        selection: Vec::new(),
    }]);
    let input = ChromeInput {
        px: (800, 600),
        scale: 1.0,
        padding: (8.0, 4.0),
        zen: false,
        metrics: CellMetrics {
            width: 8.0,
            height: 16.0,
            baseline: 12.0,
        },
        ui: icon_ui(),
        theme: &theme,
        status: &status,
        prompt: None,
        layout: Some(&window),
        hover: None,
        overlays: None,
        notice: None,
        isles: Some(Isles {
            info: &info,
            mirror: &mirror,
            config: &config,
            clock: None,
            date: None,
            home: None,
        }),
    };
    layout(&input, |text, _| text.chars().count() as f32 * 7.0)
}

fn icon_runs(info: PromptInfo, cols: u16) -> Vec<(f32, f32, String, TextFace)> {
    prompt_scene(info, cols, 0, false)
        .chrome
        .text
        .into_iter()
        .filter(|run| run.baseline >= 40.0)
        .map(|run| (run.x, run.baseline, run.text, run.face))
        .collect()
}

#[test]
fn icons_use_a_larger_regular_face_with_optical_spacing() {
    let info = PromptInfo {
        cwd: Some(PathBuf::from("/srv/one")),
        git: Some(GitInfo {
            head: Head::Branch("main".into()),
            upstream: None,
            changes: None,
        }),
        ..PromptInfo::default()
    };
    let runs = icon_runs(info, 98);
    assert_eq!(
        runs.iter().map(|run| run.2.as_str()).collect::<Vec<_>>(),
        ["\u{f114}", "/srv/one", "\u{f418}", "main"]
    );
    for pair in runs.as_chunks::<2>().0 {
        assert_eq!(pair[0].3, TextFace::PromptIcon);
        assert_eq!(pair[1].3, TextFace::Mono);
        assert_eq!(pair[1].0 - pair[0].0, 11.0 + ICON_GAP);
        assert_eq!(pair[0].1, pair[1].1 + 2.0);
    }
}

#[test]
fn icons_are_optically_centered_beside_grid_aligned_labels() {
    let runs = icon_runs(
        PromptInfo {
            cwd: Some(PathBuf::from("/srv/one")),
            ..PromptInfo::default()
        },
        98,
    );
    assert_eq!(runs[0].1, 82.0);
    assert_eq!(runs[1].1, 80.0);
}

#[test]
fn a_wide_icon_takes_two_small_cells() {
    let input_ui = icon_ui();
    assert_eq!(icon_cells('\u{f114}') as f32 * input_ui.icon.width, 7.0);
    assert_eq!(icon_cells('\u{1f40d}') as f32 * input_ui.icon.width, 14.0);
}

#[test]
fn outlined_chips_use_neutral_surfaces_and_readable_ansi_text() {
    let theme = Config::default().theme();
    let config = PromptConfig::default();
    for (chip, tone, index) in [
        (Chip::Cwd, Tone::Plain, 6),
        (Chip::Docker, Tone::Plain, 2),
        (Chip::GitBranch, Tone::Plain, 5),
        (Chip::Exit, Tone::Failed, 1),
    ] {
        let label = Label {
            chip,
            icon: None,
            text: "test".into(),
            tone,
        };
        let (fill, ink, border) = colours(&theme, &config, &label);
        assert_eq!(fill, blend(theme.background, theme.foreground, 4));
        let expected = match chip {
            Chip::Cwd => theme.bright[6],
            Chip::Docker => blend(theme.bright[2], theme.bright[6], 50),
            Chip::User
            | Chip::Host
            | Chip::GitBranch
            | Chip::GitAheadBehind
            | Chip::GitChanges
            | Chip::Venv
            | Chip::Kube
            | Chip::Exit
            | Chip::Time
            | Chip::Date => theme.normal[index],
        };
        assert_eq!(ink, expected);
        assert_eq!(border, Some(blend(theme.background, theme.foreground, 12)));
        assert!(contrast(fill, ink) >= 4.5, "{chip:?}: {fill:?} {ink:?}");
    }
}

#[test]
fn individual_segments_override_colour_appearance_and_icons() {
    let theme = Config::default().theme();
    let mut config = PromptConfig::default();
    let label = Label {
        chip: Chip::Cwd,
        icon: None,
        text: "/home/ada".into(),
        tone: Tone::Plain,
    };
    config.segments.insert(
        Chip::Cwd,
        flair_core::config::SegmentConfig {
            colour: Some(flair_core::config::AnsiColour::Yellow),
            appearance: Some(flair_core::config::PromptAppearance::Filled),
            icons: Some(false),
        },
    );
    let (fill, ink, border) = colours(&theme, &config, &label);
    assert_eq!(fill, theme.normal[3]);
    assert_eq!(border, Some(fill));
    assert!(contrast(fill, ink) >= 4.5);
    config.segments.get_mut(&Chip::Cwd).unwrap().appearance =
        Some(flair_core::config::PromptAppearance::Plain);
    let (fill, _, border) = colours(&theme, &config, &label);
    assert_eq!(fill, theme.background);
    assert_eq!(border, None);
}

#[test]
fn prompt_formats_paths_dates_and_times_from_editor_choices() {
    let mut config = all_on(false);
    config.path = flair_core::config::PromptPath::Full;
    config.time_format = flair_core::config::PromptTimeFormat::Twelve;
    config.date = true;
    let labels = labels_at(&full(), &config, Some((0, 7)), Some((2026, 11, 10)), home());
    let value = |chip| {
        labels
            .iter()
            .find(|label| label.chip == chip)
            .map(|label| label.text.as_str())
    };
    assert_eq!(value(Chip::Cwd), Some("/home/ada/src/flair"));
    assert_eq!(value(Chip::Time), Some("12:07 AM"));
    assert_eq!(value(Chip::Date), Some("2026-11-10"));
    config.path = flair_core::config::PromptPath::Folder;
    let labels = labels_at(
        &full(),
        &config,
        Some((13, 7)),
        Some((2026, 11, 10)),
        home(),
    );
    assert_eq!(
        labels
            .iter()
            .find(|label| label.chip == Chip::Cwd)
            .unwrap()
            .text,
        "flair"
    );
    assert_eq!(
        labels
            .iter()
            .find(|label| label.chip == Chip::Time)
            .unwrap()
            .text,
        "1:07 PM"
    );
}

#[test]
fn prompt_segment_order_and_icon_overrides_are_applied() {
    let mut config = all_on(true);
    config.order = "docker,cwd,git_branch".parse().unwrap();
    config.segments.insert(
        Chip::Docker,
        flair_core::config::SegmentConfig {
            icons: Some(false),
            ..Default::default()
        },
    );
    let labels = labels_at(&full(), &config, None, None, home());
    assert_eq!(
        labels
            .iter()
            .take(3)
            .map(|label| label.chip)
            .collect::<Vec<_>>(),
        [Chip::Docker, Chip::Cwd, Chip::GitBranch]
    );
    assert_eq!(labels[0].icon, None);
    assert_eq!(labels[0].text, "docker colima");
    assert_eq!(labels[1].icon, Some('\u{f114}'));
}

#[test]
fn outlined_chip_surfaces_stay_neutral_across_segment_colours() {
    let theme = Config::default().theme();
    let config = PromptConfig::default();
    let make = |chip| Label {
        chip,
        icon: None,
        text: "example".into(),
        tone: Tone::Plain,
    };
    let (directory, _, directory_border) = colours(&theme, &config, &make(Chip::Cwd));
    let (docker, _, docker_border) = colours(&theme, &config, &make(Chip::Docker));
    assert_eq!(directory, docker);
    assert_eq!(directory_border, docker_border);
    assert_eq!(directory, blend(theme.background, theme.foreground, 4));
    assert_eq!(
        directory_border,
        Some(blend(theme.background, theme.foreground, 12))
    );
}

#[test]
fn prompt_labels_are_strong_while_the_icons_keep_their_outline_weight() {
    let theme = Config::default().theme();
    let config = PromptConfig::default();
    let paint = Paint {
        theme: &theme,
        scale: 1.0,
        metrics: CellMetrics {
            width: 10.0,
            height: 24.0,
            baseline: 18.0,
        },
        config: &config,
    };
    let labels = [
        Label {
            chip: Chip::Cwd,
            icon: Some('\u{f114}'),
            text: "~".into(),
            tone: Tone::Plain,
        },
        Label {
            chip: Chip::Docker,
            icon: Some('\u{f308}'),
            text: "skynetx-prod".into(),
            tone: Tone::Plain,
        },
    ];
    let mut scene = Scene::default();
    let hits = paint.chips(
        &labels,
        PxRect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 24.0,
        },
        &mut scene,
    );
    assert!(
        scene
            .text
            .iter()
            .filter(|run| ["~", "skynetx-prod"].contains(&run.text.as_str()))
            .all(|run| run.bold)
    );
    assert!(
        scene
            .text
            .iter()
            .filter(|run| !["~", "skynetx-prod"].contains(&run.text.as_str()))
            .all(|run| !run.bold)
    );
    assert_eq!(hits[0].1.w, 40.0);
    assert_eq!(hits[1].1.x - hits[0].1.x - hits[0].1.w, 8.0);
}

#[test]
fn home_is_compact_by_default_and_full_path_remains_an_explicit_choice() {
    let info = PromptInfo {
        cwd: Some(PathBuf::from("/home/ada")),
        ..PromptInfo::default()
    };
    let mut config = PromptConfig::default();
    assert_eq!(text_of(&info, &config, Chip::Cwd).as_deref(), Some("~"));
    config.path = flair_core::config::PromptPath::Full;
    assert_eq!(
        text_of(&info, &config, Chip::Cwd).as_deref(),
        Some("/home/ada")
    );
}

#[test]
fn prompt_dividers_stay_inside_the_pane_and_above_the_chip_row() {
    let theme = Config::default().theme();
    let rect = PxRect {
        x: 480.0,
        y: 120.0,
        w: 320.0,
        h: 24.0,
    };
    let quad = divider(rect, 1.5, &theme);
    assert_eq!(
        quad.rect,
        PxRect {
            x: 480.0,
            y: 120.0,
            w: 320.0,
            h: 2.0
        }
    );
    assert_eq!(
        quad.color,
        opaque(blend(theme.background, theme.foreground, 12))
    );
    assert!(quad.border.is_none());
}

#[test]
fn prompt_dividers_skip_the_top_of_the_pane_and_publish_chip_hits() {
    let info = PromptInfo {
        cwd: Some(PathBuf::from("/srv/one")),
        ..Default::default()
    };
    let lines = |scene: &crate::chrome_layout::ChromeLayout| {
        scene
            .chrome
            .quads
            .iter()
            .filter(|quad| quad.rect.y >= 68.0 && quad.rect.h == 1.0 && quad.border.is_none())
            .count()
    };
    let reserved = prompt_scene(info.clone(), 98, 2, false);
    assert_eq!(lines(&reserved), 1);
    assert_eq!(lines(&prompt_scene(info.clone(), 98, 2, true)), 1);
    assert_eq!(lines(&prompt_scene(info.clone(), 98, 0, false)), 0);
    assert_eq!(lines(&prompt_scene(info.clone(), 98, 1, false)), 0);
    assert!(reserved.hits.iter().any(|hit| hit.target
        == HitTarget::Prompt {
            pane: PaneId(1),
            row: 2,
            chip: Chip::Cwd
        }));
}
