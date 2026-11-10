use super::*;
use crate::{
    CellMetrics, UiMetrics,
    chrome_bar::opaque,
    overlay_layout::OverlayHit,
    renderer::{Border, PxRect, Quad, TextFace, TextRun},
    ui::ELLIPSIS,
};
use flair_core::{
    config::Config,
    theme::{Rgb, Theme},
};

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn run(
    sheet: &SettingsSheet,
    px: (u32, u32),
    scale: f32,
    hover: Option<(f32, f32)>,
) -> OverlayScene {
    let theme = Config::default().theme();
    let input = OverlayInput {
        metrics: crate::CellMetrics {
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
        theme: &theme,
        hover,
    };
    layout(sheet, &input, &mut measure)
}

fn theme() -> Theme {
    Config::default().theme()
}

fn setting(title: &'static str, source: &'static str, control: Control) -> Entry {
    Entry::Setting {
        title,
        description: "Overrides the theme value.",
        source,
        control,
        error: None,
        pending: false,
    }
}

fn sheet(section: usize, crumb: &'static str, groups: Vec<Group>) -> SettingsSheet {
    SettingsSheet {
        sections: vec!["Appearance", "Font", "Window", "Terminal", "Prompt", "Keys"],
        section: Some(section),
        sections_focused: false,
        crumb,
        search: Field {
            text: String::new(),
            cursor: 0,
            placeholder: "Search settings",
        },
        groups,
        focus: Some(0),
        keys: vec![
            ("↵", "edit".into()),
            ("space", "toggle".into()),
            ("←→", "step".into()),
            ("del", "reset".into()),
            ("esc", "close".into()),
            ("/", "search".into()),
        ],
    }
}

fn appearance() -> SettingsSheet {
    let text = |value: &str| Control::Color(value.into());
    sheet(
        0,
        "Appearance",
        vec![Group {
            heading: None,
            disabled: false,
            entries: vec![
                setting("Theme", "flair.toml", Control::Choice("flair-dark".into())),
                setting("Background", "default", text("#030303")),
                setting("Foreground", "default", text("#d4d4d4")),
                setting("Cursor colour", "default", text("#d4d4d4")),
                setting("Accent", "default", text("#19aad8")),
                setting("Selection", "default", text("#19aad84d")),
            ],
        }],
    )
}

fn editor(text: &str) -> Control {
    Control::Editor(Field {
        text: text.into(),
        cursor: text.chars().count(),
        placeholder: "unset",
    })
}

fn binding(index: usize) -> Entry {
    Entry::Binding {
        chord: format!("c{index}"),
        command: format!("cmd{index}"),
        unbound: false,
        table: "prefix",
        source: "default",
        disabled: false,
    }
}

fn keys(bindings: usize) -> SettingsSheet {
    sheet(
        4,
        "Keys",
        vec![
            Group {
                heading: None,
                disabled: false,
                entries: vec![
                    setting("Bindings", "default", Control::Toggle(true)),
                    setting("Prefix key", "default", Control::Text("C-a".into())),
                    setting(
                        "Prefix timeout",
                        "flair.toml",
                        Control::Number("1500".into()),
                    ),
                ],
            },
            Group {
                heading: Some("Key bindings"),
                disabled: false,
                entries: (0..bindings).map(binding).collect(),
            },
        ],
    )
}

fn text<'a>(scene: &'a OverlayScene, text: &str) -> Option<&'a TextRun> {
    scene.text.iter().find(|run| run.text == text)
}

fn quad_at(scene: &OverlayScene, rect: PxRect) -> Option<&Quad> {
    scene.quads.iter().find(|quad| quad.rect == rect)
}

fn assert_quad(scene: &OverlayScene, quad: Quad) {
    assert!(scene.quads.contains(&quad), "{quad:?}");
}

fn styled<'a>(scene: &'a OverlayScene, text: &str, bold: bool) -> &'a TextRun {
    scene
        .text
        .iter()
        .find(|run| run.text == text && run.bold == bold)
        .unwrap_or_else(|| panic!("{text} missing"))
}

fn assert_hit(scene: &OverlayScene, hit: SettingsHit, rect: PxRect) {
    assert_eq!(control(scene, hit), rect);
    assert_eq!(scene.hits.hit_test(centre(rect)), OverlayHit::Settings(hit));
}

fn chip_quad(rect: PxRect) -> Quad {
    let surfaces = theme().surfaces();
    outlined(rect, surfaces.surface3, 6.0, surfaces.outline_strong)
}

fn key_cap(rect: PxRect) -> Quad {
    let surfaces = theme().surfaces();
    outlined(rect, surfaces.surface3, 4.0, surfaces.outline_strong)
}

fn control(scene: &OverlayScene, hit: SettingsHit) -> PxRect {
    scene
        .hits
        .controls
        .iter()
        .find(|(candidate, _)| *candidate == hit)
        .map(|(_, rect)| *rect)
        .unwrap_or_else(|| panic!("{hit:?} missing"))
}

fn centre(rect: PxRect) -> (f64, f64) {
    (
        f64::from(rect.x + rect.w / 2.0),
        f64::from(rect.y + rect.h / 2.0),
    )
}

fn outlined(rect: PxRect, color: Rgb, radius: f32, border: Rgb) -> Quad {
    Quad {
        rect,
        color: opaque(color),
        radius,
        border: Some(Border {
            color: opaque(border),
            width: 1.0,
        }),
    }
}

fn plain(rect: PxRect, color: Rgb, radius: f32) -> Quad {
    Quad {
        rect,
        color: opaque(color),
        radius,
        border: None,
    }
}

fn assert_inside(scene: &OverlayScene, window: (u32, u32)) {
    let (width, height) = (window.0 as f32, window.1 as f32);
    for quad in &scene.quads {
        let rect = quad.rect;
        assert!(
            rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.w <= width && rect.y + rect.h <= height,
            "{window:?} {rect:?}"
        );
        assert!(rect.w > 0.0 && rect.h > 0.0, "{window:?} {rect:?}");
    }
    for run in &scene.text {
        assert!(run.baseline <= height, "{window:?} {run:?}");
    }
}

#[test]
fn page_fills_the_window_below_the_bar_with_a_nav_hairline() {
    let surfaces = theme().surfaces();
    for (px, nav) in [((1920, 1200), 232.0), ((960, 600), 160.0)] {
        let scene = run(&appearance(), px, 1.0, None);
        let (width, height) = (px.0 as f32, px.1 as f32);
        let page = rect(0.0, 35.0, width, height - 35.0);
        assert_eq!(scene.quads[0], plain(page, theme().background, 0.0));
        assert_eq!(scene.hits.panel, page);
        let hairline = rect(nav, 35.0, 1.0, height - 35.0);
        assert_quad(&scene, plain(hairline, surfaces.outline, 0.0));
        assert_eq!(scene.hits.hit_test((10.0, 10.0)), OverlayHit::Outside);
        assert_eq!(
            scene.hits.hit_test((100.0, f64::from(height) - 30.0)),
            OverlayHit::Panel
        );
    }
}

#[test]
fn nav_shows_the_title_and_section_rows_with_the_selection() {
    let surfaces = theme().surfaces();
    let scene = run(&appearance(), (1920, 1200), 1.0, Some((30.0, 130.0)));
    let title = styled(&scene, "Settings", true);
    assert_eq!(
        (title.x, title.baseline, title.color),
        (16.0, 71.0, theme().foreground)
    );
    let selected = rect(16.0, 91.0, 200.0, 28.0);
    assert_quad(&scene, plain(selected, surfaces.surface3, 6.0));
    let hovered = rect(16.0, 123.0, 200.0, 28.0);
    assert_quad(&scene, plain(hovered, surfaces.surface2, 6.0));
    assert!(quad_at(&scene, rect(16.0, 219.0, 200.0, 28.0)).is_none());
    let label = styled(&scene, "Appearance", false);
    assert_eq!(
        (label.x, label.baseline, label.color),
        (26.0, 109.0, theme().foreground)
    );
    assert_eq!(control(&scene, SettingsHit::Section(0)), selected);
    assert_eq!(
        control(&scene, SettingsHit::Section(4)),
        rect(16.0, 219.0, 200.0, 28.0)
    );
    assert_eq!(
        scene.hits.hit_test((100.0, 230.0)),
        OverlayHit::Settings(SettingsHit::Section(4))
    );

    assert_eq!(
        control(&scene, SettingsHit::Section(5)),
        rect(16.0, 251.0, 200.0, 28.0)
    );

    let narrow = run(&appearance(), (960, 600), 1.0, None);
    assert_eq!(
        control(&narrow, SettingsHit::Section(0)),
        rect(16.0, 91.0, 128.0, 28.0)
    );
}

#[test]
fn six_nav_rows_fit_the_narrow_column_at_960x600() {
    let scene = run(&appearance(), (960, 600), 1.0, None);
    let sections: Vec<_> = scene
        .hits
        .controls
        .iter()
        .filter_map(|(hit, rect)| match hit {
            SettingsHit::Section(index) => Some((*index, *rect)),
            SettingsHit::Close
            | SettingsHit::Search
            | SettingsHit::Toggle(_)
            | SettingsHit::Decrease(_)
            | SettingsHit::Increase(_)
            | SettingsHit::Cycle(_)
            | SettingsHit::Edit(_)
            | SettingsHit::PromptEditor(_) => None,
        })
        .collect();
    let expected: Vec<_> = (0..6)
        .map(|index| (index, rect(16.0, 91.0 + index as f32 * 32.0, 128.0, 28.0)))
        .collect();
    assert_eq!(sections, expected);
    for name in ["Terminal", "Prompt", "Keys"] {
        let label = styled(&scene, name, false);
        assert_eq!(label.x, 26.0);
        assert!(label.x + measure(name, false) <= 144.0 - 10.0, "{name}");
    }
    assert_eq!(
        scene.hits.hit_test((80.0, 265.0)),
        OverlayHit::Settings(SettingsHit::Section(5))
    );
    assert_inside(&scene, (960, 600));
}

#[test]
fn header_shows_the_breadcrumb_and_a_close_mark() {
    let surfaces = theme().surfaces();
    for (px, x, right) in [((1920, 1200), 696.0, 1456.0), ((960, 600), 200.0, 920.0)] {
        let scene = run(&appearance(), px, 1.0, None);
        let root = styled(&scene, "Settings", false);
        assert_eq!(
            (root.x, root.baseline, root.color),
            (x, 71.0, surfaces.muted)
        );
        let gap = text(&scene, " / ").unwrap();
        assert_eq!((gap.x, gap.color), (x + 56.0, surfaces.muted));
        let crumb = styled(&scene, "Appearance", true);
        assert_eq!(
            (crumb.x, crumb.baseline, crumb.color),
            (x + 77.0, 71.0, theme().foreground)
        );
        let close = text(&scene, "close")
            .filter(|run| run.baseline == 71.0)
            .unwrap();
        assert_eq!((close.x, close.color), (right - 35.0, surfaces.muted));
        let cap = rect(right - 35.0 - 6.0 - 23.0, 57.0, 23.0, 20.0);
        assert_quad(&scene, key_cap(cap));
        assert_eq!(
            control(&scene, SettingsHit::Close),
            rect(cap.x, 55.0, right - cap.x, 24.0)
        );
        assert_eq!(
            scene.hits.hit_test((f64::from(right) - 5.0, 60.0)),
            OverlayHit::Settings(SettingsHit::Close)
        );
    }
}

#[test]
fn search_field_sits_below_the_header() {
    let surfaces = theme().surfaces();
    for (px, field) in [
        ((1920, 1200), rect(696.0, 103.0, 760.0, 32.0)),
        ((960, 600), rect(200.0, 103.0, 720.0, 32.0)),
    ] {
        let scene = run(&appearance(), px, 1.0, None);
        assert_quad(
            &scene,
            outlined(field, theme().background, 6.0, surfaces.outline),
        );
        let placeholder = text(&scene, "Search settings").unwrap();
        assert_eq!(
            (placeholder.x, placeholder.color),
            (field.x + 14.0, surfaces.faint)
        );
        assert_hit(&scene, SettingsHit::Search, field);
    }
}

#[test]
fn the_group_panel_tiles_rows_with_hairlines_between_them() {
    let surfaces = theme().surfaces();
    for (px, x, w) in [((1920, 1200), 696.0, 760.0), ((960, 600), 200.0, 720.0)] {
        let scene = run(&appearance(), px, 1.0, None);
        let panel = rect(x, 159.0, w, 336.0);
        assert_quad(
            &scene,
            outlined(panel, surfaces.surface1, 10.0, surfaces.outline),
        );
        let rows: Vec<_> = (0..6)
            .map(|index| (index, rect(x, 159.0 + 56.0 * index as f32, w, 56.0)))
            .collect();
        assert_eq!(scene.hits.rows, rows);
        assert!(quad_at(&scene, rect(x + 1.0, 159.0, w - 2.0, 1.0)).is_none());
        for index in 1..6 {
            let line = rect(x + 1.0, 159.0 + 56.0 * index as f32, w - 2.0, 1.0);
            assert_quad(&scene, plain(line, surfaces.outline, 0.0));
        }
        let focused = rect(x + 4.0, 163.0, w - 8.0, 48.0);
        assert_quad(&scene, plain(focused, surfaces.surface3, 6.0));
        assert_eq!(
            scene.hits.hit_test((f64::from(x) + 30.0, 230.0)),
            OverlayHit::Row(1)
        );
    }
}

#[test]
fn rows_pair_title_and_description_with_the_source_left_of_the_control() {
    let surfaces = theme().surfaces();
    let scene = run(&appearance(), (1920, 1200), 1.0, None);
    let title = text(&scene, "Theme").unwrap();
    assert_eq!(
        (title.x, title.baseline, title.color, title.face, title.bold),
        (712.0, 182.0, theme().foreground, TextFace::Ui, false)
    );
    let description = text(&scene, "Overrides the theme value.").unwrap();
    assert_eq!(
        (
            description.x,
            description.baseline,
            description.color,
            description.face
        ),
        (712.0, 198.0, surfaces.muted, TextFace::Small)
    );
    let chip = rect(1160.0, 175.0, 280.0, 24.0);
    assert_quad(&scene, chip_quad(chip));
    assert_hit(&scene, SettingsHit::Cycle(0), chip);
    let value = text(&scene, "flair-dark").unwrap();
    assert_eq!(
        (value.x, value.baseline, value.color),
        (1170.0, 191.0, theme().foreground)
    );
    let mark = text(&scene, "▾").unwrap();
    assert_eq!(
        (mark.x, mark.color, mark.face),
        (1427.0, surfaces.muted, TextFace::Small)
    );
    let source = text(&scene, "flair.toml").unwrap();
    assert_eq!(
        (source.x, source.color, source.face),
        (1113.0, surfaces.muted, TextFace::Small)
    );
    let chip = rect(1160.0, 231.0, 280.0, 24.0);
    assert_hit(&scene, SettingsHit::Edit(1), chip);
    assert_quad(&scene, chip_quad(chip));
    assert_eq!(text(&scene, "#030303").unwrap().x, 1190.0);
    for index in 2..6 {
        assert_eq!(
            control(&scene, SettingsHit::Edit(index)),
            rect(1160.0, 175.0 + 56.0 * index as f32, 280.0, 24.0)
        );
    }
    let sources: Vec<f32> = scene
        .text
        .iter()
        .filter(|run| run.text == "default")
        .map(|run| run.x)
        .collect();
    assert_eq!(sources, vec![1124.0; 5]);
}

#[test]
fn toggles_and_steppers_sit_on_the_control_column() {
    let surfaces = theme().surfaces();
    for (px, right) in [((1920, 1200), 1440.0), ((960, 600), 904.0)] {
        let scene = run(&keys(4), px, 1.0, None);
        let track = rect(right - 28.0, 179.0, 28.0, 16.0);
        assert_quad(&scene, outlined(track, theme().accent, 8.0, theme().accent));
        let knob = rect(right - 14.0, 181.0, 12.0, 12.0);
        assert_quad(&scene, plain(knob, theme().foreground, 6.0));
        assert_hit(&scene, SettingsHit::Toggle(0), track);
        let chip = rect(right - 280.0, 287.0, 280.0, 24.0);
        assert_quad(&scene, chip_quad(chip));
        let down = rect(chip.x, chip.y, 20.0, 24.0);
        let up = rect(chip.x + 260.0, chip.y, 20.0, 24.0);
        assert_hit(&scene, SettingsHit::Decrease(2), down);
        assert_hit(&scene, SettingsHit::Increase(2), up);
        assert_eq!(scene.hits.hit_test(centre(chip)), OverlayHit::Row(2));
        assert_eq!(text(&scene, "1500").unwrap().x, chip.x + 126.0);
        let down_mark = text(&scene, "‹").unwrap();
        assert_eq!(
            (
                down_mark.x,
                down_mark.baseline,
                down_mark.color,
                down_mark.face
            ),
            (chip.x + 7.0, 303.0, surfaces.muted, TextFace::Ui)
        );
        assert_eq!(text(&scene, "›").unwrap().color, surfaces.muted);
        let source = text(&scene, "flair.toml").unwrap();
        assert_eq!(source.x, chip.x - 12.0 - 35.0);
        let defaults: Vec<f32> = scene
            .text
            .iter()
            .filter(|run| run.text == "default")
            .map(|run| run.x)
            .collect();
        assert_eq!(defaults, vec![chip.x - 12.0 - 24.0; 2]);
    }
}

#[test]
fn an_off_toggle_has_a_background_track_and_a_muted_knob_at_the_start() {
    let surfaces = theme().surfaces();
    let mut sheet = keys(0);
    sheet.groups[0].entries[0] = setting("Bindings", "default", Control::Toggle(false));
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let track = rect(1412.0, 179.0, 28.0, 16.0);
    assert_quad(
        &scene,
        outlined(track, theme().background, 8.0, surfaces.outline_strong),
    );
    let knob = rect(1414.0, 181.0, 12.0, 12.0);
    assert_quad(&scene, plain(knob, surfaces.muted, 6.0));
}

#[test]
fn bindings_follow_a_heading_with_a_chord_cap_and_the_command() {
    let surfaces = theme().surfaces();
    let mut sheet = keys(4);
    sheet.groups[1].entries[1] = Entry::Binding {
        chord: "x".into(),
        command: "unbound".into(),
        unbound: true,
        table: "root",
        source: "flair.toml",
        disabled: false,
    };
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let heading = text(&scene, "Key bindings").unwrap();
    assert_eq!(
        (heading.x, heading.baseline, heading.bold, heading.color),
        (696.0, 367.0, true, theme().foreground)
    );
    let panel = rect(696.0, 383.0, 760.0, 128.0);
    assert_quad(
        &scene,
        outlined(panel, surfaces.surface1, 10.0, surfaces.outline),
    );
    assert_eq!(scene.hits.rows[3], (3, rect(696.0, 383.0, 760.0, 32.0)));
    let cap = rect(712.0, 389.0, 20.0, 20.0);
    assert_quad(&scene, key_cap(cap));
    let command = text(&scene, "cmd0").unwrap();
    assert_eq!(
        (command.x, command.baseline, command.color),
        (784.0, 403.0, theme().foreground)
    );
    let unbound = text(&scene, "unbound").unwrap();
    assert_eq!((unbound.x, unbound.color), (784.0, surfaces.muted));
    let source = scene
        .text
        .iter()
        .find(|run| run.text == "flair.toml" && run.baseline > 400.0)
        .unwrap();
    assert_eq!(source.x, 1405.0);
    let table = text(&scene, "root").unwrap();
    assert_eq!(
        (table.x, table.color, table.face),
        (1350.0, surfaces.muted, TextFace::Small)
    );
    let defaults = scene
        .text
        .iter()
        .filter(|run| run.text == "default" && run.baseline > 380.0)
        .count();
    assert_eq!(defaults, 0);
    let table = text(&scene, "prefix").unwrap();
    assert_eq!((table.x, table.baseline), (1343.0, 401.0));
}

#[test]
fn hover_is_surface2_and_focus_surface3_and_focus_wins() {
    let surfaces = theme().surfaces();
    let scene = run(&appearance(), (1920, 1200), 1.0, Some((824.0, 230.0)));
    let hovered = rect(700.0, 219.0, 752.0, 48.0);
    assert_quad(&scene, plain(hovered, surfaces.surface2, 6.0));
    let focused = rect(700.0, 163.0, 752.0, 48.0);
    assert_quad(&scene, plain(focused, surfaces.surface3, 6.0));

    let scene = run(&appearance(), (1920, 1200), 1.0, Some((824.0, 170.0)));
    let highlights: Vec<_> = scene
        .quads
        .iter()
        .filter(|quad| quad.rect == focused)
        .collect();
    assert_eq!(highlights, vec![&plain(focused, surfaces.surface3, 6.0)]);
}

#[test]
fn focused_sections_leave_the_entries_unhighlighted() {
    let mut sheet = appearance();
    sheet.sections_focused = true;
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    assert!(quad_at(&scene, rect(700.0, 163.0, 752.0, 48.0)).is_none());
    assert!(quad_at(&scene, rect(16.0, 91.0, 200.0, 28.0)).is_some());
}

#[test]
fn the_editor_is_a_prompt_field_with_an_accent_caret() {
    let surfaces = theme().surfaces();
    let mut sheet = keys(0);
    sheet.groups[0].entries[2] = setting("Prefix timeout", "flair.toml", editor("1500"));
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let field = rect(1160.0, 287.0, 280.0, 24.0);
    assert_quad(
        &scene,
        outlined(field, theme().background, 6.0, surfaces.outline),
    );
    let caret = rect(1198.0, 292.0, 1.0, 14.0);
    assert_quad(&scene, plain(caret, theme().accent, 0.0));
    let value = text(&scene, "1500").unwrap();
    assert_eq!(
        (value.x, value.baseline, value.color),
        (1170.0, 303.0, theme().foreground)
    );
    assert_eq!(text(&scene, "flair.toml").unwrap().x, 1113.0);
}

#[test]
fn an_error_extends_its_row_with_an_attention_line() {
    let surfaces = theme().surfaces();
    let mut sheet = appearance();
    sheet.groups[0].entries[0] = Entry::Setting {
        title: "Theme",
        description: "Colour theme.",
        source: "flair.toml",
        control: Control::Choice("flair-dark".into()),
        error: Some("flair.toml does not parse".into()),
        pending: false,
    };
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    let error = text(&scene, "flair.toml does not parse").unwrap();
    assert_eq!(
        (error.x, error.baseline, error.color, error.face),
        (712.0, 227.0, surfaces.attention, TextFace::Small)
    );
    assert_eq!(scene.hits.rows[0], (0, rect(696.0, 159.0, 760.0, 76.0)));
    assert_eq!(scene.hits.rows[1], (1, rect(696.0, 235.0, 760.0, 56.0)));
    assert_eq!(control(&scene, SettingsHit::Cycle(0)).y, 175.0);
    let focused = rect(700.0, 163.0, 752.0, 68.0);
    assert!(quad_at(&scene, focused).is_some());
    assert!(quad_at(&scene, rect(696.0, 159.0, 760.0, 356.0)).is_some());
}

#[test]
fn a_search_shows_every_group_under_its_heading_without_a_section() {
    let mut sheet = sheet(
        0,
        "Search",
        vec![
            Group {
                heading: Some("Font"),
                disabled: false,
                entries: vec![setting("Size", "default", Control::Number("13".into()))],
            },
            Group {
                heading: Some("Window"),
                disabled: false,
                entries: vec![
                    setting("Padding X", "default", Control::Number("8".into())),
                    setting("Padding Y", "default", Control::Number("0".into())),
                ],
            },
        ],
    );
    sheet.section = None;
    sheet.search = Field {
        text: "pad".into(),
        cursor: 3,
        placeholder: "Search settings",
    };
    let scene = run(&sheet, (1920, 1200), 1.0, None);
    assert!(quad_at(&scene, rect(16.0, 91.0, 200.0, 28.0)).is_none());
    let heading = |name: &str| {
        scene
            .text
            .iter()
            .find(|run| run.text == name && run.bold)
            .map(|run| (run.x, run.baseline, run.color))
    };
    assert_eq!(heading("Font"), Some((696.0, 175.0, theme().foreground)));
    assert_eq!(heading("Window"), Some((696.0, 287.0, theme().foreground)));
    assert_eq!(
        scene.hits.rows,
        vec![
            (0, rect(696.0, 191.0, 760.0, 56.0)),
            (1, rect(696.0, 303.0, 760.0, 56.0)),
            (2, rect(696.0, 359.0, 760.0, 56.0)),
        ]
    );
    assert!(quad_at(&scene, rect(696.0, 191.0, 760.0, 56.0)).is_some());
    assert!(quad_at(&scene, rect(696.0, 303.0, 760.0, 112.0)).is_some());
    let typed = text(&scene, "pad").unwrap();
    assert_eq!((typed.x, typed.color), (706.0, theme().foreground));
    assert!(text(&scene, "Search settings").is_none());
    let bold_search = scene
        .text
        .iter()
        .find(|run| run.text == "Search" && run.bold);
    assert!(bold_search.is_some());
}

#[test]
fn the_footer_draws_a_rule_and_key_caps_at_the_bottom() {
    let surfaces = theme().surfaces();
    for (px, x, w, rule) in [
        ((1920, 1200), 696.0, 760.0, 1147.0),
        ((960, 600), 200.0, 720.0, 547.0),
    ] {
        let scene = run(&appearance(), px, 1.0, None);
        let line = rect(x, rule, w, 1.0);
        assert_quad(&scene, plain(line, surfaces.outline, 0.0));
        let cap = rect(x, rule + 13.0, 20.0, 20.0);
        assert_quad(&scene, key_cap(cap));
        let verb = text(&scene, "edit").unwrap();
        assert_eq!((verb.x, verb.color), (x + 26.0, surfaces.muted));
        assert!(text(&scene, "space").is_some());
        assert!(text(&scene, "search").is_some());
    }
}

#[test]
fn scrolling_keeps_the_focused_entry_on_the_last_visible_row() {
    let mut sheet = keys(40);
    sheet.focus = Some(42);
    for (px, bottom, first) in [((1920, 1200), 1135.0, 13), ((960, 600), 535.0, 32)] {
        let scene = run(&sheet, px, 1.0, None);
        let indices: Vec<usize> = scene.hits.rows.iter().map(|(index, _)| *index).collect();
        assert_eq!(indices, (first..=42).collect::<Vec<_>>(), "{px:?}");
        let (_, last) = *scene.hits.rows.last().unwrap();
        assert_eq!(last.y + last.h, bottom, "{px:?}");
        assert!(
            scene
                .hits
                .rows
                .iter()
                .all(|(_, row)| row.y >= 159.0 && row.y + row.h <= bottom)
        );
        assert!(text(&scene, "Bindings").is_none());
        assert!(text(&scene, "Key bindings").is_none());
        assert!(text(&scene, &format!("cmd{}", first - 4)).is_none());
        assert!(text(&scene, &format!("cmd{}", first - 3)).is_some());
        for run in scene.text.iter().filter(|run| run.text.starts_with("cmd")) {
            assert!(run.baseline > 159.0 && run.baseline < bottom, "{run:?}");
        }
        let panel = scene.quads.iter().find(|quad| quad.radius == 10.0).unwrap();
        let top = scene.hits.rows[0].1.y;
        assert_eq!(panel.rect.y, top - 10.0);
        assert_eq!(panel.rect.y + panel.rect.h, bottom);
        let strip = rect(panel.rect.x, top - 10.0, panel.rect.w, 10.0);
        assert_quad(&scene, plain(strip, theme().background, 0.0));
        assert!(
            quad_at(
                &scene,
                rect(panel.rect.x + 1.0, top, panel.rect.w - 2.0, 1.0)
            )
            .is_none()
        );
        let more = text(&scene, &format!("+{first} more")).unwrap();
        assert_eq!(more.color, theme().surfaces().muted);
        let focused = rect(last.x + 4.0, last.y + 4.0, last.w - 8.0, 24.0);
        assert!(quad_at(&scene, focused).is_some());
    }
}

#[test]
fn the_keys_page_fits_three_settings_and_four_bindings_at_960x600() {
    let scene = run(&keys(30), (960, 600), 1.0, None);
    assert_eq!(scene.hits.rows.len(), 7);
    assert_eq!(scene.hits.page(), 7);
    assert_inside(&scene, (960, 600));
}

#[test]
fn narrow_windows_ellipsize_descriptions_left_of_the_source_label() {
    let mut sheet = appearance();
    sheet.groups[0].entries[1] = Entry::Setting {
        title: "Background",
        description: "A description that is far too long to fit next to the control in a narrow window, so it has to end in an ellipsis before it reaches the source label of the row, and then it keeps on going with even more words that nobody will ever read.",
        source: "default",
        control: Control::Text("#030303".into()),
        error: None,
        pending: false,
    };
    let scene = run(&sheet, (960, 600), 1.0, None);
    assert_inside(&scene, (960, 600));
    assert_eq!(
        control(&scene, SettingsHit::Edit(1)),
        rect(624.0, 231.0, 280.0, 24.0)
    );
    let description = scene
        .text
        .iter()
        .find(|run| run.text.starts_with("A description"))
        .unwrap();
    assert!(description.text.ends_with(ELLIPSIS));
    let label = scene
        .text
        .iter()
        .find(|run| run.text == "default" && run.baseline > 231.0 && run.baseline < 271.0)
        .unwrap();
    assert_eq!(label.x, 588.0);
    let end = description.x + measure(&description.text, false) * 0.5;
    assert!(end <= label.x - 12.0, "{end} {}", label.x);
}

#[path = "settings_layout_more_tests.rs"]
mod more;
