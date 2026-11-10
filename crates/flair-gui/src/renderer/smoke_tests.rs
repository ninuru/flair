use super::*;
use crate::cellgrid::{DrawCell, DrawCursor};
use flair_core::{
    config::Config,
    geom::{CellPoint, GridSize},
    proto::{CellFlags, CursorShape},
    theme::{Rgb, Rgba},
};
use std::{fs, io::Write};

fn cells_only(padding: (f32, f32), px: (u32, u32)) -> Chrome {
    Chrome {
        origin: padding,
        clip: PxRect {
            x: padding.0,
            y: padding.1,
            w: (px.0 as f32 - padding.0 * 2.0).max(0.0),
            h: (px.1 as f32 - padding.1 * 2.0).max(0.0),
        },
        ..Chrome::default()
    }
}

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
        .draw_frame(
            &grid,
            &cells_only((16.0, 8.0), (800, 480)),
            &mut fonts,
            theme.background,
        )
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
    let chrome = cells_only((16.0, 8.0), (200, 100));
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
            .draw_frame(&grid, &chrome, &mut fonts, theme.background)
            .unwrap();
        let before = renderer.read_pixels();
        grid.cells[col].extra = Some("\u{301}".into());
        renderer
            .draw_frame(&grid, &chrome, &mut fonts, theme.background)
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
fn chrome_draws_bar_border_box_backdrop_text_and_modal_text() {
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
    let size = GridSize {
        cols: (W as f32 / metrics.width) as u16 + 1,
        rows: (H as f32 / metrics.height) as u16 + 1,
    };
    let grid = filled_grid(size, white);
    let ui = fonts.ui_metrics();
    let text_width = fonts.ui_width("flair", false);
    let baseline = 22.0;
    let chrome = Chrome {
        scale: 1.0,
        origin: (0.0, 35.0),
        clip: PxRect {
            x: 0.0,
            y: 35.0,
            w: W as f32,
            h: H as f32 - 35.0,
        },
        extend: PxRect::default(),
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
            face: TextFace::Ui,
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
        modal_text: vec![TextRun {
            x: 60.0,
            baseline: 110.0,
            text: "flair".to_owned(),
            color: white,
            bold: true,
            face: TextFace::Ui,
        }],
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
    let modal_top = (110.0 - ui.ascent).floor() as usize;
    assert!(
        (modal_top..110).any(|y| (60..60 + right).any(|x| pixel(x, y) != rgb(box_fill))),
        "modal text left no pixels over the box"
    );
    drop(renderer);
    drop(ctx);
}

#[test]
fn the_icon_face_draws_terminal_glyphs_at_the_small_size() {
    const W: usize = 64;
    const H: usize = 48;
    let Some((ctx, gl)) = headless_or_skip((W as u32, H as u32), "icon") else {
        return;
    };
    let config = Config::default();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let icon = fonts.ui_metrics().icon;
    let raster = fonts.rasterize_icon('\u{f114}').unwrap();
    assert!(
        icon.height > 0.0 && icon.height < metrics.height && icon.width <= metrics.width,
        "{icon:?} {metrics:?}"
    );
    let black = Rgb { r: 0, g: 0, b: 0 };
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let size = GridSize {
        cols: (W as f32 / metrics.width) as u16 + 1,
        rows: (H as f32 / metrics.height) as u16 + 1,
    };
    let baseline = 30.0;
    let chrome = Chrome {
        clip: PxRect {
            x: 0.0,
            y: 0.0,
            w: W as f32,
            h: H as f32,
        },
        text: vec![TextRun {
            x: 10.0,
            baseline,
            text: "\u{f114}".to_owned(),
            color: white,
            bold: false,
            face: TextFace::Icon,
        }],
        ..Chrome::default()
    };
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((W as u32, H as u32));
    renderer
        .draw_frame(&filled_grid(size, black), &chrome, &mut fonts, black)
        .unwrap();
    let pixels = renderer.read_pixels();
    let lit: Vec<(usize, usize)> = (0..H)
        .flat_map(|y| (0..W).map(move |x| (x, y)))
        .filter(|(x, y)| pixels[((H - 1 - y) * W + x) * 4] > 0)
        .collect();
    assert!(!lit.is_empty(), "the icon face left no pixels");
    let left = 10 + raster.left;
    let top = baseline as i32 - raster.top;
    for (x, y) in lit {
        assert!(
            (top..top + raster.height as i32).contains(&(y as i32))
                && (left..left + raster.width as i32).contains(&(x as i32)),
            "pixel {x},{y} outside icon raster {raster:?} at {left},{top}"
        );
    }
    drop(renderer);
    drop(ctx);
}

#[test]
fn scissor_rounds_fractional_clips_outward() {
    let clip = PxRect {
        x: 17.6,
        y: 43.2,
        w: 768.0,
        h: 300.5,
    };
    assert_eq!(scissor_box(clip, 400), (17, 56, 769, 301));
}

#[test]
fn dominant_picks_the_most_frequent_colour_and_breaks_ties_by_first_cell() {
    let a = Rgb { r: 1, g: 2, b: 3 };
    let b = Rgb { r: 4, g: 5, b: 6 };
    let c = Rgb { r: 7, g: 8, b: 9 };
    assert_eq!(dominant([a, b, b, c, b, a].into_iter()), Some(b));
    assert_eq!(dominant([a, b, b, a].into_iter()), Some(a));
    assert_eq!(dominant([b, a, a, b, c].into_iter()), Some(b));
    assert_eq!(dominant([c].into_iter()), Some(c));
    assert_eq!(dominant(std::iter::empty()), None);
}

#[test]
fn top_and_bottom_margins_use_the_dominant_edge_row_colour() {
    const W: usize = 400;
    const H: usize = 300;
    let Some((ctx, gl)) = headless_or_skip((W as u32, H as u32), "edges") else {
        return;
    };
    let config = Config::default();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let colour = |r, g, b| Rgb { r, g, b };
    let base = colour(0x0f, 0x10, 0x11);
    let tab = colour(0x28, 0x28, 0x25);
    let status = colour(0x50, 0x30, 0x20);
    let foot = colour(0x20, 0x40, 0x60);
    let corner = colour(0x70, 0x10, 0x10);
    let left = colour(0x10, 0x70, 0x10);
    let right = colour(0x10, 0x10, 0x70);
    let size = GridSize { cols: 20, rows: 6 };
    let cols = usize::from(size.cols);
    let last = usize::from(size.rows) - 1;
    let mut grid = filled_grid(size, base);
    for col in 8..11 {
        grid.cells[col].bg = tab;
        grid.cells[last * cols + col].bg = status;
    }
    for col in 0..cols {
        if !(8..11).contains(&col) {
            grid.cells[last * cols + col].bg = foot;
        }
    }
    grid.cells[0].bg = corner;
    grid.cells[last * cols + cols - 1].bg = corner;
    grid.cells[2 * cols].bg = left;
    grid.cells[2 * cols + cols - 1].bg = right;
    let origin = (24.0_f32, 60.0_f32);
    let grid_w = f32::from(size.cols) * metrics.width;
    let grid_h = f32::from(size.rows) * metrics.height;
    assert!(origin.0 + grid_w < W as f32 - 8.0 && origin.1 + grid_h < H as f32 - 8.0);
    let chrome = Chrome {
        origin,
        clip: PxRect {
            x: origin.0,
            y: origin.1,
            w: grid_w,
            h: grid_h,
        },
        extend: PxRect {
            x: 0.0,
            y: 40.0,
            w: W as f32,
            h: H as f32 - 40.0,
        },
        ..Chrome::default()
    };
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((W as u32, H as u32));
    renderer
        .draw_frame(&grid, &chrome, &mut fonts, config.theme().background)
        .unwrap();
    let pixels = renderer.read_pixels();
    let pixel = |x: f32, y: f32| -> [u8; 3] {
        let i = ((H - 1 - y as usize) * W + x as usize) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    if let Some(path) = std::env::var_os("FLAIR_DUMP_EDGES_PPM") {
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n{W} {H}\n255\n").unwrap();
        for y in 0..H {
            for x in 0..W {
                file.write_all(&pixel(x as f32, y as f32)).unwrap();
            }
        }
    }
    let rgb = |c: Rgb| [c.r, c.g, c.b];
    let run_x = origin.0 + metrics.width * 9.5;
    let bottom = origin.1 + grid_h;
    let right_x = origin.0 + grid_w;
    let row_y = |row: f32| origin.1 + metrics.height * (row + 0.5);
    assert_eq!(pixel(run_x, row_y(0.0)), rgb(tab));
    assert_eq!(pixel(run_x, row_y(last as f32)), rgb(status));
    for y in [40.0, origin.1 - 1.0] {
        assert_eq!(pixel(run_x, y), rgb(base), "top margin above run at {y}");
        for x in [0.0, origin.0 + 1.0, right_x, W as f32 - 1.0] {
            assert_eq!(pixel(x, y), rgb(base), "top margin at {x},{y}");
        }
    }
    for y in [bottom, H as f32 - 1.0] {
        assert_eq!(pixel(run_x, y), rgb(foot), "bottom margin below run at {y}");
        for x in [0.0, origin.0 + 1.0, right_x, W as f32 - 1.0] {
            assert_eq!(pixel(x, y), rgb(foot), "bottom margin at {x},{y}");
        }
    }
    for x in [0.0, origin.0 - 1.0] {
        assert_eq!(pixel(x, row_y(2.0)), rgb(left), "left margin at {x}");
        assert_eq!(pixel(x, row_y(1.0)), rgb(base), "left margin at {x}");
        assert_eq!(pixel(x, row_y(0.0)), rgb(corner), "left of row 0 at {x}");
    }
    for x in [right_x, W as f32 - 1.0] {
        assert_eq!(pixel(x, row_y(2.0)), rgb(right), "right margin at {x}");
        assert_eq!(pixel(x, row_y(1.0)), rgb(base), "right margin at {x}");
        assert_eq!(
            pixel(x, row_y(last as f32)),
            rgb(corner),
            "right of last row at {x}"
        );
    }
    drop(renderer);
    drop(ctx);
}

#[test]
fn mono_chrome_text_advances_wide_characters_and_overlays_combining_marks() {
    let Some((ctx, gl)) = headless_or_skip((100, 50), "mono unicode") else {
        return;
    };
    let config = Config::default();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    let x_glyph = fonts.rasterize('x', crate::font::Style::Regular).unwrap();
    let text = |value: &str| TextRun {
        x: 10.0,
        baseline: 30.0,
        text: value.to_owned(),
        color: config.theme().foreground,
        bold: false,
        face: TextFace::Mono,
    };
    let vertices = renderer.text(&[text("界x")], &mut fonts);
    let x = vertices.as_chunks::<13>().0.iter().last().unwrap()[0];
    assert_eq!(x, 10.0 + metrics.width * 2.0 + x_glyph.left as f32);
    if let Some(mark) = fonts
        .rasterize('\u{301}', crate::font::Style::Regular)
        .filter(|glyph| glyph.width > 0 && glyph.height > 0)
    {
        let vertices = renderer.text(&[text("a\u{301}x")], &mut fonts);
        let positions: Vec<_> = vertices
            .as_chunks::<13>()
            .0
            .iter()
            .map(|instance| instance[0])
            .collect();
        assert_eq!(positions.len(), 3);
        assert_eq!(positions[1], 10.0 + mark.left as f32);
        assert_eq!(positions[2], 10.0 + metrics.width + x_glyph.left as f32);
    }
    drop(renderer);
    drop(ctx);
}
