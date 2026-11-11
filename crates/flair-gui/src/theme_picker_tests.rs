use super::*;
use crate::{
    Fonts,
    chrome_layout::{self, ChromeInput, grid_size},
    compose::compose,
    mirror::Mirror,
    overlays::Overlays,
    renderer::{HeadlessContext, Renderer},
    status::status_parts,
};
use flair_core::{
    config::Config,
    geom::CellPoint,
    ids::PaneId,
    key::{Locks, Mods},
    proto::{Cell, Cursor, CursorShape, PaneFlags, PaneUpdate, ServerMsg},
    status::StatusLine,
    theme::Rgb,
};
use std::{fs, io::Write, path::Path};

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
fn sheet_docks_the_themes_with_the_saved_label_and_the_query_caret() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    picker.key(&named(NamedKey::Down));
    let sheet = picker.sheet();
    assert_eq!(sheet.placement, Placement::Docked);
    assert_eq!(sheet.title, "Themes");
    assert_eq!(sheet.selected, 2);
    let rows: Vec<_> = sheet
        .rows
        .iter()
        .map(|row| (row.name.as_str(), row.label))
        .collect();
    assert_eq!(
        rows,
        [
            ("catppuccin-mocha", None),
            ("flair-dark", Some("saved")),
            ("gruvbox-dark", None),
        ]
    );
    assert!(sheet.rows.iter().all(|row| row.mark == Mark::Blank));
    let keys: Vec<_> = sheet
        .keys
        .iter()
        .map(|(key, verb)| (*key, verb.as_str()))
        .collect();
    assert_eq!(keys, [("↵", "save"), ("↑↓", "move"), ("esc", "revert")]);
    picker.insert("grü");
    let field = picker.sheet().field.unwrap();
    assert_eq!(
        (field.text.as_str(), field.cursor, field.placeholder),
        ("grü", 3, "Search themes")
    );
}

#[test]
fn select_previews_a_row_once_and_ignores_rows_out_of_range() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.select(0), preview("catppuccin-mocha"));
    assert_eq!(picker.select(0), ThemeOutcome::Pending);
    assert_eq!(picker.select(3), ThemeOutcome::Pending);
    assert_eq!(picker.step(5), preview("gruvbox-dark"));
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
    let grid = compose(&mirror, PaneId(1), &theme, true);
    let mut overlays = Overlays::default();
    assert!(overlays.open(
        &ServerMsg::ChooseTheme,
        &StatusLine::default(),
        &config,
        Path::new("/nonexistent/flair.toml"),
    ));
    let picker = overlays.theme_picker_mut().unwrap();
    assert_eq!(picker.insert("gruvbox-dark"), preview("gruvbox-dark"));
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
        layout: None,
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
    let panel = chrome.chrome.modal[0];
    assert!(panel.border.is_some(), "{panel:?}");
    let rect = panel.rect;
    let middle = rect.y + rect.h / 2.0;
    assert_eq!(at(2.0, 1.0), rgb(surfaces.surface1));
    assert_eq!(
        at(2.0, chrome_layout::bar_height(1.0)),
        rgb(surfaces.outline)
    );
    assert_eq!(at(rect.x - 2.0, middle), rgb(theme.background));
    assert_eq!(at(rect.x, middle), rgb(surfaces.outline));
    assert_eq!(at(rect.x + 4.0, middle), rgb(surfaces.surface1));
    assert_eq!(at(W as f32 - 10.0, middle), rgb(surfaces.surface1));
    assert_eq!(at(W as f32 - 2.0, middle), rgb(theme.background));
    assert_eq!(at(rect.x + 20.0, H as f32 - 3.0), rgb(theme.background));
    assert_eq!(at(rect.x + 20.0, rect.y - 3.0), rgb(theme.background));
    let row = chrome.overlay.as_ref().unwrap().rows[0].1;
    assert_eq!(
        at(row.x + row.w - 4.0, row.y + row.h / 2.0),
        rgb(surfaces.surface3)
    );
}

#[test]
fn clearing_an_empty_query_keeps_the_selection() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.key(&named(NamedKey::Down)), preview("gruvbox-dark"));
    assert_eq!(
        picker.key(&named(NamedKey::Backspace)),
        ThemeOutcome::Pending
    );
    assert_eq!(picker.key(&ctrl('u')), ThemeOutcome::Pending);
    assert_eq!(
        picker.key(&named(NamedKey::Enter)),
        ThemeOutcome::Save("gruvbox-dark".into())
    );
}

#[test]
fn refresh_moves_the_saved_marker_and_previews_the_selection_again() {
    let mut picker = ThemePicker::new(names(), "flair-dark");
    assert_eq!(picker.key(&named(NamedKey::Down)), preview("gruvbox-dark"));
    assert_eq!(picker.refresh("catppuccin-mocha"), preview("gruvbox-dark"));
    let labels: Vec<_> = picker.sheet().rows.iter().map(|row| row.label).collect();
    assert_eq!(labels, [Some("saved"), None, None]);
}
