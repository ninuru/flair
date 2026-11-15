use super::*;

fn glyph(layout: &ChromeLayout) -> Option<&TextRun> {
    text(layout, &FILES_KEY.to_string())
}

#[test]
fn the_explorer_button_sits_at_the_far_right_of_the_bar() {
    let surfaces = Config::default().theme().surfaces();
    let status = parts();
    let layout = Case::new(&status).run();
    let button = rect(768.0, 0.0, 24.0, 34.0);
    assert_eq!(
        hit_test(&layout.hits, (780.0, 3.0)),
        Some(HitTarget::ExplorerButton)
    );
    assert!(layout.hits.iter().any(|hit| hit.rect == button));
    let run = glyph(&layout).unwrap();
    assert_eq!(run.face, TextFace::PromptIcon);
    assert_eq!(run.color, surfaces.muted);
    assert!(run.x >= 768.0 && run.x < 780.0, "{}", run.x);
    assert!(quad_at(&layout, rect(768.0, 5.0, 24.0, 24.0)).is_none());
}

#[test]
fn hovering_the_button_fills_it_and_an_open_explorer_brightens_it() {
    let theme = Config::default().theme();
    let status = parts();
    let hovered = Case {
        hover: Some((780.0, 10.0)),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(
        quad_at(&hovered, rect(768.0, 5.0, 24.0, 24.0)),
        Some(&rounded(
            rect(768.0, 5.0, 24.0, 24.0),
            theme.surfaces().surface2,
            ROW_RADIUS,
            1.0
        ))
    );
    let mut open = Panels::default();
    open.open = true;
    let layout = Case {
        panels: Some(&open),
        ..Case::new(&status)
    }
    .run();
    assert_eq!(glyph(&layout).unwrap().color, theme.foreground);
}

#[test]
fn the_badges_move_one_item_left_of_the_button() {
    let status = copy_and_prefix();
    let layout = Case::new(&status).run();
    let copy = quad_at(&layout, rect(736.0, 7.0, 26.0, 20.0)).unwrap();
    assert_eq!(copy.rect.x + copy.rect.w + 6.0, 768.0);
}
