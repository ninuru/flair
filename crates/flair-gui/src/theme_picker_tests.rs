use super::*;
use crate::{
    Fonts,
    cellgrid::DrawCursor,
    chrome_layout::{self, ChromeInput, Modal, grid_size},
    compose::compose,
    mirror::Mirror,
    renderer::{HeadlessContext, Renderer},
    status::status_parts,
};
use flair_core::{
    config::Config,
    geom::CellPoint,
    ids::PaneId,
    key::{Locks, Mods},
    proto::{Cell, Cursor, CursorShape, PaneFlags, PaneUpdate},
    status::StatusLine,
    theme::Rgb,
};
use std::{fs, io::Write};

fn key(key: Key, text: Option<&str>) -> KeyInput {
    KeyInput {
        key,
        mods: Mods::default(),
        text: text.map(str::to_owned),
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

fn ctrl(c: char) -> KeyInput {
    KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..key(Key::Char(c), None)
    }
}

fn named(name: NamedKey) -> KeyInput {
    key(Key::Named(name), None)
}

fn typed(c: char) -> KeyInput {
    key(Key::Char(c), Some(&c.to_string()))
}

fn names() -> Vec<String> {
    ["catppuccin-mocha", "flair-dark", "gruvbox-dark"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn preview(name: &str) -> ThemeOutcome {
    ThemeOutcome::Preview(name.to_owned())
}

fn metrics(scale: f32) -> CellMetrics {
    CellMetrics {
        width: 8.0 * scale,
        height: 16.0 * scale,
        baseline: 12.0 * scale,
    }
}

fn blank(cols: u16, rows: u16) -> CellGrid {
    CellGrid {
        size: GridSize { cols, rows },
        cells: vec![
            DrawCell {
                c: '.',
                extra: None,
                fg: Rgb::default(),
                bg: Rgb::default(),
                flags: CellFlags::empty(),
            };
            usize::from(cols) * usize::from(rows)
        ],
        cursor: None,
    }
}

fn cell(grid: &CellGrid, col: u16, row: u16) -> &DrawCell {
    &grid.cells[usize::from(row) * usize::from(grid.size.cols) + usize::from(col)]
}

fn row_text(grid: &CellGrid, rect: CellRect, row: u16) -> String {
    (rect.origin.col..rect.origin.col + rect.size.cols)
        .map(|col| cell(grid, col, rect.origin.row + row).c)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

#[test]
fn opens_on_the_current_theme() {
    let mut picker = ThemePicker::new(names(), "gruvbox-dark");
    assert_eq!(
        picker.matches(),
        ["catppuccin-mocha", "flair-dark", "gruvbox-dark"]
    );
    assert_eq!(
        picker.key(&named(NamedKey::Enter)),
        ThemeOutcome::Save("gruvbox-dark".into())
    );
}

#[test]
fn unknown_current_theme_opens_on_the_first_name() {
    let mut picker = ThemePicker::new(names(), "nope");
    assert_eq!(
        picker.key(&named(NamedKey::Enter)),
        ThemeOutcome::Save("catppuccin-mocha".into())
    );
}

#[test]
fn moving_previews_each_new_selection() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.key(&named(NamedKey::Down)), preview("gruvbox-dark"));
    assert_eq!(picker.key(&named(NamedKey::Down)), ThemeOutcome::Pending);
    assert_eq!(picker.key(&named(NamedKey::Up)), preview("flair-dark"));
    assert_eq!(picker.key(&ctrl('p')), preview("catppuccin-mocha"));
    assert_eq!(picker.key(&ctrl('p')), ThemeOutcome::Pending);
    assert_eq!(picker.key(&ctrl('n')), preview("flair-dark"));
}

#[test]
fn typing_filters_case_insensitively_and_previews_the_first_match() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.key(&typed('G')), preview("gruvbox-dark"));
    assert_eq!(picker.matches(), ["gruvbox-dark"]);
    assert_eq!(
        picker.key(&named(NamedKey::Backspace)),
        preview("catppuccin-mocha")
    );
    assert_eq!(picker.insert("DARK"), preview("flair-dark"));
    assert_eq!(picker.matches(), ["flair-dark", "gruvbox-dark"]);
    assert_eq!(picker.key(&named(NamedKey::Down)), preview("gruvbox-dark"));
    assert_eq!(picker.key(&ctrl('u')), preview("catppuccin-mocha"));
    assert_eq!(picker.matches().len(), 3);
}

#[test]
fn no_match_stays_pending_until_the_filter_matches_again() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.insert("zz"), ThemeOutcome::Pending);
    assert!(picker.matches().is_empty());
    assert_eq!(picker.key(&named(NamedKey::Enter)), ThemeOutcome::Pending);
    assert_eq!(picker.key(&named(NamedKey::Down)), ThemeOutcome::Pending);
    assert_eq!(
        picker.key(&named(NamedKey::Backspace)),
        ThemeOutcome::Pending
    );
    assert_eq!(
        picker.key(&named(NamedKey::Backspace)),
        preview("catppuccin-mocha")
    );
}

#[test]
fn escape_cancels_and_enter_saves_the_selection() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.key(&named(NamedKey::Escape)), ThemeOutcome::Cancel);
    let mut picker = ThemePicker::new(names(), "flair-dark");
    picker.insert("gru");
    assert_eq!(
        picker.key(&named(NamedKey::Enter)),
        ThemeOutcome::Save("gruvbox-dark".into())
    );
}

#[test]
fn releases_and_modified_chars_do_nothing() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    let release = KeyInput {
        action: KeyAction::Release,
        ..named(NamedKey::Escape)
    };
    assert_eq!(picker.key(&release), ThemeOutcome::Pending);
    assert_eq!(picker.key(&ctrl('x')), ThemeOutcome::Pending);
    assert_eq!(picker.matches().len(), 3);
}

#[test]
fn area_is_the_right_panel_below_the_bar_inset_by_the_margin() {
    for scale in [1.0, 2.0] {
        assert_eq!(
            ThemePicker::area(GridSize { cols: 98, rows: 34 }, metrics(scale), scale),
            CellRect {
                origin: CellPoint { col: 61, row: 2 },
                size: GridSize { cols: 34, rows: 30 },
            },
            "scale {scale}"
        );
    }
    assert_eq!(
        ThemePicker::area(GridSize { cols: 20, rows: 10 }, metrics(1.0), 1.0),
        CellRect {
            origin: CellPoint { col: 3, row: 2 },
            size: GridSize { cols: 14, rows: 6 },
        }
    );
    let tiny = ThemePicker::area(GridSize { cols: 1, rows: 1 }, metrics(1.0), 1.0);
    assert_eq!(tiny.size, GridSize { cols: 1, rows: 1 });
}

#[test]
fn draw_lists_themes_with_selection_and_saved_marker_inside_the_area() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut picker = ThemePicker::new(names(), "flair-dark");
    picker.key(&named(NamedKey::Down));
    let mut grid = blank(60, 12);
    grid.cursor = Some(DrawCursor {
        point: CellPoint { col: 0, row: 0 },
        shape: CursorShape::Block,
        color: Rgb::default(),
    });
    let area = CellRect {
        origin: CellPoint { col: 30, row: 1 },
        size: GridSize { cols: 26, rows: 10 },
    };
    assert_eq!(picker.draw(&mut grid, area, &theme), area);
    assert!(grid.cursor.is_none());
    let rows: Vec<String> = (0..7).map(|row| row_text(&grid, area, row)).collect();
    assert_eq!(
        rows,
        [
            "Themes  3",
            "> _",
            "",
            "  catppuccin-mocha",
            "  flair-dark *",
            "> gruvbox-dark",
            "",
        ]
    );
    for row in 0..grid.size.rows {
        for col in 0..grid.size.cols {
            let drawn = cell(&grid, col, row);
            if area.contains(CellPoint { col, row }) {
                let bg = if row == area.origin.row + 5 {
                    surfaces.surface3
                } else {
                    surfaces.surface1
                };
                assert_eq!(drawn.bg, bg, "{col},{row}");
            } else {
                assert_eq!(drawn.c, '.', "{col},{row}");
            }
        }
    }
}

#[test]
fn draw_scrolls_to_keep_the_selection_visible() {
    let theme = Config::default().theme();
    let names: Vec<String> = (0..10).map(|i| format!("theme-{i}")).collect();
    let mut picker = ThemePicker::new(names, "theme-9");
    let mut grid = blank(20, 6);
    let area = CellRect {
        origin: CellPoint { col: 0, row: 0 },
        size: GridSize { cols: 20, rows: 6 },
    };
    picker.draw(&mut grid, area, &theme);
    let rows: Vec<String> = (3..6).map(|row| row_text(&grid, area, row)).collect();
    assert_eq!(rows, ["  theme-7", "  theme-8", "> theme-9 *"]);
    assert_eq!(picker.key(&named(NamedKey::Up)), preview("theme-8"));
}

fn rgb(color: Rgb) -> [u8; 3] {
    [color.r, color.g, color.b]
}

#[test]
fn gpu_frame_previews_gruvbox_in_a_docked_panel_without_backdrop() {
    const W: u32 = 800;
    const H: u32 = 400;
    let (ctx, gl) = match HeadlessContext::new((W, H)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped theme picker EGL frame: {error}");
            return;
        }
    };
    let config = Config::default();
    let theme = config.with_theme("gruvbox-dark").theme();
    let surfaces = theme.surfaces();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let padding = (
        f32::from(config.window.padding_x),
        f32::from(config.window.padding_y),
    );
    let size = grid_size((W, H), padding, 1.0, metrics);
    let mut mirror = Mirror::default();
    mirror.apply(&[PaneUpdate {
        pane: PaneId(1),
        size,
        full: true,
        rows: vec![(
            0,
            "$ flair themes"
                .chars()
                .map(|c| Cell {
                    c,
                    ..Cell::default()
                })
                .collect(),
        )],
        cursor: Cursor {
            point: CellPoint { col: 14, row: 0 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags::default(),
        selection: vec![],
    }]);
    let mut grid = compose(&mirror, PaneId(1), &theme, true);
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.key(&named(NamedKey::Down)), preview("gruvbox-dark"));
    let lifted = picker.draw(&mut grid, ThemePicker::area(size, metrics, 1.0), &theme);
    let status = StatusLine {
        session: "main".into(),
        ..StatusLine::default()
    };
    let parts = status_parts(&status);
    let input = ChromeInput {
        px: (W, H),
        scale: 1.0,
        padding,
        metrics,
        ui: fonts.ui_metrics(),
        theme: &theme,
        status: &parts,
        prompt: None,
        separators: &[],
        modal: Some(Modal::Docked(lifted)),
    };
    let chrome = chrome_layout::layout(&input, |text, bold| fonts.ui_width(text, bold));
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((W, H));
    renderer
        .draw_frame(&grid, &chrome.chrome, &mut fonts, theme.background)
        .unwrap();
    let pixels = renderer.read_pixels();
    drop(renderer);
    drop(ctx);
    let (width, height) = (W as usize, H as usize);
    let at = |x: f32, y: f32| -> [u8; 3] {
        let i = ((height - 1 - y as usize) * width + x as usize) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    if let Some(path) = std::env::var_os("FLAIR_THEMES_PPM") {
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for y in 0..height {
            for x in 0..width {
                file.write_all(&at(x as f32, y as f32)).unwrap();
            }
        }
    }
    let [panel, line] = chrome.chrome.modal.as_slice() else {
        panic!("docked quads {:?}", chrome.chrome.modal);
    };
    let bottom = H as f32 - 3.0;
    assert_eq!(at(2.0, 1.0), rgb(surfaces.surface1));
    assert_eq!(
        at(2.0, chrome_layout::bar_height(1.0)),
        rgb(surfaces.outline)
    );
    assert_eq!(at(panel.rect.x - 2.0, bottom), rgb(theme.background));
    assert_eq!(at(line.rect.x, bottom), rgb(surfaces.outline));
    assert_eq!(at(panel.rect.x + 4.0, bottom), rgb(surfaces.surface1));
    assert_eq!(at(W as f32 - 2.0, bottom), rgb(surfaces.surface1));
}
