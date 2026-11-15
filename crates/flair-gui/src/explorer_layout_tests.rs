use std::{ffi::OsString, path::PathBuf, time::Instant};

use flair_core::{
    config::{Config, Side},
    ids::{PaneId, WindowId},
    key::{Key, KeyAction, KeyInput, Locks, Mods},
    keys::KeysConfig,
    status::ActivePane,
    theme::{Rgb, Rgba, Theme},
};

use super::*;
use crate::{
    CellMetrics, UiMetrics,
    chrome_layout::PanelView,
    chrome_layout::{ChromeLayout, Dock, frame, hit_test, layout},
    explorer::{Entry, Listing},
    panels::{Pages, Panels},
    renderer::{Quad, TextFace, TextRun},
    status::{ChipParts, StatusParts, TabParts},
    ui::ELLIPSIS,
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

fn theme() -> Theme {
    Config::default().theme()
}

fn status() -> StatusParts {
    StatusParts {
        chip: ChipParts {
            session: "main".into(),
            hidden: 0,
            running: false,
            finished: false,
        },
        tabs: vec![TabParts {
            window: WindowId(1),
            index: 1,
            icon: None,
            name: "shell".into(),
            active: true,
            last: false,
            activity: false,
            bell: false,
        }],
        badges: Vec::new(),
    }
}

fn listing(dir: &str, entries: &[(&str, bool)], more: usize) -> Listing {
    Listing {
        dir: PathBuf::from(dir),
        entries: entries
            .iter()
            .map(|(name, dir)| Entry {
                name: OsString::from(name),
                dir: *dir,
            })
            .collect(),
        more,
        error: None,
    }
}

const LONG: &str = "a-very-long-file-name-that-cannot-fit-the-panel.txt";

fn panels(at_prompt: bool) -> Panels {
    let mut panels = Panels::default();
    panels.show_hidden(true);
    panels.follow(Some(ActivePane {
        pane: PaneId(1),
        cwd: Some(PathBuf::from("/home/u/proj")),
        at_prompt,
    }));
    panels.explorer_command();
    panels.explorer.insert(listing(
        "/home/u/proj",
        &[
            (".git", true),
            ("src", true),
            ("target", true),
            (".env", false),
            (LONG, false),
            ("Cargo.toml", false),
        ],
        0,
    ));
    panels.explorer.click(2, false);
    panels.explorer.insert(listing(
        "/home/u/proj/src",
        &[("main.rs", false), ("lib.rs", false)],
        3,
    ));
    panels.explorer.key(&key('k'), false, 20);
    panels
}

fn key(c: char) -> KeyInput {
    KeyInput {
        key: Key::Char(c),
        mods: Mods::default(),
        text: Some(c.to_string()),
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

struct Case {
    panels: Panels,
    height: u32,
    hover: Option<(f32, f32)>,
    tabs: Option<Side>,
}

impl Case {
    fn new(panels: Panels) -> Case {
        Case {
            panels,
            height: 600,
            hover: None,
            tabs: None,
        }
    }

    fn run(&self) -> ChromeLayout {
        let theme = theme();
        let status = status();
        let px = (1000, self.height);
        let dock = Dock {
            tabs: self.tabs.map(|side| (side, 200.0)),
            explorer: Some((Side::Left, 280.0)),
            shown: self.panels.shown,
        };
        let input = ChromeInput {
            px,
            scale: 1.0,
            padding: (8.0, 4.0),
            zen: false,
            metrics: metrics(),
            ui: ui(),
            theme: &theme,
            status: &status,
            prompt: None,
            layout: None,
            hover: self.hover,
            overlays: None,
            notice: None,
            isles: None,
            frame: frame(px, 1.0, false, (8.0, 4.0), metrics(), dock),
            panels: Some(&self.panels),
        };
        layout(&input, measure)
    }
}

fn run_text<'a>(layout: &'a ChromeLayout, text: &str) -> Option<&'a TextRun> {
    layout.chrome.text.iter().find(|run| run.text == text)
}

fn glyphs(layout: &ChromeLayout, glyph: char) -> Vec<&TextRun> {
    layout
        .chrome
        .text
        .iter()
        .filter(|run| run.text == glyph.to_string())
        .collect()
}

fn quads_at(layout: &ChromeLayout, area: PxRect) -> Vec<&Quad> {
    layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.rect == area)
        .collect()
}

#[test]
fn rows_indent_by_depth_with_chevrons_and_icons() {
    let layout = Case::new(panels(false)).run();
    let theme = theme();
    let faint = theme.surfaces().faint;
    let root = run_text(&layout, "proj").unwrap();
    assert_eq!((root.x, root.baseline), (52.0, 59.0));
    assert_eq!(root.face, TextFace::Ui);
    let src = run_text(&layout, "src").unwrap();
    assert_eq!((src.x, src.baseline), (68.0, 107.0));
    assert_eq!(run_text(&layout, "main.rs").unwrap().x, 84.0);
    assert_eq!(run_text(&layout, "main.rs").unwrap().baseline, 131.0);
    let downs = glyphs(&layout, CHEVRON_DOWN);
    assert_eq!(
        downs.iter().map(|run| run.x).collect::<Vec<_>>(),
        [18.0, 34.0]
    );
    assert!(downs.iter().all(|run| run.color == faint));
    assert_eq!(glyphs(&layout, CHEVRON_RIGHT).len(), 2);
    assert_eq!(glyphs(&layout, '\u{eaf7}').len(), 2);
    assert_eq!(glyphs(&layout, '\u{ea83}').len(), 2);
    let rust = glyphs(&layout, '\u{e7a8}');
    assert_eq!(rust.len(), 2);
    assert_eq!(rust[0].x, 67.0);
    assert_eq!(rust[0].color, theme.surfaces().muted);
    assert_eq!(rust[0].face, TextFace::PromptIcon);
    assert!(run_text(&layout, ".git").is_some());
    assert!(run_text(&layout, ".env").is_some());
    assert_eq!(
        run_text(&layout, "Cargo.toml").unwrap().color,
        theme.surfaces().bright
    );
}

#[test]
fn filler_rows_show_more_loading_and_errors_in_faint() {
    let mut panels = panels(false);
    let theme = theme();
    let target = panels
        .explorer
        .rows()
        .iter()
        .position(|row| row.name == "target")
        .unwrap();
    panels.explorer.click(target, false);
    let layout = Case::new(panels.clone()).run();
    let more = run_text(&layout, "+3 more").unwrap();
    assert_eq!(more.face, TextFace::Small);
    assert_eq!(more.color, theme.surfaces().faint);
    assert_eq!(more.x, 67.0);
    assert_eq!(
        run_text(&layout, ELLIPSIS).unwrap().color,
        theme.surfaces().faint
    );
    panels.explorer.insert(Listing {
        error: Some("Permission denied".into()),
        ..listing("/home/u/proj/target", &[], 0)
    });
    let layout = Case::new(panels).run();
    let error = run_text(&layout, "Permission denied").unwrap();
    assert_eq!(error.color, theme.surfaces().faint);
}

#[test]
fn a_long_name_ends_in_an_ellipsis_inside_the_row() {
    let layout = Case::new(panels(false)).run();
    let name = layout
        .chrome
        .text
        .iter()
        .find(|run| run.text.starts_with("a-very"))
        .unwrap();
    assert!(name.text.ends_with(ELLIPSIS), "{}", name.text);
    assert!(name.x + measure(&name.text, false) <= 8.0 + 263.0 - 10.0);
}

#[test]
fn the_focused_selection_uses_the_accent_and_the_unfocused_one_a_quiet_fill() {
    let mut panels = panels(false);
    let theme = theme();
    let row = rect(8.0, 67.0, 263.0, 24.0);
    let layout = Case::new(panels.clone()).run();
    let fills = quads_at(&layout, row);
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].color, opaque(theme.surfaces().accent_soft));
    assert_eq!(quads_at(&layout, rect(8.0, 74.0, 2.0, 10.0)).len(), 1);
    assert_eq!(run_text(&layout, ".git").unwrap().color, theme.foreground);
    panels.focused = false;
    let layout = Case::new(panels).run();
    let fills = quads_at(&layout, row);
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].color, opaque(theme.surfaces().surface3));
    assert!(quads_at(&layout, rect(8.0, 74.0, 2.0, 10.0)).is_empty());
}

#[test]
fn hovering_a_row_fills_it_quietly() {
    let theme = theme();
    let layout = Case {
        hover: Some((100.0, 160.0)),
        ..Case::new(panels(false))
    }
    .run();
    let fills = quads_at(&layout, rect(8.0, 139.0, 263.0, 24.0));
    assert_eq!(fills.len(), 1);
    assert_eq!(fills[0].color, opaque(theme.surfaces().surface2));
}

#[test]
fn rows_are_hit_by_their_absolute_index_and_the_rest_is_panel_body() {
    let layout = Case::new(panels(false)).run();
    assert_eq!(
        hit_test(&layout.hits, (40.0, 50.0)),
        Some(HitTarget::Row(0))
    );
    assert_eq!(
        hit_test(&layout.hits, (40.0, 140.0)),
        Some(HitTarget::Row(4))
    );
    assert_eq!(
        hit_test(&layout.hits, (40.0, 520.0)),
        Some(HitTarget::PanelBody)
    );
    assert_eq!(
        hit_test(&layout.hits, (279.0, 300.0)),
        Some(HitTarget::Edge(Side::Left))
    );
}

#[test]
fn rows_scroll_whole_and_keep_absolute_hits() {
    let mut panels = panels(false);
    panels.place(Pages {
        tabs: 0,
        explorer: 3,
    });
    panels.explorer.wheel(2, 3);
    let layout = Case::new(panels).run();
    assert!(run_text(&layout, "proj").is_none());
    assert_eq!(run_text(&layout, "src").unwrap().baseline, 59.0);
    assert_eq!(
        hit_test(&layout.hits, (40.0, 50.0)),
        Some(HitTarget::Row(2))
    );
}

#[test]
fn rows_that_do_not_fit_are_dropped_whole() {
    let mut panels = panels(false);
    let focused = Case {
        height: 200,
        ..Case::new(panels.clone())
    }
    .run();
    assert_eq!(focused.pages.explorer, 4);
    assert!(run_text(&focused, "main.rs").is_some());
    assert!(run_text(&focused, "lib.rs").is_none());
    panels.focused = false;
    let quiet = Case {
        height: 200,
        ..Case::new(panels.clone())
    }
    .run();
    assert_eq!(quiet.pages.explorer, 6);
    let tall = Case::new(panels).run();
    assert_eq!(tall.pages.explorer, 22);
}

#[test]
fn the_footer_shows_keys_while_focused_and_cd_only_at_a_prompt_on_a_folder() {
    let theme = theme();
    let busy = Case::new(panels(false)).run();
    assert_eq!(busy.pages.explorer, 21);
    assert!(run_text(&busy, "open").is_some());
    assert!(run_text(&busy, "cd").is_none());
    assert_eq!(
        quads_at(&busy, rect(0.0, 559.0, 279.0, 1.0))[0].color,
        opaque(theme.surfaces().outline)
    );
    let ready = Case::new(panels(true)).run();
    assert!(run_text(&ready, "insert").is_some());
    let mut on_file = panels(true);
    on_file.explorer.click(4, false);
    let file = Case::new(on_file).run();
    assert!(run_text(&file, "insert").is_some());
    assert!(run_text(&file, "cd").is_none());
    let mut quiet = panels(true);
    quiet.focused = false;
    let quiet = Case::new(quiet).run();
    assert!(run_text(&quiet, "open").is_none());
}

#[test]
fn footer_caps_that_do_not_fit_are_dropped_from_the_right() {
    let layout = Case::new(panels(true)).run();
    let verbs: Vec<&str> = ["open", "insert", "cd", "copy", "filter", "back"]
        .into_iter()
        .filter(|verb| run_text(&layout, verb).is_some())
        .collect();
    assert_eq!(verbs, ["open", "insert", "cd"]);
    let busy = Case::new(panels(false)).run();
    let verbs: Vec<&str> = ["open", "insert", "cd", "copy", "filter", "back"]
        .into_iter()
        .filter(|verb| run_text(&busy, verb).is_some())
        .collect();
    assert_eq!(verbs, ["open", "insert", "copy"]);
}

#[test]
fn the_filter_field_sits_above_the_rows_with_a_caret_and_bold_matches() {
    let mut panels = panels(false);
    let keys = KeysConfig::default();
    panels.key(&key('/'), &keys, false, Instant::now());
    let theme = theme();
    let empty = Case::new(panels.clone()).run();
    let field = rect(8.0, 43.0, 263.0, 28.0);
    let border = quads_at(&empty, field);
    assert_eq!(border.len(), 1);
    assert!(border[0].border.is_some());
    let placeholder = run_text(&empty, "filter").unwrap();
    assert_eq!(placeholder.color, theme.surfaces().faint);
    assert_eq!(run_text(&empty, "proj").unwrap().baseline, 95.0);
    assert!(run_text(&empty, "keep").is_some());
    panels.text("main");
    let typed = Case::new(panels.clone()).run();
    let runs: Vec<&TextRun> = typed
        .chrome
        .text
        .iter()
        .filter(|run| run.text == "main")
        .collect();
    assert!(
        runs.iter()
            .any(|run| !run.bold && run.color == theme.foreground)
    );
    assert!(
        runs.iter()
            .any(|run| run.bold && run.color == theme.foreground)
    );
    assert!(
        typed
            .chrome
            .quads
            .iter()
            .any(|quad| quad.color == opaque(theme.accent) && quad.rect.w == 1.0)
    );
    assert!(run_text(&typed, ".rs").is_some());
    panels.key(
        &KeyInput {
            key: Key::Named(flair_core::key::NamedKey::Enter),
            text: None,
            ..key('x')
        },
        &keys,
        false,
        Instant::now(),
    );
    let kept = Case::new(panels).run();
    assert!(run_text(&kept, "main").is_some());
    assert!(
        !kept
            .chrome
            .quads
            .iter()
            .any(|quad| quad.color == opaque(theme.accent) && quad.rect.w == 1.0)
    );
}

#[test]
fn without_a_cwd_one_quiet_row_says_so() {
    let mut panels = Panels::default();
    panels.explorer_command();
    let layout = Case::new(panels).run();
    let row = run_text(&layout, "no directory").unwrap();
    assert_eq!(row.color, theme().surfaces().faint);
}

#[test]
fn a_shared_slot_shows_the_tree_below_the_switcher() {
    let layout = Case {
        tabs: Some(Side::Left),
        ..Case::new(panels(false))
    }
    .run();
    assert_eq!(
        run_text(&layout, "Files").unwrap().color,
        theme().foreground
    );
    assert_eq!(run_text(&layout, "proj").unwrap().baseline, 96.0);
    assert!(run_text(&layout, "shell").is_none());
    assert_eq!(
        hit_test(&layout.hits, (30.0, 50.0)),
        Some(HitTarget::Switch(PanelView::Tabs))
    );
    assert_eq!(
        hit_test(&layout.hits, (40.0, 90.0)),
        Some(HitTarget::Row(0))
    );
}
