use super::*;
use crate::CellMetrics;
use flair_core::settings::{Kind, SETTINGS, Section};

#[test]
fn long_text_values_ellipsize_inside_the_chip() {
    let mut sheet = appearance();
    sheet.groups[0].entries[1] = setting("Background", "default", Control::Text("x".repeat(80)));
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let chip = control(&scene, SettingsHit::Edit(1));
    assert_eq!(chip.x + chip.w, 1440.0);
    assert!(chip.w <= 280.0, "{chip:?}");
    let value = scene
        .text
        .iter()
        .find(|run| run.text.starts_with("xxx"))
        .unwrap();
    assert!(value.text.ends_with(ELLIPSIS));
    assert_eq!(value.x, chip.x + 10.0);
    assert!(value.x + measure(&value.text, false) <= chip.x + chip.w - 10.0);
}

#[test]
fn every_rect_and_text_lands_on_whole_pixels_at_scale_one_and_a_half() {
    let mut editing = keys(12);
    editing.groups[0].entries[1] = setting("Prefix key", "default", editor("C-b"));
    for sheet in [appearance(), keys(12), editing] {
        let scene = run(&sheet, (1500, 900), 1.5, Some((700.0, 400.0)));
        assert_inside(&scene, (1500, 900));
        let whole = |rect: PxRect| {
            [rect.x, rect.y, rect.w, rect.h]
                .iter()
                .all(|v| v.fract() == 0.0)
        };
        for quad in &scene.quads {
            assert!(whole(quad.rect) && quad.radius.fract() == 0.0, "{quad:?}");
        }
        for run in &scene.text {
            assert_eq!((run.x.fract(), run.baseline.fract()), (0.0, 0.0), "{run:?}");
        }
        for (_, rect) in &scene.hits.rows {
            assert!(whole(*rect), "{rect:?}");
        }
        for (_, rect) in &scene.hits.controls {
            assert!(whole(*rect), "{rect:?}");
        }
        assert!(!scene.hits.rows.is_empty());
    }
}

#[test]
fn small_windows_never_panic_and_keep_every_quad_inside() {
    for (px, scale) in [
        ((200, 100), 1.0),
        ((50, 40), 1.0),
        ((400, 300), 1.0),
        ((960, 200), 1.0),
        ((1300, 150), 1.0),
        ((300, 220), 1.5),
        ((1, 1), 2.0),
        ((0, 0), 1.0),
    ] {
        for sheet in [appearance(), keys(20)] {
            let scene = run(&sheet, px, scale, Some((10.0, 60.0)));
            assert_inside(&scene, px);
        }
    }
    let scene = run(&appearance(), (200, 100), 1.0, None);
    assert!(scene.hits.rows.is_empty());
    assert!(text(&scene, "space").is_none());
    let scene = run(&appearance(), (960, 200), 1.0, None);
    assert!(scene.hits.rows.is_empty());
    assert!(text(&scene, "space").is_none());
    assert!(text(&scene, "Search settings").is_some());
}

#[test]
fn a_short_window_drops_the_footer_before_the_entries() {
    let scene = run(&appearance(), (960, 260), 1.0, None);
    assert_eq!(scene.hits.rows.len(), 1);
    assert!(text(&scene, "space").is_none());
    assert_inside(&scene, (960, 260));
}

#[test]
fn a_clipped_list_counts_hidden_entries_and_runs_the_panel_past_the_last_row() {
    let surfaces = theme().surfaces();
    let mut sheet = keys(40);
    sheet.focus = Some(0);
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    assert_eq!(scene.hits.rows.len(), 26);
    assert_eq!(scene.hits.rows[25].1, rect(696.0, 1087.0, 760.0, 32.0));
    let settings = rect(696.0, 159.0, 760.0, 168.0);
    assert_quad(
        &scene,
        outlined(settings, surfaces.surface1, 10.0, surfaces.outline),
    );
    let bindings = rect(696.0, 383.0, 760.0, 746.0);
    assert_quad(
        &scene,
        outlined(bindings, surfaces.surface1, 10.0, surfaces.outline),
    );
    let strip = rect(696.0, 1119.0, 760.0, 10.0);
    let position = |quad: Quad| scene.quads.iter().position(|candidate| *candidate == quad);
    let panel_index = position(outlined(
        bindings,
        surfaces.surface1,
        10.0,
        surfaces.outline,
    ));
    let strip_index = position(plain(strip, theme().background, 0.0));
    assert!(strip_index > panel_index, "{panel_index:?} {strip_index:?}");
    let outline = Some(Border {
        color: opaque(surfaces.outline),
        width: 1.0,
    });
    assert!(scene.quads.iter().all(|quad| quad.border != outline
        || quad.rect.y + quad.rect.h <= 327.0
        || quad.rect.y + quad.rect.h > 1119.0));
    assert!(quad_at(&scene, rect(697.0, 1119.0, 758.0, 1.0)).is_none());
    let more = text(&scene, "+17 more").unwrap();
    assert_eq!(
        (more.x, more.color, more.face),
        (1428.0, surfaces.muted, TextFace::Small)
    );
    assert_inside(&scene, (1920, 1200));

    let scene = run(&appearance(), (1920, 1200), 1.0, None);
    assert!(scene.text.iter().all(|run| !run.text.starts_with('+')));
}

#[test]
fn content_keeps_even_gutters_on_a_narrow_window() {
    let scene = run(&appearance(), (960, 600), 1.0, None);
    let search = control(&scene, SettingsHit::Search);
    assert_eq!(search, rect(200.0, 103.0, 720.0, 32.0));
    assert_eq!(search.x - 160.0, 960.0 - search.x - search.w);
}

#[test]
fn a_pending_value_marks_its_source_as_unsaved() {
    let surfaces = theme().surfaces();
    let mut sheet = keys(0);
    sheet.groups[0].entries[2] = Entry::Setting {
        title: "Prefix timeout",
        description: "Overrides the theme value.",
        source: "flair.toml",
        control: Control::Number("1600".into()),
        error: None,
        pending: true,
    };
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let unsaved = text(&scene, "unsaved").unwrap();
    assert_eq!(
        (unsaved.x, unsaved.color, unsaved.face),
        (1124.0, theme().foreground, TextFace::Small)
    );
    assert!(text(&scene, "flair.toml").is_none());
    assert_eq!(text(&scene, "default").unwrap().color, surfaces.muted);
}

#[test]
fn colour_chips_lead_with_an_outlined_swatch() {
    let surfaces = theme().surfaces();
    let scene = run(&appearance(), (1920, 1200), 1.0, None);
    assert_quad(
        &scene,
        Quad {
            rect: rect(1170.0, 236.0, 14.0, 14.0),
            color: opaque(Rgb::from_hex("#030303").unwrap()),
            radius: 4.0,
            border: Some(Border {
                color: opaque(surfaces.outline_strong),
                width: 1.0,
            }),
        },
    );
    let selection = control(&scene, SettingsHit::Edit(5));
    let swatch = quad_at(
        &scene,
        rect(selection.x + 10.0, selection.y + 5.0, 14.0, 14.0),
    )
    .unwrap();
    assert_eq!(swatch.color.a, 0x4d);

    let mut sheet = appearance();
    sheet.groups[0].entries[1] = setting("Background", "default", Control::Color("nope".into()));
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let chip = rect(1160.0, 231.0, 280.0, 24.0);
    assert_hit(&scene, SettingsHit::Edit(1), chip);
    assert!(quad_at(&scene, rect(1170.0, 236.0, 14.0, 14.0)).is_none());
    assert_eq!(text(&scene, "nope").unwrap().x, 1170.0);
}

#[test]
fn a_disabled_binding_list_mutes_commands_and_marks_the_heading_off() {
    let surfaces = theme().surfaces();
    let mut sheet = keys(2);
    sheet.groups[1].disabled = true;
    sheet.groups[1].entries = (0..2)
        .map(|index| Entry::Binding {
            chord: format!("c{index}"),
            command: format!("cmd{index}"),
            unbound: false,
            table: "prefix",
            source: "default",
            disabled: true,
        })
        .collect();
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let off = text(&scene, " off").unwrap();
    assert_eq!(
        (off.x, off.baseline, off.color, off.bold),
        (780.0, 367.0, surfaces.muted, false)
    );
    assert_eq!(styled(&scene, "Key bindings", true).x, 696.0);
    assert_eq!(text(&scene, "cmd0").unwrap().color, surfaces.muted);
    assert_eq!(text(&scene, "cmd1").unwrap().color, surfaces.muted);

    let scene = run(&keys(2), (1920, 1200), 1.0, None);
    assert!(text(&scene, " off").is_none());
}

fn prompt() -> SettingsSheet {
    let entries = SETTINGS
        .iter()
        .filter(|setting| setting.section == Section::Prompt)
        .map(|setting| Entry::Setting {
            title: setting.title,
            description: setting.description,
            source: "default",
            control: match setting.kind {
                Kind::Toggle => Control::Toggle(true),
                Kind::Integer { .. } | Kind::Float { .. } => Control::Number("2000".into()),
                Kind::Choice(_) => Control::Choice("chips".into()),
                Kind::Text | Kind::Font | Kind::Words => Control::Text("x".into()),
                Kind::Color { .. } => Control::Color("#000000".into()),
            },
            error: None,
            pending: false,
        })
        .collect();
    let mut sheet = sheet(
        4,
        "Prompt",
        vec![Group {
            heading: None,
            disabled: false,
            entries,
        }],
    );
    sheet.keys = vec![
        ("←→", "step".into()),
        ("esc", "close".into()),
        ("/", "search".into()),
    ];
    sheet
}

fn wide(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 12.0
}

#[test]
fn the_prompt_page_fits_at_960x600_with_whole_descriptions_beside_the_source() {
    let theme = theme();
    let input = OverlayInput {
        px: (960, 600),
        scale: 1.0,
        ui: UiMetrics {
            height: 14.0,
            ascent: 11.0,
            small: 0.5,
            cap: 8.0,
            icon: CellMetrics {
                width: 7.0,
                height: 12.0,
                baseline: 9.0,
            },
        },
        theme: &theme,
        hover: None,
    };
    let sheet = prompt();
    assert_eq!(sheet.groups[0].entries.len(), 14);
    let scene = layout(&sheet, &input, &mut wide);
    assert_inside(&scene, (960, 600));
    assert_eq!(scene.hits.rows.len(), 6);
    assert_eq!(
        control(&scene, SettingsHit::Cycle(0)),
        rect(624.0, 175.0, 280.0, 24.0)
    );
    assert_eq!(
        control(&scene, SettingsHit::Toggle(1)),
        rect(876.0, 235.0, 28.0, 16.0)
    );
    let label = scene
        .text
        .iter()
        .find(|run| run.text == "default" && run.baseline > 159.0 && run.baseline < 215.0)
        .unwrap();
    for description in [
        "Shows a chip row above each pane.",
        "Shows an icon in each chip. Needs a Nerd Font.",
    ] {
        let run = text(&scene, description).unwrap();
        let end = run.x + wide(&run.text, false) * 0.5;
        assert!(end <= label.x - 12.0, "{end} {}", label.x);
    }
    assert!(text(&scene, "+8 more").is_some());
}
