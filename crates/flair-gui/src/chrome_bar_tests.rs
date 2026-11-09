use super::*;
use crate::{
    CellMetrics,
    chrome_layout::{BarField, ChromeLayout, hit_test, layout},
    notice::Notice,
    status::{ChipParts, StatusParts},
};
use flair_core::{
    config::Config,
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
                name: "shell".into(),
                active: true,
                last: false,
            },
            TabParts {
                window: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: false,
                last: false,
            },
        ],
        badges: Vec::new(),
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
        }
    }

    fn run(&self) -> ChromeLayout {
        let theme = Config::default().theme();
        let scale = self.scale;
        let input = ChromeInput {
            px: (self.width * scale as u32, 600 * scale as u32),
            scale,
            padding: (8.0 * scale, 4.0 * scale),
            metrics: CellMetrics {
                width: 8.0 * scale,
                height: 16.0 * scale,
                baseline: 12.0 * scale,
            },
            ui: UiMetrics {
                height: 14.0 * scale,
                ascent: 11.0 * scale,
                small: 0.5,
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
fn bar_and_line_span_the_window_top() {
    let surfaces = Config::default().theme().surfaces();
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
                surfaces.surface1
            )
        );
        assert_eq!(
            layout.chrome.quads[1],
            fill(
                rect(0.0, 34.0 * scale, 800.0 * scale, scale),
                surfaces.outline
            )
        );
        assert!(layout.chrome.modal.is_empty());
        assert!(layout.chrome.modal_text.is_empty());
        assert_eq!(layout.overlay, None);
    }
}

#[test]
fn session_item_draws_a_monogram_and_the_bold_name_without_a_box() {
    let theme = Config::default().theme();
    let status = parts();
    let layout = Case::new(&status).run();
    let mark = monogram("main");
    assert_eq!(
        quad_at(&layout, rect(12.0, 8.0, 18.0, 18.0)),
        Some(&Quad {
            rect: rect(12.0, 8.0, 18.0, 18.0),
            color: opaque(mark.hue),
            radius: 4.0,
            border: None,
        })
    );
    let letters = text(&layout, "MA").unwrap();
    assert_eq!(
        (
            letters.x,
            letters.baseline,
            letters.color,
            letters.bold,
            letters.face
        ),
        (18.0, 19.0, mark.text, true, TextFace::Small)
    );
    let name = text(&layout, "main").unwrap();
    assert_eq!(
        (name.x, name.baseline, name.color, name.bold, name.face),
        (38.0, 21.0, theme.foreground, true, TextFace::Ui)
    );
    assert!(quad_at(&layout, rect(8.0, 5.0, 68.0, 24.0)).is_none());
    assert!(layout.chrome.quads.iter().all(|quad| quad.rect.w != 6.0));
    assert!(
        layout
            .chrome
            .text
            .iter()
            .all(|run| !run.text.starts_with('+'))
    );
    assert!(layout.chrome.text.iter().all(|run| run.text != "!"));

    let layout = Case {
        scale: 2.0,
        ..Case::new(&status)
    }
    .run();
    let quad = quad_at(&layout, rect(24.0, 16.0, 36.0, 36.0)).unwrap();
    assert_eq!((quad.color, quad.radius), (opaque(mark.hue), 8.0));
}

#[test]
fn session_item_shows_only_monogram_and_name() {
    let mut status = parts();
    status.chip.hidden = 2;
    status.chip.running = true;
    status.chip.finished = true;
    let layout = Case::new(&status).run();
    assert!(text(&layout, "+2").is_none());
    assert_eq!(
        layout
            .chrome
            .quads
            .iter()
            .filter(|quad| quad.rect.w == 6.0)
            .count(),
        0
    );
    assert_eq!(layout.hits[0].rect, rect(8.0, 0.0, 68.0, 34.0));
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
fn active_tab_is_filled_last_is_bright_and_the_rest_muted() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut status = parts();
    status.tabs.push(TabParts {
        window: WindowId(3),
        index: 3,
        name: "zsh".into(),
        active: false,
        last: true,
    });
    let layout = Case::new(&status).run();
    assert_eq!(
        quad_at(&layout, rect(82.0, 5.0, 69.0, 24.0)),
        Some(&Quad {
            rect: rect(82.0, 5.0, 69.0, 24.0),
            color: opaque(surfaces.surface3),
            radius: 6.0,
            border: None,
        })
    );
    let active = text(&layout, "1 shell").unwrap();
    assert_eq!(
        (active.x, active.color, active.bold),
        (92.0, theme.foreground, false)
    );
    let other = text(&layout, "2 editor").unwrap();
    assert_eq!((other.x, other.color), (167.0, surfaces.muted));
    let last = text(&layout, "3 zsh").unwrap();
    assert_eq!((last.x, last.color), (249.0, surfaces.bright));
    assert!(quad_at(&layout, rect(157.0, 5.0, 76.0, 24.0)).is_none());
    assert!(quad_at(&layout, rect(239.0, 5.0, 55.0, 24.0)).is_none());
}

#[test]
fn tab_hits_span_the_full_bar_height() {
    let status = parts();
    let layout = Case::new(&status).run();
    assert_eq!(
        layout.hits,
        [
            Hit {
                rect: rect(8.0, 0.0, 68.0, 34.0),
                target: HitTarget::Session,
            },
            Hit {
                rect: rect(82.0, 0.0, 69.0, 34.0),
                target: HitTarget::Window(WindowId(1)),
            },
            Hit {
                rect: rect(157.0, 0.0, 76.0, 34.0),
                target: HitTarget::Window(WindowId(2)),
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
        hit_test(&layout.hits, (10.0, 33.0)),
        Some(HitTarget::Session)
    );
    assert_eq!(hit_test(&layout.hits, (154.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (79.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (160.0, 34.0)), None);
}

#[test]
fn hover_fills_the_hovered_tab_or_session_item_with_surface2() {
    let surfaces = Config::default().theme().surfaces();
    let status = parts();
    let hovered = |hover| {
        Case {
            hover: Some(hover),
            ..Case::new(&status)
        }
        .run()
        .chrome
        .quads
        .into_iter()
        .filter(|quad| quad.color == opaque(surfaces.surface2))
        .map(|quad| (quad.rect, quad.radius))
        .collect::<Vec<_>>()
    };
    assert_eq!(
        hovered((160.0, 30.0)),
        [(rect(157.0, 5.0, 76.0, 24.0), 6.0)]
    );
    assert_eq!(hovered((10.0, 20.0)), [(rect(8.0, 5.0, 68.0, 24.0), 6.0)]);
    assert_eq!(hovered((100.0, 2.0)), []);
    assert_eq!(hovered((400.0, 20.0)), []);
    assert_eq!(hovered((160.0, 100.0)), []);
}

fn many(count: u16, active: u16) -> StatusParts {
    StatusParts {
        tabs: (1..=count)
            .map(|index| TabParts {
                window: WindowId(u32::from(index)),
                index,
                name: "shell".into(),
                active: index == active,
                last: false,
            })
            .collect(),
        ..parts()
    }
}

#[test]
fn overflow_counts_hidden_tabs_on_the_side_where_they_are_hidden() {
    let surfaces = Config::default().theme().surfaces();
    for (count, active, width, shown, tab, labels) in [
        (30, 30, 300, vec![29, 30], "30 shell", vec![(92.0, "+28")]),
        (30, 30, 200, vec![30], "30 sh…", vec![(92.0, "+29")]),
        (30, 30, 160, vec![30], "30 she…", vec![]),
        (30, 1, 300, vec![1, 2], "1 shell", vec![(242.0, "+28")]),
        (
            30,
            15,
            300,
            vec![15],
            "15 shell",
            vec![(92.0, "+14"), (221.0, "+15")],
        ),
        (3, 1, 800, vec![1, 2, 3], "3 shell", vec![]),
    ] {
        let layout = Case {
            width,
            ..Case::new(&many(count, active))
        }
        .run();
        let windows: Vec<HitTarget> = shown
            .into_iter()
            .map(|window| HitTarget::Window(WindowId(window)))
            .collect();
        let targets: Vec<HitTarget> = layout.hits[1..].iter().map(|hit| hit.target).collect();
        assert_eq!(targets, windows, "{active} {width}");
        assert!(text(&layout, tab).is_some(), "{active} {width}");
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
fn prefix_is_a_key_cap_and_copy_an_accent_chip_at_the_far_right() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let status = copy_and_prefix();
    let layout = Case::new(&status).run();
    assert_eq!(
        quad_at(&layout, rect(766.0, 7.0, 26.0, 20.0)),
        Some(&Quad {
            rect: rect(766.0, 7.0, 26.0, 20.0),
            color: opaque(surfaces.accent_soft),
            radius: 4.0,
            border: Some(Border {
                color: opaque(theme.accent),
                width: 1.0,
            }),
        })
    );
    let copy = text(&layout, "COPY").unwrap();
    assert_eq!(
        (copy.x, copy.baseline, copy.color, copy.bold, copy.face),
        (772.0, 19.0, theme.accent, false, TextFace::Small)
    );
    assert_eq!(
        quad_at(&layout, rect(741.0, 7.0, 19.0, 20.0)),
        Some(&Quad {
            rect: rect(741.0, 7.0, 19.0, 20.0),
            color: opaque(surfaces.surface3),
            radius: 4.0,
            border: Some(Border {
                color: opaque(surfaces.outline_strong),
                width: 1.0,
            }),
        })
    );
    let prefix = text(&layout, "^A").unwrap();
    assert_eq!(
        (
            prefix.x,
            prefix.baseline,
            prefix.color,
            prefix.bold,
            prefix.face
        ),
        (747.0, 19.0, theme.foreground, false, TextFace::Small)
    );
    assert!(text(&layout, "ZOOM").is_none());

    let layout = Case {
        scale: 2.0,
        ..Case::new(&status)
    }
    .run();
    assert!(quad_at(&layout, rect(1546.0, 14.0, 38.0, 40.0)).is_some());
    assert_eq!(text(&layout, "COPY").unwrap().x, 1558.0);
}

#[test]
fn zoom_chip_sits_between_copy_and_the_prefix_when_zoomed() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let status = copy_and_prefix();
    let window = zoomed();
    let layout = Case {
        window: Some(&window),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(
        quad_at(&layout, rect(734.0, 7.0, 26.0, 20.0)),
        Some(&Quad {
            rect: rect(734.0, 7.0, 26.0, 20.0),
            color: opaque(surfaces.accent_soft),
            radius: 4.0,
            border: Some(Border {
                color: opaque(theme.accent),
                width: 1.0,
            }),
        })
    );
    let zoom = text(&layout, "ZOOM").unwrap();
    assert_eq!(
        (zoom.x, zoom.color, zoom.bold, zoom.face),
        (740.0, theme.accent, false, TextFace::Small)
    );
    assert_eq!(text(&layout, "^A").unwrap().x, 715.0);

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
        quad_at(&layout, rect(82.0, 5.0, 710.0, 24.0)),
        Some(&Quad {
            rect: rect(82.0, 5.0, 710.0, 24.0),
            color: opaque(theme.background),
            radius: 6.0,
            border: Some(Border {
                color: opaque(surfaces.outline),
                width: 1.0,
            }),
        })
    );
    let label = text(&layout, "/").unwrap();
    assert_eq!(
        (label.x, label.baseline, label.color),
        (92.0, 21.0, surfaces.muted)
    );
    let prompt = text(&layout, "foo").unwrap();
    assert_eq!(
        (prompt.x, prompt.baseline, prompt.color),
        (99.0, 21.0, theme.foreground)
    );
    assert_eq!(
        quad_at(&layout, rect(120.0, 10.0, 1.0, 14.0)),
        Some(&fill(rect(120.0, 10.0, 1.0, 14.0), theme.accent))
    );
    assert_eq!(
        layout.hits.iter().map(|hit| hit.target).collect::<Vec<_>>(),
        [HitTarget::Session]
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
    assert_eq!(text(&layout, "/").map(|run| run.x), Some(92.0));
    assert_eq!(text(&layout, "foo").map(|run| run.x), Some(99.0));
    assert_eq!(
        quad_at(&layout, rect(106.0, 10.0, 1.0, 14.0)),
        Some(&fill(rect(106.0, 10.0, 1.0, 14.0), theme.accent))
    );
    let prompt = format!("{}end", "x".repeat(200));
    let layout = Case {
        label: "/",
        prompt: Some(&prompt),
        cursor: Some(0),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(text(&layout, "/").map(|run| run.x), Some(92.0));
    let run = layout.chrome.text.iter().find(|run| run.x == 99.0).unwrap();
    assert!(run.text.starts_with('x') && run.text.ends_with('…'));
    assert_eq!(
        quad_at(&layout, rect(99.0, 10.0, 1.0, 14.0)).map(|quad| quad.color),
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
    assert_eq!(text(&layout, "/").map(|run| run.x), Some(92.0));
    let run = layout.chrome.text.iter().find(|run| run.x == 99.0).unwrap();
    assert!(run.text.starts_with('…') && run.text.ends_with("end"));
    assert!(run.x + measure(&run.text, false) <= field_end - 10.0);
    assert!(caret(&layout).unwrap().x <= field_end - 10.0);

    let layout = Case {
        width: 140,
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
            .all(|run| run.text != "/" && !run.text.ends_with("end"))
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
        notice_dot(&layout, 92.0),
        Some(&rounded(
            rect(92.0, 14.0, 6.0, 6.0),
            surfaces.muted,
            3.0,
            1.0
        ))
    );
    let run = text(&layout, "config reloaded").unwrap();
    assert_eq!(
        (run.x, run.baseline, run.color, run.bold, run.face),
        (104.0, 21.0, surfaces.muted, false, TextFace::Ui)
    );
    assert_eq!(
        layout.hits.iter().map(|hit| hit.target).collect::<Vec<_>>(),
        [HitTarget::Session]
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
        notice_dot(&layout, 92.0).map(|quad| quad.color),
        Some(opaque(surfaces.attention))
    );
    let run = layout
        .chrome
        .text
        .iter()
        .find(|run| run.text.starts_with("could not"))
        .unwrap();
    assert_eq!((run.x, run.color), (104.0, surfaces.muted));
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
        .find(|quad| quad.color == opaque(theme.background))
        .unwrap();
    assert_eq!(field.rect, rect(82.0, 5.0, 503.0, 24.0));
    assert_eq!(
        notice_dot(&layout, 591.0).map(|quad| quad.color),
        Some(opaque(surfaces.attention))
    );
    let run = text(&layout, "window name cannot be empty").unwrap();
    assert_eq!((run.x, run.color), (603.0, surfaces.muted));
    let label = text(&layout, "(rename-window) ").unwrap();
    assert_eq!((label.x, label.color), (92.0, surfaces.muted));
}

#[test]
fn a_notice_next_to_a_prompt_keeps_the_field_at_least_120_wide() {
    let theme = Config::default().theme();
    let status = parts();
    let notice = Notice::error("window name cannot be empty");
    let field_rect = |layout: &ChromeLayout| {
        layout
            .chrome
            .quads
            .iter()
            .find(|quad| quad.color == opaque(theme.background))
            .unwrap()
            .rect
    };
    let layout = Case {
        width: 300,
        label: "(rename-window) ",
        prompt: Some(""),
        notice: Some(&notice),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(field_rect(&layout), rect(82.0, 5.0, 120.0, 24.0));
    assert_eq!(text(&layout, "window na…").unwrap().x, 220.0);

    let layout = Case {
        width: 220,
        label: "(rename-window) ",
        prompt: Some(""),
        notice: Some(&notice),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(field_rect(&layout), rect(82.0, 5.0, 130.0, 24.0));
    assert!(
        layout
            .chrome
            .text
            .iter()
            .all(|run| !run.text.starts_with("window"))
    );
}
