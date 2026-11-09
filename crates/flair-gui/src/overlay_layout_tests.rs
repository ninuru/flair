use super::*;
use crate::{renderer::TextFace, ui::ELLIPSIS};
use flair_core::config::Config;

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn input(theme: &Theme, px: (u32, u32), scale: f32, hover: Option<(f32, f32)>) -> OverlayInput<'_> {
    OverlayInput {
        px,
        scale,
        ui: UiMetrics {
            height: 14.0 * scale,
            ascent: 11.0 * scale,
            small: 0.5,
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
        mark: Mark::Monogram(name.into()),
        name: name.into(),
        detail: detail.map(str::to_owned),
        label,
    }
}

fn sessions(rows: Vec<Row>, selected: usize) -> Sheet {
    Sheet {
        placement: Placement::Centred,
        title: "Sessions",
        field: Some(Field {
            text: String::new(),
            cursor: 0,
            placeholder: "Search sessions",
        }),
        rows,
        selected,
        keys: vec![
            ("↵", "switch".into()),
            ("↑↓", "move".into()),
            ("esc", "cancel".into()),
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
        ((960, 600), 1.0, rect(120.0, 200.0, 720.0, 263.0)),
        ((1920, 1200), 1.0, rect(600.0, 200.0, 720.0, 263.0)),
        ((1920, 1200), 2.0, rect(240.0, 400.0, 1440.0, 526.0)),
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
fn title_shows_the_count_right_aligned_in_muted() {
    let theme = Config::default().theme();
    let scene = run(&three(), (960, 600), 1.0, None);
    let title = text(&scene, "Sessions").unwrap();
    assert!(title.bold);
    assert_eq!((title.x, title.color), (144.0, theme.foreground));
    let count = text(&scene, "3").unwrap();
    assert_eq!(
        (count.x, count.color),
        (144.0 + 672.0 - 7.0, theme.surfaces().muted)
    );
}

#[test]
fn search_field_draws_the_placeholder_or_the_text_with_a_caret_at_the_cursor() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let field = rect(144.0, 250.0, 672.0, 32.0);
    let scene = run(&three(), (960, 600), 1.0, None);
    assert_eq!(
        quad_at(&scene, field).unwrap().color,
        opaque(theme.background)
    );
    let placeholder = text(&scene, "Search sessions").unwrap();
    assert_eq!((placeholder.x, placeholder.color), (158.0, surfaces.faint));
    let caret = rect(154.0, 259.0, 1.0, 14.0);
    assert_eq!(quad_at(&scene, caret).unwrap().color, opaque(theme.accent));
    let mut sheet = three();
    sheet.field = Some(Field {
        text: "flair".into(),
        cursor: 2,
        placeholder: "Search sessions",
    });
    let scene = run(&sheet, (960, 600), 1.0, None);
    assert_eq!(text(&scene, "flair").map(|run| run.x), Some(154.0));
    assert!(quad_at(&scene, rect(168.0, 259.0, 1.0, 14.0)).is_some());
    assert!(text(&scene, "Search sessions").is_none());
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
    assert!(caret.rect.x <= 144.0 + 672.0 - 10.0, "{caret:?}");
    let shown = scene
        .text
        .iter()
        .find(|run| run.text.starts_with('q'))
        .unwrap();
    assert!(measure(&shown.text, false) <= 672.0 - 21.0);
}

#[test]
fn selected_row_is_surface3_and_hovered_row_surface2() {
    let surfaces = Config::default().theme().surfaces();
    let rows: Vec<PxRect> = (0..3)
        .map(|index| rect(144.0, 294.0 + 36.0 * index as f32, 672.0, 32.0))
        .collect();
    let scene = run(&three(), (960, 600), 1.0, Some((300.0, 340.0)));
    assert_eq!(
        quad_at(&scene, rows[0]).unwrap().color,
        opaque(surfaces.surface3)
    );
    assert_eq!(quad_at(&scene, rows[0]).unwrap().radius, 6.0);
    assert_eq!(
        quad_at(&scene, rows[1]).unwrap().color,
        opaque(surfaces.surface2)
    );
    assert!(quad_at(&scene, rows[2]).is_none());
    assert_eq!(
        scene.hits.rows,
        rows.iter().copied().enumerate().collect::<Vec<_>>()
    );
}

#[test]
fn rows_draw_a_monogram_the_name_a_muted_regular_path_and_the_trailing_label() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let scene = run(&three(), (960, 600), 1.0, None);
    let mark = quad_at(&scene, rect(154.0, 301.0, 18.0, 18.0)).unwrap();
    assert_eq!(mark.color, opaque(monogram("flair").hue));
    let letters = text(&scene, "FL").unwrap();
    assert_eq!((letters.face, letters.bold), (TextFace::Small, true));
    let name = text(&scene, "flair").unwrap();
    assert_eq!(
        (name.x, name.color, name.face, name.bold),
        (180.0, theme.foreground, TextFace::Ui, false)
    );
    let path = text(&scene, "~/IdeaProjects/flair").unwrap();
    assert_eq!(
        (path.x, path.color, path.face, path.bold),
        (225.0, surfaces.muted, TextFace::Ui, false)
    );
    assert_eq!(path.baseline, name.baseline);
    let current = text(&scene, "current").unwrap();
    assert_eq!(
        (current.x, current.color, current.face),
        (782.0, surfaces.muted, TextFace::Small)
    );
    assert_eq!(
        scene
            .text
            .iter()
            .filter(|run| run.text == "current")
            .count(),
        1
    );
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
        (TextFace::Ui, Config::default().theme().surfaces().muted)
    );
    assert!(path.x + measure(&path.text, false) <= 144.0 + 672.0 - 10.0);
    let shown = scene
        .text
        .iter()
        .find(|run| run.text.starts_with('n'))
        .unwrap();
    assert!(shown.text.ends_with(ELLIPSIS));
    assert!(measure(&shown.text, false) <= 180.0);
}

#[test]
fn empty_state_shows_no_matches_and_keeps_the_footer() {
    let theme = Config::default().theme();
    let scene = run(&many(0, 0), (960, 600), 1.0, None);
    let empty = text(&scene, "No matches").unwrap();
    assert_eq!((empty.x, empty.color), (154.0, theme.surfaces().muted));
    assert!(scene.hits.rows.is_empty());
    assert!(text(&scene, "switch").is_some());
    assert_eq!(scene.hits.panel.h, 159.0 + 32.0);
}

#[test]
fn many_rows_scroll_to_the_selection_and_count_the_hidden_rows() {
    let scene = run(&many(20, 15), (960, 600), 1.0, None);
    let shown: Vec<usize> = scene.hits.rows.iter().map(|(index, _)| *index).collect();
    assert_eq!(shown, (10..16).collect::<Vec<_>>());
    assert_eq!(scene.hits.rows[0].1.y, 294.0);
    let more = text(&scene, "+14 more").unwrap();
    assert_eq!(more.face, TextFace::Small);
    assert_eq!(more.x, (144.0 + 672.0 - 8.0 * 7.0 * 0.5_f32).round());
    assert_eq!(
        scene.hits.panel.y + scene.hits.panel.h,
        200.0 + 159.0 + 212.0
    );
    let top = run(&many(20, 0), (960, 600), 1.0, None);
    assert_eq!(top.hits.rows[0].0, 0);
    assert!(text(&run(&three(), (960, 600), 1.0, None), "+0 more").is_none());
}

#[test]
fn footer_draws_a_rule_then_key_caps_and_verbs() {
    let surfaces = Config::default().theme().surfaces();
    let scene = run(&three(), (960, 600), 1.0, None);
    let rule = quad_at(&scene, rect(144.0, 410.0, 672.0, 1.0)).unwrap();
    assert_eq!(rule.color, opaque(surfaces.outline));
    let enter = quad_at(&scene, rect(144.0, 423.0, 20.0, 20.0)).unwrap();
    assert_eq!(enter.color, opaque(surfaces.surface3));
    assert_eq!(
        enter.border,
        Some(Border {
            color: opaque(surfaces.outline_strong),
            width: 1.0,
        })
    );
    assert_eq!(enter.radius, 4.0);
    assert_eq!(text(&scene, "↵").unwrap().face, TextFace::Small);
    let switch = text(&scene, "switch").unwrap();
    assert_eq!((switch.x, switch.color), (170.0, surfaces.muted));
    let moves = quad_at(&scene, rect(170.0 + 42.0 + 16.0, 423.0, 20.0, 20.0));
    assert!(moves.is_some());
    assert!(text(&scene, "cancel").is_some());
    let narrow = run(&three(), (200, 600), 1.0, None);
    assert!(text(&narrow, "cancel").is_none());
}

#[test]
fn hit_test_finds_rows_the_panel_and_outside() {
    let scene = run(&three(), (960, 600), 1.0, None);
    assert_eq!(scene.hits.hit_test((300.0, 340.0)), OverlayHit::Row(1));
    assert_eq!(scene.hits.hit_test((300.0, 210.0)), OverlayHit::Panel);
    assert_eq!(scene.hits.hit_test((300.0, 328.0)), OverlayHit::Panel);
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
        title: "Themes",
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
fn window_chooser_rows_lead_with_an_index_cap() {
    let surfaces = Config::default().theme().surfaces();
    let sheet = Sheet {
        title: "Windows",
        field: None,
        rows: vec![
            Row {
                mark: Mark::Index(1),
                name: "nvim".into(),
                detail: None,
                label: Some("active"),
            },
            Row {
                mark: Mark::Index(12),
                name: "zsh".into(),
                detail: None,
                label: Some("last"),
            },
        ],
        ..three()
    };
    let scene = run(&sheet, (960, 600), 1.0, None);
    let first = scene.hits.rows[0].1;
    assert_eq!(first.y, 250.0);
    let cap = quad_at(&scene, rect(154.0, 256.0, 20.0, 20.0)).unwrap();
    assert_eq!(cap.color, opaque(surfaces.surface3));
    assert_eq!(cap.border.unwrap().color, opaque(surfaces.outline_strong));
    assert!(quad_at(&scene, rect(154.0, 292.0, 20.0, 20.0)).is_some());
    let one = text(&scene, "1").unwrap();
    assert_eq!(one.face, TextFace::Small);
    assert_eq!(text(&scene, "nvim").unwrap().x, 182.0);
    assert!(text(&scene, "active").is_some());
    assert!(text(&scene, "last").is_some());
    let background = opaque(Config::default().theme().background);
    assert!(scene.quads.iter().all(|quad| quad.color != background));
}

#[test]
fn names_without_detail_use_the_whole_row() {
    let name = "w".repeat(60);
    let sheet = sessions(
        vec![Row {
            mark: Mark::Index(1),
            name: name.clone(),
            detail: None,
            label: None,
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
        ((600, 400), 1.0, 4),
        ((400, 300), 1.0, 1),
        ((600, 400), 1.5, 1),
    ] {
        let scene = run(&many(12, 0), px, scale, None);
        assert_inside(&scene, px, scale);
        assert_eq!(scene.hits.rows.len(), rows, "{px:?} {scale}");
        assert!(text(&scene, &format!("+{} more", 12 - rows)).is_some());
        let count = text(&scene, "12").unwrap();
        assert!(count.x + measure("12", false) <= scene.hits.panel.x + scene.hits.panel.w);
        assert!(text(&scene, "Sessions").is_some());
    }
}

#[test]
fn rows_then_the_footer_drop_before_the_search_field() {
    let theme = Config::default().theme();
    let field_quad = |scene: &OverlayScene| {
        scene
            .quads
            .iter()
            .any(|quad| quad.color == opaque(theme.background) && quad.rect.h == 32.0)
    };
    let scene = run(&many(12, 0), (400, 230), 1.0, None);
    assert_inside(&scene, (400, 230), 1.0);
    assert_eq!(scene.hits.rows.len(), 1);
    assert!(text(&scene, "esc").is_none());
    assert!(text(&scene, "+11 more").is_none());
    assert!(field_quad(&scene));

    let scene = run(&many(12, 0), (400, 180), 1.0, None);
    assert_inside(&scene, (400, 180), 1.0);
    assert_eq!(scene.hits.rows.len(), 1);
    assert!(!field_quad(&scene));
    assert!(text(&scene, "Sessions").is_some());
    assert!(text(&scene, "12").is_some());
}

#[test]
fn the_search_field_drops_before_the_panel_leaves_a_tiny_window() {
    let theme = Config::default().theme();
    let field_quad = |scene: &OverlayScene| {
        scene
            .quads
            .iter()
            .any(|quad| quad.color == opaque(theme.background) && quad.rect.h == 32.0)
    };
    let scene = run(&many(12, 0), (400, 160), 1.0, None);
    assert_inside(&scene, (400, 160), 1.0);
    assert!(scene.hits.rows.is_empty());
    assert!(!field_quad(&scene));
    assert!(text(&scene, "Sessions").is_some());

    let scene = run(&many(12, 0), (400, 120), 1.0, None);
    assert_inside(&scene, (400, 120), 1.0);
    assert!(!field_quad(&scene));
    assert!(text(&scene, "Sessions").is_some());
}

#[test]
fn non_ascii_row_names_ellipsize_on_whole_chars() {
    let long = "Überlanger Fenstername äöü ß";
    let cjk = "開発サーバーのログ監視ウィンドウとその他の長い名前";
    let sheet = sessions(
        vec![row(long, None, Some("current")), row(cjk, None, None)],
        0,
    );
    let scene = run(&sheet, (300, 300), 1.0, None);
    let first = scene
        .text
        .iter()
        .find(|run| run.face == TextFace::Ui && run.text.starts_with("Über"))
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
        .find(|run| run.face == TextFace::Ui && run.text.starts_with('開'))
        .unwrap();
    assert!(cjk.starts_with(second.text.trim_end_matches(ELLIPSIS)));
    assert!(second.text.ends_with(ELLIPSIS));
}
