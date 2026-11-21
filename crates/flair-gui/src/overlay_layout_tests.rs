use super::*;
use crate::{
    CellMetrics,
    renderer::TextFace,
    t3_style::{
        DIM, EDIT_ICON, FOREGROUND, MONOGRAM, MUTED, PALETTE, PALETTE_FOOTER, PALETTE_HIGHLIGHT,
        PALETTE_KEYCAP, PANEL_BORDER, QUIET, SEARCH_ICON,
    },
    ui::{ELLIPSIS, ENTER_KEY, ESCAPE_KEY, MOVE_KEY},
};
use flair_core::{config::Config, theme::Rgb};

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
        icons: true,
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
            (MOVE_KEY, "Navigate".into()),
            (ENTER_KEY, "Switch".into()),
            (ESCAPE_KEY, "Close".into()),
        ],
        empty: Some("No matching sessions."),
        glyph: SEARCH_ICON,
        grow: false,
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
        title: Some("Windows"),
        field: None,
        count: (2, 2),
        rows: vec![
            Row {
                number: Some(1),
                icon: Some('\u{f120}'),
                name: "nvim".into(),
                label: Some("active"),
                current: true,
                ..Row::default()
            },
            Row {
                number: Some(2),
                icon: Some('\u{f120}'),
                name: "zsh".into(),
                label: Some("last"),
                ..Row::default()
            },
        ],
        selected: 1,
        ..three()
    }
}

fn text<'a>(scene: &'a OverlayScene, text: &str) -> Option<&'a TextRun> {
    scene.text.iter().find(|run| run.text == text)
}

fn quad_at(scene: &OverlayScene, rect: PxRect) -> Option<&Quad> {
    scene.quads.iter().find(|quad| quad.rect == rect)
}

fn colour(color: Rgb) -> flair_core::theme::Rgba {
    opaque(color)
}

#[test]
fn the_palette_dims_the_window_and_centres_a_576_wide_panel() {
    let scene = run(&three(), (1600, 1000), 1.0, None);
    assert_eq!(
        scene.quads[0],
        Quad {
            rect: rect(0.0, 0.0, 1600.0, 1000.0),
            color: DIM,
            radius: 0.0,
            border: None,
        }
    );
    let panel = rect(512.0, 100.0, 576.0, 46.0 + 16.0 + 96.0 + 40.0);
    assert_eq!(scene.hits.panel, panel);
    let quad = quad_at(&scene, panel).unwrap();
    assert_eq!((quad.color, quad.radius), (colour(PALETTE), 16.0));
    assert_eq!(quad.border.unwrap().color, colour(PANEL_BORDER));
    assert!(
        scene
            .quads
            .iter()
            .any(|quad| quad.rect.y > panel.y && quad.rect.w > panel.w)
    );
}

#[test]
fn the_palette_sits_below_the_bar_on_short_windows() {
    let scene = run(&three(), (800, 300), 1.0, None);
    assert_eq!(scene.hits.panel.y, 42.0);
    assert_eq!(scene.hits.panel.w, 576.0);
    let narrow = run(&three(), (400, 600), 1.0, None);
    assert_eq!(narrow.hits.panel.x, 16.0);
    assert_eq!(narrow.hits.panel.w, 368.0);
}

#[test]
fn the_input_row_has_a_search_glyph_a_placeholder_a_caret_and_no_divider() {
    let scene = run(&three(), (1600, 1000), 1.0, None);
    let glyph = text(&scene, &SEARCH_ICON.to_string()).unwrap();
    assert_eq!(glyph.face, TextFace::PromptIcon);
    let placeholder = text(&scene, "Search sessions").unwrap();
    assert_eq!((placeholder.x, placeholder.color), (552.0, MUTED));
    let count = text(&scene, "3").unwrap();
    assert_eq!((count.color, count.face), (QUIET, TextFace::Small));
    assert!(quad_at(&scene, rect(552.0, 116.0, 1.0, 14.0)).is_some());
    assert!(
        scene
            .quads
            .iter()
            .all(|quad| !(quad.rect.h == 1.0 && quad.rect.y > 110.0 && quad.rect.y < 150.0))
    );
}

#[test]
fn a_query_shows_with_the_caret_and_counts_matches_of_all() {
    let mut sheet = three();
    sheet.field = Some(Field {
        text: "fl".into(),
        cursor: 1,
        placeholder: "Search sessions",
    });
    sheet.count = (1, 3);
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let query = text(&scene, "fl").unwrap();
    assert_eq!((query.x, query.color), (552.0, FOREGROUND));
    assert!(text(&scene, "1 of 3").is_some());
    assert!(quad_at(&scene, rect(559.0, 116.0, 1.0, 14.0)).is_some());
}

#[test]
fn rows_are_32_tall_with_a_highlight_for_the_selection_and_hover() {
    let scene = run(&three(), (1600, 1000), 1.0, Some((800.0, 200.0)));
    let rows: Vec<PxRect> = scene.hits.rows.iter().map(|(_, rect)| *rect).collect();
    assert_eq!(
        rows,
        [
            rect(520.0, 154.0, 560.0, 32.0),
            rect(520.0, 186.0, 560.0, 32.0),
            rect(520.0, 218.0, 560.0, 32.0),
        ]
    );
    for index in [0, 1] {
        let quad = quad_at(&scene, rows[index]).unwrap();
        assert_eq!((quad.color, quad.radius), (colour(PALETTE_HIGHLIGHT), 4.0));
    }
    assert!(quad_at(&scene, rows[2]).is_none());
    let name = text(&scene, "flair").unwrap();
    assert_eq!((name.x, name.color), (528.0, FOREGROUND));
    let label = text(&scene, "current").unwrap();
    assert_eq!((label.color, label.face), (QUIET, TextFace::Small));
}

#[test]
fn the_current_item_has_no_fill_but_an_accent_bar_at_its_left_edge() {
    let theme = Config::default().theme();
    let scene = run(&windows(), (1600, 1000), 1.0, None);
    let first = scene.hits.rows[0].1;
    assert!(quad_at(&scene, first).is_none());
    let bar = quad_at(&scene, rect(first.x, first.y + 7.0, 2.0, 18.0)).unwrap();
    assert_eq!(bar.color, colour(theme.accent));
    let second = scene.hits.rows[1].1;
    assert_eq!(
        quad_at(&scene, second).unwrap().color,
        colour(PALETTE_HIGHLIGHT)
    );
    assert!(quad_at(&scene, rect(second.x, second.y + 7.0, 2.0, 18.0)).is_none());
}

#[test]
fn a_selected_current_item_has_the_highlight_and_the_accent_bar() {
    let theme = Config::default().theme();
    let scene = run(
        &Sheet {
            selected: 0,
            ..windows()
        },
        (1600, 1000),
        1.0,
        None,
    );
    let first = scene.hits.rows[0].1;
    assert_eq!(
        quad_at(&scene, first).unwrap().color,
        colour(PALETTE_HIGHLIGHT)
    );
    let bar = quad_at(&scene, rect(first.x, first.y + 7.0, 2.0, 18.0)).unwrap();
    assert_eq!(bar.color, colour(theme.accent));
}

#[test]
fn titled_sheets_show_a_group_label_and_numbers_as_right_shortcuts() {
    let scene = run(&windows(), (1600, 1000), 1.0, None);
    let label = text(&scene, "Windows").unwrap();
    assert_eq!(
        (label.x, label.color, label.face),
        (528.0, MUTED, TextFace::Small)
    );
    assert!(text(&scene, &SEARCH_ICON.to_string()).is_none());
    assert_eq!(scene.hits.rows[0].1, rect(520.0, 136.0, 560.0, 32.0));
    let number = text(&scene, "1").unwrap();
    assert_eq!((number.color, number.face), (QUIET, TextFace::Small));
    assert!(number.x > text(&scene, "active").unwrap().x);
    assert_eq!(number.x, (1072.0_f32 - 3.5).round());
    let icon = text(&scene, "\u{f120}").unwrap();
    assert_eq!(icon.color, MUTED);
    assert_eq!(text(&scene, "nvim").unwrap().x, 528.0 + 16.0 + 8.0);
}

#[test]
fn rows_without_a_number_reserve_the_number_column_when_others_have_one() {
    let numbered = Row {
        number: Some(1),
        ..row("main", Some("xx"), None)
    };
    let sheet = sessions(vec![numbered, row("1", Some("yy"), None)], 0);
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let numbered = text(&scene, "xx").unwrap().x;
    assert_eq!(text(&scene, "yy").unwrap().x, numbered);
    assert_eq!(numbered, (1072.0_f32 - 3.5 - 8.0 - 7.0).round());
}

#[test]
fn session_rows_lead_with_a_monogram() {
    let mut sheet = three();
    for row in &mut sheet.rows {
        row.monogram = true;
    }
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let tile = quad_at(&scene, rect(528.0, 162.0, 16.0, 16.0)).unwrap();
    assert_eq!((tile.color, tile.radius), (colour(MONOGRAM), 4.0));
    assert_eq!(text(&scene, "F").unwrap().color, FOREGROUND);
    assert_eq!(text(&scene, "flair").unwrap().x, 552.0);
}

#[test]
fn rows_draw_a_right_aligned_small_path_centred_with_the_label() {
    let scene = run(&three(), (1600, 1000), 1.0, None);
    let path: Vec<&TextRun> = scene
        .text
        .iter()
        .filter(|run| run.text.starts_with("~/IdeaProjects/flair"))
        .collect();
    assert_eq!(path.len(), 1);
    assert_eq!((path[0].face, path[0].color), (TextFace::Small, QUIET));
    let start = path[0].x;
    assert_eq!(start + 20.0 * 7.0 * 0.5, 1072.0);
    let label = text(&scene, "current").unwrap();
    assert_eq!(label.x, (start - 8.0 - 3.5 * 7.0).round());
    assert_eq!(label.baseline, path[0].baseline);
    assert!(scene.text.iter().all(|run| run.face != TextFace::Icon));
}

#[test]
fn matched_characters_are_bold() {
    let mut sheet = three();
    sheet.rows[1].name_matches = vec![0, 1];
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let bold = scene.text.iter().find(|run| run.text == "ru").unwrap();
    assert!(bold.bold);
    assert_eq!(bold.color, FOREGROUND);
    assert!(!text(&scene, "st-catchers").unwrap().bold);
}

#[test]
fn long_names_and_paths_ellipsize() {
    let sheet = sessions(
        vec![row(
            &"n".repeat(80),
            Some(&format!("~/{}", "p".repeat(200))),
            None,
        )],
        0,
    );
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let name = scene
        .text
        .iter()
        .find(|run| run.text.starts_with("nnn"))
        .unwrap();
    assert!(name.text.ends_with(ELLIPSIS));
    assert!(measure(&name.text, false) <= 220.0);
    assert!(scene.text.iter().any(|run| run.face == TextFace::Small
        && run.text.starts_with(ELLIPSIS)
        && run.text.ends_with('p')));
}

#[test]
fn depth_indents_the_row() {
    let mut sheet = windows();
    sheet.rows[1].depth = 1;
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    assert_eq!(text(&scene, "zsh").unwrap().x, 528.0 + 16.0 + 16.0 + 8.0);
}

#[test]
fn an_empty_list_shows_the_sheet_message_centred_and_keeps_the_footer() {
    let scene = run(&many(0, 0), (1600, 1000), 1.0, None);
    let message = text(&scene, "No matching sessions.").unwrap();
    assert_eq!(message.color, MUTED);
    assert_eq!(message.x, (520.0 + (560.0 - 21.0 * 7.0) / 2.0_f32).round());
    assert_eq!(scene.hits.panel.h, 46.0 + 16.0 + 100.0 + 40.0);
    assert!(text(&scene, "Navigate").is_some());
}

#[test]
fn a_field_without_a_list_is_just_the_input_and_the_footer() {
    let sheet = Sheet {
        title: None,
        rows: Vec::new(),
        count: (0, 0),
        empty: None,
        glyph: EDIT_ICON,
        keys: vec![(ENTER_KEY, "Rename".into()), (ESCAPE_KEY, "Cancel".into())],
        field: Some(Field {
            text: "zsh".into(),
            cursor: 3,
            placeholder: "Rename window",
        }),
        ..three()
    };
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    assert_eq!(scene.hits.panel.h, 46.0 + 40.0);
    assert!(text(&scene, &EDIT_ICON.to_string()).is_some());
    assert!(text(&scene, "0").is_none());
    assert!(scene.hits.rows.is_empty());
}

#[test]
fn many_rows_cap_the_panel_and_scroll_to_the_selection() {
    let scene = run(&many(50, 40), (1600, 1000), 1.0, None);
    assert_eq!(scene.hits.rows.len(), 9);
    assert_eq!(scene.hits.rows.last().unwrap().0, 40);
    assert!(scene.hits.panel.h <= 420.0);
}

#[test]
fn a_growing_sheet_shows_every_row_past_the_palette_cap() {
    let sheet = Sheet {
        grow: true,
        ..many(12, 0)
    };
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let shown: Vec<usize> = scene.hits.rows.iter().map(|(index, _)| *index).collect();
    assert_eq!(shown, (0..12).collect::<Vec<_>>());
    assert!(scene.hits.panel.h > 420.0);
}

#[test]
fn a_growing_sheet_stops_at_the_window_and_scrolls_to_the_selection() {
    let sheet = Sheet {
        grow: true,
        ..many(200, 150)
    };
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    let panel = scene.hits.panel;
    assert!(panel.h > 420.0);
    assert!(panel.y + panel.h <= 1000.0 - 16.0);
    assert!(scene.hits.rows.len() > 9);
    assert!(scene.hits.rows.len() < 200);
    assert_eq!(scene.hits.rows.last().unwrap().0, 150);
}

#[test]
fn the_footer_has_a_tinted_band_with_key_caps_and_verbs() {
    let scene = run(&three(), (1600, 1000), 1.0, None);
    let panel = scene.hits.panel;
    let band = rect(513.0, panel.y + panel.h - 41.0, 574.0, 40.0);
    assert_eq!(quad_at(&scene, band).unwrap().color, colour(PALETTE_FOOTER));
    let caps: Vec<&Quad> = scene
        .quads
        .iter()
        .filter(|quad| quad.color == colour(PALETTE_KEYCAP))
        .collect();
    assert_eq!(caps.len(), 3);
    assert_eq!(caps[0].rect.x, 529.0);
    assert_eq!(caps[0].rect.h, 20.0);
    let verbs: Vec<&str> = scene
        .text
        .iter()
        .filter(|run| run.color == MUTED && run.face == TextFace::Ui && run.baseline > band.y)
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(verbs, ["Navigate", "Switch", "Close"]);
    assert_eq!(text(&scene, MOVE_KEY).unwrap().color, FOREGROUND);
}

#[test]
fn hit_test_finds_rows_the_panel_and_outside() {
    let scene = run(&three(), (1600, 1000), 1.0, None);
    assert_eq!(scene.hits.hit_test((600.0, 190.0)), OverlayHit::Row(1));
    assert_eq!(scene.hits.hit_test((600.0, 120.0)), OverlayHit::Panel);
    assert_eq!(scene.hits.hit_test((100.0, 500.0)), OverlayHit::Outside);
}

#[test]
fn hit_test_prefers_controls_over_rows() {
    let mut hits = OverlayHits {
        panel: rect(0.0, 0.0, 100.0, 100.0),
        ..OverlayHits::default()
    };
    hits.rows.push((0, rect(0.0, 0.0, 100.0, 50.0)));
    hits.controls
        .push((SettingsHit::Close, rect(0.0, 0.0, 20.0, 20.0)));
    assert_eq!(
        hits.hit_test((5.0, 5.0)),
        OverlayHit::Settings(SettingsHit::Close)
    );
    assert_eq!(hits.hit_test((50.0, 30.0)), OverlayHit::Row(0));
}

#[test]
fn the_theme_palette_docks_right_below_the_bar_without_a_dim() {
    let sheet = Sheet {
        placement: Placement::Docked,
        ..three()
    };
    let scene = run(&sheet, (1600, 1000), 1.0, None);
    assert_eq!(scene.hits.panel.x, 1600.0 - 16.0 - 360.0);
    assert_eq!(scene.hits.panel.y, 42.0);
    assert!(scene.quads.iter().all(|quad| quad.color != DIM));
}

fn assert_inside(scene: &OverlayScene, window: (u32, u32)) {
    let (width, height) = (window.0 as f32, window.1 as f32);
    for quad in &scene.quads {
        let rect = quad.rect;
        assert!(
            rect.x >= 0.0 && rect.y >= 0.0 && rect.x + rect.w <= width && rect.y + rect.h <= height,
            "{window:?} {rect:?}"
        );
    }
}

#[test]
fn small_windows_keep_everything_inside_and_drop_the_footer_when_no_row_fits() {
    for (px, scale) in [
        ((300, 200), 1.0),
        ((200, 120), 1.0),
        ((60, 40), 1.0),
        ((1, 1), 2.0),
    ] {
        for sheet in [three(), many(30, 29), windows(), many(0, 0)] {
            let scene = run(&sheet, px, scale, None);
            assert_inside(&scene, px);
        }
    }
    let scene = run(&many(30, 0), (800, 200), 1.0, None);
    assert_eq!(scene.hits.rows.len(), 1);
    assert!(text(&scene, "Navigate").is_some());
    let tight = run(&many(30, 0), (800, 170), 1.0, None);
    assert!(text(&tight, "Navigate").is_none());
}

#[test]
fn every_rect_and_text_lands_on_whole_pixels_at_scale_one_and_a_half() {
    let mut sheet = windows();
    sheet.rows[0].monogram = true;
    for sheet in [three(), sheet, many(0, 0)] {
        let scene = run(&sheet, (1500, 900), 1.5, Some((700.0, 300.0)));
        assert_inside(&scene, (1500, 900));
        for quad in &scene.quads {
            let r = quad.rect;
            assert!(
                [r.x, r.y, r.w, r.h, quad.radius]
                    .iter()
                    .all(|v| v.fract() == 0.0),
                "{quad:?}"
            );
        }
        for run in &scene.text {
            assert_eq!((run.x.fract(), run.baseline.fract()), (0.0, 0.0), "{run:?}");
        }
    }
}

#[test]
fn non_ascii_row_names_ellipsize_on_whole_chars() {
    let sheet = sessions(vec![row(&"ä".repeat(200), None, None)], 0);
    let scene = run(&sheet, (800, 600), 1.0, None);
    let name = scene
        .text
        .iter()
        .find(|run| run.text.starts_with('ä'))
        .unwrap();
    assert!(name.text.ends_with(ELLIPSIS));
}

#[test]
fn a_short_window_budgets_the_empty_message_and_the_footer_inside_the_panel() {
    for height in [1000, 260, 200, 170, 140, 120, 90, 60] {
        let scene = run(&many(0, 0), (800, height), 1.0, None);
        let panel = scene.hits.panel;
        assert!(panel.y + panel.h <= height as f32, "{height}");
        for run in &scene.text {
            assert!(
                run.baseline - 11.0 >= panel.y && run.baseline + 3.0 <= panel.y + panel.h,
                "{height} {run:?}"
            );
        }
        let message = text(&scene, "No matching sessions.");
        let footer = scene
            .quads
            .iter()
            .find(|quad| quad.color == colour(PALETTE_FOOTER) && quad.radius == 0.0);
        if let (Some(message), Some(footer)) = (message, footer) {
            assert!(message.baseline + 3.0 <= footer.rect.y, "{height}");
        }
    }
    let roomy = run(&many(0, 0), (800, 1000), 1.0, None);
    assert!(text(&roomy, "No matching sessions.").is_some());
    assert!(text(&roomy, "Navigate").is_some());
    let short = run(&many(0, 0), (800, 200), 1.0, None);
    assert!(text(&short, "No matching sessions.").is_some());
    assert!(text(&short, "Navigate").is_none());
}

#[test]
fn without_icons_keeps_fold_chevrons() {
    let sheet = sessions(
        vec![
            Row {
                icon: Some('x'),
                ..row("shell", None, None)
            },
            Row {
                icon: Some(CHEVRON_RIGHT),
                ..row("Temporary", None, None)
            },
        ],
        0,
    )
    .without_icons();
    assert_eq!(
        sheet.rows.iter().map(|row| row.icon).collect::<Vec<_>>(),
        [None, Some(CHEVRON_RIGHT)]
    );
}
