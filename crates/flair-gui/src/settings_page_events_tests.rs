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
    geom::CellPoint,
    ids::PaneId,
    proto::{Cell, Cursor, CursorShape, PaneFlags, PaneUpdate, ServerMsg},
    status::StatusLine,
    theme::Rgb,
};
use std::{fs, io::Write, path::Path};

#[test]
fn clicks_map_to_sections_and_controls() {
    let mut page = page("");
    type_text(&mut page, "pad");
    section(&mut page, 3);
    let sheet = page.sheet();
    assert_eq!(sheet.search.text, "");
    assert_eq!(sheet.section, Some(3));
    assert!(!sheet.sections_focused);
    assert_eq!(page.click(SettingsHit::Close), SettingsOutcome::Close);
    section(&mut page, 5);
    assert_eq!(
        page.click(SettingsHit::Toggle(0)),
        save("keys.enabled", Some(Value::Bool(false)))
    );
    let more = Value::Integer(1600);
    assert_eq!(
        page.click(SettingsHit::Increase(2)),
        preview_of("", "keys.timeout_ms", Some(&more))
    );
    assert_eq!(page.sheet().focus, Some(2));
    assert_eq!(
        page.click(SettingsHit::Decrease(2)),
        SettingsOutcome::Revert
    );
    section(&mut page, 0);
    let next = Value::Text("gruvbox-dark".to_owned());
    assert_eq!(
        page.click(SettingsHit::Cycle(0)),
        preview_of("", "theme", Some(&next))
    );
    assert_eq!(
        page.click(SettingsHit::Edit(2)),
        save("theme", Some(next.clone()))
    );
    assert_eq!(page.sheet().focus, Some(2));
    assert!(!matches!(control(&page), Control::Editor(_)));
    let text = "theme = \"gruvbox-dark\"\n";
    let config = Config::from_toml(text).unwrap();
    assert_eq!(
        page.refresh(&config, config.theme(), text.to_owned()),
        SettingsOutcome::Pending
    );
    assert!(matches!(control(&page), Control::Editor(_)));
    assert_eq!(page.click(SettingsHit::Search), SettingsOutcome::Revert);
    assert!(!matches!(control(&page), Control::Editor(_)));
}

#[test]
fn search_click_focuses_the_entries() {
    let mut page = page("");
    press(&mut page, NamedKey::Tab);
    assert_eq!(page.click(SettingsHit::Search), SettingsOutcome::Pending);
    assert!(!page.sheet().sections_focused);
}

#[test]
fn a_section_click_reverts_an_open_editor() {
    let mut page = page("");
    focus_key(&mut page, "shell.program");
    press(&mut page, NamedKey::Enter);
    assert_eq!(page.click(SettingsHit::Section(1)), SettingsOutcome::Revert);
    assert_eq!(page.sheet().keys, edit_keys());
}

#[test]
fn a_control_click_on_another_entry_saves_the_pending_value_first() {
    let mut page = page("");
    section(&mut page, 5);
    page.focus(2);
    press(&mut page, NamedKey::Right);
    assert_eq!(
        page.click(SettingsHit::Toggle(0)),
        save("keys.timeout_ms", Some(Value::Integer(1600)))
    );
    assert_eq!(page.sheet().focus, Some(0));
    assert_eq!(control(&page), Control::Toggle(true));
    let text = "[keys]\ntimeout_ms = 1600\n";
    let config = Config::from_toml(text).unwrap();
    assert_eq!(
        page.refresh(&config, config.theme(), text.to_owned()),
        save("keys.enabled", Some(Value::Bool(false)))
    );
    assert_eq!(
        page.refresh(&config, config.theme(), text.to_owned()),
        SettingsOutcome::Pending
    );
}

#[test]
fn a_row_click_moves_the_focus_and_closes_other_editors() {
    let mut page = page("");
    section(&mut page, 3);
    press(&mut page, NamedKey::Enter);
    assert_eq!(page.focus(0), SettingsOutcome::Pending);
    assert!(matches!(control(&page), Control::Editor(_)));
    assert_eq!(page.focus(1), SettingsOutcome::Revert);
    assert_eq!(page.sheet().focus, Some(1));
    page.focus(2);
    press(&mut page, NamedKey::Right);
    assert_eq!(
        page.focus(0),
        save("scrollback.lines", Some(Value::Integer(11000)))
    );
    press(&mut page, NamedKey::Tab);
    assert_eq!(page.focus(1), SettingsOutcome::Pending);
    assert!(!page.sheet().sections_focused);
}

#[test]
fn the_wheel_moves_the_focus_unless_editing() {
    let mut page = page("");
    press(&mut page, NamedKey::Tab);
    assert_eq!(page.wheel(3), SettingsOutcome::Pending);
    assert!(!page.sheet().sections_focused);
    assert_eq!(page.sheet().focus, Some(3));
    page.wheel(-10);
    assert_eq!(page.sheet().focus, Some(0));
    page.focus(1);
    press(&mut page, NamedKey::Enter);
    assert_eq!(page.wheel(2), SettingsOutcome::Pending);
    assert_eq!(page.sheet().focus, Some(1));
    press(&mut page, NamedKey::Escape);
    page.focus(0);
    press(&mut page, NamedKey::Right);
    assert_eq!(
        page.wheel(1),
        save("theme", Some(Value::Text("gruvbox-dark".to_owned())))
    );
}

#[test]
fn refresh_updates_sources_and_re_previews_an_open_editor() {
    let mut page = page("");
    focus_key(&mut page, "window.padding_x");
    assert_eq!(source(&page), "default");
    press(&mut page, NamedKey::Enter);
    page.key(&ctrl('u'));
    type_text(&mut page, "4");
    let text = "[window]\npadding_y = 3\n";
    let config = Config::from_toml(text).unwrap();
    assert_eq!(
        page.refresh(&config, config.theme(), text.to_owned()),
        preview_of(text, "window.padding_x", Some(&Value::Integer(4)))
    );
    press(&mut page, NamedKey::Escape);
    page.focus(1);
    assert_eq!(source(&page), "flair.toml");
    assert_eq!(control(&page), Control::Number("3".to_owned()));
}

#[test]
fn refresh_re_previews_a_pending_value_and_clears_errors() {
    let mut page = page("");
    focus_key(&mut page, "window.padding_x");
    press(&mut page, NamedKey::Right);
    page.fail("window.padding_x", "disk full");
    assert_eq!(error(&page).as_deref(), Some("disk full"));
    let config = Config::default();
    assert_eq!(
        page.refresh(&config, config.theme(), String::new()),
        preview_of("", "window.padding_x", Some(&Value::Integer(9)))
    );
    assert_eq!(error(&page), None);
    press(&mut page, NamedKey::Escape);
    assert_eq!(
        page.refresh(&config, config.theme(), String::new()),
        SettingsOutcome::Pending
    );
}

#[test]
fn fail_shows_the_message_on_that_entry_only() {
    let mut page = page("");
    page.fail("colors.accent", "could not write flair.toml");
    page.focus(4);
    assert_eq!(error(&page).as_deref(), Some("could not write flair.toml"));
    page.focus(3);
    assert_eq!(error(&page), None);
    page.fail("no.such.key", "ignored");
    assert_eq!(error(&page), None);
}

fn rgb(color: Rgb) -> [u8; 3] {
    [color.r, color.g, color.b]
}

#[test]
fn gpu_frame_draws_the_keys_section_of_the_settings_page() {
    const W: u32 = 960;
    const H: u32 = 600;
    let (ctx, gl) = match HeadlessContext::new((W, H)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped settings page EGL frame: {error}");
            return;
        }
    };
    let config = Config::from_toml("[keys]\nenabled = false\n").unwrap();
    let theme = config.theme();
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
            "$ flair"
                .chars()
                .map(|c| Cell {
                    c,
                    ..Cell::default()
                })
                .collect(),
        )],
        cursor: Cursor {
            point: CellPoint { col: 7, row: 0 },
            shape: CursorShape::Block,
            visible: true,
        },
        flags: PaneFlags::default(),
        selection: vec![],
    }]);
    let grid = compose(&mirror, PaneId(1), &theme, true);
    let mut overlays = Overlays::default();
    assert!(overlays.open(
        &ServerMsg::Settings,
        &StatusLine::default(),
        &config,
        Path::new("/nonexistent/flair.toml"),
    ));
    let page = overlays.settings_mut().unwrap();
    section(page, 5);
    page.focus(1);
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
    if let Some(path) = std::env::var_os("FLAIR_SETTINGS_PPM") {
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n{width} {height}\n255\n").unwrap();
        for y in 0..height {
            for x in 0..width {
                file.write_all(&at(x as f32, y as f32)).unwrap();
            }
        }
    }
    let hits = chrome.overlay.as_ref().unwrap();
    let control = |wanted: SettingsHit| {
        hits.controls
            .iter()
            .find(|(hit, _)| *hit == wanted)
            .map(|(_, rect)| *rect)
            .unwrap()
    };
    let row = |index: usize| {
        hits.rows
            .iter()
            .find(|(item, _)| *item == index)
            .map(|(_, rect)| *rect)
            .unwrap()
    };
    let nav = control(SettingsHit::Section(5));
    let first = row(0);
    let second = row(1);
    let page_rect = hits.panel;
    assert_eq!(at(2.0, 1.0), rgb(surfaces.surface1));
    assert_eq!(
        at(2.0, chrome_layout::bar_height(1.0)),
        rgb(surfaces.outline)
    );
    assert_eq!(
        at(first.x + first.w + 4.0, first.y + first.h / 2.0),
        rgb(theme.background)
    );
    assert_eq!(
        at(nav.x + nav.w + 16.0, page_rect.y + page_rect.h - 4.0),
        rgb(surfaces.outline)
    );
    assert_eq!(at(nav.x + 4.0, nav.y + nav.h / 2.0), rgb(surfaces.surface3));
    assert_eq!(
        at(first.x + 8.0, first.y + first.h / 2.0),
        rgb(surfaces.surface1)
    );
    assert_eq!(
        at(second.x + 8.0, second.y + second.h / 2.0),
        rgb(surfaces.surface3)
    );
    let track = control(SettingsHit::Toggle(0));
    assert_eq!(
        at(track.x + track.w - 4.0, track.y + track.h / 2.0),
        rgb(theme.background)
    );
}
