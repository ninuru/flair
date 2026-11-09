use super::*;
use crate::{
    CellMetrics, Fonts,
    chrome_layout::{
        self, ChromeInput, ChromeLayout, Modal, bar_height, grid_origin, grid_size, modal_area,
    },
    picker::Picker,
    renderer::{HeadlessContext, Renderer},
    status::status_parts,
};
use flair_core::{
    config::Config,
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::WindowId,
    proto::{
        Cell, Cursor, CursorShape, PaneFlags, PanePlacement, PaneUpdate, SelectionSpan, Separator,
        SessionInfo, WindowLayout,
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
fn frame_places_panes_from_row_zero_and_only_shows_active_cursor() {
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
        origin: CellPoint { col, row: 0 },
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
    let size = GridSize { cols: 5, rows: 1 };
    let mut grid = compose_frame(&mirror, &layout, size, &theme, true);
    assert_eq!(grid.to_lines(), ["ab cd"]);
    assert_eq!(grid.cells[2].bg, theme.background);
    assert_eq!(grid.cursor.unwrap().point, CellPoint { col: 4, row: 0 });
    assert!(
        compose_frame(&mirror, &layout, size, &theme, false)
            .cursor
            .is_none()
    );
    separator_glyphs(&mut grid, &layout);
    assert_eq!(grid.to_lines(), ["ab│cd"]);
    assert_eq!(mirror.pane(PaneId(2)).unwrap().cursor.point.row, 0);
}

#[test]
fn frame_clips_stale_rectangles_and_wide_glyphs_at_the_edge() {
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
    let layout = |origin| {
        let rect = CellRect {
            origin,
            size: GridSize { cols: 2, rows: 1 },
        };
        WindowLayout {
            window: WindowId(1),
            area: rect,
            panes: vec![PanePlacement {
                pane: PaneId(1),
                rect,
            }],
            separators: vec![],
            active: PaneId(1),
            zoomed: false,
        }
    };
    let sizes = [
        GridSize { cols: 0, rows: 0 },
        GridSize { cols: 2, rows: 1 },
        GridSize { cols: 2, rows: 2 },
    ];
    for origin in [
        CellPoint { col: 1, row: 1 },
        CellPoint {
            col: u16::MAX,
            row: u16::MAX,
        },
    ] {
        for size in sizes {
            let grid = compose_frame(&mirror, &layout(origin), size, &theme, true);
            assert_eq!(
                grid.cells.len(),
                usize::from(size.cols) * usize::from(size.rows)
            );
            assert!(grid.cells.iter().all(|cell| cell.c != '界'));
            assert!(
                grid.cursor
                    .is_none_or(|cursor| cursor.point.row < size.rows)
            );
        }
    }
    let top = layout(CellPoint { col: 0, row: 0 });
    let grid = compose_frame(&mirror, &top, sizes[1], &theme, true);
    assert_eq!(grid.to_lines(), ["界"]);
    assert_eq!(grid.cursor.unwrap().point, CellPoint { col: 0, row: 0 });
    assert!(
        compose_frame(&mirror, &top, sizes[0], &theme, true)
            .cells
            .is_empty()
    );
}

struct Frame {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

impl Frame {
    fn at(&self, x: f32, y: f32) -> [u8; 3] {
        let (x, y) = (x as usize, y as usize);
        let i = ((self.height - 1 - y) * self.width + x) * 4;
        [self.pixels[i], self.pixels[i + 1], self.pixels[i + 2]]
    }
    fn dump(&self, var: &str) {
        let Some(path) = std::env::var_os(var) else {
            return;
        };
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n{} {}\n255\n", self.width, self.height).unwrap();
        for y in 0..self.height {
            for x in 0..self.width {
                file.write_all(&self.at(x as f32, y as f32)).unwrap();
            }
        }
    }
}

fn rgb(color: Rgb) -> [u8; 3] {
    [color.r, color.g, color.b]
}

fn chrome_status() -> StatusLine {
    StatusLine {
        session: "main".into(),
        hidden: 2,
        running: true,
        finished: false,
        pending: None,
        copy: false,
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
    }
}

fn split_frame(size: GridSize) -> (Mirror, WindowLayout) {
    let left = (size.cols - 1) / 2;
    let rect = |col, cols| CellRect {
        origin: CellPoint { col, row: 0 },
        size: GridSize {
            cols,
            rows: size.rows,
        },
    };
    let mut mirror = Mirror::default();
    for (id, cols, text) in [
        (1, left, " left pane: shell"),
        (2, size.cols - left - 1, "right pane: editor"),
    ] {
        mirror.apply(&[PaneUpdate {
            pane: PaneId(id),
            size: GridSize {
                cols,
                rows: size.rows,
            },
            full: true,
            rows: vec![(
                0,
                text.chars()
                    .enumerate()
                    .map(|(index, c)| Cell {
                        c,
                        flags: if index == 0 {
                            CellFlags::INVERSE
                        } else {
                            CellFlags::empty()
                        },
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
    let layout = WindowLayout {
        window: WindowId(2),
        area: rect(0, size.cols),
        panes: vec![
            PanePlacement {
                pane: PaneId(1),
                rect: rect(0, left),
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(left + 1, size.cols - left - 1),
            },
        ],
        separators: vec![Separator {
            axis: Axis::Horizontal,
            rect: rect(left, 1),
        }],
        active: PaneId(2),
        zoomed: false,
    };
    (mirror, layout)
}

fn draw_chrome_frame(picker: bool, name: &str) -> Option<(Frame, ChromeLayout, CellMetrics)> {
    const W: u32 = 800;
    const H: u32 = 300;
    let (ctx, gl) = match HeadlessContext::new((W, H)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped {name} EGL frame: {error}");
            return None;
        }
    };
    let config = Config::default();
    let theme = config.theme();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let padding = (
        f32::from(config.window.padding_x),
        f32::from(config.window.padding_y),
    );
    let size = grid_size((W, H), padding, 1.0, metrics);
    let (mirror, layout) = split_frame(size);
    let mut grid = compose_frame(&mirror, &layout, size, &theme, true);
    let lifted = picker.then(|| {
        Picker::sessions(
            ["main", "notes", "work"]
                .into_iter()
                .map(|name| SessionInfo {
                    name: name.into(),
                    root: format!("/tmp/{name}").into(),
                    windows: 1,
                })
                .collect(),
            "main",
        )
        .draw(&mut grid, modal_area(size, metrics, 1.0), &theme)
    });
    let parts = status_parts(&chrome_status());
    let input = ChromeInput {
        px: (W, H),
        scale: 1.0,
        padding,
        metrics,
        ui: fonts.ui_metrics(),
        theme: &theme,
        status: &parts,
        prompt: None,
        separators: &layout.separators,
        modal: lifted.map(Modal::Centered),
    };
    let chrome = chrome_layout::layout(&input, |text, bold| fonts.ui_width(text, bold));
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((W, H));
    renderer
        .draw_frame(&grid, &chrome.chrome, &mut fonts, theme.background)
        .unwrap();
    let frame = Frame {
        width: W as usize,
        height: H as usize,
        pixels: renderer.read_pixels(),
    };
    drop(renderer);
    drop(ctx);
    Some((frame, chrome, metrics))
}

#[test]
fn gpu_frame_draws_bar_border_pane_border_and_grid_below_the_bar() {
    let Some((frame, chrome, metrics)) = draw_chrome_frame(false, "chrome") else {
        return;
    };
    frame.dump("FLAIR_FRAME_PPM");
    let config = Config::default();
    let theme = config.theme();
    let surfaces = theme.surfaces();
    let bar = bar_height(1.0);
    assert_eq!(bar, 34.0);
    for x in [2.0, 400.0, 797.0] {
        assert_eq!(frame.at(x, 1.0), rgb(surfaces.surface1), "bar at {x}");
        assert_eq!(frame.at(x, bar - 1.0), rgb(surfaces.surface1), "bar at {x}");
        assert_eq!(frame.at(x, bar), rgb(surfaces.outline), "border at {x}");
    }
    let dot = chrome
        .chrome
        .quads
        .iter()
        .find(|quad| quad.rect.w == 6.0 && quad.radius == 3.0)
        .expect("running dot");
    assert_eq!(
        frame.at(dot.rect.x + 3.0, dot.rect.y + 3.0),
        rgb(theme.accent)
    );
    let origin = grid_origin(
        (
            f32::from(config.window.padding_x),
            f32::from(config.window.padding_y),
        ),
        1.0,
    );
    assert_eq!(origin, (16.0, 35.0 + 8.0));
    assert_eq!(chrome.chrome.origin, origin);
    assert_eq!(frame.at(origin.0 + 1.0, origin.1), rgb(theme.foreground));
    assert_eq!(
        frame.at(origin.0 + 1.0, origin.1 - 1.0),
        rgb(theme.background)
    );
    let size = grid_size((800, 300), (16.0, 8.0), 1.0, metrics);
    let gap = origin.0 + f32::from((size.cols - 1) / 2) * metrics.width;
    let line = (gap + (metrics.width - 1.0) / 2.0).round();
    let y = origin.1 + metrics.height * 3.5;
    assert_eq!(frame.at(line, y), rgb(surfaces.pane_border));
    assert_eq!(frame.at(line - 1.0, y), rgb(theme.background));
    assert_eq!(frame.at(line + 1.0, y), rgb(theme.background));
}

#[test]
fn gpu_frame_draws_a_picker_in_a_rounded_modal_over_a_backdrop() {
    let Some((frame, chrome, _)) = draw_chrome_frame(true, "modal") else {
        return;
    };
    frame.dump("FLAIR_MODAL_PPM");
    let surfaces = Config::default().theme().surfaces();
    let [backdrop, modal] = chrome.chrome.modal.as_slice() else {
        panic!("modal quads {:?}", chrome.chrome.modal);
    };
    assert_eq!(backdrop.color.a, 179);
    let dimmed = frame.at(797.0, 1.0);
    assert!(
        dimmed.iter().all(|channel| *channel <= 5),
        "backdrop over the bar is {dimmed:?}"
    );
    let rect = modal.rect;
    assert_eq!(modal.radius, 8.0);
    assert_ne!(frame.at(rect.x, rect.y), rgb(surfaces.surface1));
    assert_eq!(frame.at(rect.x + 8.0, rect.y + 8.0), rgb(surfaces.surface1));
    assert_eq!(
        frame.at(rect.x + rect.w / 2.0, rect.y),
        rgb(surfaces.outline)
    );
    assert_eq!(
        frame.at(rect.x, rect.y + rect.h / 2.0),
        rgb(surfaces.outline)
    );
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
            origin: CellPoint { col: 0, row: 0 },
            size: GridSize { cols: 3, rows: 3 },
        },
        panes: vec![
            PanePlacement {
                pane: PaneId(1),
                rect: rect(0),
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(2),
            },
        ],
        separators: vec![Separator {
            axis: Axis::Vertical,
            rect: rect(1),
        }],
        active: PaneId(1),
        zoomed: false,
    };
    let mut grid = compose_frame(
        &mirror,
        &layout,
        GridSize { cols: 3, rows: 3 },
        &theme,
        true,
    );
    assert_eq!(grid.to_lines(), ["ab", "", "cd"]);
    separator_glyphs(&mut grid, &layout);
    assert_eq!(grid.to_lines(), ["ab", "───", "cd"]);
    assert_eq!(grid.cursor.unwrap().point, CellPoint { col: 0, row: 0 });
    assert_eq!(
        grid.at(CellPoint { col: 0, row: 2 }).bg,
        Rgb {
            r: theme.selection.r,
            g: theme.selection.g,
            b: theme.selection.b
        }
    );
}
