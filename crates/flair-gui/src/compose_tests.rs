use super::*;
use crate::{
    Fonts,
    renderer::{HeadlessContext, Renderer},
};
use flair_core::{
    config::Config,
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::WindowId,
    proto::{
        Cell, Cursor, CursorShape, PaneFlags, PanePlacement, PaneUpdate, SelectionSpan, Separator,
        WindowLayout,
    },
    status::{StatusLine, StatusWindow},
};
use std::{fs, io::Write};

#[test]
fn compose_resolves_inverse_and_cursor() {
    let theme = Config::default().theme();
    let mut mirror = Mirror::default();
    mirror.apply(&[PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols: 2, rows: 1 },
        full: true,
        rows: vec![(
            0,
            vec![
                Cell {
                    c: 'a',
                    flags: CellFlags::INVERSE,
                    ..Cell::default()
                },
                Cell::default(),
            ],
        )],
        cursor: Cursor {
            point: CellPoint { col: 1, row: 0 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags::default(),
        selection: vec![],
    }]);
    let grid = compose(&mirror, PaneId(1), &theme, true);
    assert_eq!(grid.to_lines(), vec!["a"]);
    assert_eq!(grid.cells[0].fg, theme.background);
    assert_eq!(grid.cells[0].bg, theme.foreground);
    assert_eq!(grid.cursor.unwrap().color, theme.cursor);
    assert!(compose(&mirror, PaneId(1), &theme, false).cursor.is_none());
}

#[test]
fn selection_overlay_blends_the_theme_color_and_clears_on_next_update() {
    let mut theme = Config::default().theme();
    theme.background = Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    theme.selection = flair_core::theme::Rgba {
        r: 110,
        g: 120,
        b: 130,
        a: 255,
    };
    let mut update = PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols: 2, rows: 1 },
        full: true,
        rows: vec![(0, vec![Cell::default(); 2])],
        cursor: Cursor {
            point: CellPoint { col: 0, row: 0 },
            shape: CursorShape::Block,
            visible: false,
        },
        flags: PaneFlags::default(),
        selection: vec![SelectionSpan {
            row: 0,
            start: 0,
            end: 0,
        }],
    };
    let mut mirror = Mirror::default();
    mirror.apply(&[update.clone()]);
    let grid = compose(&mirror, PaneId(1), &theme, false);
    assert_eq!(
        grid.cells[0].bg,
        Rgb {
            r: 110,
            g: 120,
            b: 130
        }
    );
    assert_eq!(grid.cells[1].bg, theme.background);
    update.full = false;
    update.rows.clear();
    update.selection.clear();
    mirror.apply(&[update]);
    let grid = compose(&mirror, PaneId(1), &theme, false);
    assert!(grid.cells.iter().all(|cell| cell.bg == theme.background));
}

#[test]
fn frame_places_panes_below_status_and_only_shows_active_cursor() {
    let theme = Config::default().theme();
    let make_pane = |id, text: &str| PaneUpdate {
        pane: PaneId(id),
        size: GridSize { cols: 2, rows: 1 },
        full: true,
        rows: vec![(
            0,
            text.chars()
                .map(|c| Cell {
                    c,
                    ..Cell::default()
                })
                .collect(),
        )],
        cursor: Cursor {
            point: CellPoint { col: 1, row: 0 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags::default(),
        selection: vec![],
    };
    let mut mirror = Mirror::default();
    mirror.apply(&[make_pane(1, "ab"), make_pane(2, "cd")]);
    let rect = |col, cols| CellRect {
        origin: CellPoint { col, row: 1 },
        size: GridSize { cols, rows: 1 },
    };
    let layout = WindowLayout {
        window: WindowId(1),
        area: rect(0, 5),
        panes: vec![
            PanePlacement {
                pane: PaneId(1),
                rect: rect(0, 2),
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(3, 2),
            },
        ],
        separators: vec![Separator {
            axis: Axis::Horizontal,
            rect: rect(2, 1),
        }],
        active: PaneId(2),
        zoomed: false,
    };
    let size = GridSize { cols: 5, rows: 2 };
    let grid = compose_frame(&mirror, &layout, &StatusLine::default(), size, &theme, true);
    assert_eq!(grid.to_lines()[1], "ab│cd");
    assert_eq!(grid.cursor.unwrap().point, CellPoint { col: 4, row: 1 });
    assert_eq!(grid.cells[0].bg, Rgb::default());
    assert_eq!(grid.cells[5].bg, theme.background);
    assert!(
        compose_frame(
            &mirror,
            &layout,
            &StatusLine::default(),
            size,
            &theme,
            false
        )
        .cursor
        .is_none()
    );
    assert_eq!(mirror.pane(PaneId(2)).unwrap().cursor.point.row, 0);
}

#[test]
fn frame_clips_stale_rectangles_without_drawing_over_status() {
    let theme = Config::default().theme();
    let mut mirror = Mirror::default();
    mirror.apply(&[PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols: 2, rows: 1 },
        full: true,
        rows: vec![(
            0,
            vec![
                Cell {
                    c: '界',
                    flags: CellFlags::WIDE,
                    ..Cell::default()
                },
                Cell {
                    flags: CellFlags::WIDE_SPACER,
                    ..Cell::default()
                },
            ],
        )],
        cursor: Cursor {
            point: CellPoint { col: 0, row: 0 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags::default(),
        selection: vec![],
    }]);
    for origin in [
        CellPoint { col: 0, row: 0 },
        CellPoint { col: 1, row: 1 },
        CellPoint {
            col: u16::MAX,
            row: u16::MAX,
        },
    ] {
        let rect = CellRect {
            origin,
            size: GridSize { cols: 2, rows: 1 },
        };
        let layout = WindowLayout {
            window: WindowId(1),
            area: rect,
            panes: vec![PanePlacement {
                pane: PaneId(1),
                rect,
            }],
            separators: vec![],
            active: PaneId(1),
            zoomed: false,
        };
        for size in [
            GridSize { cols: 0, rows: 0 },
            GridSize { cols: 2, rows: 1 },
            GridSize { cols: 2, rows: 2 },
        ] {
            let grid = compose_frame(&mirror, &layout, &StatusLine::default(), size, &theme, true);
            assert_eq!(
                grid.cells.len(),
                usize::from(size.cols) * usize::from(size.rows)
            );
            assert!(grid.cells.iter().all(|cell| cell.c != '界'));
            assert!(grid.cursor.is_none_or(|cursor| cursor.point.row > 0));
        }
    }
}

#[test]
fn gpu_draws_status_split_panes_and_active_cursor() {
    let (ctx, gl) = match HeadlessContext::new((800, 240)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped frame EGL smoke: {error}");
            return;
        }
    };
    let config = Config::default();
    let theme = config.theme();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let mut mirror = Mirror::default();
    for (id, cols, text) in [(1, 29, "left pane: shell"), (2, 30, "right pane: editor")] {
        mirror.apply(&[PaneUpdate {
            pane: PaneId(id),
            size: GridSize { cols, rows: 5 },
            full: true,
            rows: vec![(
                0,
                text.chars()
                    .map(|c| Cell {
                        c,
                        ..Cell::default()
                    })
                    .collect(),
            )],
            cursor: Cursor {
                point: CellPoint { col: 0, row: 1 },
                shape: CursorShape::Block,
                visible: true,
            },
            flags: PaneFlags::default(),
            selection: vec![],
        }]);
    }
    let rect = |col, cols| CellRect {
        origin: CellPoint { col, row: 1 },
        size: GridSize { cols, rows: 5 },
    };
    let layout = WindowLayout {
        window: WindowId(2),
        area: rect(0, 60),
        panes: vec![
            PanePlacement {
                pane: PaneId(1),
                rect: rect(0, 29),
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(30, 30),
            },
        ],
        separators: vec![Separator {
            axis: Axis::Horizontal,
            rect: rect(29, 1),
        }],
        active: PaneId(2),
        zoomed: false,
    };
    let status = StatusLine {
        session: "main".into(),
        pending: Some("^A".into()),
        windows: vec![
            StatusWindow {
                id: WindowId(1),
                index: 1,
                name: "shell".into(),
                active: false,
                last: true,
            },
            StatusWindow {
                id: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: true,
                last: false,
            },
        ],
    };
    let grid = compose_frame(
        &mirror,
        &layout,
        &status,
        GridSize { cols: 60, rows: 6 },
        &theme,
        true,
    );
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((800, 240));
    renderer
        .draw(&grid, &mut fonts, (16.0, 8.0), theme.background)
        .unwrap();
    let pixels = renderer.read_pixels();
    let pixel =
        |x: usize, y: usize| &pixels[((239 - y) * 800 + x) * 4..((239 - y) * 800 + x) * 4 + 3];
    for col in [1, 31] {
        assert!(
            (8 + metrics.height as usize..8 + 2 * metrics.height as usize).any(|y| {
                ((16.0 + col as f32 * metrics.width) as usize
                    ..(16.0 + (col + 10) as f32 * metrics.width) as usize)
                    .any(|x| pixel(x, y).iter().any(|c| *c > 30))
            })
        );
    }
    assert_eq!(pixel(16, 8), &[0, 0, 0]);
    let cursor_x = (16.0 + 30.5 * metrics.width) as usize;
    let cursor_y = (8.0 + 2.5 * metrics.height) as usize;
    assert_eq!(
        pixel(cursor_x, cursor_y),
        &[theme.cursor.r, theme.cursor.g, theme.cursor.b]
    );
    if let Some(path) = std::env::var_os("FLAIR_FRAME_PPM") {
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n800 240\n255\n").unwrap();
        for y in 0..240 {
            for x in 0..800 {
                file.write_all(pixel(x, y)).unwrap();
            }
        }
    }
    drop(renderer);
    drop(ctx);
}

#[test]
fn stacked_frame_moves_selection_with_cells_and_draws_horizontal_separator() {
    let mut theme = Config::default().theme();
    theme.selection.a = 255;
    let mut mirror = Mirror::default();
    for (id, text) in [(1, "ab"), (2, "cd")] {
        mirror.apply(&[PaneUpdate {
            pane: PaneId(id),
            size: GridSize { cols: 3, rows: 1 },
            full: true,
            rows: vec![(
                0,
                text.chars()
                    .map(|c| Cell {
                        c,
                        ..Cell::default()
                    })
                    .collect(),
            )],
            cursor: Cursor {
                point: CellPoint { col: 0, row: 0 },
                shape: CursorShape::Block,
                visible: true,
            },
            flags: PaneFlags::default(),
            selection: vec![SelectionSpan {
                row: 0,
                start: 0,
                end: 0,
            }],
        }]);
    }
    let rect = |row| CellRect {
        origin: CellPoint { col: 0, row },
        size: GridSize { cols: 3, rows: 1 },
    };
    let layout = WindowLayout {
        window: WindowId(1),
        area: CellRect {
            origin: CellPoint { col: 0, row: 1 },
            size: GridSize { cols: 3, rows: 3 },
        },
        panes: vec![
            PanePlacement {
                pane: PaneId(1),
                rect: rect(1),
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(3),
            },
        ],
        separators: vec![Separator {
            axis: Axis::Vertical,
            rect: rect(2),
        }],
        active: PaneId(1),
        zoomed: false,
    };
    let grid = compose_frame(
        &mirror,
        &layout,
        &StatusLine::default(),
        GridSize { cols: 3, rows: 4 },
        &theme,
        true,
    );
    assert_eq!(&grid.to_lines()[1..], &["ab", "───", "cd"]);
    assert_eq!(grid.cursor.unwrap().point, CellPoint { col: 0, row: 1 });
    assert_eq!(
        grid.at(CellPoint { col: 0, row: 3 }).bg,
        Rgb {
            r: theme.selection.r,
            g: theme.selection.g,
            b: theme.selection.b
        }
    );
}
