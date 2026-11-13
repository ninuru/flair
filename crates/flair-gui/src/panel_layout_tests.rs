use super::*;
use crate::{
    CellMetrics, UiMetrics,
    chrome_layout::{ChromeLayout, Dock, frame, hit_test, layout},
    panels::Panels,
    renderer::{Quad, TextFace},
    status::{ChipParts, StatusParts},
    ui::ELLIPSIS,
};
use flair_core::{
    config::Config,
    ids::WindowId,
    theme::{Rgba, Theme},
};

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

fn metrics() -> CellMetrics {
    CellMetrics {
        width: 8.0,
        height: 16.0,
        baseline: 12.0,
    }
}

fn ui() -> UiMetrics {
    UiMetrics {
        height: 14.0,
        ascent: 11.0,
        small: 0.5,
        cap: 8.0,
        icon: CellMetrics {
            width: 7.0,
            height: 12.0,
            baseline: 9.0,
        },
    }
}

fn tab(index: u16, name: &str) -> TabParts {
    TabParts {
        window: WindowId(u32::from(index)),
        index,
        icon: None,
        name: name.into(),
        active: index == 1,
        last: false,
        activity: false,
        bell: false,
    }
}

fn status(tabs: Vec<TabParts>) -> StatusParts {
    StatusParts {
        chip: ChipParts {
            session: "main".into(),
            hidden: 0,
            running: false,
            finished: false,
        },
        tabs,
        badges: Vec::new(),
    }
}

fn three() -> StatusParts {
    status(vec![tab(1, "shell"), tab(2, "editor"), tab(3, "logs")])
}

struct Case<'a> {
    status: &'a StatusParts,
    side: Side,
    height: u32,
    hover: Option<(f32, f32)>,
    panels: Panels,
    explorer: Option<Side>,
}

impl<'a> Case<'a> {
    fn new(status: &'a StatusParts) -> Case<'a> {
        Case {
            status,
            side: Side::Left,
            height: 600,
            hover: None,
            panels: Panels::default(),
            explorer: None,
        }
    }

    fn dock(&self) -> Dock {
        Dock {
            tabs: Some((self.side, 200.0)),
            explorer: self.explorer.map(|side| (side, 280.0)),
            shown: self.panels.shown,
        }
    }

    fn run(&self) -> ChromeLayout {
        let theme = Config::default().theme();
        let px = (800, self.height);
        let input = ChromeInput {
            px,
            scale: 1.0,
            padding: (8.0, 4.0),
            zen: false,
            metrics: metrics(),
            ui: ui(),
            theme: &theme,
            status: self.status,
            prompt: None,
            layout: None,
            hover: self.hover,
            overlays: None,
            notice: None,
            isles: None,
            frame: frame(px, 1.0, false, (8.0, 4.0), metrics(), self.dock()),
            panels: Some(&self.panels),
        };
        layout(&input, measure)
    }
}

fn theme() -> Theme {
    Config::default().theme()
}

fn panel_quads(layout: &ChromeLayout) -> Vec<&Quad> {
    layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.rect.y >= 35.0)
        .collect()
}

fn run_text<'a>(layout: &'a ChromeLayout, text: &str) -> Option<&'a TextRun> {
    layout.chrome.text.iter().find(|run| run.text == text)
}

#[test]
fn a_left_panel_has_a_ground_a_hairline_and_rows_inside_its_inset() {
    let parts = three();
    let layout = Case::new(&parts).run();
    let theme = theme();
    let quads = panel_quads(&layout);
    assert_eq!(quads[0].rect, rect(0.0, 35.0, 200.0, 565.0));
    assert_eq!(quads[0].color, opaque(theme.background));
    assert_eq!(quads[1].rect, rect(199.0, 35.0, 1.0, 565.0));
    assert_eq!(quads[1].color, opaque(theme.surfaces().outline));
    let shell = run_text(&layout, "shell").unwrap();
    assert_eq!(shell.x, 55.0);
    assert_eq!(shell.face, TextFace::Ui);
    let index = run_text(&layout, "1").unwrap();
    assert_eq!(index.x, 18.0);
    assert_eq!(index.face, TextFace::Small);
    assert_eq!(index.color, theme.surfaces().faint);
    assert_eq!(run_text(&layout, "editor").unwrap().baseline, 93.0);
    assert_eq!(layout.pages.tabs, 17);
}

#[test]
fn a_right_panel_puts_the_hairline_on_its_left_edge() {
    let parts = three();
    let layout = Case {
        side: Side::Right,
        ..Case::new(&parts)
    }
    .run();
    let quads = panel_quads(&layout);
    assert_eq!(quads[0].rect, rect(600.0, 35.0, 200.0, 565.0));
    assert_eq!(quads[1].rect, rect(600.0, 35.0, 1.0, 565.0));
    assert_eq!(run_text(&layout, "shell").unwrap().x, 656.0);
    assert_eq!(
        hit_test(&layout.hits, (610.0, 50.0)),
        Some(HitTarget::Tab(WindowId(1)))
    );
    assert_eq!(
        hit_test(&layout.hits, (599.0, 300.0)),
        Some(HitTarget::Edge(Side::Right))
    );
}

#[test]
fn the_active_row_has_the_soft_accent_fill_and_the_accent_bar() {
    let parts = three();
    let layout = Case::new(&parts).run();
    let theme = theme();
    let quads = panel_quads(&layout);
    let row = rect(8.0, 43.0, 183.0, 28.0);
    assert!(
        quads
            .iter()
            .any(|quad| quad.rect == row && quad.color == opaque(theme.surfaces().accent_soft))
    );
    assert!(
        quads
            .iter()
            .any(|quad| quad.rect == rect(8.0, 50.0, 2.0, 14.0)
                && quad.color == opaque(theme.accent))
    );
    assert_eq!(run_text(&layout, "shell").unwrap().color, theme.foreground);
    assert_eq!(
        run_text(&layout, "editor").unwrap().color,
        theme.surfaces().muted
    );
}

#[test]
fn a_long_name_ends_in_an_ellipsis_before_the_trailing_slot() {
    let parts = status(vec![tab(1, "a very long window name that cannot fit")]);
    let layout = Case::new(&parts).run();
    let name = layout
        .chrome
        .text
        .iter()
        .find(|run| run.text.starts_with("a very"))
        .unwrap();
    assert!(name.text.ends_with(ELLIPSIS), "{}", name.text);
    assert!(name.x + measure(&name.text, false) <= 161.0 - 6.0);
}

#[test]
fn activity_brightens_the_name_and_a_bell_shows_its_glyph() {
    let mut tabs = vec![tab(1, "shell"), tab(2, "editor"), tab(3, "logs")];
    tabs[1].activity = true;
    tabs[2].bell = true;
    let parts = status(tabs);
    let layout = Case::new(&parts).run();
    let theme = theme();
    assert_eq!(run_text(&layout, "editor").unwrap().color, theme.foreground);
    assert_eq!(
        run_text(&layout, "logs").unwrap().color,
        theme.surfaces().muted
    );
    let bells: Vec<_> = layout
        .chrome
        .text
        .iter()
        .filter(|run| run.text == BELL_KEY.to_string())
        .collect();
    assert_eq!(bells.len(), 1);
    assert_eq!(bells[0].color, theme.surfaces().attention);
    assert_eq!(bells[0].face, TextFace::PromptIcon);
    assert!(bells[0].x >= 161.0 && bells[0].x < 181.0);
}

#[test]
fn the_close_glyph_shows_only_on_hover_and_replaces_the_bell() {
    let mut tabs = vec![tab(1, "shell"), tab(2, "editor")];
    tabs[1].bell = true;
    let parts = status(tabs);
    let close = CLOSE_KEY.to_string();
    let idle = Case::new(&parts).run();
    assert!(idle.chrome.text.iter().all(|run| run.text != close));
    let hovered = Case {
        hover: Some((60.0, 85.0)),
        ..Case::new(&parts)
    }
    .run();
    let theme = theme();
    let glyphs: Vec<_> = hovered
        .chrome
        .text
        .iter()
        .filter(|run| run.text == close || run.text == BELL_KEY.to_string())
        .collect();
    assert_eq!(glyphs.len(), 1);
    assert_eq!(glyphs[0].text, close);
    assert_eq!(glyphs[0].color, theme.surfaces().muted);
    assert_eq!(
        run_text(&hovered, "editor").unwrap().color,
        theme.surfaces().bright
    );
    let on_close = Case {
        hover: Some((170.0, 85.0)),
        ..Case::new(&parts)
    }
    .run();
    let glyph = on_close
        .chrome
        .text
        .iter()
        .find(|run| run.text == close)
        .unwrap();
    assert_eq!(glyph.color, theme.foreground);
    assert!(
        on_close
            .chrome
            .quads
            .iter()
            .any(|quad| quad.rect == rect(161.0, 79.0, 20.0, 20.0)
                && quad.color == opaque(theme.surfaces().surface3))
    );
}

#[test]
fn hits_find_close_tab_plus_edge_switcher_and_body_in_priority() {
    let parts = three();
    let layout = Case {
        explorer: Some(Side::Left),
        ..Case::new(&parts)
    }
    .run();
    let at = |x: f32, y: f32| hit_test(&layout.hits, (f64::from(x), f64::from(y)));
    assert_eq!(at(20.0, 60.0), Some(HitTarget::Switch(PanelView::Tabs)));
    assert_eq!(
        at(100.0, 60.0),
        Some(HitTarget::Switch(PanelView::Explorer))
    );
    assert_eq!(at(30.0, 100.0), Some(HitTarget::Tab(WindowId(1))));
    assert_eq!(at(250.0, 102.0), Some(HitTarget::CloseTab(WindowId(1))));
    assert_eq!(at(30.0, 200.0), Some(HitTarget::NewTab));
    assert_eq!(at(279.0, 400.0), Some(HitTarget::Edge(Side::Left)));
    assert_eq!(at(281.0, 400.0), Some(HitTarget::Edge(Side::Left)));
    assert_eq!(at(100.0, 400.0), Some(HitTarget::PanelBody));
    assert_eq!(at(300.0, 400.0), None);
}

#[test]
fn a_shared_slot_shows_the_switcher_with_the_shown_view_underlined() {
    let parts = three();
    let layout = Case {
        explorer: Some(Side::Left),
        ..Case::new(&parts)
    }
    .run();
    let theme = theme();
    assert_eq!(
        run_text(&layout, "Windows").unwrap().color,
        theme.foreground
    );
    assert_eq!(
        run_text(&layout, "Files").unwrap().color,
        theme.surfaces().muted
    );
    let quads = panel_quads(&layout);
    assert!(
        quads
            .iter()
            .any(|quad| quad.rect == rect(16.0, 69.0, 53.0, 2.0)
                && quad.color == opaque(theme.accent))
    );
    assert!(
        quads
            .iter()
            .any(|quad| quad.rect == rect(0.0, 71.0, 279.0, 1.0)
                && quad.color == opaque(theme.surfaces().outline))
    );
    assert_eq!(run_text(&layout, "shell").unwrap().baseline, 98.0);
    let mut panels = Panels::default();
    panels.shown = PanelView::Explorer;
    let explorer = Case {
        explorer: Some(Side::Left),
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(
        run_text(&explorer, "Files").unwrap().color,
        theme.foreground
    );
    assert!(run_text(&explorer, "shell").is_none());
    assert_eq!(explorer.pages.tabs, 0);
}

#[test]
fn the_plus_row_follows_the_last_tab() {
    let parts = three();
    let layout = Case {
        hover: Some((30.0, 150.0)),
        ..Case::new(&parts)
    }
    .run();
    let theme = theme();
    let label = run_text(&layout, "New window").unwrap();
    assert_eq!(label.color, theme.surfaces().bright);
    assert_eq!(label.x, 55.0);
    let add = run_text(&layout, &ADD_KEY.to_string()).unwrap();
    assert_eq!(add.x, 38.0);
    assert!(
        layout
            .chrome
            .quads
            .iter()
            .any(|quad| quad.rect == rect(8.0, 139.0, 183.0, 28.0)
                && quad.color == opaque(theme.surfaces().surface2))
    );
    assert_eq!(
        hit_test(&layout.hits, (30.0, 150.0)),
        Some(HitTarget::NewTab)
    );
}

#[test]
fn a_short_window_scrolls_whole_rows_and_keeps_rows_inside_the_panel() {
    let tabs: Vec<_> = (1..=12).map(|index| tab(index, "shell")).collect();
    let parts = status(tabs);
    let mut panels = Panels::default();
    panels.tabs_scroll = 4;
    let layout = Case {
        height: 200,
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert_eq!(layout.pages.tabs, 4);
    let indices: Vec<_> = layout
        .chrome
        .text
        .iter()
        .filter(|run| run.face == TextFace::Small)
        .map(|run| run.text.as_str())
        .collect();
    assert_eq!(indices, ["5", "6", "7", "8"]);
    assert!(
        panel_quads(&layout)
            .iter()
            .filter(|quad| quad.rect.x > 0.0 && quad.rect.w > 2.0)
            .all(|quad| quad.rect.y + quad.rect.h <= 192.0)
    );
    let mut panels = Panels::default();
    panels.tabs_scroll = 40;
    let end = Case {
        height: 200,
        panels,
        ..Case::new(&parts)
    }
    .run();
    assert!(run_text(&end, "New window").is_some());
    assert!(run_text(&end, "12").is_some());
    assert!(run_text(&end, "9").is_none());
}

#[test]
fn a_dragged_tab_keeps_a_raised_fill_and_shows_the_drop_line() {
    let parts = three();
    let mut panels = Panels::default();
    panels.drag = Some(Drag::Tab {
        window: WindowId(1),
        from: 50.0,
        moved: true,
    });
    let layout = Case {
        hover: Some((30.0, 120.0)),
        panels,
        ..Case::new(&parts)
    }
    .run();
    let theme = theme();
    let quads = panel_quads(&layout);
    assert!(
        quads
            .iter()
            .any(|quad| quad.rect == rect(8.0, 43.0, 183.0, 28.0)
                && quad.color == opaque(theme.surfaces().surface3))
    );
    assert!(quads.iter().any(
        |quad| quad.rect == rect(8.0, 136.0, 183.0, 2.0) && quad.color == opaque(theme.accent)
    ));
}
