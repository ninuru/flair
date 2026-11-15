use super::*;
use crate::{
    CellMetrics,
    chrome_layout::{BarField, ChromeLayout, Dock, frame, hit_test, layout},
    notice::Notice,
    panels::Panels,
    status::{ChipParts, StatusParts},
};
use flair_core::{
    config::{Config, IconSide},
    geom::{CellRect, GridSize},
    ids::{PaneId, WindowId},
    proto::WindowLayout,
};

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn parts() -> StatusParts {
    StatusParts {
        tab_icon: Some(IconSide::After),
        chip: ChipParts {
            session: "main".into(),
            hidden: 0,
            running: false,
            finished: false,
        },
        tabs: vec![
            TabParts {
                window: WindowId(1),
                index: 1,
                icon: None,
                name: "shell".into(),
                active: true,
                last: false,
                activity: false,
                bell: false,
            },
            TabParts {
                window: WindowId(2),
                index: 2,
                icon: None,
                name: "editor".into(),
                active: false,
                last: false,
                activity: false,
                bell: false,
            },
        ],
        badges: Vec::new(),
        agents: Vec::new(),
    }
}

fn zoomed() -> WindowLayout {
    WindowLayout {
        window: WindowId(1),
        area: CellRect {
            origin: Default::default(),
            size: GridSize { cols: 98, rows: 34 },
        },
        panes: Vec::new(),
        separators: Vec::new(),
        active: PaneId(1),
        zoomed: true,
    }
}

struct Case<'a> {
    scale: f32,
    width: u32,
    status: &'a StatusParts,
    label: &'a str,
    prompt: Option<&'a str>,
    cursor: Option<usize>,
    window: Option<&'a WindowLayout>,
    hover: Option<(f32, f32)>,
    notice: Option<&'a Notice>,
    panels: Option<&'a Panels>,
    dock: Dock,
}

impl<'a> Case<'a> {
    fn new(status: &'a StatusParts) -> Case<'a> {
        Case {
            scale: 1.0,
            width: 800,
            status,
            label: "",
            prompt: None,
            cursor: None,
            window: None,
            hover: None,
            notice: None,
            panels: None,
            dock: Dock::default(),
        }
    }

    fn run(&self) -> ChromeLayout {
        let theme = Config::default().theme();
        let scale = self.scale;
        let input = ChromeInput {
            px: (self.width * scale as u32, 600 * scale as u32),
            scale,
            padding: (8.0 * scale, 4.0 * scale),
            zen: false,
            metrics: CellMetrics {
                width: 8.0 * scale,
                height: 16.0 * scale,
                baseline: 12.0 * scale,
            },
            ui: UiMetrics {
                height: 14.0 * scale,
                ascent: 11.0 * scale,
                small: 0.5,
                cap: 8.0 * scale,
                icon: CellMetrics {
                    width: 7.0 * scale,
                    height: 12.0 * scale,
                    baseline: 9.0 * scale,
                },
            },
            theme: &theme,
            status: self.status,
            prompt: self.prompt.map(|text| BarField {
                label: self.label,
                text,
                cursor: self.cursor.unwrap_or_else(|| text.chars().count()),
            }),
            layout: self.window,
            hover: self.hover,
            overlays: None,
            notice: self.notice,
            isles: None,
            frame: frame(
                (self.width * scale as u32, 600 * scale as u32),
                scale,
                false,
                (8.0 * scale, 4.0 * scale),
                CellMetrics {
                    width: 8.0 * scale,
                    height: 16.0 * scale,
                    baseline: 12.0 * scale,
                },
                self.dock,
            ),
            panels: self.panels,
        };
        layout(&input, measure)
    }
}

fn text<'a>(layout: &'a ChromeLayout, text: &str) -> Option<&'a TextRun> {
    layout.chrome.text.iter().find(|run| run.text == text)
}

fn quad_at(layout: &ChromeLayout, rect: PxRect) -> Option<&Quad> {
    layout.chrome.quads.iter().find(|quad| quad.rect == rect)
}

fn copy_and_prefix() -> StatusParts {
    StatusParts {
        badges: vec![
            BadgeParts {
                kind: BadgeKind::Copy,
                text: "COPY".into(),
            },
            BadgeParts {
                kind: BadgeKind::Prefix,
                text: "^A".into(),
            },
        ],
        ..parts()
    }
}

#[test]
fn bar_and_a_faint_line_span_the_window_top() {
    let theme = Config::default().theme();
    for scale in [1.0, 2.0] {
        let status = parts();
        let layout = Case {
            scale,
            ..Case::new(&status)
        }
        .run();
        assert_eq!(
            layout.chrome.quads[0],
            fill(
                rect(0.0, 0.0, 800.0 * scale, 34.0 * scale),
                theme.background
            )
        );
        assert_eq!(
            layout.chrome.quads[1],
            fill(
                rect(0.0, 34.0 * scale, 800.0 * scale, scale),
                theme.surfaces().surface1
            )
        );
        assert!(layout.chrome.modal.is_empty());
        assert!(layout.chrome.modal_text.is_empty());
        assert_eq!(layout.overlay, None);
    }
}

fn tab<'a>(
    layout: &'a ChromeLayout,
    index: &str,
    name: &str,
) -> Option<(&'a TextRun, &'a TextRun)> {
    layout
        .chrome
        .text
        .windows(2)
        .find(|pair| pair[0].text == index && pair[1].text == name)
        .map(|pair| (&pair[0], &pair[1]))
}

#[test]
fn the_session_is_a_breadcrumb_with_a_muted_slash_before_the_tabs() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let status = parts();
    let layout = Case::new(&status).run();
    let name = text(&layout, "main").unwrap();
    assert_eq!(
        (name.x, name.baseline, name.color, name.bold, name.face),
        (12.0, 21.0, theme.foreground, false, TextFace::Ui)
    );
    let crumb = text(&layout, CRUMB_SLASH).unwrap();
    assert_eq!(
        (crumb.x, crumb.baseline, crumb.color, crumb.bold, crumb.face),
        (48.0, 21.0, surfaces.muted, false, TextFace::Ui)
    );
    let (index, _) = tab(&layout, "1", "shell").unwrap();
    assert_eq!(index.x, 71.0);
    let order: Vec<&str> = layout
        .chrome
        .text
        .iter()
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(order[..3], ["main", CRUMB_SLASH, "1"]);
    assert_eq!(layout.chrome.quads.len(), 3);

    let layout = Case {
        scale: 2.0,
        ..Case::new(&status)
    }
    .run();
    assert_eq!(text(&layout, "main").unwrap().x, 24.0);
    assert_eq!(text(&layout, CRUMB_SLASH).unwrap().x, 68.0);
}

#[test]
fn session_item_shows_only_the_name_and_the_crumb() {
    let mut status = parts();
    status.chip.hidden = 2;
    status.chip.running = true;
    status.chip.finished = true;
    let layout = Case::new(&status).run();
    assert!(text(&layout, "+2").is_none());
    assert!(layout.chrome.text.iter().all(|run| run.text != "!"));
    assert!(layout.chrome.quads.iter().all(|quad| quad.rect.w != 6.0));
    assert_eq!(layout.hits[0].rect, rect(12.0, 0.0, 43.0, 34.0));
}

#[test]
fn long_session_names_are_ellipsized() {
    let mut long = parts();
    long.chip.session = "x".repeat(40);
    let layout = Case::new(&long).run();
    let expected = format!("{}…", "x".repeat(24));
    assert!(text(&layout, &expected).is_some());
}

#[test]
fn the_active_tab_is_a_tinted_pill_without_an_underline() {
    let theme = Config::default().theme();
    for scale in [1.0, 2.0] {
        let status = parts();
        let layout = Case {
            scale,
            ..Case::new(&status)
        }
        .run();
        let tab_rect = layout.hits[1].rect;
        assert!(
            layout
                .chrome
                .quads
                .iter()
                .all(|quad| quad.color != opaque(theme.accent))
        );
        let pill = rect(tab_rect.x, 5.0 * scale, tab_rect.w, 24.0 * scale);
        let quad = quad_at(&layout, pill).unwrap();
        assert_eq!(quad.color, opaque(tint(&theme, 8)));
        assert_eq!(quad.radius, 6.0 * scale);
        assert_eq!(layout.chrome.quads.len(), 3);
    }
    let status = parts();
    let layout = Case::new(&status).run();
    assert_eq!(layout.hits[1].rect, rect(63.0, 0.0, 65.0, 34.0));
}

#[test]
fn tab_index_is_faint_inactive_names_muted_and_the_active_name_fg() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut status = parts();
    status.tabs.push(TabParts {
        window: WindowId(3),
        index: 3,
        icon: None,
        name: "zsh".into(),
        active: false,
        last: true,
        activity: false,
        bell: false,
    });
    let layout = Case::new(&status).run();
    for (index, name, x, name_x, color) in [
        ("1", "shell", 71.0, 85.0, theme.foreground),
        ("2", "editor", 140.0, 154.0, surfaces.muted),
        ("3", "zsh", 216.0, 230.0, surfaces.muted),
    ] {
        let (index_run, name_run) = tab(&layout, index, name).unwrap();
        assert_eq!(
            (index_run.x, index_run.color, index_run.bold),
            (x, surfaces.faint, false)
        );
        assert_eq!(
            (name_run.x, name_run.color, name_run.bold),
            (name_x, color, false)
        );
        assert_eq!(index_run.baseline, name_run.baseline);
    }
}

#[test]
fn tab_hits_span_the_full_bar_height() {
    let status = parts();
    let layout = Case::new(&status).run();
    assert_eq!(
        layout.hits,
        [
            Hit {
                rect: rect(12.0, 0.0, 43.0, 34.0),
                target: HitTarget::Session,
            },
            Hit {
                rect: rect(63.0, 0.0, 65.0, 34.0),
                target: HitTarget::Window(WindowId(1)),
            },
            Hit {
                rect: rect(132.0, 0.0, 72.0, 34.0),
                target: HitTarget::Window(WindowId(2)),
            },
            Hit {
                rect: rect(760.0, 0.0, 28.0, 34.0),
                target: HitTarget::ExplorerButton,
            },
        ]
    );
    assert_eq!(
        hit_test(&layout.hits, (160.0, 1.0)),
        Some(HitTarget::Window(WindowId(2)))
    );
    assert_eq!(
        hit_test(&layout.hits, (160.0, 33.0)),
        Some(HitTarget::Window(WindowId(2)))
    );
    assert_eq!(
        hit_test(&layout.hits, (15.0, 33.0)),
        Some(HitTarget::Session)
    );
    assert_eq!(hit_test(&layout.hits, (130.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (58.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (160.0, 34.0)), None);
}

#[test]
fn hovering_an_inactive_tab_fills_its_pill_and_brightens_its_name() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let status = parts();
    let hovered = |hover| {
        Case {
            hover: Some(hover),
            ..Case::new(&status)
        }
        .run()
    };
    let name_color = |layout: &ChromeLayout, index, name| tab(layout, index, name).unwrap().1.color;
    for (hover, quads) in [
        ((160.0, 30.0), 4),
        ((10.0, 20.0), 3),
        ((100.0, 2.0), 3),
        ((400.0, 20.0), 3),
        ((160.0, 100.0), 3),
    ] {
        let layout = hovered(hover);
        assert_eq!(layout.chrome.quads.len(), quads, "{hover:?}");
        assert_eq!(text(&layout, "main").unwrap().color, theme.foreground);
        assert_eq!(name_color(&layout, "1", "shell"), theme.foreground);
        assert_eq!(tab(&layout, "2", "editor").unwrap().0.color, surfaces.faint);
    }
    let layout = hovered((160.0, 30.0));
    assert_eq!(name_color(&layout, "2", "editor"), theme.foreground);
    assert_eq!(
        quad_at(&layout, rect(132.0, 5.0, 72.0, 24.0))
            .unwrap()
            .color,
        opaque(tint(&theme, 4))
    );
    assert_eq!(
        name_color(&hovered((160.0, 100.0)), "2", "editor"),
        surfaces.muted
    );
}

fn many(count: u16, active: u16) -> StatusParts {
    StatusParts {
        tabs: (1..=count)
            .map(|index| TabParts {
                window: WindowId(u32::from(index)),
                index,
                icon: None,
                name: "shell".into(),
                active: index == active,
                last: false,
                activity: false,
                bell: false,
            })
            .collect(),
        ..parts()
    }
}

#[test]
fn overflow_counts_hidden_tabs_on_the_side_where_they_are_hidden() {
    let surfaces = Config::default().theme().surfaces();
    for (count, active, width, shown, label, labels) in [
        (
            30,
            30,
            291,
            vec![29, 30],
            ("30", "shell"),
            vec![(71.0, "+28")],
        ),
        (30, 30, 191, vec![30], ("30", "shell"), vec![(71.0, "+29")]),
        (30, 30, 151, vec![30], ("30", "shell"), vec![]),
        (30, 1, 291, vec![1, 2], ("1", "shell"), vec![(209.0, "+28")]),
        (30, 30, 140, vec![30], ("30", "sh…"), vec![]),
        (
            30,
            15,
            291,
            vec![15],
            ("15", "shell"),
            vec![(71.0, "+14"), (188.0, "+15")],
        ),
        (3, 1, 791, vec![1, 2, 3], ("3", "shell"), vec![]),
    ] {
        let width = width + 30;
        let layout = Case {
            width,
            ..Case::new(&many(count, active))
        }
        .run();
        let windows: Vec<HitTarget> = shown
            .into_iter()
            .map(|window| HitTarget::Window(WindowId(window)))
            .collect();
        let targets: Vec<HitTarget> = layout.hits[1..layout.hits.len() - 1]
            .iter()
            .map(|hit| hit.target)
            .collect();
        assert_eq!(targets, windows, "{active} {width}");
        assert!(tab(&layout, label.0, label.1).is_some(), "{active} {width}");
        let more: Vec<&TextRun> = layout
            .chrome
            .text
            .iter()
            .filter(|run| run.text.starts_with('+'))
            .collect();
        let found: Vec<(f32, &str)> = more.iter().map(|run| (run.x, run.text.as_str())).collect();
        assert_eq!(found, labels, "{active} {width}");
        for run in more {
            assert_eq!((run.color, run.bold), (surfaces.muted, false));
            for x in [run.x, run.x + measure(&run.text, false)] {
                assert_eq!(hit_test(&layout.hits, (f64::from(x), 10.0)), None);
            }
        }
        for hit in &layout.hits[1..] {
            assert!(hit.rect.x + hit.rect.w <= width as f32 - 8.0);
        }
    }
}

#[test]
fn copy_is_an_accent_tinted_badge_and_the_prefix_a_neutral_one_at_the_far_right() {
    let theme = Config::default().theme();
    let status = copy_and_prefix();
    let layout = Case::new(&status).run();
    assert_eq!(
        quad_at(&layout, rect(734.0, 9.0, 22.0, 16.0)),
        Some(&Quad {
            rect: rect(734.0, 9.0, 22.0, 16.0),
            color: opaque(blend(theme.background, theme.accent, 16)),
            radius: 4.0,
            border: None,
        })
    );
    let copy = text(&layout, "COPY").unwrap();
    assert_eq!(
        (copy.x, copy.color, copy.bold, copy.face),
        (738.0, theme.accent, false, TextFace::Small)
    );
    assert_eq!(
        quad_at(&layout, rect(712.0, 9.0, 16.0, 16.0)),
        Some(&Quad {
            rect: rect(712.0, 9.0, 16.0, 16.0),
            color: opaque(tint(&theme, 10)),
            radius: 4.0,
            border: None,
        })
    );
    let prefix = text(&layout, "^A").unwrap();
    assert_eq!(
        (prefix.x, prefix.color, prefix.bold, prefix.face),
        (716.0, theme.foreground, false, TextFace::Small)
    );
    assert!(text(&layout, "ZOOM").is_none());

    let layout = Case {
        scale: 2.0,
        ..Case::new(&status)
    }
    .run();
    assert!(quad_at(&layout, rect(1480.0, 18.0, 32.0, 32.0)).is_some());
    assert_eq!(text(&layout, "COPY").unwrap().x, 1488.0);
}

#[test]
fn zoom_chip_sits_between_copy_and_the_prefix_when_zoomed() {
    let theme = Config::default().theme();
    let status = copy_and_prefix();
    let window = zoomed();
    let layout = Case {
        window: Some(&window),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(
        quad_at(&layout, rect(706.0, 9.0, 22.0, 16.0)),
        Some(&Quad {
            rect: rect(706.0, 9.0, 22.0, 16.0),
            color: opaque(blend(theme.background, theme.accent, 16)),
            radius: 4.0,
            border: None,
        })
    );
    let zoom = text(&layout, "ZOOM").unwrap();
    assert_eq!(
        (zoom.x, zoom.color, zoom.bold, zoom.face),
        (710.0, theme.accent, false, TextFace::Small)
    );
    assert_eq!(text(&layout, "^A").unwrap().x, 688.0);

    let unzoomed = WindowLayout {
        zoomed: false,
        ..zoomed()
    };
    let layout = Case {
        window: Some(&unzoomed),
        ..Case::new(&status)
    }
    .run();
    assert!(text(&layout, "ZOOM").is_none());
}

fn field_label(layout: &ChromeLayout) -> Option<&TextRun> {
    layout
        .chrome
        .text
        .iter()
        .find(|run| run.text == "/" && run.x > 60.0)
}

#[test]
fn prompt_replaces_the_tabs_with_a_field_and_a_caret() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let status = parts();
    let layout = Case {
        label: "/",
        prompt: Some("foo"),
        ..Case::new(&status)
    }
    .run();
    assert!(text(&layout, "1 shell").is_none());
    assert!(text(&layout, "2 editor").is_none());
    assert_eq!(
        quad_at(&layout, rect(63.0, 5.0, 693.0, 24.0)),
        Some(&Quad {
            rect: rect(63.0, 5.0, 693.0, 24.0),
            color: opaque(tint(&theme, 4)),
            radius: 6.0,
            border: Some(Border {
                color: opaque(surfaces.outline),
                width: 1.0,
            }),
        })
    );
    let label = field_label(&layout).unwrap();
    assert_eq!(
        (label.x, label.baseline, label.color),
        (73.0, 21.0, surfaces.muted)
    );
    let prompt = text(&layout, "foo").unwrap();
    assert_eq!(
        (prompt.x, prompt.baseline, prompt.color),
        (80.0, 21.0, theme.foreground)
    );
    assert_eq!(
        quad_at(&layout, rect(101.0, 10.0, 1.0, 14.0)),
        Some(&fill(rect(101.0, 10.0, 1.0, 14.0), theme.accent))
    );
    assert_eq!(
        layout.hits.iter().map(|hit| hit.target).collect::<Vec<_>>(),
        [HitTarget::Session, HitTarget::ExplorerButton]
    );
}

#[test]
fn the_caret_follows_the_prompt_cursor() {
    let theme = Config::default().theme();
    let status = parts();
    let layout = Case {
        label: "/",
        prompt: Some("foo"),
        cursor: Some(1),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(field_label(&layout).map(|run| run.x), Some(73.0));
    assert_eq!(text(&layout, "foo").map(|run| run.x), Some(80.0));
    assert_eq!(
        quad_at(&layout, rect(87.0, 10.0, 1.0, 14.0)),
        Some(&fill(rect(87.0, 10.0, 1.0, 14.0), theme.accent))
    );
    let prompt = format!("{}end", "x".repeat(200));
    let layout = Case {
        label: "/",
        prompt: Some(&prompt),
        cursor: Some(0),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(field_label(&layout).map(|run| run.x), Some(73.0));
    let run = layout.chrome.text.iter().find(|run| run.x == 80.0).unwrap();
    assert!(run.text.starts_with('x') && run.text.ends_with('…'));
    assert_eq!(
        quad_at(&layout, rect(80.0, 10.0, 1.0, 14.0)).map(|quad| quad.color),
        Some(opaque(theme.accent))
    );
}

#[test]
fn long_prompts_show_their_tail_inside_the_field() {
    let theme = Config::default().theme();
    let status = StatusParts {
        badges: vec![BadgeParts {
            kind: BadgeKind::Copy,
            text: "COPY".into(),
        }],
        ..parts()
    };
    let prompt = format!("{}end", "x".repeat(200));
    let caret = |layout: &ChromeLayout| {
        layout
            .chrome
            .quads
            .iter()
            .find(|quad| quad.color == opaque(theme.accent) && quad.rect.w == 1.0)
            .map(|quad| quad.rect)
    };
    let layout = Case {
        label: "/",
        prompt: Some(&prompt),
        ..Case::new(&status)
    }
    .run();
    let field_end = 760.0;
    assert_eq!(field_label(&layout).map(|run| run.x), Some(73.0));
    let run = layout.chrome.text.iter().find(|run| run.x == 80.0).unwrap();
    assert!(run.text.starts_with('…') && run.text.ends_with("end"));
    assert!(run.x + measure(&run.text, false) <= field_end - 10.0);
    assert!(caret(&layout).unwrap().x <= field_end - 10.0);

    let layout = Case {
        width: 131,
        label: "/",
        prompt: Some(&prompt),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(caret(&layout), None);
    assert!(
        layout
            .chrome
            .text
            .iter()
            .all(|run| (run.text != "/" || run.x < 60.0) && !run.text.ends_with("end"))
    );
}

#[test]
fn ellipsize_measures_a_long_name_in_one_pass() {
    let name = "x".repeat(10_000);
    let mut calls = 0;
    let mut chars = 0;
    let result = ellipsize(&name, 160.0, false, &mut |text: &str, bold| {
        calls += 1;
        chars += text.chars().count();
        measure(text, bold)
    });
    assert_eq!(result, format!("{}…", "x".repeat(21)));
    assert!(calls <= 2 * name.len(), "{calls} calls");
    assert!(chars <= 2 * name.len(), "{chars} chars measured");
}

fn notice_dot(layout: &ChromeLayout, x: f32) -> Option<&Quad> {
    quad_at(layout, rect(x, 14.0, 6.0, 6.0))
}

#[test]
fn an_info_notice_replaces_the_tabs_in_muted_text_after_a_dot() {
    let surfaces = Config::default().theme().surfaces();
    let status = parts();
    let notice = Notice::info("config reloaded");
    let layout = Case {
        notice: Some(&notice),
        ..Case::new(&status)
    }
    .run();
    assert!(text(&layout, "1 shell").is_none());
    assert!(text(&layout, "2 editor").is_none());
    assert_eq!(
        notice_dot(&layout, 73.0),
        Some(&rounded(
            rect(73.0, 14.0, 6.0, 6.0),
            surfaces.muted,
            3.0,
            1.0
        ))
    );
    let run = text(&layout, "config reloaded").unwrap();
    assert_eq!(
        (run.x, run.baseline, run.color, run.bold, run.face),
        (85.0, 21.0, surfaces.muted, false, TextFace::Ui)
    );
    assert_eq!(
        layout.hits.iter().map(|hit| hit.target).collect::<Vec<_>>(),
        [HitTarget::Session, HitTarget::ExplorerButton]
    );
}

#[test]
fn an_error_notice_has_an_attention_dot_and_muted_ellipsized_text() {
    let surfaces = Config::default().theme().surfaces();
    let status = parts();
    let notice = Notice::error("could not save theme: permission denied (os error 13)");
    let layout = Case {
        width: 300,
        notice: Some(&notice),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(
        notice_dot(&layout, 73.0).map(|quad| quad.color),
        Some(opaque(surfaces.attention))
    );
    let run = layout
        .chrome
        .text
        .iter()
        .find(|run| run.text.starts_with("could not"))
        .unwrap();
    assert_eq!((run.x, run.color), (85.0, surfaces.muted));
    assert!(run.text.ends_with(ELLIPSIS));
    assert!(run.x + measure(&run.text, false) <= 292.0);
}

#[test]
fn a_notice_sits_right_of_an_open_prompt_field() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let status = parts();
    let notice = Notice::error("window name cannot be empty");
    let layout = Case {
        label: "(rename-window) ",
        prompt: Some(""),
        notice: Some(&notice),
        ..Case::new(&status)
    }
    .run();
    let field = layout
        .chrome
        .quads
        .iter()
        .find(|quad| quad.border.is_some())
        .unwrap();
    assert_eq!(field.rect, rect(63.0, 5.0, 486.0, 24.0));
    assert_eq!(
        notice_dot(&layout, 555.0).map(|quad| quad.color),
        Some(opaque(surfaces.attention))
    );
    let run = text(&layout, "window name cannot be empty").unwrap();
    assert_eq!((run.x, run.color), (567.0, surfaces.muted));
    let label = text(&layout, "(rename-window) ").unwrap();
    assert_eq!((label.x, label.color), (73.0, surfaces.muted));
}

#[test]
fn a_notice_next_to_a_prompt_keeps_the_field_at_least_120_wide() {
    let status = parts();
    let notice = Notice::error("window name cannot be empty");
    let field_rect = |layout: &ChromeLayout| {
        layout
            .chrome
            .quads
            .iter()
            .find(|quad| quad.border.is_some())
            .unwrap()
            .rect
    };
    let layout = Case {
        width: 321,
        label: "(rename-window) ",
        prompt: Some(""),
        notice: Some(&notice),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(field_rect(&layout), rect(63.0, 5.0, 120.0, 24.0));
    assert_eq!(text(&layout, "window na…").unwrap().x, 201.0);

    let layout = Case {
        width: 241,
        label: "(rename-window) ",
        prompt: Some(""),
        notice: Some(&notice),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(field_rect(&layout), rect(63.0, 5.0, 134.0, 24.0));
    assert!(
        layout
            .chrome
            .text
            .iter()
            .all(|run| !run.text.starts_with("window"))
    );
}

#[path = "chrome_bar_icon_tests.rs"]
mod icons;

#[path = "chrome_bar_button_tests.rs"]
mod button;
