use super::*;
use crate::{
    CellMetrics, Fonts,
    chrome_layout::{self, ChromeInput, ChromeLayout, bar_height, grid_origin, grid_size},
    isles::Isles,
    overlays::Overlays,
    renderer::{HeadlessContext, Renderer},
    status::status_parts,
};
use flair_core::{
    config::{Config, PromptConfig},
    geom::{Axis, CellPoint, CellRect, GridSize},
    ids::WindowId,
    isles::{AheadBehind, Changes, GitInfo, Head, LastCommand, PromptInfo, ShellFacts},
    proto::{
        Cell, Cursor, CursorShape, PaneFlags, PanePlacement, PaneUpdate, PromptSnapshot,
        SelectionSpan, Separator, ServerMsg, SessionInfo, WindowLayout,
    },
    status::{StatusLine, StatusWindow},
    time::Millis,
};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

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
fn failed_command_blocks_get_a_red_tint_up_to_the_next_prompt() {
    let theme = Config::default().theme();
    let text = |c| {
        vec![Cell {
            c,
            ..Cell::default()
        }]
    };
    let mut mirror = Mirror::default();
    mirror.apply(&[PaneUpdate {
        pane: PaneId(1),
        size: GridSize { cols: 1, rows: 7 },
        full: true,
        rows: ['_', ' ', 'a', 'x', ' ', ' ', 'b']
            .into_iter()
            .enumerate()
            .map(|(row, c)| (row as u16, text(c)))
            .collect(),
        cursor: Cursor {
            point: CellPoint { col: 0, row: 6 },
            shape: CursorShape::Block,
            visible: false,
        },
        flags: PaneFlags::default(),
        selection: vec![],
    }]);
    let prompt = |row, exit: Option<i32>| PromptSnapshot {
        row,
        id: u64::from(row),
        info: PromptInfo {
            last: exit.map(|exit| LastCommand {
                exit: Some(exit),
                duration: Millis(0),
            }),
            ..PromptInfo::default()
        },
        timestamp: 0,
    };
    let tinted = |mirror: &Mirror| -> Vec<bool> {
        compose(mirror, PaneId(1), &theme, false)
            .cells
            .iter()
            .map(|cell| cell.bg != theme.background)
            .collect()
    };
    mirror.insert_history(PaneId(1), vec![prompt(1, None), prompt(5, Some(1))]);
    assert_eq!(
        tinted(&mirror),
        [false, true, true, true, false, false, false]
    );
    mirror.insert_history(PaneId(1), vec![prompt(1, None), prompt(5, Some(0))]);
    assert_eq!(tinted(&mirror), [false; 7]);
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
                band: 0,
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(3, 2),
                band: 0,
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
                band: 0,
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
                band: 0,
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(left + 1, size.cols - left - 1),
                band: 0,
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

fn draw_chrome_frame(
    picker: bool,
    paint: Option<(Rgb, Rgb)>,
    name: &str,
) -> Option<(Frame, ChromeLayout, CellMetrics)> {
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
    let size = grid_size((W, H), padding, 1.0, false, metrics);
    let (mirror, layout) = split_frame(size);
    let mut grid = compose_frame(&mirror, &layout, size, &theme, true);
    if let Some((first, rest)) = paint {
        for (index, cell) in grid.cells.iter_mut().enumerate() {
            cell.bg = if index < usize::from(size.cols) {
                first
            } else {
                rest
            };
        }
    }
    let mut overlays = Overlays::default();
    if picker {
        assert!(
            overlays.open(
                &ServerMsg::ChooseSession {
                    sessions: ["main", "notes", "work"]
                        .into_iter()
                        .map(|name| SessionInfo {
                            name: name.into(),
                            root: format!("/tmp/{name}").into(),
                            windows: 1,
                        })
                        .collect(),
                    current: "main".into(),
                },
                &StatusLine::default(),
                &config,
                Path::new("/nonexistent/flair.toml"),
            )
        );
    }
    let parts = status_parts(&chrome_status());
    let input = ChromeInput {
        px: (W, H),
        scale: 1.0,
        padding,
        zen: false,
        metrics,
        ui: fonts.ui_metrics(),
        theme: &theme,
        status: &parts,
        prompt: None,
        layout: Some(&layout),
        hover: None,
        overlays: Some(&overlays),
        notice: None,
        isles: None,
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
fn gpu_frame_draws_bar_border_active_separator_and_grid_below_the_bar() {
    let Some((frame, chrome, metrics)) = draw_chrome_frame(false, None, "chrome") else {
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
    let mark = chrome
        .chrome
        .quads
        .iter()
        .find(|quad| quad.rect.w == 18.0 && quad.radius == 4.0)
        .expect("session monogram");
    assert_eq!(
        frame.at(mark.rect.x + 2.0, mark.rect.y + 2.0),
        [mark.color.r, mark.color.g, mark.color.b]
    );
    let right = mark.rect.x + mark.rect.w;
    for y in mark.rect.y as usize..(mark.rect.y + mark.rect.h) as usize {
        for x in [mark.rect.x - 2.0, mark.rect.x - 1.0, right, right + 1.0] {
            assert_eq!(
                frame.at(x, y as f32),
                rgb(surfaces.surface1),
                "monogram letters spill at {x},{y}"
            );
        }
    }
    let padding = (
        f32::from(config.window.padding_x),
        f32::from(config.window.padding_y),
    );
    let origin = grid_origin((800, 300), padding, 1.0, false, metrics);
    assert!(
        origin.0 >= padding.0 && origin.1 >= 35.0 + padding.1,
        "{origin:?}"
    );
    assert_eq!(chrome.chrome.origin, origin);
    assert_eq!(frame.at(origin.0 + 1.0, origin.1), rgb(theme.foreground));
    let above = if origin.1 > 35.0 {
        rgb(theme.background)
    } else {
        rgb(surfaces.outline)
    };
    for y in [35.0, origin.1 - 1.0] {
        assert_eq!(frame.at(origin.0 + 1.0, y), above, "top margin at {y}");
    }
    assert_eq!(
        frame.at(origin.0 - 1.0, origin.1 + 1.0),
        rgb(theme.foreground)
    );
    let size = grid_size((800, 300), padding, 1.0, false, metrics);
    let bottom = origin.1 + f32::from(size.rows) * metrics.height;
    assert!(
        (300.0 - padding.1 - bottom).abs() < 1.0,
        "{bottom} {padding:?}"
    );
    let gap = origin.0 + f32::from((size.cols - 1) / 2) * metrics.width;
    let line = (gap + (metrics.width - 1.0) / 2.0).round();
    let y = origin.1 + metrics.height * 3.5;
    assert_eq!(frame.at(line, y), rgb(theme.accent));
    assert_eq!(frame.at(line - 1.0, y), rgb(theme.background));
    assert_eq!(frame.at(line + 1.0, y), rgb(theme.background));
    for y in [35.0, origin.1 - 1.0, 299.0] {
        assert_eq!(frame.at(line, y), rgb(theme.accent), "separator at {y}");
    }
}

#[test]
fn gpu_frame_extends_edge_cell_colours_into_the_padding() {
    let first = Rgb {
        r: 0x1a,
        g: 0x2b,
        b: 0x3c,
    };
    let rest = Rgb {
        r: 0x0f,
        g: 0x10,
        b: 0x11,
    };
    let Some((frame, chrome, metrics)) = draw_chrome_frame(false, Some((first, rest)), "extend")
    else {
        return;
    };
    frame.dump("FLAIR_EXTEND_PPM");
    let config = Config::default();
    let theme = config.theme();
    let surfaces = theme.surfaces();
    let origin = chrome.chrome.origin;
    let padding = (
        f32::from(config.window.padding_x),
        f32::from(config.window.padding_y),
    );
    let size = grid_size((800, 300), padding, 1.0, false, metrics);
    let right = origin.0 + f32::from(size.cols) * metrics.width;
    let bottom = origin.1 + f32::from(size.rows) * metrics.height;
    let row = origin.1 + metrics.height * 3.5;
    for x in [0.0, 400.0, 799.0] {
        assert_eq!(frame.at(x, 34.0), rgb(surfaces.outline), "bar line at {x}");
        assert_eq!(frame.at(x, 35.0), rgb(first), "top margin at {x}");
        assert_eq!(frame.at(x, 299.0), rgb(rest), "bottom margin at {x}");
    }
    assert!(bottom <= 300.0 && right < 799.0, "{right} {bottom}");
    for x in [0.0, origin.0 - 1.0, right, 799.0] {
        assert_eq!(frame.at(x, row), rgb(rest), "side margin at {x}");
        assert_eq!(
            frame.at(x, origin.1 + 1.0),
            rgb(first),
            "first row margin at {x}"
        );
    }
    let gap = origin.0 + f32::from((size.cols - 1) / 2) * metrics.width;
    let line = (gap + (metrics.width - 1.0) / 2.0).round();
    assert_eq!(frame.at(line, row), rgb(theme.accent));
    assert_eq!(frame.at(line - 1.0, row), rgb(rest));
    assert_eq!(frame.at(line + 1.0, row), rgb(rest));
}

#[test]
fn gpu_frame_draws_a_picker_in_a_rounded_modal_over_a_backdrop() {
    let Some((frame, chrome, _)) = draw_chrome_frame(true, None, "modal") else {
        return;
    };
    frame.dump("FLAIR_MODAL_PPM");
    let surfaces = Config::default().theme().surfaces();
    let [backdrop, modal, ..] = chrome.chrome.modal.as_slice() else {
        panic!("modal quads {:?}", chrome.chrome.modal);
    };
    assert_eq!(backdrop.color.a, 102);
    assert!(!chrome.chrome.modal_text.is_empty());
    assert_eq!(frame.at(797.0, 1.0), rgb(surfaces.surface1));
    assert_eq!(frame.at(797.0, 34.0), rgb(surfaces.outline));
    let rect = modal.rect;
    assert_eq!(modal.radius, 10.0);
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
    assert_eq!(
        frame.at(rect.x + rect.w / 2.0, rect.y + 4.0),
        rgb(surfaces.surface1)
    );
    let hits = chrome.overlay.as_ref().unwrap();
    let (index, row) = hits.rows[0];
    assert_eq!(index, 0);
    assert_eq!(
        frame.at(row.x + row.w / 2.0, row.y + row.h - 3.0),
        rgb(surfaces.surface3)
    );
    assert_eq!(
        frame.at(row.x + row.w / 2.0, row.y + row.h + 1.0),
        rgb(surfaces.surface1)
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
                band: 0,
            },
            PanePlacement {
                pane: PaneId(2),
                rect: rect(2),
                band: 0,
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

fn banded(layout: WindowLayout) -> WindowLayout {
    WindowLayout {
        panes: layout
            .panes
            .iter()
            .map(|placement| PanePlacement {
                rect: CellRect {
                    origin: CellPoint {
                        row: placement.rect.origin.row + 1,
                        ..placement.rect.origin
                    },
                    size: GridSize {
                        rows: placement.rect.size.rows - 1,
                        ..placement.rect.size
                    },
                },
                band: 1,
                ..*placement
            })
            .collect(),
        ..layout
    }
}

#[test]
fn the_band_row_above_a_pane_stays_blank_in_the_cell_grid() {
    let theme = Config::default().theme();
    let size = GridSize { cols: 20, rows: 4 };
    let (mirror, layout) = split_frame(size);
    let layout = banded(layout);
    let grid = compose_frame(&mirror, &layout, size, &theme, true);
    assert!(
        grid.cells[..20]
            .iter()
            .all(|cell| cell.c == ' ' && cell.bg == theme.background)
    );
    assert!(
        grid.to_lines()[1].contains("left pan"),
        "{:?}",
        grid.to_lines()
    );
    assert_eq!(grid.cursor.map(|cursor| cursor.point.row), Some(2));
}

fn band_prompts() -> BTreeMap<PaneId, PromptInfo> {
    let left = PromptInfo {
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
        kube: Some("prod".into()),
        ..PromptInfo::default()
    };
    let right = PromptInfo {
        cwd: Some(PathBuf::from(
            "/home/ada/work/a-rather-long-project-name/crates/server",
        )),
        git: Some(GitInfo {
            head: Head::Detached("a1b2c3d".into()),
            upstream: None,
            changes: None,
        }),
        last: Some(LastCommand {
            exit: Some(0),
            duration: Millis(12_400),
        }),
        shell: Some(ShellFacts {
            user: "ada".into(),
            host: "box".into(),
            venv: Some("env".into()),
        }),
        docker: Some("colima".into()),
        kube: Some("prod".into()),
    };
    BTreeMap::from([(PaneId(1), left), (PaneId(2), right)])
}

#[test]
fn gpu_frame_draws_chips_on_each_panes_shell_prompt_row() {
    const W: u32 = 960;
    const H: u32 = 400;
    let (ctx, gl) = match HeadlessContext::new((W, H)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped band EGL frame: {error}");
            return;
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
    let size = grid_size((W, H), padding, 1.0, false, metrics);
    let (mut mirror, layout) = split_frame(size);
    let updates: Vec<_> = layout
        .panes
        .iter()
        .map(|placement| PaneUpdate {
            pane: placement.pane,
            size: placement.rect.size,
            full: false,
            rows: Vec::new(),
            cursor: Cursor {
                point: CellPoint { col: 0, row: 3 },
                shape: CursorShape::Block,
                visible: true,
            },
            flags: PaneFlags {
                prompt_row: Some(2),
                ..Default::default()
            },
            selection: Vec::new(),
        })
        .collect();
    mirror.apply(&updates);
    let grid = compose_frame(&mirror, &layout, size, &theme, true);
    let prompts = band_prompts();
    let prompt = PromptConfig {
        time: true,
        user: true,
        host: true,
        venv: true,
        docker: true,
        kube: true,
        ..PromptConfig::default()
    };
    let parts = status_parts(&chrome_status());
    let input = ChromeInput {
        px: (W, H),
        scale: 1.0,
        padding,
        zen: false,
        metrics,
        ui: fonts.ui_metrics(),
        theme: &theme,
        status: &parts,
        prompt: None,
        layout: Some(&layout),
        hover: None,
        overlays: None,
        notice: None,
        isles: Some(Isles {
            info: &prompts,
            mirror: &mirror,
            config: &prompt,
            clock: Some((15, 48)),
            date: Some((2026, 11, 10)),
            home: Some(Path::new("/home/ada")),
        }),
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
    frame.dump("FLAIR_BAND_PPM");
    let origin = grid_origin((W, H), padding, 1.0, false, metrics);
    let prompt_y = origin.1 + 2.0 * metrics.height;
    let chips: Vec<_> = chrome
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.rect.y >= prompt_y && quad.border.is_some())
        .collect();
    assert!(chips.len() >= 4, "{chips:?}");
    for chip in &chips {
        let rect = chip.rect;
        assert!(rect.y >= prompt_y && rect.y + rect.h <= prompt_y + metrics.height);
        assert_eq!(
            frame.at(rect.x + 3.0, rect.y + rect.h / 2.0),
            [chip.color.r, chip.color.g, chip.color.b],
            "{rect:?}"
        );
        assert_eq!(
            frame.at(rect.x - 2.0, rect.y + rect.h / 2.0),
            rgb(theme.background),
            "{rect:?}"
        );
    }
    let row = (origin.1 + metrics.height * 1.5) as usize;
    assert!(
        (0..W as usize).any(|x| frame.at(x as f32, row as f32) != rgb(theme.background)),
        "content starts below the band row"
    );
}
