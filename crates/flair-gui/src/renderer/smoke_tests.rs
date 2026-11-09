use super::*;
use crate::cellgrid::{DrawCell, DrawCursor};
use flair_core::{
    config::Config,
    geom::{CellPoint, CellRect, GridSize},
    proto::{CellFlags, CursorShape},
    theme::{Rgb, Rgba},
};
use std::{fs, io::Write};

#[test]
fn headless_draws_background_glyphs_and_cursor() {
    let (ctx, gl) = match HeadlessContext::new((800, 480)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped EGL smoke: {error}");
            return;
        }
    };
    let config = Config::default();
    let mut fonts = Fonts::load(&config.font, 1.0).expect("system monospace font");
    let metrics = fonts.metrics();
    assert!(
        metrics.height > metrics.width,
        "invalid font metrics {metrics:?}"
    );
    let theme = config.theme();
    let red = Rgb {
        r: 240,
        g: 40,
        b: 30,
    };
    let mut cells = vec![
        DrawCell {
            c: ' ',
            extra: None,
            fg: red,
            bg: theme.background,
            flags: CellFlags::empty()
        };
        80 * 24
    ];
    for (cell, c) in cells.iter_mut().zip("hello".chars()) {
        cell.c = c;
    }
    for (row, (text, flags)) in [
        ("Regular 0123456789 [] {} ()", CellFlags::empty()),
        ("Bold 0123456789 [] {} ()", CellFlags::BOLD),
        ("Italic 0123456789 [] {} ()", CellFlags::ITALIC),
        (
            "Bold italic 0123456789",
            CellFlags::BOLD | CellFlags::ITALIC,
        ),
        (
            "Underlined and struck text",
            CellFlags::UNDERLINE | CellFlags::STRIKE,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        for (col, c) in text.chars().enumerate() {
            let cell = &mut cells[(row + 2) * 80 + col];
            cell.c = c;
            cell.flags = flags;
            cell.fg = theme.foreground;
        }
    }
    let grid = CellGrid {
        size: GridSize { cols: 80, rows: 24 },
        cells,
        cursor: Some(DrawCursor {
            point: CellPoint { col: 5, row: 0 },
            shape: CursorShape::Block,
            color: theme.cursor,
        }),
    };
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((800, 480));
    renderer
        .draw(&grid, &mut fonts, (16.0, 8.0), theme.background)
        .unwrap();
    let pixels = renderer.read_pixels();
    let pixel =
        |x: usize, y: usize| &pixels[((479 - y) * 800 + x) * 4..((479 - y) * 800 + x) * 4 + 3];
    assert_eq!(pixel(0, 0), &[3, 3, 3]);
    let mut red_pixels = 0;
    for y in 8..8 + metrics.height as usize {
        for x in 16..16 + metrics.width as usize {
            let p = pixel(x, y);
            red_pixels += usize::from(p[0] > p[1] * 2 && p[0] > 30);
        }
    }
    assert!(red_pixels > 0, "first glyph has no red pixels");
    let cursor_x = 16 + (metrics.width * 5.0 + metrics.width / 2.0) as usize;
    assert_eq!(
        pixel(cursor_x, 10),
        &[theme.cursor.r, theme.cursor.g, theme.cursor.b]
    );
    if let Some(path) = std::env::var_os("FLAIR_DUMP_PPM") {
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n800 480\n255\n").unwrap();
        for y in 0..480 {
            for x in 0..800 {
                file.write_all(pixel(x, y)).unwrap();
            }
        }
    }
    drop(renderer);
    drop(ctx);
}

#[test]
fn combining_marks_stay_in_their_base_cell() {
    let (ctx, gl) = match HeadlessContext::new((200, 100)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped EGL combining smoke: {error}");
            return;
        }
    };
    let config = Config::default();
    let theme = config.theme();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((200, 100));
    for (col, wide) in [(0, false), (1, false), (1, true)] {
        let mut grid = CellGrid {
            size: GridSize { cols: 4, rows: 1 },
            cells: vec![
                DrawCell {
                    c: ' ',
                    extra: None,
                    fg: theme.foreground,
                    bg: theme.background,
                    flags: CellFlags::empty()
                };
                4
            ],
            cursor: None,
        };
        grid.cells[col].c = if wide { '界' } else { 'e' };
        if wide {
            grid.cells[col].flags = CellFlags::WIDE;
            grid.cells[col + 1].flags = CellFlags::WIDE_SPACER;
        }
        renderer
            .draw(&grid, &mut fonts, (16.0, 8.0), theme.background)
            .unwrap();
        let before = renderer.read_pixels();
        grid.cells[col].extra = Some("\u{301}".into());
        renderer
            .draw(&grid, &mut fonts, (16.0, 8.0), theme.background)
            .unwrap();
        let after = renderer.read_pixels();
        let left = 16 + col * metrics.width as usize;
        let right = left + metrics.width as usize * if wide { 2 } else { 1 };
        let mut changed = 0;
        for y in 0..100 {
            for x in 0..200 {
                let i = (y * 200 + x) * 4;
                if before[i..i + 3] != after[i..i + 3] {
                    assert!(
                        (left..right).contains(&x),
                        "combining accent escaped base cell at x={x}, col={col}, wide={wide}"
                    );
                    changed += 1;
                }
            }
        }
        assert!(
            changed > 0,
            "combining accent was clipped or invisible at col={col}, wide={wide}"
        );
    }
    drop(renderer);
    drop(ctx);
}

fn headless_or_skip(px: (u32, u32), name: &str) -> Option<(HeadlessContext, glow::Context)> {
    match HeadlessContext::new(px) {
        Ok(pair) => Some(pair),
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped EGL {name}: {error}");
            None
        }
    }
}

fn filled_grid(size: GridSize, bg: Rgb) -> CellGrid {
    CellGrid {
        size,
        cells: vec![
            DrawCell {
                c: ' ',
                extra: None,
                fg: Rgb { r: 0, g: 0, b: 0 },
                bg,
                flags: CellFlags::empty()
            };
            usize::from(size.cols) * usize::from(size.rows)
        ],
        cursor: None,
    }
}

fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

#[test]
fn chrome_draws_bar_border_box_backdrop_text_and_lifted_cells() {
    const W: usize = 400;
    const H: usize = 300;
    let Some((ctx, gl)) = headless_or_skip((W as u32, H as u32), "chrome") else {
        return;
    };
    let config = Config::default();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let fill = Rgb {
        r: 24,
        g: 28,
        b: 34,
    };
    let outline = Rgb {
        r: 200,
        g: 100,
        b: 50,
    };
    let box_fill = Rgb {
        r: 40,
        g: 50,
        b: 60,
    };
    let box_border = Rgb {
        r: 120,
        g: 140,
        b: 160,
    };
    let lifted_bg = Rgb {
        r: 60,
        g: 70,
        b: 80,
    };
    let size = GridSize {
        cols: (W as f32 / metrics.width) as u16 + 1,
        rows: (H as f32 / metrics.height) as u16 + 1,
    };
    let mut grid = filled_grid(size, white);
    let lifted = CellRect {
        origin: CellPoint {
            col: (300.0 / metrics.width) as u16,
            row: (120.0 / metrics.height) as u16,
        },
        size: GridSize { cols: 2, rows: 1 },
    };
    for col in lifted.origin.col..lifted.origin.col + 2 {
        let index = usize::from(lifted.origin.row) * usize::from(size.cols) + usize::from(col);
        grid.cells[index].bg = lifted_bg;
    }
    let ui = fonts.ui_metrics();
    let text_width = fonts.ui_width("flair", false);
    let baseline = 22.0;
    let chrome = Chrome {
        origin: (0.0, 35.0),
        clip: PxRect {
            x: 0.0,
            y: 35.0,
            w: W as f32,
            h: H as f32 - 35.0,
        },
        quads: vec![
            Quad {
                rect: PxRect {
                    x: 0.0,
                    y: 0.0,
                    w: W as f32,
                    h: 34.0,
                },
                color: opaque(fill),
                radius: 0.0,
                border: None,
            },
            Quad {
                rect: PxRect {
                    x: 0.0,
                    y: 34.0,
                    w: W as f32,
                    h: 1.0,
                },
                color: opaque(outline),
                radius: 0.0,
                border: None,
            },
        ],
        text: vec![TextRun {
            x: 10.0,
            baseline,
            text: "flair".to_owned(),
            color: white,
            bold: false,
        }],
        modal: vec![
            Quad {
                rect: PxRect {
                    x: 220.0,
                    y: 35.0,
                    w: 180.0,
                    h: H as f32 - 35.0,
                },
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
                rect: PxRect {
                    x: 40.0,
                    y: 60.0,
                    w: 160.0,
                    h: 100.0,
                },
                color: opaque(box_fill),
                radius: 8.0,
                border: Some(Border {
                    color: opaque(box_border),
                    width: 1.0,
                }),
            },
        ],
        lifted: Some(lifted),
    };
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((W as u32, H as u32));
    renderer
        .draw_frame(&grid, &chrome, &mut fonts, config.theme().background)
        .unwrap();
    let pixels = renderer.read_pixels();
    let pixel = |x: usize, y: usize| -> [u8; 3] {
        let i = ((H - 1 - y) * W + x) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    let rgb = |c: Rgb| [c.r, c.g, c.b];
    if let Some(path) = std::env::var_os("FLAIR_DUMP_CHROME_PPM") {
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n{W} {H}\n255\n").unwrap();
        for y in 0..H {
            for x in 0..W {
                file.write_all(&pixel(x, y)).unwrap();
            }
        }
    }
    assert_eq!(pixel(380, 17), rgb(fill));
    assert_eq!(pixel(200, 34), rgb(outline));
    assert_eq!(pixel(200, 33), rgb(fill));
    assert_eq!(pixel(200, 35), rgb(white));
    assert_eq!(pixel(40, 60), rgb(white));
    assert_eq!(pixel(48, 68), rgb(box_fill));
    assert_eq!(pixel(120, 60), rgb(box_border));
    assert_eq!(pixel(40, 110), rgb(box_border));
    assert_eq!(pixel(199, 110), rgb(box_border));
    assert_eq!(pixel(120, 159), rgb(box_border));
    assert_eq!(pixel(120, 61), rgb(box_fill));
    let dark = pixel(390, 280);
    for channel in dark {
        assert!(
            (75..=79).contains(&channel),
            "backdrop over white is {dark:?}"
        );
    }
    let top = (baseline - ui.ascent).floor().max(0.0) as usize;
    let bottom = ((baseline - ui.ascent + ui.height).ceil() as usize).min(34);
    let right = (10.0 + text_width).ceil() as usize;
    assert!(text_width > 0.0 && ui.height > 0.0, "{ui:?} {text_width}");
    assert!(
        (top..bottom).any(|y| (10..right).any(|x| pixel(x, y) != rgb(fill))),
        "ui text left no pixels in its box"
    );
    let lifted_x = (f32::from(lifted.origin.col) * metrics.width + metrics.width / 2.0) as usize;
    let lifted_y =
        (35.0 + f32::from(lifted.origin.row) * metrics.height + metrics.height / 2.0) as usize;
    assert_eq!(pixel(lifted_x, lifted_y), rgb(lifted_bg));
    drop(renderer);
    drop(ctx);
}

#[test]
fn cells_only_chrome_draws_like_the_old_draw() {
    let Some((ctx, gl)) = headless_or_skip((320, 200), "cells only") else {
        return;
    };
    let config = Config::default();
    let theme = config.theme();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let mut grid = filled_grid(GridSize { cols: 30, rows: 12 }, theme.background);
    for (cell, c) in grid.cells.iter_mut().zip("chrome only cells".chars()) {
        cell.c = c;
        cell.fg = theme.foreground;
        cell.flags = CellFlags::UNDERLINE;
    }
    grid.cursor = Some(DrawCursor {
        point: CellPoint { col: 3, row: 1 },
        shape: CursorShape::Beam,
        color: theme.cursor,
    });
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((320, 200));
    renderer
        .draw(&grid, &mut fonts, (16.0, 8.0), theme.background)
        .unwrap();
    let old = renderer.read_pixels();
    renderer
        .draw_frame(
            &grid,
            &Chrome::cells_only((16.0, 8.0), (320, 200)),
            &mut fonts,
            theme.background,
        )
        .unwrap();
    assert!(old == renderer.read_pixels(), "cells_only frame differs");
    drop(renderer);
    drop(ctx);
}
