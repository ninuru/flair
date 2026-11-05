use super::*;
use crate::cellgrid::{DrawCell, DrawCursor};
use flair_core::{
    config::Config,
    geom::{CellPoint, GridSize},
    proto::{CellFlags, CursorShape},
    theme::Rgb,
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
