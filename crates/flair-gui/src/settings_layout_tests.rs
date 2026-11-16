use super::*;
use crate::{
    CellMetrics, UiMetrics,
    overlay_layout::OverlayHit,
    renderer::{TextFace, TextRun},
    t3_style::{
        APPEARANCE_ICON, CHEVRON, CHEVRON_ICON, CHORD, ERROR, INPUT, INPUT_BORDER, KEYS_ICON,
        NAV_CURRENT, PRIMARY, QUIET, ROW_FOCUS, ROW_HOVER, SHADOW, SIDEBAR, SWITCH_OFF,
        SWITCH_THUMB_ON,
    },
};
use flair_core::{config::Config, theme::Rgb};

pub(super) fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

pub(super) fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

pub(super) fn run(
    sheet: &SettingsSheet,
    px: (u32, u32),
    scale: f32,
    hover: Option<(f32, f32)>,
) -> OverlayScene {
    let theme = Config::default().theme();
    let input = input(&theme, px, scale, hover);
    layout(sheet, &input, &mut measure)
}

pub(super) fn input(
    theme: &flair_core::theme::Theme,
    px: (u32, u32),
    scale: f32,
    hover: Option<(f32, f32)>,
) -> OverlayInput<'_> {
    OverlayInput {
        metrics: CellMetrics {
            width: 8.0,
            height: 16.0,
            baseline: 12.0,
        },
        px,
        scale,
        ui: UiMetrics {
            height: 14.0 * scale,
            ascent: 11.0 * scale,
            small: 0.5,
            cap: 8.0 * scale,
            icon: CellMetrics {
                width: 7.0 * scale,
                height: 12.0 * scale,
                baseline: 9.0 * scale,
            },
        },
        theme,
        hover,
        icons: true,
    }
}

pub(super) fn setting(title: &'static str, modified: bool, control: Control) -> Entry {
    Entry::Setting {
        title,
        description: "Overrides the theme value.",
        modified,
        control,
        error: None,
        pending: false,
    }
}

pub(super) fn sheet(section: usize, crumb: &'static str, groups: Vec<Group>) -> SettingsSheet {
    SettingsSheet {
        sections: Section::ALL.to_vec(),
        section: Some(section),
        sections_focused: false,
        crumb,
        search: Field {
            text: String::new(),
            cursor: 0,
            placeholder: "Search settings",
        },
        groups,
        dropdown: None,
        focus: Some(0),
    }
}

pub(super) fn group(entries: Vec<Entry>) -> Group {
    Group {
        heading: None,
        disabled: false,
        entries,
    }
}

pub(super) fn appearance() -> SettingsSheet {
    let colour = |value: &str| Control::Color(value.into());
    sheet(
        0,
        "Appearance",
        vec![group(vec![
            setting("Theme", true, Control::Choice("flair-dark".into())),
            setting("Background", false, colour("#030303")),
            setting("Foreground", false, colour("#d4d4d4")),
            setting("Cursor colour", false, colour("#d4d4d4")),
            setting("Accent", false, colour("#19aad8")),
            setting("Selection", false, colour("#19aad84d")),
        ])],
    )
}

pub(super) fn binding(index: usize) -> Entry {
    Entry::Binding {
        chord: format!("c{index}"),
        command: format!("cmd{index}"),
        unbound: false,
        table: "prefix",
        disabled: false,
    }
}

pub(super) fn keys(bindings: usize) -> SettingsSheet {
    sheet(
        6,
        "Keys",
        vec![
            group(vec![
                setting("Bindings", false, Control::Toggle(true)),
                setting("Prefix key", false, Control::Text("C-a".into())),
                setting("Prefix timeout", true, Control::Number("1500".into())),
            ]),
            Group {
                heading: Some("Key bindings"),
                disabled: false,
                entries: (0..bindings).map(binding).collect(),
            },
        ],
    )
}

pub(super) fn text<'a>(scene: &'a OverlayScene, text: &str) -> Option<&'a TextRun> {
    scene.text.iter().find(|run| run.text == text)
}

pub(super) fn quad_at(scene: &OverlayScene, rect: PxRect) -> Option<&Quad> {
    scene.quads.iter().find(|quad| quad.rect == rect)
}

pub(super) fn control(scene: &OverlayScene, hit: SettingsHit) -> Option<PxRect> {
    scene
        .hits
        .controls
        .iter()
        .find(|(candidate, _)| *candidate == hit)
        .map(|(_, rect)| *rect)
}

pub(super) fn centre(rect: PxRect) -> (f64, f64) {
    (
        f64::from(rect.x + rect.w / 2.0),
        f64::from(rect.y + rect.h / 2.0),
    )
}

pub(super) fn colour(color: Rgb) -> flair_core::theme::Rgba {
    opaque(color)
}

#[test]
fn the_modal_dims_the_whole_window_and_centres_a_capped_panel() {
    let scene = run(&appearance(), (1600, 1000), 1.0, None);
    assert_eq!(
        scene.quads[0],
        Quad {
            rect: rect(0.0, 0.0, 1600.0, 1000.0),
            color: DIM,
            radius: 0.0,
            border: None,
        }
    );
    let panel = rect(160.0, 40.0, 1280.0, 920.0);
    let quad = quad_at(&scene, panel).unwrap();
    assert_eq!(quad.color, colour(BACKGROUND));
    assert_eq!(quad.radius, 16.0);
    assert_eq!(
        quad.border,
        Some(Border {
            color: colour(PANEL_BORDER),
            width: 1.0,
        })
    );
    assert_eq!(scene.hits.panel, panel);
    assert!(
        scene
            .quads
            .iter()
            .any(|quad| quad.color == SHADOW && quad.rect.y > panel.y)
    );
    assert_eq!(scene.hits.hit_test((100.0, 500.0)), OverlayHit::Outside);
    assert_eq!(scene.hits.hit_test((800.0, 700.0)), OverlayHit::Panel);
}

#[test]
fn small_windows_shrink_the_inset() {
    let scene = run(&appearance(), (900, 700), 1.0, None);
    assert_eq!(scene.hits.panel, rect(24.0, 24.0, 852.0, 652.0));
    let large = run(&appearance(), (2400, 1400), 1.0, None);
    assert_eq!(large.hits.panel, rect(560.0, 220.0, 1280.0, 960.0));
}

#[test]
fn the_sidebar_holds_search_and_grouped_sections_with_icons() {
    let scene = run(&appearance(), (1600, 1000), 1.0, None);
    assert_eq!(
        control(&scene, SettingsHit::Search),
        Some(rect(168.0, 48.0, 224.0, 32.0))
    );
    let tops: Vec<f32> = (0..7)
        .map(|index| control(&scene, SettingsHit::Section(index)).unwrap().y)
        .collect();
    assert_eq!(tops, [88.0, 124.0, 160.0, 196.0, 264.0, 300.0, 368.0]);
    let first = control(&scene, SettingsHit::Section(0)).unwrap();
    assert_eq!(first, rect(168.0, 88.0, 224.0, 32.0));
    let current = quad_at(&scene, first).unwrap();
    assert_eq!((current.color, current.radius), (colour(NAV_CURRENT), 8.0));
    for heading in ["Terminal", "Input"] {
        let run = scene
            .text
            .iter()
            .find(|run| run.text == heading && run.face == TextFace::Small)
            .unwrap();
        assert_eq!(run.color, QUIET);
    }
    let label = text(&scene, "Font").unwrap();
    assert_eq!((label.x, label.color), (202.0, QUIET));
    assert_eq!(text(&scene, "Appearance").unwrap().color, FOREGROUND);
    let icons: Vec<char> = scene
        .text
        .iter()
        .filter(|run| run.face == TextFace::PromptIcon)
        .filter_map(|run| run.text.chars().next())
        .collect();
    assert!(icons.contains(&APPEARANCE_ICON) && icons.contains(&KEYS_ICON));
    assert!(text(&scene, "Search").is_some());
    assert!(text(&scene, "/").is_some());
    assert!(
        scene
            .quads
            .iter()
            .any(|quad| quad.color == colour(SIDEBAR) && quad.rect.x == 161.0)
    );
    assert_eq!(
        scene
            .hits
            .hit_test(centre(control(&scene, SettingsHit::Section(6)).unwrap())),
        OverlayHit::Settings(SettingsHit::Section(6))
    );
}

#[test]
fn focused_sections_ring_the_current_item_and_drop_the_row_focus() {
    let mut sheet = appearance();
    sheet.sections_focused = true;
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let first = control(&scene, SettingsHit::Section(0)).unwrap();
    assert_eq!(
        quad_at(&scene, first)
            .unwrap()
            .border
            .map(|border| border.color),
        Some(colour(PRIMARY))
    );
    assert!(quad_at(&scene, rect(500.0, 150.0, 840.0, 58.0)).is_none());
}

#[test]
fn narrow_panels_collapse_the_sidebar_to_an_icon_rail_then_hide_it() {
    let scene = run(&appearance(), (640, 560), 1.0, None);
    let first = control(&scene, SettingsHit::Section(0)).unwrap();
    assert_eq!(first.w, 40.0);
    assert!(text(&scene, "Font").is_none());
    assert!(text(&scene, "Search").is_none());
    assert_eq!(control(&scene, SettingsHit::Search).unwrap().w, 40.0);
    let hidden = run(&appearance(), (400, 500), 1.0, None);
    assert!(control(&hidden, SettingsHit::Section(0)).is_none());
    assert!(control(&hidden, SettingsHit::Close).is_some());
}

#[test]
fn the_header_shows_the_breadcrumb_a_close_button_and_a_hairline() {
    let scene = run(&appearance(), (1600, 1000), 1.0, None);
    let crumbs: Vec<(&str, f32, Rgb)> = ["Settings", "/", "Appearance"]
        .iter()
        .map(|label| {
            let run = scene
                .text
                .iter()
                .find(|run| run.text == *label && run.x >= 400.0 && !run.bold)
                .unwrap();
            (run.text.as_str(), run.x, run.color)
        })
        .collect();
    assert_eq!(
        crumbs,
        [
            ("Settings", 420.0, MUTED),
            ("/", 488.0, MUTED),
            ("Appearance", 507.0, FOREGROUND)
        ]
    );
    let close = control(&scene, SettingsHit::Close).unwrap();
    assert_eq!(close, rect(1404.0, 50.0, 28.0, 28.0));
    assert!(
        scene
            .text
            .iter()
            .any(|run| run.text == CLOSE_ICON.to_string() && run.color == MUTED)
    );
    assert!(quad_at(&scene, rect(400.0, 87.0, 1039.0, 1.0)).is_some());
    let hovered = run(&appearance(), (1600, 1000), 1.0, Some((1418.0, 64.0)));
    assert_eq!(quad_at(&hovered, close).unwrap().color, colour(GHOST_HOVER));
}

#[test]
fn rows_sit_in_a_card_under_the_section_heading() {
    let scene = run(&appearance(), (1600, 1000), 1.0, None);
    let heading = scene
        .text
        .iter()
        .find(|run| run.text == "Appearance" && run.bold)
        .unwrap();
    assert_eq!((heading.x, heading.color), (512.0, FOREGROUND));
    let rows: Vec<PxRect> = scene.hits.rows.iter().map(|(_, rect)| *rect).collect();
    assert_eq!(rows[0], rect(496.0, 146.0, 848.0, 66.0));
    assert_eq!(rows[5], rect(496.0, 476.0, 848.0, 66.0));
    let card = quad_at(&scene, rect(496.0, 146.0, 848.0, 396.0)).unwrap();
    assert_eq!((card.color, card.radius), (colour(CARD), 12.0));
    assert_eq!(card.border.unwrap().color, colour(CARD_BORDER));
    assert_eq!(
        quad_at(&scene, rect(497.0, 212.0, 846.0, 1.0))
            .unwrap()
            .color,
        colour(DIVIDER)
    );
    let title = text(&scene, "Background").unwrap();
    assert_eq!(
        (title.x, title.color, title.bold),
        (512.0, FOREGROUND, false)
    );
    let description = scene
        .text
        .iter()
        .find(|run| run.text == "Overrides the theme value." && run.face == TextFace::Small)
        .unwrap();
    assert_eq!((description.x, description.color), (512.0, MUTED));
}

#[test]
fn modified_values_show_an_undo_button_after_the_title() {
    let scene = run(&appearance(), (1600, 1000), 1.0, None);
    let undo = control(&scene, SettingsHit::Reset(0)).unwrap();
    assert_eq!(undo, rect(553.0, 158.0, 20.0, 20.0));
    assert!(control(&scene, SettingsHit::Reset(1)).is_none());
    assert_eq!(
        scene.hits.hit_test(centre(undo)),
        OverlayHit::Settings(SettingsHit::Reset(0))
    );
    let mut pending = appearance();
    pending.groups[0].entries[1] = Entry::Setting {
        title: "Background",
        description: "",
        modified: false,
        control: Control::Number("1".into()),
        error: None,
        pending: true,
    };
    let scene = run(&pending, (1600, 1000), 1.0, None);
    assert!(control(&scene, SettingsHit::Reset(1)).is_some());
}

#[test]
fn the_focused_row_is_tinted_and_hover_is_quieter() {
    let scene = run(&appearance(), (1600, 1000), 1.0, Some((800.0, 250.0)));
    let focus = quad_at(&scene, rect(500.0, 150.0, 840.0, 58.0)).unwrap();
    assert_eq!(focus.color, colour(ROW_FOCUS));
    let hover = quad_at(&scene, rect(500.0, 216.0, 840.0, 58.0)).unwrap();
    assert_eq!(hover.color, colour(ROW_HOVER));
}

#[test]
fn selects_are_right_aligned_triggers_with_a_chevron() {
    let scene = run(&appearance(), (1600, 1000), 1.0, None);
    let trigger = control(&scene, SettingsHit::Cycle(0)).unwrap();
    assert_eq!(trigger, rect(1152.0, 165.0, 176.0, 28.0));
    let quad = quad_at(&scene, trigger).unwrap();
    assert_eq!((quad.color, quad.radius), (colour(INPUT), 8.0));
    assert_eq!(quad.border.unwrap().color, colour(INPUT_BORDER));
    assert_eq!(text(&scene, "flair-dark").unwrap().x, 1161.0);
    assert!(
        scene
            .text
            .iter()
            .any(|run| run.text == CHEVRON_ICON.to_string() && run.color == CHEVRON)
    );
    let swatch = control(&scene, SettingsHit::Edit(1)).unwrap();
    assert_eq!(swatch, rect(1152.0, 231.0, 176.0, 28.0));
    let chip = quad_at(&scene, rect(1161.0, 238.0, 14.0, 14.0)).unwrap();
    assert_eq!(
        chip.color,
        flair_core::theme::Rgba::from_hex("#030303").unwrap()
    );
    assert_eq!(text(&scene, "#030303").unwrap().x, 1183.0);
}

#[test]
fn switches_are_pills_with_a_sliding_thumb() {
    let sheet = sheet(
        2,
        "Window",
        vec![group(vec![
            setting("On", false, Control::Toggle(true)),
            setting("Off", false, Control::Toggle(false)),
        ])],
    );
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let on = quad_at(&scene, rect(1298.0, 170.0, 30.0, 18.0)).unwrap();
    assert_eq!((on.color, on.radius), (colour(PRIMARY), 9.0));
    let thumb = quad_at(&scene, rect(1312.0, 172.0, 14.0, 14.0)).unwrap();
    assert_eq!(thumb.color, colour(SWITCH_THUMB_ON));
    let off = quad_at(&scene, rect(1298.0, 236.0, 30.0, 18.0)).unwrap();
    assert_eq!(off.color, colour(SWITCH_OFF));
    assert_eq!(
        quad_at(&scene, rect(1300.0, 238.0, 14.0, 14.0))
            .unwrap()
            .color,
        colour(MUTED)
    );
    assert_eq!(
        control(&scene, SettingsHit::Toggle(0)),
        Some(rect(1296.0, 168.0, 34.0, 22.0))
    );
}

#[test]
fn numbers_step_from_minus_and_plus_zones_inside_the_trigger() {
    let scene = run(&keys(0), (1600, 1000), 1.0, None);
    let trigger = quad_at(&scene, rect(1152.0, 297.0, 176.0, 28.0)).unwrap();
    assert_eq!(trigger.color, colour(INPUT));
    assert_eq!(
        control(&scene, SettingsHit::Increase(2)),
        Some(rect(1305.0, 297.0, 22.0, 28.0))
    );
    assert_eq!(
        control(&scene, SettingsHit::Decrease(2)),
        Some(rect(1283.0, 297.0, 22.0, 28.0))
    );
    assert_eq!(text(&scene, "1500").unwrap().x, 1161.0);
    assert_eq!(
        control(&scene, SettingsHit::Edit(1)),
        Some(rect(1088.0, 231.0, 240.0, 28.0))
    );
}

#[test]
fn the_editor_is_a_primary_ringed_field_with_a_caret() {
    let mut sheet = keys(0);
    sheet.groups[0].entries[1] = setting(
        "Prefix key",
        false,
        Control::Editor(Field {
            text: "C-b".into(),
            cursor: 3,
            placeholder: "unset",
        }),
    );
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let field = quad_at(&scene, rect(1088.0, 231.0, 240.0, 28.0)).unwrap();
    assert_eq!(field.border.unwrap().color, colour(PRIMARY));
    assert_eq!(text(&scene, "C-b").unwrap().x, 1097.0);
    assert!(quad_at(&scene, rect(1118.0, 238.0, 1.0, 14.0)).is_some());
}

#[test]
fn an_error_extends_its_row_with_an_error_line() {
    let mut sheet = appearance();
    sheet.groups[0].entries[1] = Entry::Setting {
        title: "Background",
        description: "Overrides the theme value.",
        modified: false,
        control: Control::Text("nope".into()),
        error: Some("invalid colour".into()),
        pending: false,
    };
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    assert_eq!(scene.hits.rows[1].1, rect(496.0, 212.0, 848.0, 84.0));
    assert_eq!(scene.hits.rows[2].1.y, 296.0);
    let error = text(&scene, "invalid colour").unwrap();
    assert_eq!(
        (error.x, error.color, error.face),
        (512.0, ERROR, TextFace::Small)
    );
}

#[test]
fn bindings_show_the_command_with_table_and_chord_cap_on_the_right() {
    let mut sheet = keys(2);
    if let Entry::Binding {
        unbound, command, ..
    } = &mut sheet.groups[1].entries[1]
    {
        *unbound = true;
        *command = "unbound".into();
    }
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let heading = scene
        .text
        .iter()
        .find(|run| run.text == "Key bindings" && run.bold)
        .unwrap();
    assert_eq!(heading.x, 512.0);
    let row = scene.hits.rows[3].1;
    assert_eq!(row.h, 44.0);
    let command = text(&scene, "cmd0").unwrap();
    assert_eq!((command.x, command.color), (512.0, FOREGROUND));
    assert_eq!(text(&scene, "unbound").unwrap().color, MUTED);
    let cap = quad_at(&scene, rect(1308.0, row.y + 12.0, 20.0, 20.0)).unwrap();
    assert_eq!(cap.color, colour(CHORD));
    let chord = text(&scene, "c0").unwrap();
    assert_eq!(chord.face, TextFace::Small);
    let table = scene
        .text
        .iter()
        .find(|run| run.text == "prefix" && run.baseline < row.y + row.h)
        .unwrap();
    assert_eq!((table.x, table.color), (1275.0, QUIET));
}

#[test]
fn a_disabled_binding_list_marks_the_heading_off() {
    let mut sheet = keys(2);
    sheet.groups[1].disabled = true;
    for entry in &mut sheet.groups[1].entries {
        if let Entry::Binding { disabled, .. } = entry {
            *disabled = true;
        }
    }
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    assert_eq!(text(&scene, " off").unwrap().color, MUTED);
    assert_eq!(text(&scene, "cmd0").unwrap().color, MUTED);
}

#[test]
fn a_query_shows_in_the_sidebar_search_and_every_group_gets_a_heading() {
    let mut sheet = sheet(
        0,
        "Search",
        vec![
            Group {
                heading: Some("Window"),
                disabled: false,
                entries: vec![setting("Padding X", true, Control::Number("12".into()))],
            },
            Group {
                heading: Some("Terminal"),
                disabled: false,
                entries: vec![setting("Padding", false, Control::Number("1".into()))],
            },
        ],
    );
    sheet.section = None;
    sheet.search.text = "pad".into();
    sheet.search.cursor = 3;
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let query = text(&scene, "pad").unwrap();
    assert_eq!((query.x, query.color), (200.0, FOREGROUND));
    assert!(quad_at(&scene, rect(221.0, 57.0, 1.0, 14.0)).is_some());
    assert!(text(&scene, "Search").is_some_and(|run| run.x >= 400.0));
    assert!(text(&scene, "/").is_some_and(|run| run.x >= 400.0));
    let headings: Vec<(&str, f32)> = scene
        .text
        .iter()
        .filter(|run| run.bold)
        .map(|run| (run.text.as_str(), run.baseline))
        .collect();
    assert_eq!(headings, [("Window", 128.0), ("Terminal", 260.0)]);
    assert!(
        scene
            .quads
            .iter()
            .all(|quad| quad.color != colour(NAV_CURRENT))
    );
}

#[test]
fn scrolling_keeps_the_focused_entry_inside_the_viewport() {
    let mut sheet = keys(40);
    sheet.focus = Some(42);
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let (item, last) = *scene.hits.rows.last().unwrap();
    assert_eq!(item, 42);
    assert!(last.y + last.h <= 936.0 && last.y + last.h > 936.0 - 44.0);
    assert!(scene.hits.rows.iter().all(|(_, row)| row.y >= 112.0));
    assert!(text(&scene, "Bindings").is_none());
    let first = scene.hits.rows[0].1;
    let card = scene
        .quads
        .iter()
        .find(|quad| quad.color == colour(CARD))
        .unwrap();
    assert_eq!(card.rect.y, first.y - 12.0);
    assert!(quad_at(&scene, rect(496.0, first.y - 12.0, 848.0, 12.0)).is_some());
}

#[test]
fn the_editor_rect_is_the_content_viewport() {
    let theme = Config::default().theme();
    assert_eq!(
        editor_rect(&input(&theme, (1600, 1000), 1.0, None)),
        rect(496.0, 112.0, 848.0, 824.0)
    );
}

#[cfg(test)]
#[path = "settings_layout_more_tests.rs"]
mod more;
