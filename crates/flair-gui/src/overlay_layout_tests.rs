use super::*;
use crate::{
    CellMetrics,
    renderer::TextFace,
    ui::{ELLIPSIS, ENTER_KEY, MOVE_KEY},
};
use flair_core::config::Config;

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn input(theme: &Theme, px: (u32, u32), scale: f32, hover: Option<(f32, f32)>) -> OverlayInput<'_> {
    OverlayInput {
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
        theme,
        hover,
    }
}

fn run(sheet: &Sheet, px: (u32, u32), scale: f32, hover: Option<(f32, f32)>) -> OverlayScene {
    let theme = Config::default().theme();
    layout(sheet, &input(&theme, px, scale, hover), &mut measure)
}

fn row(name: &str, detail: Option<&str>, label: Option<&'static str>) -> Row {
    Row {
        name: name.into(),
        detail: detail.map(str::to_owned),
        label,
        ..Row::default()
    }
}

fn sessions(rows: Vec<Row>, selected: usize) -> Sheet {
    Sheet {
        strip: None,
        placement: Placement::Centred,
        title: None,
        field: Some(Field {
            text: String::new(),
            cursor: 0,
            placeholder: "Search sessions",
        }),
        count: (rows.len(), rows.len()),
        rows,
        selected,
        keys: vec![
            (ENTER_KEY, "switch".into()),
            (MOVE_KEY, "move".into()),
            ("esc", "close".into()),
        ],
    }
}

fn three() -> Sheet {
    sessions(
        vec![
            row("flair", Some("~/IdeaProjects/flair"), Some("current")),
            row("rust-catchers", Some("~/projects/rust-catchers"), None),
            row("DENIC-Configs", Some("~/projects/DENIC-Configs"), None),
        ],
        0,
    )
}

fn many(count: usize, selected: usize) -> Sheet {
    sessions(
        (0..count)
            .map(|index| row(&format!("s{index}"), None, None))
            .collect(),
        selected,
    )
}

fn windows() -> Sheet {
    Sheet {
        strip: None,
        title: Some("Windows"),
        field: None,
        count: (2, 2),
        rows: vec![
            Row {
                number: Some(1),
                name: "nvim".into(),
                label: Some("active"),
                ..Row::default()
            },
            Row {
                number: Some(12),
                name: "zsh".into(),
                label: Some("last"),
                ..Row::default()
            },
        ],
        ..three()
    }
}

fn text<'a>(scene: &'a OverlayScene, text: &str) -> Option<&'a TextRun> {
    scene.text.iter().find(|run| run.text == text)
}

fn quad_at(scene: &OverlayScene, rect: PxRect) -> Option<&Quad> {
    scene.quads.iter().find(|quad| quad.rect == rect)
}

#[test]
fn centred_panel_sits_at_the_panel_top_and_fits_its_rows() {
    let surfaces = Config::default().theme().surfaces();
    for (px, scale, panel) in [
        ((960, 600), 1.0, rect(120.0, 200.0, 720.0, 222.0)),
        ((1920, 1200), 1.0, rect(600.0, 200.0, 720.0, 222.0)),
        ((1920, 1200), 2.0, rect(240.0, 400.0, 1440.0, 444.0)),
    ] {
        let scene = run(&three(), px, scale, None);
        assert_eq!(
            scene.quads[0],
            Quad {
                rect: rect(0.0, 35.0 * scale, px.0 as f32, px.1 as f32 - 35.0 * scale),
                color: BACKDROP,
                radius: 0.0,
                border: None,
            }
        );
        assert_eq!(
            scene.quads[1],
            Quad {
                rect: panel,
                color: opaque(surfaces.surface1),
                radius: 10.0 * scale,
                border: Some(Border {
                    color: opaque(surfaces.outline),
                    width: scale,
                }),
            },
            "{px:?} {scale}"
        );
        assert_eq!(scene.hits.panel, panel);
    }
}

#[test]
fn short_windows_centre_the_panel_below_the_bar() {
    let scene = run(&three(), (600, 400), 1.0, None);
    let panel = scene.hits.panel;
    assert_eq!(panel.w, 552.0);
    assert_eq!(panel.x, 24.0);
    let room = 400.0 - 35.0 - 40.0;
    assert_eq!(panel.y, 55.0 + ((room - panel.h) / 2.0).floor());
}

#[test]
fn palette_field_is_a_borderless_band_with_the_total_and_a_full_width_hairline() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let scene = run(&three(), (960, 600), 1.0, None);
    let hairline = quad_at(&scene, rect(120.0, 244.0, 720.0, 1.0)).unwrap();
    assert_eq!(hairline.color, opaque(surfaces.outline));
    assert!(
        scene
            .quads
            .iter()
            .all(|quad| quad.color != opaque(theme.background))
    );
    let placeholder = text(&scene, "Search sessions").unwrap();
    assert_eq!(
        (placeholder.x, placeholder.baseline, placeholder.color),
        (158.0, 226.0, surfaces.faint)
    );
    let caret = quad_at(&scene, rect(154.0, 215.0, 1.0, 14.0)).unwrap();
    assert_eq!(caret.color, opaque(theme.accent));
    let count = text(&scene, "3").unwrap();
    assert_eq!(
        (count.x, count.color, count.face),
        (803.0, surfaces.muted, TextFace::Small)
    );
    assert_eq!(scene.hits.rows[0].1.y, 253.0);
}

#[test]
fn palette_field_draws_the_query_with_a_caret_and_counts_matches_of_all() {
    let theme = Config::default().theme();
    let mut sheet = three();
    sheet.field = Some(Field {
        text: "flair".into(),
        cursor: 2,
        placeholder: "Search sessions",
    });
    sheet.count = (1, 3);
    let scene = run(&sheet, (960, 600), 1.0, None);
    let query = text(&scene, "flair").unwrap();
    assert_eq!((query.x, query.color), (154.0, theme.foreground));
    assert!(quad_at(&scene, rect(168.0, 215.0, 1.0, 14.0)).is_some());
    assert!(text(&scene, "Search sessions").is_none());
    let count = text(&scene, "1 of 3").unwrap();
    assert_eq!((count.x, count.face), (785.0, TextFace::Small));
}

#[test]
fn long_queries_scroll_to_keep_the_caret_inside_the_field() {
    let mut sheet = three();
    sheet.field = Some(Field {
        text: "q".repeat(200),
        cursor: 200,
        placeholder: "",
    });
    let scene = run(&sheet, (960, 600), 1.0, None);
    let caret = scene
        .quads
        .iter()
        .find(|quad| quad.rect.w == 1.0 && quad.rect.h == 14.0)
        .unwrap();
    let count = text(&scene, "3 of 3").unwrap();
    assert!(caret.rect.x < count.x, "{caret:?}");
    let shown = scene
        .text
        .iter()
        .find(|run| run.text.starts_with('q'))
        .unwrap();
    assert!(shown.x + measure(&shown.text, false) <= count.x);
}

#[test]
fn sheets_without_a_field_show_a_bold_title_and_the_count() {
    let theme = Config::default().theme();
    let scene = run(&windows(), (960, 600), 1.0, None);
    let title = text(&scene, "Windows").unwrap();
    assert!(title.bold);
    assert_eq!(
        (title.x, title.baseline, title.color),
        (154.0, 226.0, theme.foreground)
    );
    let count = text(&scene, "2").unwrap();
    assert_eq!((count.x, count.color), (803.0, theme.surfaces().muted));
    assert!(quad_at(&scene, rect(120.0, 244.0, 720.0, 1.0)).is_some());
    assert_eq!(scene.hits.rows[0].1.y, 253.0);
}

#[test]
fn selected_row_is_accent_soft_with_a_left_bar_and_hovered_row_surface2() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let rows: Vec<PxRect> = (0..3)
        .map(|index| rect(144.0, 253.0 + 36.0 * index as f32, 672.0, 32.0))
        .collect();
    let scene = run(&three(), (960, 600), 1.0, Some((300.0, 300.0)));
    let selected = quad_at(&scene, rows[0]).unwrap();
    assert_eq!(
        (selected.color, selected.radius),
        (opaque(surfaces.accent_soft), 6.0)
    );
    let bar = quad_at(&scene, rect(144.0, 260.0, 2.0, 18.0)).unwrap();
    assert_eq!(bar.color, opaque(theme.accent));
    assert_eq!(
        quad_at(&scene, rows[1]).unwrap().color,
        opaque(surfaces.surface2)
    );
    assert!(quad_at(&scene, rows[2]).is_none());
    assert_eq!(
        scene
            .quads
            .iter()
            .filter(|quad| quad.color == opaque(theme.accent) && quad.rect.w == 2.0)
            .count(),
        1
    );
    assert_eq!(
        scene.hits.rows,
        rows.iter().copied().enumerate().collect::<Vec<_>>()
    );
    let double = run(&three(), (1920, 1200), 2.0, None);
    let first = double.hits.rows[0].1;
    let bar = rect(first.x, first.y + 14.0, 4.0, first.h - 28.0);
    assert!(quad_at(&double, bar).is_some());
}

#[test]
fn rows_draw_the_name_a_right_aligned_mono_path_and_the_label_left_of_it() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let scene = run(&three(), (960, 600), 1.0, None);
    let name = text(&scene, "flair").unwrap();
    assert_eq!(
        (name.x, name.baseline, name.color, name.face, name.bold),
        (154.0, 273.0, theme.foreground, TextFace::Ui, false)
    );
    let other = text(&scene, "rust-catchers").unwrap();
    assert_eq!((other.color, other.bold), (surfaces.bright, false));
    let path = text(&scene, "~/IdeaProjects/flair").unwrap();
    assert_eq!(
        (path.x, path.baseline, path.color, path.face, path.bold),
        (666.0, 273.0, surfaces.muted, TextFace::Icon, false)
    );
    let current = text(&scene, "current").unwrap();
    assert_eq!(
        (current.x, current.color, current.face),
        (632.0, surfaces.muted, TextFace::Small)
    );
    let other_path = text(&scene, "~/projects/rust-catchers").unwrap();
    assert_eq!(other_path.x + 24.0 * 7.0, 806.0);
    assert_eq!(current.baseline, name.baseline);
}

#[test]
fn matched_characters_are_bold_foreground_and_the_rest_bright() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut sheet = three();
    sheet.rows[1].name_matches = vec![0, 1, 5];
    sheet.rows[1].detail_matches = vec![2, 3];
    sheet.rows[0].name_matches = vec![0];
    let scene = run(&sheet, (960, 600), 1.0, None);
    let runs: Vec<_> = scene
        .text
        .iter()
        .filter(|run| run.face == TextFace::Ui && run.baseline == 309.0)
        .map(|run| (run.text.as_str(), run.x, run.color, run.bold))
        .collect();
    assert_eq!(
        runs,
        [
            ("ru", 154.0, theme.foreground, true),
            ("st-", 168.0, surfaces.bright, false),
            ("c", 189.0, theme.foreground, true),
            ("atchers", 196.0, surfaces.bright, false),
        ]
    );
    let detail: Vec<_> = scene
        .text
        .iter()
        .filter(|run| run.face == TextFace::Icon && run.baseline == 309.0)
        .map(|run| (run.text.as_str(), run.x, run.color))
        .collect();
    assert_eq!(
        detail,
        [
            ("~/", 638.0, surfaces.muted),
            ("pr", 652.0, surfaces.bright),
            ("ojects/rust-catchers", 666.0, surfaces.muted),
        ]
    );
    assert_eq!(text(&scene, "f").unwrap().color, theme.foreground);
    assert_eq!(text(&scene, "lair").unwrap().color, surfaces.bright);
}

#[test]
fn long_paths_lose_their_head_and_long_names_their_tail() {
    let long = format!("~/{}/tail", "deep/".repeat(80));
    let name = "n".repeat(60);
    let sheet = sessions(vec![row(&name, Some(&long), None)], 0);
    let scene = run(&sheet, (960, 600), 1.0, None);
    let path = scene
        .text
        .iter()
        .find(|run| run.text.ends_with("/tail"))
        .unwrap();
    assert!(path.text.starts_with(ELLIPSIS), "{}", path.text);
    assert_eq!(
        (path.face, path.color),
        (TextFace::Icon, Config::default().theme().surfaces().muted)
    );
    assert_eq!(path.x + path.text.chars().count() as f32 * 7.0, 806.0);
    let shown = scene
        .text
        .iter()
        .find(|run| run.text.starts_with('n'))
        .unwrap();
    assert!(shown.text.ends_with(ELLIPSIS));
    assert!(measure(&shown.text, false) <= 180.0);
    assert!(path.x >= shown.x + measure(&shown.text, false) + 10.0);
}

#[test]
fn numbers_sit_in_a_faint_column_and_depth_indents_the_row() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut sheet = windows();
    sheet.rows.push(Row {
        depth: 1,
        number: Some(3),
        name: "logs".into(),
        ..Row::default()
    });
    sheet.rows.push(Row {
        depth: 1,
        name: "bare".into(),
        ..Row::default()
    });
    let scene = run(&sheet, (960, 600), 1.0, None);
    let one = text(&scene, "1").unwrap();
    assert_eq!(
        (one.x, one.color, one.face),
        (154.0, surfaces.faint, TextFace::Small)
    );
    assert_eq!(text(&scene, "12").unwrap().x, 154.0);
    assert_eq!(text(&scene, "nvim").unwrap().x, 174.0);
    assert_eq!(one.baseline, text(&scene, "nvim").unwrap().baseline);
    assert_eq!(text(&scene, "3").unwrap().x, 174.0);
    assert_eq!(text(&scene, "logs").unwrap().x, 194.0);
    assert_eq!(text(&scene, "bare").unwrap().x, 174.0);
    let active = text(&scene, "active").unwrap();
    assert_eq!((active.x, active.face), (785.0, TextFace::Small));
}

#[test]
fn empty_state_shows_no_matches_and_keeps_the_footer() {
    let theme = Config::default().theme();
    let scene = run(&many(0, 0), (960, 600), 1.0, None);
    let empty = text(&scene, "No matches").unwrap();
    assert_eq!((empty.x, empty.color), (154.0, theme.surfaces().muted));
    assert!(scene.hits.rows.is_empty());
    assert!(text(&scene, "switch").is_some());
    assert_eq!(scene.hits.panel.h, 150.0);
}

#[test]
fn many_rows_scroll_to_the_selection_and_count_the_hidden_rows() {
    let scene = run(&many(20, 15), (960, 600), 1.0, None);
    let shown: Vec<usize> = scene.hits.rows.iter().map(|(index, _)| *index).collect();
    assert_eq!(shown, (9..16).collect::<Vec<_>>());
    assert_eq!(scene.hits.rows[0].1.y, 253.0);
    let more = text(&scene, "+13 more").unwrap();
    assert_eq!(more.face, TextFace::Small);
    assert_eq!(more.x, 788.0);
    assert_eq!(scene.hits.panel.y + scene.hits.panel.h, 566.0);
    let top = run(&many(20, 0), (960, 600), 1.0, None);
    assert_eq!(top.hits.rows[0].0, 0);
    assert!(text(&run(&three(), (960, 600), 1.0, None), "+0 more").is_none());
}

#[test]
fn footer_draws_a_rule_then_key_caps_and_verbs() {
    let surfaces = Config::default().theme().surfaces();
    let scene = run(&three(), (960, 600), 1.0, None);
    let rule = quad_at(&scene, rect(144.0, 369.0, 672.0, 1.0)).unwrap();
    assert_eq!(rule.color, opaque(surfaces.outline));
    let enter = quad_at(&scene, rect(144.0, 382.0, 20.0, 20.0)).unwrap();
    assert_eq!(enter.color, opaque(surfaces.surface3));
    assert_eq!(
        enter.border,
        Some(Border {
            color: opaque(surfaces.outline_strong),
            width: 1.0,
        })
    );
    assert_eq!(enter.radius, 4.0);
    assert_eq!(text(&scene, ENTER_KEY).unwrap().face, TextFace::Small);
    let switch = text(&scene, "switch").unwrap();
    assert_eq!((switch.x, switch.color), (170.0, surfaces.muted));
    let moves = quad_at(&scene, rect(170.0 + 42.0 + 16.0, 382.0, 20.0, 20.0));
    assert!(moves.is_some());
    assert!(text(&scene, "close").is_some());
    let narrow = run(&three(), (200, 600), 1.0, None);
    assert!(text(&narrow, "close").is_none());
}

#[test]
fn hit_test_finds_rows_the_panel_and_outside() {
    let scene = run(&three(), (960, 600), 1.0, None);
    assert_eq!(scene.hits.hit_test((300.0, 300.0)), OverlayHit::Row(1));
    assert_eq!(scene.hits.hit_test((300.0, 210.0)), OverlayHit::Panel);
    assert_eq!(scene.hits.hit_test((300.0, 287.0)), OverlayHit::Panel);
    assert_eq!(scene.hits.hit_test((5.0, 100.0)), OverlayHit::Outside);
    assert_eq!(scene.hits.hit_test((300.0, 20.0)), OverlayHit::Outside);
    assert_eq!(scene.hits.page(), 3);
    assert_eq!(OverlayHits::default().page(), 1);
    assert!(scene.hits.controls.is_empty());
}

#[test]
fn hit_test_prefers_controls_over_rows() {
    let mut hits = run(&three(), (960, 600), 1.0, None).hits;
    let row = hits.rows[1].1;
    hits.controls.push((
        SettingsHit::Toggle(1),
        rect(row.x + row.w - 40.0, row.y, 40.0, row.h),
    ));
    let y = f64::from(row.y + 4.0);
    assert_eq!(
        hits.hit_test((f64::from(row.x + row.w - 10.0), y)),
        OverlayHit::Settings(SettingsHit::Toggle(1))
    );
    assert_eq!(
        hits.hit_test((f64::from(row.x + 10.0), y)),
        OverlayHit::Row(1)
    );
    assert_eq!(hits.page(), 3);
}

#[test]
fn theme_panel_docks_right_below_the_bar_without_backdrop() {
    let surfaces = Config::default().theme().surfaces();
    let sheet = Sheet {
        placement: Placement::Docked,
        ..many(3, 0)
    };
    for (scale, panel) in [
        (1.0, rect(472.0, 43.0, 320.0, 549.0)),
        (2.0, rect(944.0, 86.0, 640.0, 1098.0)),
    ] {
        let px = (800 * scale as u32, 600 * scale as u32);
        let scene = run(&sheet, px, scale, None);
        assert_eq!(
            scene.quads[0],
            Quad {
                rect: panel,
                color: opaque(surfaces.surface1),
                radius: 10.0 * scale,
                border: Some(Border {
                    color: opaque(surfaces.outline),
                    width: scale,
                }),
            }
        );
        assert!(scene.quads.iter().all(|quad| quad.color != BACKDROP));
    }
    let scene = run(&sheet, (800, 600), 1.0, None);
    let rule = 43.0 + 549.0 - 20.0 - 20.0 - 12.0 - 1.0;
    assert!(quad_at(&scene, rect(496.0, rule, 272.0, 1.0)).is_some());
    assert!(quad_at(&scene, rect(472.0, 87.0, 320.0, 1.0)).is_some());
    let tall = run(&many(40, 39), (800, 600), 1.0, None);
    let docked = run(
        &Sheet {
            placement: Placement::Docked,
            ..many(40, 39)
        },
        (800, 600),
        1.0,
        None,
    );
    assert!(docked.hits.rows.len() > tall.hits.rows.len());
    assert_eq!(docked.hits.rows.last().unwrap().0, 39);
}

#[test]
fn names_without_detail_use_the_whole_row() {
    let name = "w".repeat(60);
    let sheet = sessions(
        vec![Row {
            number: Some(1),
            name: name.clone(),
            ..Row::default()
        }],
        0,
    );
    let scene = run(&sheet, (960, 600), 1.0, None);
    assert!(text(&scene, &name).is_some());
}

fn assert_inside(scene: &OverlayScene, window: (u32, u32), scale: f32) {
    let panel = scene.hits.panel;
    let margin = px(PANEL_PAD_X, scale);
    assert!(panel.x >= margin, "{panel:?}");
    assert!(panel.x + panel.w <= window.0 as f32 - margin, "{panel:?}");
    assert!(panel.y >= chrome_bottom(scale), "{panel:?}");
    assert!(panel.y + panel.h <= window.1 as f32, "{panel:?}");
    for quad in &scene.quads[1..] {
        let rect = quad.rect;
        assert!(
            rect.x >= panel.x && rect.x + rect.w <= panel.x + panel.w,
            "{rect:?}"
        );
        assert!(
            rect.y >= panel.y && rect.y + rect.h <= panel.y + panel.h,
            "{rect:?}"
        );
    }
}

#[test]
fn small_windows_keep_the_panel_inside_with_its_margins() {
    for (px, scale, rows) in [
        ((600, 400), 1.0, 5),
        ((400, 300), 1.0, 3),
        ((600, 400), 1.5, 2),
    ] {
        let scene = run(&many(12, 0), px, scale, None);
        assert_inside(&scene, px, scale);
        assert_eq!(scene.hits.rows.len(), rows, "{px:?} {scale}");
        assert!(text(&scene, &format!("+{} more", 12 - rows)).is_some());
        let count = text(&scene, "12").unwrap();
        assert!(count.x + measure("12", false) <= scene.hits.panel.x + scene.hits.panel.w);
        assert!(text(&scene, "Search sessions").is_some());
    }
}

#[test]
fn rows_then_the_footer_drop_before_the_palette_field() {
    let scene = run(&many(12, 0), (400, 200), 1.0, None);
    assert_inside(&scene, (400, 200), 1.0);
    assert_eq!(scene.hits.rows.len(), 1);
    assert!(text(&scene, "esc").is_none());
    assert!(text(&scene, "+11 more").is_none());
    assert!(text(&scene, "Search sessions").is_some());
}

#[test]
fn the_palette_field_drops_before_the_panel_leaves_a_tiny_window() {
    for height in [130, 100] {
        let scene = run(&many(12, 0), (400, height), 1.0, None);
        assert_inside(&scene, (400, height), 1.0);
        assert!(scene.hits.rows.is_empty());
        assert!(text(&scene, "Search sessions").is_none());
        assert!(text(&scene, "12").is_none());
    }
}

#[test]
fn non_ascii_row_names_ellipsize_on_whole_chars() {
    let long = "\u{dc}berlanger Fenstername \u{e4}\u{f6}\u{fc} \u{df}";
    let cjk = &"\u{958b}\u{767a}\u{30b5}\u{30fc}\u{30d0}\u{30fc}\u{306e}\u{30ed}\u{30b0}".repeat(8);
    let sheet = sessions(
        vec![row(long, None, Some("current")), row(cjk, None, None)],
        0,
    );
    let scene = run(&sheet, (300, 300), 1.0, None);
    let first = scene
        .text
        .iter()
        .find(|run| run.face == TextFace::Ui && run.text.starts_with("\u{dc}ber"))
        .unwrap();
    let kept = first.text.strip_suffix(ELLIPSIS).unwrap();
    assert!(long.starts_with(kept));
    let scene = run(
        &sessions(vec![row(cjk, None, None)], 0),
        (300, 300),
        1.0,
        None,
    );
    let second = scene
        .text
        .iter()
        .find(|run| run.face == TextFace::Ui && run.text.starts_with('\u{958b}'))
        .unwrap();
    assert!(cjk.starts_with(second.text.trim_end_matches(ELLIPSIS)));
    assert!(second.text.ends_with(ELLIPSIS));
}

#[test]
fn icons_sit_in_a_muted_column_after_the_number_that_aligns_every_name() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut sheet = windows();
    sheet.rows[0].icon = Some('\u{e62b}');
    let scene = run(&sheet, (960, 600), 1.0, None);
    let icons: Vec<_> = scene
        .text
        .iter()
        .filter(|run| run.text == "\u{e62b}")
        .map(|run| (run.x, run.baseline, run.color, run.face, run.bold))
        .collect();
    assert_eq!(
        icons,
        [(174.0, 271.0, surfaces.muted, TextFace::PromptIcon, false)]
    );
    assert_eq!(text(&scene, "nvim").unwrap().x, 191.0);
    assert_eq!(text(&scene, "zsh").unwrap().x, 191.0);
    assert_eq!(text(&scene, "nvim").unwrap().baseline, 273.0);
}

#[test]
fn sheets_without_icons_reserve_no_icon_column() {
    let scene = run(&windows(), (960, 600), 1.0, None);
    assert!(
        scene
            .text
            .iter()
            .all(|run| !run.text.starts_with('\u{e62b}'))
    );
    assert_eq!(text(&scene, "nvim").unwrap().x, 174.0);
    assert_eq!(text(&scene, "zsh").unwrap().x, 174.0);
    let scene = run(&three(), (960, 600), 1.0, None);
    assert_eq!(text(&scene, "flair").unwrap().x, 154.0);
}

#[test]
fn the_icon_column_only_shifts_rows_at_the_depth_that_has_icons() {
    let mut sheet = windows();
    sheet.rows.push(Row {
        depth: 1,
        number: Some(3),
        icon: Some('\u{f120}'),
        name: "logs".into(),
        ..Row::default()
    });
    let scene = run(&sheet, (960, 600), 1.0, None);
    assert_eq!(text(&scene, "nvim").unwrap().x, 174.0);
    assert_eq!(text(&scene, "\u{f120}").unwrap().x, 194.0);
    assert_eq!(text(&scene, "logs").unwrap().x, 211.0);
}
