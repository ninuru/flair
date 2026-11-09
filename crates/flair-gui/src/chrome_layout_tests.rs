use super::*;
use crate::status::{BadgeKind, BadgeParts, ChipParts, TabParts};
use flair_core::{
    config::Config,
    geom::{Axis, CellPoint},
    theme::Rgb,
};

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn metrics(scale: f32) -> CellMetrics {
    CellMetrics {
        width: 8.0 * scale,
        height: 16.0 * scale,
        baseline: 12.0 * scale,
    }
}

fn ui(scale: f32) -> UiMetrics {
    UiMetrics {
        height: 14.0 * scale,
        ascent: 11.0 * scale,
    }
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
            },
            TabParts {
                window: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: false,
            },
        ],
        badges: Vec::new(),
    }
}

fn run_layout(
    scale: f32,
    width: u32,
    status: &StatusParts,
    prompt: Option<&str>,
    separators: &[Separator],
    modal: Option<Modal>,
) -> ChromeLayout {
    let theme = Config::default().theme();
    let input = ChromeInput {
        px: (width * scale as u32, 600 * scale as u32),
        scale,
        padding: (8.0 * scale, 4.0 * scale),
        metrics: metrics(scale),
        ui: ui(scale),
        theme: &theme,
        status,
        prompt,
        separators,
        modal,
    };
    layout(&input, measure)
}

fn text<'a>(layout: &'a ChromeLayout, text: &str) -> Option<&'a TextRun> {
    layout.chrome.text.iter().find(|run| run.text == text)
}

fn quad_at(layout: &ChromeLayout, rect: PxRect) -> Option<&Quad> {
    layout.chrome.quads.iter().find(|quad| quad.rect == rect)
}

#[test]
fn bar_and_border_span_the_window_top() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    for scale in [1.0, 2.0] {
        let layout = run_layout(scale, 800, &parts(), None, &[], None);
        assert_eq!(
            layout.chrome.quads[0],
            Quad {
                rect: rect(0.0, 0.0, 800.0 * scale, 34.0 * scale),
                color: opaque(surfaces.surface1),
                radius: 0.0,
                border: None,
            }
        );
        assert_eq!(
            layout.chrome.quads[1],
            Quad {
                rect: rect(0.0, 34.0 * scale, 800.0 * scale, scale),
                color: opaque(surfaces.outline),
                radius: 0.0,
                border: None,
            }
        );
        assert!(layout.chrome.modal.is_empty());
        assert_eq!(layout.chrome.lifted, None);
    }
}

#[test]
fn grid_starts_below_bar_and_border() {
    assert_eq!(bar_height(1.0), 34.0);
    assert_eq!(bar_height(2.0), 68.0);
    assert_eq!(bar_height(1.5), 51.0);
    assert_eq!(grid_origin((8.0, 4.0), 1.0), (8.0, 39.0));
    assert_eq!(grid_origin((16.0, 8.0), 2.0), (16.0, 78.0));
    assert_eq!(
        grid_size((800, 600), (8.0, 4.0), 1.0, metrics(1.0)),
        GridSize { cols: 98, rows: 34 }
    );
    assert_eq!(
        grid_size((1600, 1200), (16.0, 8.0), 2.0, metrics(2.0)),
        GridSize { cols: 98, rows: 34 }
    );
    assert_eq!(
        grid_size((10, 20), (8.0, 4.0), 1.0, metrics(1.0)),
        GridSize { cols: 1, rows: 1 }
    );
    let layout = run_layout(1.0, 800, &parts(), None, &[], None);
    assert_eq!(layout.chrome.origin, (8.0, 39.0));
    assert_eq!(layout.chrome.clip, rect(8.0, 39.0, 784.0, 557.0));
    let layout = run_layout(2.0, 800, &parts(), None, &[], None);
    assert_eq!(layout.chrome.origin, (16.0, 78.0));
    assert_eq!(layout.chrome.clip, rect(16.0, 78.0, 1568.0, 1114.0));
}

#[test]
fn modal_area_insets_the_grid_by_the_modal_margin() {
    for scale in [1.0, 2.0] {
        assert_eq!(
            modal_area(GridSize { cols: 98, rows: 34 }, metrics(scale), scale),
            CellRect {
                origin: CellPoint { col: 3, row: 2 },
                size: GridSize { cols: 92, rows: 30 },
            }
        );
    }
    let tiny = modal_area(GridSize { cols: 4, rows: 3 }, metrics(1.0), 1.0);
    assert!(tiny.origin.col + tiny.size.cols <= 4);
    assert!(tiny.origin.row + tiny.size.rows <= 3);
}

#[test]
fn chip_shows_only_the_parts_that_are_set() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let layout = run_layout(1.0, 800, &parts(), None, &[], None);
    let name = text(&layout, "main").unwrap();
    assert_eq!((name.x, name.baseline), (14.0, 21.0));
    assert!(name.bold);
    assert_eq!(name.color, theme.foreground);
    assert_eq!(
        quad_at(&layout, rect(8.0, 6.0, 40.0, 22.0)),
        Some(&Quad {
            rect: rect(8.0, 6.0, 40.0, 22.0),
            color: opaque(surfaces.surface2),
            radius: 4.0,
            border: Some(Border {
                color: opaque(surfaces.outline),
                width: 1.0,
            }),
        })
    );
    assert!(layout.chrome.text.iter().all(|run| run.text != "!"));
    assert!(layout.chrome.quads.iter().all(|quad| quad.radius != 3.0));
    assert!(
        layout
            .chrome
            .text
            .iter()
            .all(|run| !run.text.starts_with("+"))
    );

    let mut full = parts();
    full.chip.hidden = 2;
    full.chip.running = true;
    full.chip.finished = true;
    let layout = run_layout(1.0, 800, &full, None, &[], None);
    let hidden = text(&layout, "+2").unwrap();
    assert_eq!(
        (hidden.x, hidden.color, hidden.bold),
        (46.0, surfaces.muted, false)
    );
    assert_eq!(
        quad_at(&layout, rect(64.0, 14.0, 6.0, 6.0)),
        Some(&Quad {
            rect: rect(64.0, 14.0, 6.0, 6.0),
            color: opaque(theme.accent),
            radius: 3.0,
            border: None,
        })
    );
    let finished = text(&layout, "!").unwrap();
    assert_eq!(
        (finished.x, finished.color, finished.bold),
        (74.0, theme.accent, true)
    );
    assert!(quad_at(&layout, rect(8.0, 6.0, 79.0, 22.0)).is_some());

    let layout = run_layout(2.0, 800, &full, None, &[], None);
    let name = text(&layout, "main").unwrap();
    assert_eq!((name.x, name.baseline), (28.0, 42.0));
    assert_eq!(text(&layout, "+2").unwrap().x, 64.0);
    assert!(quad_at(&layout, rect(86.0, 28.0, 12.0, 12.0)).is_some());
    assert_eq!(text(&layout, "!").unwrap().x, 106.0);
    let chip = quad_at(&layout, rect(16.0, 12.0, 109.0, 44.0)).unwrap();
    assert_eq!(chip.radius, 8.0);
    assert_eq!(chip.border.unwrap().width, 2.0);
}

#[test]
fn long_session_names_are_ellipsized() {
    let mut long = parts();
    long.chip.session = "x".repeat(40);
    let layout = run_layout(1.0, 800, &long, None, &[], None);
    let expected = format!("{}…", "x".repeat(24));
    assert!(text(&layout, &expected).is_some());
}

#[test]
fn active_tab_has_a_fill_and_inactive_tabs_are_muted() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let layout = run_layout(1.0, 800, &parts(), None, &[], None);
    assert_eq!(
        quad_at(&layout, rect(56.0, 4.0, 69.0, 26.0)),
        Some(&Quad {
            rect: rect(56.0, 4.0, 69.0, 26.0),
            color: opaque(surfaces.surface3),
            radius: 4.0,
            border: None,
        })
    );
    let active = text(&layout, "1 shell").unwrap();
    assert_eq!((active.x, active.color), (66.0, theme.foreground));
    let inactive = text(&layout, "2 editor").unwrap();
    assert_eq!((inactive.x, inactive.color), (137.0, surfaces.muted));
    assert!(quad_at(&layout, rect(127.0, 4.0, 76.0, 26.0)).is_none());
    assert_eq!(
        layout.hits,
        [
            Hit {
                rect: rect(8.0, 6.0, 40.0, 22.0),
                target: HitTarget::Session,
            },
            Hit {
                rect: rect(56.0, 4.0, 69.0, 26.0),
                target: HitTarget::Window(WindowId(1)),
            },
            Hit {
                rect: rect(127.0, 4.0, 76.0, 26.0),
                target: HitTarget::Window(WindowId(2)),
            },
        ]
    );
}

#[test]
fn overflow_drops_leading_tabs_and_keeps_the_active_one() {
    let many = StatusParts {
        tabs: (1..=30)
            .map(|index| TabParts {
                window: WindowId(u32::from(index)),
                index,
                name: "shell".into(),
                active: index == 30,
            })
            .collect(),
        ..parts()
    };
    let layout = run_layout(1.0, 300, &many, None, &[], None);
    assert!(text(&layout, "30 shell").is_some());
    assert!(text(&layout, "1 shell").is_none());
    let windows: Vec<_> = layout
        .hits
        .iter()
        .filter(|hit| matches!(hit.target, HitTarget::Window(_)))
        .collect();
    assert_eq!(
        windows.last().unwrap().target,
        HitTarget::Window(WindowId(30))
    );
    assert!(windows.len() > 1);
    for hit in &windows {
        assert!(hit.rect.x >= 56.0 && hit.rect.x + hit.rect.w <= 292.0);
    }

    let layout = run_layout(1.0, 120, &many, None, &[], None);
    assert!(text(&layout, "30 s…").is_some());
    let windows: Vec<_> = layout
        .hits
        .iter()
        .filter(|hit| matches!(hit.target, HitTarget::Window(_)))
        .collect();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].target, HitTarget::Window(WindowId(30)));
    assert!(windows[0].rect.x + windows[0].rect.w <= 112.0);
}

#[test]
fn badges_sit_at_the_right_inset_with_copy_at_the_far_right() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let badged = StatusParts {
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
    };
    let layout = run_layout(1.0, 800, &badged, None, &[], None);
    assert_eq!(
        quad_at(&layout, rect(748.0, 6.0, 44.0, 22.0)),
        Some(&Quad {
            rect: rect(748.0, 6.0, 44.0, 22.0),
            color: opaque(surfaces.accent_soft),
            radius: 4.0,
            border: Some(Border {
                color: opaque(theme.accent),
                width: 1.0,
            }),
        })
    );
    let copy = text(&layout, "COPY").unwrap();
    assert_eq!((copy.x, copy.color, copy.bold), (756.0, theme.accent, true));
    assert_eq!(
        quad_at(&layout, rect(712.0, 6.0, 30.0, 22.0)),
        Some(&Quad {
            rect: rect(712.0, 6.0, 30.0, 22.0),
            color: opaque(surfaces.surface3),
            radius: 4.0,
            border: Some(Border {
                color: opaque(surfaces.outline),
                width: 1.0,
            }),
        })
    );
    let prefix = text(&layout, "^A").unwrap();
    assert_eq!(
        (prefix.x, prefix.color, prefix.bold),
        (720.0, theme.foreground, true)
    );

    let layout = run_layout(2.0, 800, &badged, None, &[], None);
    assert!(quad_at(&layout, rect(1524.0, 12.0, 60.0, 44.0)).is_some());
    assert!(quad_at(&layout, rect(1466.0, 12.0, 46.0, 44.0)).is_some());
    assert_eq!(text(&layout, "COPY").unwrap().x, 1540.0);
}

#[test]
fn prompt_replaces_the_tabs_and_shows_a_caret() {
    let theme = Config::default().theme();
    let layout = run_layout(1.0, 800, &parts(), Some("/foo"), &[], None);
    assert!(text(&layout, "1 shell").is_none());
    assert!(text(&layout, "2 editor").is_none());
    let prompt = text(&layout, "/foo").unwrap();
    assert_eq!((prompt.x, prompt.color), (56.0, theme.foreground));
    assert_eq!(
        quad_at(&layout, rect(84.0, 10.0, 1.0, 14.0)),
        Some(&Quad {
            rect: rect(84.0, 10.0, 1.0, 14.0),
            color: opaque(theme.accent),
            radius: 0.0,
            border: None,
        })
    );
    assert_eq!(
        layout.hits.iter().map(|hit| hit.target).collect::<Vec<_>>(),
        [HitTarget::Session]
    );
}

#[test]
fn separators_become_lines_centred_in_the_gap() {
    let surfaces = Config::default().theme().surfaces();
    let separators = [
        Separator {
            axis: Axis::Horizontal,
            rect: CellRect {
                origin: CellPoint { col: 10, row: 0 },
                size: GridSize { cols: 1, rows: 20 },
            },
        },
        Separator {
            axis: Axis::Vertical,
            rect: CellRect {
                origin: CellPoint { col: 0, row: 5 },
                size: GridSize { cols: 10, rows: 1 },
            },
        },
    ];
    for (scale, vertical, horizontal) in [
        (
            1.0,
            rect(92.0, 39.0, 1.0, 320.0),
            rect(8.0, 127.0, 80.0, 1.0),
        ),
        (
            2.0,
            rect(183.0, 78.0, 2.0, 640.0),
            rect(16.0, 253.0, 160.0, 2.0),
        ),
    ] {
        let layout = run_layout(scale, 800, &parts(), None, &separators, None);
        for expected in [vertical, horizontal] {
            assert_eq!(
                quad_at(&layout, expected),
                Some(&Quad {
                    rect: expected,
                    color: opaque(surfaces.pane_border),
                    radius: 0.0,
                    border: None,
                })
            );
        }
    }
}

#[test]
fn centred_modal_grows_the_lifted_rect_over_a_backdrop() {
    let surfaces = Config::default().theme().surfaces();
    let lifted = CellRect {
        origin: CellPoint { col: 10, row: 2 },
        size: GridSize { cols: 20, rows: 5 },
    };
    for (scale, expected) in [
        (1.0, rect(64.0, 47.0, 208.0, 128.0)),
        (2.0, rect(128.0, 94.0, 416.0, 256.0)),
    ] {
        let layout = run_layout(
            scale,
            800,
            &parts(),
            None,
            &[],
            Some(Modal::Centered(lifted)),
        );
        assert_eq!(layout.chrome.lifted, Some(lifted));
        assert_eq!(
            layout.chrome.modal,
            [
                Quad {
                    rect: rect(0.0, 0.0, 800.0 * scale, 600.0 * scale),
                    color: Rgba {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 179,
                    },
                    radius: 0.0,
                    border: None,
                },
                Quad {
                    rect: expected,
                    color: opaque(surfaces.surface1),
                    radius: 8.0 * scale,
                    border: Some(Border {
                        color: opaque(surfaces.outline),
                        width: scale,
                    }),
                },
            ]
        );
    }
}

#[test]
fn hit_test_finds_tabs_and_chip_but_not_gaps_or_the_grid() {
    let layout = run_layout(1.0, 800, &parts(), None, &[], None);
    assert_eq!(
        hit_test(&layout.hits, (60.0, 10.0)),
        Some(HitTarget::Window(WindowId(1)))
    );
    assert_eq!(
        hit_test(&layout.hits, (130.0, 10.0)),
        Some(HitTarget::Window(WindowId(2)))
    );
    assert_eq!(
        hit_test(&layout.hits, (10.0, 10.0)),
        Some(HitTarget::Session)
    );
    assert_eq!(hit_test(&layout.hits, (126.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (52.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (60.0, 100.0)), None);
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

#[test]
fn long_prompts_show_their_tail_left_of_the_badges() {
    let badged = StatusParts {
        badges: vec![BadgeParts {
            kind: BadgeKind::Copy,
            text: "COPY".into(),
        }],
        ..parts()
    };
    let prompt = format!("/{}end", "x".repeat(200));
    let layout = run_layout(1.0, 800, &badged, Some(&prompt), &[], None);
    let end = 740.0;
    let run = layout.chrome.text.iter().find(|run| run.x == 56.0).unwrap();
    assert_eq!(run.text, format!("…{}end", "x".repeat(93)));
    assert!(run.x + measure(&run.text, false) <= end);
    let theme = Config::default().theme();
    let caret = layout
        .chrome
        .quads
        .iter()
        .find(|quad| quad.color == opaque(theme.accent) && quad.rect.w == 1.0)
        .unwrap();
    assert_eq!(caret.rect, rect(735.0, 10.0, 1.0, 14.0));

    let layout = run_layout(1.0, 120, &badged, Some(&prompt), &[], None);
    let caret = layout
        .chrome
        .quads
        .iter()
        .find(|quad| quad.color == opaque(theme.accent) && quad.rect.w == 1.0)
        .unwrap();
    assert!(caret.rect.x + caret.rect.w <= 60.0);
    assert!(
        layout
            .chrome
            .text
            .iter()
            .filter(|run| run.x == 56.0)
            .all(|run| run.x + measure(&run.text, false) <= 60.0)
    );
}

#[test]
fn separators_are_clipped_to_the_grid_area() {
    let surfaces = Config::default().theme().surfaces();
    let separators = [
        Separator {
            axis: Axis::Horizontal,
            rect: CellRect {
                origin: CellPoint { col: 10, row: 0 },
                size: GridSize { cols: 1, rows: 100 },
            },
        },
        Separator {
            axis: Axis::Vertical,
            rect: CellRect {
                origin: CellPoint { col: 0, row: 5 },
                size: GridSize { cols: 60, rows: 1 },
            },
        },
        Separator {
            axis: Axis::Horizontal,
            rect: CellRect {
                origin: CellPoint { col: 50, row: 0 },
                size: GridSize { cols: 1, rows: 10 },
            },
        },
    ];
    let layout = run_layout(1.0, 200, &parts(), None, &separators, None);
    let borders: Vec<PxRect> = layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.color == opaque(surfaces.pane_border) && quad.rect.y >= 39.0)
        .map(|quad| quad.rect)
        .collect();
    assert_eq!(layout.chrome.clip, rect(8.0, 39.0, 184.0, 557.0));
    assert_eq!(
        borders,
        [rect(92.0, 39.0, 1.0, 557.0), rect(8.0, 127.0, 184.0, 1.0)]
    );
}
