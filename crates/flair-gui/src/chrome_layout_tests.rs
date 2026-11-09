use super::*;
use crate::status::{ChipParts, TabParts};
use flair_core::{
    config::Config,
    geom::{Axis, CellPoint},
    ids::PaneId,
    proto::PanePlacement,
    theme::{Rgb, Rgba},
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
        small: 0.5,
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

fn cells(col: u16, row: u16, cols: u16, rows: u16) -> CellRect {
    CellRect {
        origin: CellPoint { col, row },
        size: GridSize { cols, rows },
    }
}

fn window(separators: &[Separator], panes: &[(u32, CellRect)], active: u32) -> WindowLayout {
    WindowLayout {
        window: WindowId(1),
        area: cells(0, 0, 98, 34),
        panes: panes
            .iter()
            .map(|(pane, rect)| PanePlacement {
                pane: PaneId(*pane),
                rect: *rect,
            })
            .collect(),
        separators: separators.to_vec(),
        active: PaneId(active),
        zoomed: false,
    }
}

fn run_layout(
    scale: f32,
    width: u32,
    window: Option<&WindowLayout>,
    modal: Option<Modal>,
) -> ChromeLayout {
    let theme = Config::default().theme();
    let status = parts();
    let input = ChromeInput {
        px: (width * scale as u32, 600 * scale as u32),
        scale,
        padding: (8.0 * scale, 4.0 * scale),
        metrics: metrics(scale),
        ui: ui(scale),
        theme: &theme,
        status: &status,
        prompt: None,
        layout: window,
        hover: None,
        modal,
    };
    layout(&input, measure)
}

fn quad_at(layout: &ChromeLayout, rect: PxRect) -> Option<&Quad> {
    layout.chrome.quads.iter().find(|quad| quad.rect == rect)
}

#[test]
fn grid_starts_below_bar_and_border() {
    assert_eq!(bar_height(1.0), 34.0);
    assert_eq!(bar_height(2.0), 68.0);
    assert_eq!(bar_height(1.5), 51.0);
    assert_eq!(
        grid_origin((800, 600), (8.0, 4.0), 1.0, metrics(1.0)),
        (8.0, 52.0)
    );
    assert_eq!(
        grid_origin((1600, 1200), (16.0, 8.0), 2.0, metrics(2.0)),
        (16.0, 104.0)
    );
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
    let layout = run_layout(1.0, 800, None, None);
    assert_eq!(layout.chrome.origin, (8.0, 52.0));
    assert_eq!(layout.chrome.clip, rect(8.0, 39.0, 784.0, 557.0));
    assert_eq!(layout.chrome.extend, rect(0.0, 35.0, 800.0, 565.0));
    let layout = run_layout(2.0, 800, None, None);
    assert_eq!(layout.chrome.origin, (16.0, 104.0));
    assert_eq!(layout.chrome.clip, rect(16.0, 78.0, 1568.0, 1114.0));
    assert_eq!(layout.chrome.extend, rect(0.0, 70.0, 1600.0, 1130.0));
}

#[test]
fn grid_origin_centres_columns_and_puts_the_last_row_on_the_bottom_edge() {
    for (scale, width, origin) in [(1.0, 803, (9.0, 52.0)), (2.0, 803, (19.0, 104.0))] {
        let px = (width * scale as u32, 600 * scale as u32);
        let padding = (8.0 * scale, 4.0 * scale);
        let size = grid_size(px, padding, scale, metrics(scale));
        assert_eq!(size, GridSize { cols: 98, rows: 34 });
        assert_eq!(grid_origin(px, padding, scale, metrics(scale)), origin);
        let right = px.0 as f32 - origin.0 - f32::from(size.cols) * metrics(scale).width;
        let bottom = px.1 as f32 - origin.1 - f32::from(size.rows) * metrics(scale).height;
        assert!(
            (right - origin.0).abs() <= 1.0,
            "left {} right {right}",
            origin.0
        );
        assert!(
            (bottom - padding.1).abs() < 1.0,
            "top {} bottom {bottom}",
            origin.1
        );
        let layout = run_layout(scale, width, None, None);
        assert_eq!(layout.chrome.origin, origin);
    }
}

#[test]
fn zero_padding_puts_the_last_row_on_the_bottom_edge() {
    for (scale, width) in [(1.0, 803), (2.0, 803)] {
        let px = (width * scale as u32, 600 * scale as u32);
        let cell = metrics(scale);
        let size = grid_size(px, (0.0, 0.0), scale, cell);
        let top = 35.0 * scale;
        let leftover = (
            px.0 as f32 - f32::from(size.cols) * cell.width,
            px.1 as f32 - top - f32::from(size.rows) * cell.height,
        );
        assert!(leftover.0 >= 0.0 && leftover.1 >= 0.0, "{leftover:?}");
        assert_eq!(
            grid_origin(px, (0.0, 0.0), scale, cell),
            ((leftover.0 / 2.0).floor(), top + leftover.1.floor())
        );
        let theme = Config::default().theme();
        let status = parts();
        let input = ChromeInput {
            px,
            scale,
            padding: (0.0, 0.0),
            metrics: cell,
            ui: ui(scale),
            theme: &theme,
            status: &status,
            prompt: None,
            layout: None,
            hover: None,
            modal: None,
        };
        let chrome = layout(&input, measure).chrome;
        let full = rect(0.0, top, px.0 as f32, px.1 as f32 - top);
        assert_eq!(chrome.origin, grid_origin(px, (0.0, 0.0), scale, cell));
        assert_eq!(chrome.clip, full);
        assert_eq!(chrome.extend, full);
    }
}

#[test]
fn mouse_mapping_uses_the_drawn_grid_origin() {
    for scale in [1.0, 2.0] {
        let layout = run_layout(scale, 803, None, None);
        let origin = layout.chrome.origin;
        let cell = metrics(scale);
        let size = GridSize { cols: 98, rows: 34 };
        let at = |x: f32, y: f32| {
            crate::mouse::to_cell((f64::from(x), f64::from(y)), &cell, origin, size)
        };
        assert_eq!(at(origin.0, origin.1), CellPoint { col: 0, row: 0 });
        assert_eq!(
            at(origin.0 + cell.width - 0.5, origin.1 + cell.height - 0.5),
            CellPoint { col: 0, row: 0 }
        );
        assert_eq!(
            at(origin.0 + cell.width, origin.1 + cell.height),
            CellPoint { col: 1, row: 1 }
        );
        assert_eq!(
            at(
                origin.0 + f32::from(size.cols) * cell.width - 0.5,
                origin.1 + f32::from(size.rows) * cell.height - 0.5
            ),
            CellPoint { col: 97, row: 33 }
        );
    }
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
            rect(92.0, 52.0, 1.0, 320.0),
            rect(8.0, 140.0, 80.0, 1.0),
        ),
        (
            2.0,
            rect(183.0, 104.0, 2.0, 640.0),
            rect(16.0, 279.0, 160.0, 2.0),
        ),
    ] {
        let window = window(&separators, &[], 9);
        let layout = run_layout(scale, 800, Some(&window), None);
        for expected in [vertical, horizontal] {
            assert_eq!(
                quad_at(&layout, expected),
                Some(&Quad {
                    rect: expected,
                    color: opaque(surfaces.outline),
                    radius: 0.0,
                    border: None,
                })
            );
        }
    }
}

#[test]
fn centred_modal_grows_the_lifted_rect_into_a_panel() {
    let surfaces = Config::default().theme().surfaces();
    let lifted = cells(10, 2, 20, 5);
    for (scale, expected) in [
        (1.0, rect(64.0, 64.0, 208.0, 120.0)),
        (2.0, rect(128.0, 128.0, 416.0, 240.0)),
    ] {
        let layout = run_layout(scale, 800, None, Some(Modal::Centered(lifted)));
        assert_eq!(layout.chrome.lifted, Some(lifted));
        assert_eq!(
            layout.chrome.modal[1],
            Quad {
                rect: expected,
                color: opaque(surfaces.surface1),
                radius: 10.0 * scale,
                border: Some(Border {
                    color: opaque(surfaces.outline),
                    width: scale,
                }),
            }
        );
    }
}

#[test]
fn backdrop_starts_below_the_bar_line() {
    for scale in [1.0, 2.0] {
        let layout = run_layout(scale, 800, None, Some(Modal::Centered(cells(10, 2, 20, 5))));
        assert_eq!(
            layout.chrome.modal[0],
            Quad {
                rect: rect(0.0, 35.0 * scale, 800.0 * scale, 565.0 * scale),
                color: Rgba {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 102,
                },
                radius: 0.0,
                border: None,
            }
        );
        assert_eq!(layout.chrome.modal.len(), 2);
    }
}

#[test]
fn docked_modal_is_an_inset_panel_below_the_bar_without_backdrop() {
    let surfaces = Config::default().theme().surfaces();
    let lifted = cells(61, 2, 34, 30);
    for (scale, panel) in [
        (1.0, rect(472.0, 43.0, 320.0, 549.0)),
        (2.0, rect(944.0, 86.0, 640.0, 1098.0)),
    ] {
        let layout = run_layout(scale, 800, None, Some(Modal::Docked(lifted)));
        assert_eq!(layout.chrome.lifted, Some(lifted));
        assert_eq!(
            layout.chrome.modal,
            [Quad {
                rect: panel,
                color: opaque(surfaces.surface1),
                radius: 10.0 * scale,
                border: Some(Border {
                    color: opaque(surfaces.outline),
                    width: scale,
                }),
            }],
            "scale {scale}"
        );
    }
}

#[test]
fn hit_test_finds_tabs_and_the_session_but_not_gaps_or_the_grid() {
    let layout = run_layout(1.0, 800, None, None);
    assert_eq!(
        hit_test(&layout.hits, (90.0, 10.0)),
        Some(HitTarget::Window(WindowId(1)))
    );
    assert_eq!(
        hit_test(&layout.hits, (160.0, 10.0)),
        Some(HitTarget::Window(WindowId(2)))
    );
    assert_eq!(
        hit_test(&layout.hits, (10.0, 10.0)),
        Some(HitTarget::Session)
    );
    assert_eq!(hit_test(&layout.hits, (154.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (79.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (90.0, 100.0)), None);
}

#[test]
fn separator_segments_next_to_the_active_pane_are_accent() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let separators = [
        Separator {
            axis: Axis::Horizontal,
            rect: cells(10, 0, 1, 20),
        },
        Separator {
            axis: Axis::Vertical,
            rect: cells(11, 9, 20, 1),
        },
    ];
    let panes = [
        (1, cells(0, 0, 10, 20)),
        (2, cells(11, 0, 20, 9)),
        (3, cells(11, 10, 20, 10)),
    ];
    let lines = |active| {
        let window = window(&separators, &panes, active);
        run_layout(1.0, 800, Some(&window), None)
            .chrome
            .quads
            .into_iter()
            .filter(|quad| quad.rect.y >= 39.0)
            .map(|quad| (quad.rect, quad.color))
            .collect::<Vec<_>>()
    };
    let outline = opaque(surfaces.outline);
    let accent = opaque(theme.accent);
    let vertical = rect(92.0, 52.0, 1.0, 320.0);
    let horizontal = rect(96.0, 204.0, 160.0, 1.0);
    assert_eq!(
        lines(1),
        [
            (vertical, outline),
            (vertical, accent),
            (horizontal, outline)
        ]
    );
    assert_eq!(
        lines(2),
        [
            (vertical, outline),
            (rect(92.0, 52.0, 1.0, 144.0), accent),
            (horizontal, outline),
            (horizontal, accent),
        ]
    );
    assert_eq!(
        lines(3),
        [
            (vertical, outline),
            (rect(92.0, 212.0, 1.0, 160.0), accent),
            (horizontal, outline),
            (horizontal, accent),
        ]
    );
}

#[test]
fn zoomed_windows_draw_no_separators() {
    let window = WindowLayout {
        zoomed: true,
        ..window(&[], &[(1, cells(0, 0, 98, 34))], 1)
    };
    let layout = run_layout(1.0, 800, Some(&window), None);
    assert!(layout.chrome.quads.iter().all(|quad| quad.rect.y < 39.0));
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
    let window = window(&separators, &[], 9);
    let layout = run_layout(1.0, 200, Some(&window), None);
    let borders: Vec<PxRect> = layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.color == opaque(surfaces.outline) && quad.rect.y >= 39.0)
        .map(|quad| quad.rect)
        .collect();
    assert_eq!(layout.chrome.clip, rect(8.0, 39.0, 184.0, 557.0));
    assert_eq!(
        borders,
        [rect(92.0, 52.0, 1.0, 544.0), rect(8.0, 140.0, 184.0, 1.0)]
    );
}
