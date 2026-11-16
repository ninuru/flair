use super::*;
use crate::{t3_style::POPUP_SELECTED, ui::ELLIPSIS};
use flair_core::settings::{Kind, SETTINGS};

fn assert_inside(scene: &OverlayScene, window: (u32, u32)) {
    let (width, height) = (window.0 as f32, window.1 as f32);
    for quad in &scene.quads {
        let rect = quad.rect;
        assert!(
            rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.w <= width && rect.y + rect.h <= height,
            "{window:?} {rect:?}"
        );
    }
    for run in &scene.text {
        assert!(run.baseline <= height, "{window:?} {run:?}");
    }
}

fn whole(rect: PxRect) -> bool {
    [rect.x, rect.y, rect.w, rect.h]
        .iter()
        .all(|v| v.fract() == 0.0)
}

fn dropdown(options: Vec<String>, selected: usize) -> SettingsSheet {
    let mut sheet = appearance();
    sheet.dropdown = Some(ChoiceMenu {
        item: 0,
        options,
        selected,
    });
    sheet
}

#[test]
fn long_text_values_ellipsize_inside_the_trigger() {
    let mut sheet = appearance();
    sheet.groups[0].entries[1] = setting("Background", false, Control::Text("x".repeat(80)));
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let trigger = control(&scene, SettingsHit::Edit(1)).unwrap();
    assert_eq!(trigger.x + trigger.w, 1328.0);
    let value = scene
        .text
        .iter()
        .find(|run| run.text.starts_with("xxx"))
        .unwrap();
    assert!(value.text.ends_with(ELLIPSIS));
    assert_eq!(value.x, trigger.x + 9.0);
    assert!(value.x + measure(&value.text, false) <= trigger.x + trigger.w - 9.0);
}

#[test]
fn every_rect_and_text_lands_on_whole_pixels_at_scale_one_and_a_half() {
    let mut editing = keys(12);
    editing.groups[0].entries[1] = setting(
        "Prefix key",
        false,
        Control::Editor(Field {
            text: "C-b".into(),
            cursor: 1,
            placeholder: "unset",
        }),
    );
    for sheet in [
        appearance(),
        keys(12),
        editing,
        dropdown(vec!["a".into()], 0),
    ] {
        let scene = run(&sheet, (1500, 900), 1.5, Some((700.0, 400.0)));
        assert_inside(&scene, (1500, 900));
        for quad in &scene.quads {
            assert!(whole(quad.rect) && quad.radius.fract() == 0.0, "{quad:?}");
        }
        for run in &scene.text {
            assert_eq!((run.x.fract(), run.baseline.fract()), (0.0, 0.0), "{run:?}");
        }
        let rows = scene.hits.rows.iter().map(|(_, rect)| *rect);
        for rect in rows.chain(scene.hits.controls.iter().map(|(_, rect)| *rect)) {
            assert!(whole(rect), "{rect:?}");
        }
    }
}

#[test]
fn scale_two_doubles_the_geometry() {
    let scene = run(&appearance(), (3200, 2000), 2.0, None);
    assert_eq!(scene.hits.panel, rect(320.0, 80.0, 2560.0, 1840.0));
    assert_eq!(
        control(&scene, SettingsHit::Section(0)),
        Some(rect(336.0, 176.0, 448.0, 64.0))
    );
    assert_eq!(
        control(&scene, SettingsHit::Cycle(0)),
        Some(rect(2304.0, 330.0, 352.0, 56.0))
    );
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
    ] {
        for sheet in [appearance(), keys(30), dropdown(vec!["a".into()], 0)] {
            let scene = run(&sheet, px, scale, Some((10.0, 10.0)));
            assert_inside(&scene, px);
        }
    }
}

fn prompt() -> SettingsSheet {
    let entries = SETTINGS
        .iter()
        .filter(|setting| setting.section == Section::Prompt)
        .map(|setting| Entry::Setting {
            title: setting.title,
            description: setting.description,
            modified: false,
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
    sheet(5, "Prompt", vec![group(entries)])
}

#[test]
fn the_prompt_section_fits_at_960x600_with_descriptions_left_of_the_controls() {
    let sheet = prompt();
    let scene = run(&sheet, (960, 600), 1.0, None);
    assert_inside(&scene, (960, 600));
    assert!(scene.hits.rows.len() >= 5);
    let trigger = control(&scene, SettingsHit::Cycle(0)).unwrap();
    for run in scene
        .text
        .iter()
        .filter(|run| run.face == crate::renderer::TextFace::Small && run.x < trigger.x)
    {
        assert!(
            run.x + measure(&run.text, false) * 0.5 <= trigger.x - 32.0,
            "{run:?}"
        );
    }
}

#[test]
fn dropdown_options_override_underlying_hits_and_stay_on_screen() {
    for (size, scale) in [((960, 600), 1.0), ((640, 400), 1.0), ((1440, 900), 1.5)] {
        let sheet = dropdown((0..100).map(|index| format!("Theme {index}")).collect(), 99);
        let scene = run(&sheet, size, scale, None);
        let options: Vec<_> = scene
            .hits
            .controls
            .iter()
            .filter_map(|(hit, rect)| {
                if let SettingsHit::Choose(index) = hit {
                    Some((*index, *rect))
                } else {
                    None
                }
            })
            .collect();
        assert!(!options.is_empty());
        assert!(options.len() <= 10);
        assert!(options.iter().any(|(index, _)| *index == 99));
        for (index, rect) in options {
            assert!(rect.y >= 0.0 && rect.y + rect.h <= size.1 as f32);
            assert!(rect.x >= 0.0 && rect.x + rect.w <= size.0 as f32);
            assert_eq!(
                scene.hits.hit_test(centre(rect)),
                OverlayHit::Settings(SettingsHit::Choose(index))
            );
        }
        assert_eq!(
            scene.hits.hit_test((5.0, 5.0)),
            OverlayHit::Settings(SettingsHit::DismissDropdown)
        );
        assert!(scene.hits.rows.is_empty());
    }
}

#[test]
fn the_popup_opens_below_the_trigger_right_aligned_with_the_selection_tinted() {
    let sheet = dropdown(
        vec![
            "one".into(),
            "a much longer option name".into(),
            "three".into(),
        ],
        1,
    );
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let trigger = rect(1152.0, 165.0, 176.0, 28.0);
    assert_eq!(
        quad_at(&scene, trigger).unwrap().border.unwrap().color,
        colour(PRIMARY)
    );
    let first = control(&scene, SettingsHit::Choose(0)).unwrap();
    assert_eq!(first.y, 165.0 + 28.0 + 4.0 + 4.0);
    assert_eq!(first.x + first.w + 4.0, 1328.0);
    assert!(first.w + 8.0 >= 176.0);
    let selected = control(&scene, SettingsHit::Choose(1)).unwrap();
    assert_eq!(
        quad_at(&scene, selected).unwrap().color,
        colour(POPUP_SELECTED)
    );
}

#[test]
fn dropdown_occludes_text_under_the_menu() {
    let sheet = dropdown(vec!["one".into(), "two".into(), "three".into()], 0);
    let scene = run(&sheet, (1100, 700), 1.0, None);
    let last = control(&scene, SettingsHit::Choose(2)).unwrap();
    let first = control(&scene, SettingsHit::Choose(0)).unwrap();
    let bottom = last.y + last.h + 4.0;
    let top = first.y - 4.0;
    assert!(
        scene
            .text
            .iter()
            .filter(|run| run.x >= first.x && run.face == crate::renderer::TextFace::Ui)
            .filter(|run| !["one", "two", "three"].contains(&run.text.as_str()))
            .all(|run| run.baseline - 11.0 >= bottom || run.baseline + 3.0 <= top)
    );
}

#[test]
fn a_rail_keeps_the_typed_query_visible_in_the_header() {
    let mut sheet = appearance();
    sheet.crumb = "Search";
    sheet.section = None;
    sheet.search.text = "pad".into();
    sheet.search.cursor = 3;
    let scene = run(&sheet, (640, 560), 1.0, None);
    let query = text(&scene, "pad").unwrap();
    assert_eq!(query.color, FOREGROUND);
    assert!(query.x > scene.hits.panel.x + 56.0);
    assert!(
        scene
            .text
            .iter()
            .all(|run| run.text != "Search" || run.baseline > 72.0)
    );
    assert!(quad_at(&scene, rect(query.x + 21.0, 41.0, 1.0, 14.0)).is_some());
    let wide = run(&sheet, (1600, 1000), 1.0, None);
    assert!(text(&wide, "Search").is_some_and(|run| run.x >= 400.0));
}
