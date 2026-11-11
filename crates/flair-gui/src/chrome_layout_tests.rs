use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use super::*;
use crate::{
    chrome_bar::fit_letters,
    notice::Notice,
    overlay_layout::OverlayHit,
    renderer::{Quad, TextFace, TextRun},
    status::{BadgeKind, BadgeParts, ChipParts, TabParts, status_parts},
    ui::{ELLIPSIS, SMALL_TEXT},
};
use flair_core::{
    config::{Config, PromptConfig, PromptStyle},
    geom::{Axis, CellPoint},
    ids::PaneId,
    isles::{GitInfo, Head, LastCommand, PromptInfo},
    proto::{PanePlacement, ServerMsg, SessionInfo},
    status::{StatusLine, StatusWindow},
    theme::{Rgb, Rgba},
    time::Millis,
};

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn opaque(color: Rgb) -> Rgba {
    Rgba {
        r: color.r,
        g: color.g,
        b: color.b,
        a: 255,
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn metrics(scale: f32) -> CellMetrics {
    CellMetrics {
        width: 8.0 * scale,
        height: 16.0 * scale,
        baseline: 12.0 * scale,
    }
}

fn ui(scale: f32) -> UiMetrics {
    UiMetrics {
        height: 14.0 * scale,
        ascent: 11.0 * scale,
        small: 0.5,
        cap: 8.0 * scale,
        icon: CellMetrics {
            width: 7.0 * scale,
            height: 12.0 * scale,
            baseline: 9.0 * scale,
        },
    }
}

fn parts() -> StatusParts {
    StatusParts {
        chip: ChipParts {
            session: "main".into(),
            hidden: 0,
            running: false,
            finished: false,
        },
        tabs: vec![
            TabParts {
                window: WindowId(1),
                index: 1,
                name: "shell".into(),
                active: true,
                last: false,
            },
            TabParts {
                window: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: false,
                last: false,
            },
        ],
        badges: Vec::new(),
    }
}

fn cells(col: u16, row: u16, cols: u16, rows: u16) -> CellRect {
    CellRect {
        origin: CellPoint { col, row },
        size: GridSize { cols, rows },
    }
}

fn window(separators: &[Separator], panes: &[(u32, CellRect)], active: u32) -> WindowLayout {
    WindowLayout {
        window: WindowId(1),
        area: cells(0, 0, 98, 34),
        panes: panes
            .iter()
            .map(|(pane, rect)| PanePlacement {
                pane: PaneId(*pane),
                rect: *rect,
                band: 0,
            })
            .collect(),
        separators: separators.to_vec(),
        active: PaneId(active),
        zoomed: false,
    }
}

fn run_layout(scale: f32, width: u32, window: Option<&WindowLayout>) -> ChromeLayout {
    let theme = Config::default().theme();
    let status = parts();
    let input = ChromeInput {
        px: (width * scale as u32, 600 * scale as u32),
        scale,
        padding: (8.0 * scale, 4.0 * scale),
        metrics: metrics(scale),
        ui: ui(scale),
        theme: &theme,
        status: &status,
        prompt: None,
        layout: window,
        hover: None,
        overlays: None,
        notice: None,
        isles: None,
    };
    layout(&input, measure)
}

#[test]
fn grid_starts_below_bar_and_border() {
    assert_eq!(bar_height(1.0), 34.0);
    assert_eq!(bar_height(2.0), 68.0);
    assert_eq!(bar_height(1.5), 51.0);
    assert_eq!(
        grid_origin((800, 600), (8.0, 4.0), 1.0, metrics(1.0)),
        (8.0, 52.0)
    );
    assert_eq!(
        grid_origin((1600, 1200), (16.0, 8.0), 2.0, metrics(2.0)),
        (16.0, 104.0)
    );
    assert_eq!(
        grid_size((800, 600), (8.0, 4.0), 1.0, metrics(1.0)),
        GridSize { cols: 98, rows: 34 }
    );
    assert_eq!(
        grid_size((1600, 1200), (16.0, 8.0), 2.0, metrics(2.0)),
        GridSize { cols: 98, rows: 34 }
    );
    assert_eq!(
        grid_size((10, 20), (8.0, 4.0), 1.0, metrics(1.0)),
        GridSize { cols: 1, rows: 1 }
    );
    let layout = run_layout(1.0, 800, None);
    assert_eq!(layout.chrome.origin, (8.0, 52.0));
    assert_eq!(layout.chrome.clip, rect(8.0, 39.0, 784.0, 557.0));
    assert_eq!(layout.chrome.extend, rect(0.0, 35.0, 800.0, 565.0));
    let layout = run_layout(2.0, 800, None);
    assert_eq!(layout.chrome.origin, (16.0, 104.0));
    assert_eq!(layout.chrome.clip, rect(16.0, 78.0, 1568.0, 1114.0));
    assert_eq!(layout.chrome.extend, rect(0.0, 70.0, 1600.0, 1130.0));
}

#[test]
fn grid_origin_centres_columns_and_puts_the_last_row_on_the_bottom_edge() {
    for (scale, width, origin) in [(1.0, 803, (9.0, 52.0)), (2.0, 803, (19.0, 104.0))] {
        let px = (width * scale as u32, 600 * scale as u32);
        let padding = (8.0 * scale, 4.0 * scale);
        let size = grid_size(px, padding, scale, metrics(scale));
        assert_eq!(size, GridSize { cols: 98, rows: 34 });
        assert_eq!(grid_origin(px, padding, scale, metrics(scale)), origin);
        let right = px.0 as f32 - origin.0 - f32::from(size.cols) * metrics(scale).width;
        let bottom = px.1 as f32 - origin.1 - f32::from(size.rows) * metrics(scale).height;
        assert!(
            (right - origin.0).abs() <= 1.0,
            "left {} right {right}",
            origin.0
        );
        assert!(
            (bottom - padding.1).abs() < 1.0,
            "top {} bottom {bottom}",
            origin.1
        );
        let layout = run_layout(scale, width, None);
        assert_eq!(layout.chrome.origin, origin);
    }
}

#[test]
fn zero_padding_puts_the_last_row_on_the_bottom_edge() {
    for (scale, width) in [(1.0, 803), (2.0, 803)] {
        let px = (width * scale as u32, 600 * scale as u32);
        let cell = metrics(scale);
        let size = grid_size(px, (0.0, 0.0), scale, cell);
        let top = 35.0 * scale;
        let leftover = (
            px.0 as f32 - f32::from(size.cols) * cell.width,
            px.1 as f32 - top - f32::from(size.rows) * cell.height,
        );
        assert!(leftover.0 >= 0.0 && leftover.1 >= 0.0, "{leftover:?}");
        assert_eq!(
            grid_origin(px, (0.0, 0.0), scale, cell),
            ((leftover.0 / 2.0).floor(), top + leftover.1.floor())
        );
        let theme = Config::default().theme();
        let status = parts();
        let input = ChromeInput {
            px,
            scale,
            padding: (0.0, 0.0),
            metrics: cell,
            ui: ui(scale),
            theme: &theme,
            status: &status,
            prompt: None,
            layout: None,
            hover: None,
            overlays: None,
            notice: None,
            isles: None,
        };
        let chrome = layout(&input, measure).chrome;
        let full = rect(0.0, top, px.0 as f32, px.1 as f32 - top);
        assert_eq!(chrome.origin, grid_origin(px, (0.0, 0.0), scale, cell));
        assert_eq!(chrome.clip, full);
        assert_eq!(chrome.extend, full);
    }
}

#[test]
fn mouse_mapping_uses_the_drawn_grid_origin() {
    for scale in [1.0, 2.0] {
        let layout = run_layout(scale, 803, None);
        let origin = layout.chrome.origin;
        let cell = metrics(scale);
        let size = GridSize { cols: 98, rows: 34 };
        let at = |x: f32, y: f32| {
            crate::mouse::to_cell((f64::from(x), f64::from(y)), &cell, origin, size)
        };
        assert_eq!(at(origin.0, origin.1), CellPoint { col: 0, row: 0 });
        assert_eq!(
            at(origin.0 + cell.width - 0.5, origin.1 + cell.height - 0.5),
            CellPoint { col: 0, row: 0 }
        );
        assert_eq!(
            at(origin.0 + cell.width, origin.1 + cell.height),
            CellPoint { col: 1, row: 1 }
        );
        assert_eq!(
            at(
                origin.0 + f32::from(size.cols) * cell.width - 0.5,
                origin.1 + f32::from(size.rows) * cell.height - 0.5
            ),
            CellPoint { col: 97, row: 33 }
        );
    }
}

fn lines(layout: ChromeLayout) -> Vec<(PxRect, Rgba)> {
    layout
        .chrome
        .quads
        .into_iter()
        .filter(|quad| quad.rect.y >= 35.0)
        .map(|quad| (quad.rect, quad.color))
        .collect()
}

#[test]
fn a_vertical_split_line_runs_from_the_bar_line_to_the_window_bottom() {
    let theme = Config::default().theme();
    let outline = opaque(theme.surfaces().outline);
    let accent = opaque(theme.accent);
    let separators = [Separator {
        axis: Axis::Horizontal,
        rect: cells(49, 0, 1, 34),
    }];
    let panes = [(1, cells(0, 0, 49, 34)), (2, cells(50, 0, 48, 34))];
    for (scale, line) in [
        (1.0, rect(404.0, 35.0, 1.0, 565.0)),
        (2.0, rect(807.0, 70.0, 2.0, 1130.0)),
    ] {
        let window = window(&separators, &panes, 2);
        let layout = run_layout(scale, 800, Some(&window));
        let found: Vec<_> = layout
            .chrome
            .quads
            .iter()
            .filter(|quad| quad.rect.y >= chrome_bottom(scale))
            .map(|quad| (quad.rect, quad.color))
            .collect();
        assert_eq!(found, [(line, outline), (line, accent)], "{scale}");
    }
}

#[test]
fn hit_test_finds_tabs_and_the_session_but_not_gaps_or_the_grid() {
    let layout = run_layout(1.0, 800, None);
    assert_eq!(
        hit_test(&layout.hits, (90.0, 10.0)),
        Some(HitTarget::Window(WindowId(1)))
    );
    assert_eq!(
        hit_test(&layout.hits, (160.0, 10.0)),
        Some(HitTarget::Window(WindowId(2)))
    );
    assert_eq!(
        hit_test(&layout.hits, (10.0, 10.0)),
        Some(HitTarget::Session)
    );
    assert_eq!(hit_test(&layout.hits, (154.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (79.0, 10.0)), None);
    assert_eq!(hit_test(&layout.hits, (90.0, 100.0)), None);
}

#[test]
fn accent_segments_meet_the_crossing_separator_without_a_gap() {
    let theme = Config::default().theme();
    let outline = opaque(theme.surfaces().outline);
    let accent = opaque(theme.accent);
    let separators = [
        Separator {
            axis: Axis::Horizontal,
            rect: cells(49, 0, 1, 34),
        },
        Separator {
            axis: Axis::Vertical,
            rect: cells(50, 16, 48, 1),
        },
    ];
    let panes = [
        (1, cells(0, 0, 49, 34)),
        (2, cells(50, 0, 48, 16)),
        (3, cells(50, 17, 48, 17)),
    ];
    let lines_for = |active| {
        lines(run_layout(
            1.0,
            800,
            Some(&window(&separators, &panes, active)),
        ))
    };
    let vertical = rect(404.0, 35.0, 1.0, 565.0);
    let horizontal = rect(404.0, 316.0, 396.0, 1.0);
    assert_eq!(
        lines_for(1),
        [
            (vertical, outline),
            (horizontal, outline),
            (vertical, accent)
        ]
    );
    let upper = rect(404.0, 35.0, 1.0, 282.0);
    assert_eq!(
        lines_for(2),
        [
            (vertical, outline),
            (horizontal, outline),
            (upper, accent),
            (horizontal, accent),
        ]
    );
    assert_eq!(upper.y + upper.h - 1.0, horizontal.y);
    assert_eq!(upper.x, horizontal.x);
    let lower = rect(404.0, 316.0, 1.0, 284.0);
    assert_eq!(
        lines_for(3),
        [
            (vertical, outline),
            (horizontal, outline),
            (lower, accent),
            (horizontal, accent),
        ]
    );
    assert_eq!((lower.x, lower.y), (horizontal.x, horizontal.y));
}

#[test]
fn zoomed_windows_draw_no_separators() {
    let window = WindowLayout {
        zoomed: true,
        ..window(&[], &[(1, cells(0, 0, 98, 34))], 1)
    };
    let layout = run_layout(1.0, 800, Some(&window));
    assert!(layout.chrome.quads.iter().all(|quad| quad.rect.y < 39.0));
}

#[test]
fn separators_reach_the_window_edges_and_are_clipped_to_the_window() {
    let surfaces = Config::default().theme().surfaces();
    let separators = [
        Separator {
            axis: Axis::Horizontal,
            rect: CellRect {
                origin: CellPoint { col: 10, row: 0 },
                size: GridSize { cols: 1, rows: 100 },
            },
        },
        Separator {
            axis: Axis::Vertical,
            rect: CellRect {
                origin: CellPoint { col: 0, row: 5 },
                size: GridSize { cols: 60, rows: 1 },
            },
        },
        Separator {
            axis: Axis::Horizontal,
            rect: CellRect {
                origin: CellPoint { col: 50, row: 0 },
                size: GridSize { cols: 1, rows: 10 },
            },
        },
    ];
    let window = window(&separators, &[], 9);
    let layout = run_layout(1.0, 200, Some(&window));
    let borders: Vec<PxRect> = layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.color == opaque(surfaces.outline) && quad.rect.y >= 35.0)
        .map(|quad| quad.rect)
        .collect();
    assert_eq!(layout.chrome.clip, rect(8.0, 39.0, 184.0, 557.0));
    assert_eq!(
        borders,
        [rect(92.0, 35.0, 1.0, 565.0), rect(0.0, 140.0, 200.0, 1.0)]
    );
}

fn fractional(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.3
}

fn scaled_status() -> StatusLine {
    StatusLine {
        session: "main".into(),
        windows: (1..=4)
            .map(|index| StatusWindow {
                id: WindowId(u32::from(index)),
                index,
                name: format!("window {index}"),
                active: index == 2,
                last: index == 1,
            })
            .collect(),
        pending: Some("^A".into()),
        copy: true,
        ..StatusLine::default()
    }
}

fn overlay_messages() -> [ServerMsg; 3] {
    [
        ServerMsg::ChooseSession {
            sessions: ["main", "notes", "work"]
                .into_iter()
                .map(|name| SessionInfo {
                    name: name.into(),
                    root: format!("/srv/{name}").into(),
                    windows: 2,
                })
                .collect(),
            current: "main".into(),
        },
        ServerMsg::ChooseWindow,
        ServerMsg::ChooseTheme,
    ]
}

fn scaled_layout(
    scale: f32,
    status: &StatusLine,
    overlays: &Overlays,
    notice: Option<&Notice>,
) -> ChromeLayout {
    let theme = Config::default().theme();
    let parts = status_parts(status);
    let separators = [
        Separator {
            axis: Axis::Horizontal,
            rect: cells(40, 0, 1, 30),
        },
        Separator {
            axis: Axis::Vertical,
            rect: cells(0, 12, 40, 1),
        },
    ];
    let window = window(
        &separators,
        &[(1, cells(0, 0, 40, 12)), (2, cells(41, 0, 40, 30))],
        1,
    );
    let input = ChromeInput {
        px: (1203, 757),
        scale,
        padding: ((8.0 * scale).round(), (4.0 * scale).round()),
        metrics: CellMetrics {
            width: 8.4 * scale,
            height: 17.0 * scale,
            baseline: 13.0 * scale,
        },
        ui: ui(scale),
        theme: &theme,
        status: &parts,
        prompt: overlays.bar_field(),
        layout: Some(&window),
        hover: None,
        overlays: Some(overlays),
        notice,
        isles: None,
    };
    layout(&input, fractional)
}

fn assert_whole(layout: &ChromeLayout, scale: f32) {
    let line = px(LINE, scale);
    let quads = layout.chrome.quads.iter().chain(&layout.chrome.modal);
    for quad in quads {
        let PxRect { x, y, w, h } = quad.rect;
        for value in [x, y, w, h] {
            assert_eq!(value.fract(), 0.0, "{scale} {quad:?}");
        }
        if let Some(border) = quad.border {
            assert_eq!(border.width, line, "{scale} {quad:?}");
        }
    }
    let runs = layout.chrome.text.iter().chain(&layout.chrome.modal_text);
    for run in runs {
        assert_eq!(run.x.fract(), 0.0, "{scale} {run:?}");
        assert_eq!(run.baseline.fract(), 0.0, "{scale} {run:?}");
    }
}

#[test]
fn every_line_and_text_origin_lands_on_whole_pixels_at_fractional_scales() {
    for scale in [1.25, 1.5] {
        let line = px(LINE, scale);
        assert_eq!(line, (scale).round().max(1.0));
        let status = scaled_status();
        let empty = Overlays::default();
        let layout = scaled_layout(scale, &status, &empty, None);
        assert_whole(&layout, scale);
        assert!(layout.chrome.quads.iter().any(|quad| quad.rect.h == line));
        for message in overlay_messages() {
            let mut overlays = Overlays::default();
            assert!(overlays.open(
                &message,
                &status,
                &Config::default(),
                Path::new("/nonexistent/flair.toml"),
            ));
            let layout = scaled_layout(scale, &status, &overlays, None);
            assert!(!layout.chrome.modal.is_empty());
            assert_whole(&layout, scale);
        }
        let mut overlays = Overlays::default();
        assert!(overlays.open(
            &ServerMsg::PromptRename {
                current: "editör".into(),
            },
            &status,
            &Config::default(),
            Path::new("/nonexistent/flair.toml"),
        ));
        let notice = Notice::error("window name cannot be empty");
        let layout = scaled_layout(scale, &status, &overlays, Some(&notice));
        assert!(
            layout
                .chrome
                .text
                .iter()
                .any(|run| run.text == "window name cannot be empty")
        );
        assert_whole(&layout, scale);
    }
}

#[test]
fn tab_and_row_hit_targets_match_their_drawn_rects_at_scale_1_5() {
    let scale = 1.5;
    let status = scaled_status();
    let surfaces = Config::default().theme().surfaces();
    let layout = scaled_layout(scale, &status, &Overlays::default(), None);
    let windows: Vec<&Hit> = layout
        .hits
        .iter()
        .filter(|hit| matches!(hit.target, HitTarget::Window(_)))
        .collect();
    assert_eq!(windows.len(), 4);
    for hit in &windows {
        assert_eq!((hit.rect.y, hit.rect.h), (0.0, bar_height(scale)));
        let centre = (
            f64::from(hit.rect.x + hit.rect.w / 2.0),
            f64::from(hit.rect.h / 2.0),
        );
        assert_eq!(hit_test(&layout.hits, centre), Some(hit.target));
    }
    let active = windows
        .iter()
        .find(|hit| hit.target == HitTarget::Window(WindowId(2)))
        .unwrap();
    let fill = layout
        .chrome
        .quads
        .iter()
        .find(|quad| quad.color == opaque(surfaces.surface3) && quad.border.is_none())
        .unwrap();
    assert_eq!((fill.rect.x, fill.rect.w), (active.rect.x, active.rect.w));

    for message in overlay_messages() {
        let mut overlays = Overlays::default();
        overlays.open(
            &message,
            &status,
            &Config::default(),
            Path::new("/nonexistent/flair.toml"),
        );
        let layout = scaled_layout(scale, &status, &overlays, None);
        let hits = layout.overlay.unwrap();
        assert!(!hits.rows.is_empty());
        for (index, rect) in &hits.rows {
            let centre = (
                f64::from(rect.x + rect.w / 2.0),
                f64::from(rect.y + rect.h / 2.0),
            );
            assert_eq!(hits.hit_test(centre), OverlayHit::Row(*index));
        }
        let selected = layout
            .chrome
            .modal
            .iter()
            .find(|quad| quad.color == opaque(surfaces.surface3) && quad.border.is_none())
            .unwrap();
        assert!(hits.rows.iter().any(|(_, rect)| *rect == selected.rect));
    }
}

fn bar_layout(width: u32, status: &StatusParts) -> ChromeLayout {
    let theme = Config::default().theme();
    let input = ChromeInput {
        px: (width, 600),
        scale: 1.0,
        padding: (8.0, 4.0),
        metrics: metrics(1.0),
        ui: ui(1.0),
        theme: &theme,
        status,
        prompt: None,
        layout: None,
        hover: None,
        overlays: None,
        notice: None,
        isles: None,
    };
    layout(&input, measure)
}

fn text<'a>(layout: &'a ChromeLayout, text: &str) -> Option<&'a TextRun> {
    layout.chrome.text.iter().find(|run| run.text == text)
}

fn narrow(session: &str, names: &[&str], active: usize) -> StatusParts {
    StatusParts {
        chip: ChipParts {
            session: session.into(),
            ..parts().chip
        },
        tabs: names
            .iter()
            .enumerate()
            .map(|(offset, name)| TabParts {
                window: WindowId(offset as u32 + 1),
                index: offset as u16 + 1,
                name: (*name).into(),
                active: offset == active,
                last: false,
            })
            .collect(),
        badges: vec![
            BadgeParts {
                kind: BadgeKind::Copy,
                text: "COPY".into(),
            },
            BadgeParts {
                kind: BadgeKind::Prefix,
                text: "^A".into(),
            },
        ],
    }
}

#[test]
fn a_400_px_bar_keeps_the_active_tab_between_session_and_badges() {
    let status = narrow(
        "infrastructure",
        &["shell", "editor", "cargo watch", "logs", "nvim"],
        4,
    );
    let layout = bar_layout(400, &status);
    let session = layout.hits[0].rect;
    let copy = text(&layout, "COPY").unwrap().x - 6.0;
    let tabs: Vec<Hit> = layout.hits[1..].to_vec();
    assert_eq!(tabs.last().unwrap().target, HitTarget::Window(WindowId(5)));
    for hit in &tabs {
        assert!(hit.rect.x >= session.x + session.w + 6.0);
        assert!(hit.rect.x + hit.rect.w <= copy - 6.0);
    }
    if let Some(more) = layout
        .chrome
        .text
        .iter()
        .find(|run| run.text.starts_with('+'))
    {
        assert!(more.x + measure(&more.text, false) <= copy - 6.0);
    }
}

#[test]
fn badges_win_over_a_long_session_name_in_a_narrow_bar() {
    let status = narrow("a-really-long-session-name", &["shell"], 0);
    let layout = bar_layout(260, &status);
    let session = layout.hits[0].rect;
    let copy = text(&layout, "COPY").unwrap().x - 6.0;
    assert!(session.x + session.w + 6.0 <= copy);
    let name = layout
        .chrome
        .text
        .iter()
        .find(|run| run.bold && run.text.starts_with("a-really"))
        .unwrap();
    assert!(name.text.ends_with(ELLIPSIS));
}

#[test]
fn non_ascii_names_ellipsize_on_whole_chars() {
    let long = "Überlanger Fenstername äöü ß";
    let cjk = "開発サーバーのログ監視ウィンドウ";
    let status = narrow("Größe", &[long, cjk], 0);
    let layout = bar_layout(260, &status);
    let tab = layout
        .chrome
        .text
        .iter()
        .find(|run| run.text.starts_with("1 Üb"))
        .unwrap();
    let kept = tab.text.strip_suffix(ELLIPSIS).unwrap();
    assert!(format!("1 {long}").starts_with(kept));
    let status = narrow("Größe", &["short", cjk], 1);
    let layout = bar_layout(800, &status);
    let tab = text(&layout, &format!("2 {cjk}")).or_else(|| {
        layout
            .chrome
            .text
            .iter()
            .find(|run| run.text.starts_with("2 開"))
    });
    let tab = tab.unwrap();
    let kept = tab.text.trim_end_matches(ELLIPSIS);
    assert!(format!("2 {cjk}").starts_with(kept));
    assert!(text(&layout, "Größe").is_some());
}

#[test]
fn monogram_letters_at_the_small_size_fit_inside_the_square() {
    let theme = Config::default().theme();
    let small = SMALL_TEXT / 12.0;
    for session in ["MA", "WW"] {
        let status = narrow(session, &["shell"], 0);
        let input = ChromeInput {
            px: (800, 600),
            scale: 1.0,
            padding: (8.0, 4.0),
            metrics: metrics(1.0),
            ui: UiMetrics { small, ..ui(1.0) },
            theme: &theme,
            status: &status,
            prompt: None,
            layout: None,
            hover: None,
            overlays: None,
            notice: None,
            isles: None,
        };
        let layout = layout(&input, measure);
        let letters = text(&layout, session).unwrap();
        let right = letters.x + measure(session, true) * small;
        assert!(letters.x >= 12.0 && right <= 30.0, "{session} {right}");
    }
}

#[test]
fn a_monogram_that_does_not_fit_its_square_keeps_only_its_first_letter() {
    let wide = |text: &str, bold: bool| {
        if bold {
            text.chars().count() as f32 * 11.0
        } else {
            measure(text, bold)
        }
    };
    let mut wide = wide;
    assert_eq!(fit_letters("WW", 18.0, 1.0, &mut wide), "W");
    assert_eq!(fit_letters("MA", 18.0, 0.8, &mut wide), "MA");
    assert_eq!(fit_letters("Ü", 18.0, 1.0, &mut wide), "Ü");
}

fn banded(separators: &[Separator], panes: &[(u32, CellRect)], active: u32) -> WindowLayout {
    let layout = window(separators, panes, active);
    WindowLayout {
        panes: layout
            .panes
            .iter()
            .map(|placement| PanePlacement {
                rect: cells(
                    placement.rect.origin.col,
                    placement.rect.origin.row + 1,
                    placement.rect.size.cols,
                    placement.rect.size.rows - 1,
                ),
                band: 1,
                ..*placement
            })
            .collect(),
        ..layout
    }
}

fn prompt(cwd: &str) -> PromptInfo {
    PromptInfo {
        cwd: Some(PathBuf::from(cwd)),
        git: Some(GitInfo {
            head: Head::Branch("main".into()),
            upstream: None,
            changes: None,
        }),
        last: Some(LastCommand {
            exit: Some(1),
            duration: Millis(0),
        }),
        ..PromptInfo::default()
    }
}

fn band_layout(
    window: &WindowLayout,
    info: &BTreeMap<PaneId, PromptInfo>,
    config: &PromptConfig,
) -> ChromeLayout {
    let theme = Config::default().theme();
    let status = parts();
    let mut mirror = crate::mirror::Mirror::default();
    let updates: Vec<_> = window
        .panes
        .iter()
        .map(|placement| flair_core::proto::PaneUpdate {
            pane: placement.pane,
            size: placement.rect.size,
            full: true,
            rows: Vec::new(),
            cursor: flair_core::proto::Cursor {
                point: CellPoint { col: 0, row: 1 },
                shape: flair_core::proto::CursorShape::Block,
                visible: true,
            },
            flags: flair_core::proto::PaneFlags {
                prompt_row: (placement.band > 0).then_some(0),
                ..Default::default()
            },
            selection: Vec::new(),
        })
        .collect();
    mirror.apply(&updates);
    let input = ChromeInput {
        px: (800, 600),
        scale: 1.0,
        padding: (8.0, 4.0),
        metrics: metrics(1.0),
        ui: ui(1.0),
        theme: &theme,
        status: &status,
        prompt: None,
        layout: Some(window),
        hover: None,
        overlays: None,
        notice: None,
        isles: Some(Isles {
            info,
            mirror: &mirror,
            config,
            clock: None,
            date: None,
            home: None,
        }),
    };
    layout(&input, measure)
}

fn band_chips(layout: &ChromeLayout) -> Vec<Quad> {
    layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.rect.y >= chrome_bottom(1.0) && quad.border.is_some())
        .copied()
        .collect()
}

fn band_text(layout: &ChromeLayout) -> Vec<(f32, f32, &str, TextFace)> {
    layout
        .chrome
        .text
        .iter()
        .filter(|run| run.baseline >= chrome_bottom(1.0))
        .map(|run| (run.x, run.baseline, run.text.as_str(), run.face))
        .collect()
}

fn plain_config() -> PromptConfig {
    PromptConfig {
        icons: false,
        ..PromptConfig::default()
    }
}

#[test]
fn each_pane_uses_subtle_outlined_chips_on_its_prompt_row() {
    let theme = Config::default().theme();
    let separators = [Separator {
        axis: Axis::Horizontal,
        rect: cells(49, 0, 1, 34),
    }];
    let window = banded(
        &separators,
        &[(1, cells(0, 0, 49, 34)), (2, cells(50, 0, 48, 34))],
        2,
    );
    let info = BTreeMap::from([
        (PaneId(1), prompt("/srv/one")),
        (PaneId(2), prompt("/srv/two")),
    ]);
    let layout = band_layout(&window, &info, &plain_config());
    let chips = band_chips(&layout);
    assert_eq!(chips.len(), 6);
    for chip in &chips {
        assert_eq!(
            chip.color,
            opaque(flair_core::theme::blend(
                theme.background,
                theme.foreground,
                4
            ))
        );
        assert_eq!(
            chip.border.unwrap().color,
            opaque(flair_core::theme::blend(
                theme.background,
                theme.foreground,
                12
            ))
        );
        assert_eq!(chip.rect.y, 68.0);
        assert_eq!(chip.rect.h, 16.0);
        assert_eq!(chip.radius, 3.0);
    }
    let text = band_text(&layout);
    assert_eq!(
        text.iter().map(|run| run.2).collect::<Vec<_>>(),
        ["/srv/one", "main", "1", "/srv/two", "main", "1"]
    );
    assert!(
        text.iter()
            .all(|run| run.3 == TextFace::Mono && run.1 == 80.0)
    );
}

#[test]
fn successful_exit_uses_neutral_chrome_and_readable_status_text() {
    let theme = Config::default().theme();
    let window = banded(&[], &[(1, cells(0, 0, 98, 34))], 1);
    let info = BTreeMap::from([(
        PaneId(1),
        PromptInfo {
            last: Some(LastCommand {
                exit: Some(0),
                duration: Millis(420),
            }),
            ..PromptInfo::default()
        },
    )]);
    let config = PromptConfig {
        duration_ms: 0,
        ..plain_config()
    };
    let layout = band_layout(&window, &info, &config);
    let chips = band_chips(&layout);
    assert_eq!(chips.len(), 1);
    assert_eq!(
        chips[0].color,
        opaque(flair_core::theme::blend(
            theme.background,
            theme.foreground,
            4
        ))
    );
    assert_eq!(band_text(&layout), [(14.0, 80.0, "0.42s", TextFace::Mono)]);
}

#[test]
fn prompt_icons_are_centered_beside_terminal_labels() {
    let window = banded(&[], &[(1, cells(0, 0, 98, 34))], 1);
    let info = BTreeMap::from([(PaneId(1), prompt("/srv/one"))]);
    let layout = band_layout(&window, &info, &PromptConfig::default());
    let text = band_text(&layout);
    assert_eq!(
        text.iter().map(|run| run.2).collect::<Vec<_>>(),
        ["\u{f114}", "/srv/one", "\u{f418}", "main", "1"]
    );
    for (index, run) in text.iter().enumerate() {
        let icon = index == 0 || index == 2;
        assert_eq!(
            run.3,
            if icon {
                TextFace::PromptIcon
            } else {
                TextFace::Mono
            }
        );
        assert_eq!(run.1, if icon { 82.0 } else { 80.0 });
    }
    assert_eq!(text[1].0 - text[0].0, 15.0);
    assert_eq!(text[3].0 - text[2].0, 15.0);
}

#[test]
fn a_narrow_pane_shortens_the_cwd_before_dropping_chips() {
    let info = BTreeMap::from([(PaneId(1), prompt("/srv/projects/flair"))]);
    let texts = |cols| {
        let window = banded(&[], &[(1, cells(0, 0, cols, 34))], 1);
        band_text(&band_layout(&window, &info, &plain_config()))
            .into_iter()
            .map(|(_, _, text, _)| text.to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(texts(98), ["/srv/projects/flair", "main", "1"]);
    assert_eq!(texts(20), ["…/flair", "main", "1"]);
    assert_eq!(texts(15), ["…/flair", "main"]);
    assert_eq!(texts(12), ["…/flair"]);
    assert!(texts(2).is_empty());
    for cols in 2..=98 {
        let window = banded(&[], &[(1, cells(0, 0, cols, 34))], 1);
        let layout = band_layout(&window, &info, &plain_config());
        for chip in band_chips(&layout) {
            let right = chip.rect.x + chip.rect.w;
            assert!(right <= 8.0 + f32::from(cols) * 8.0, "{cols}: {chip:?}");
        }
    }
}

#[test]
fn chips_centre_on_the_shell_prompt_row_in_stacked_panes() {
    let info = BTreeMap::from([
        (PaneId(1), prompt("/srv/one")),
        (PaneId(2), prompt("/srv/two")),
    ]);
    let window = banded(
        &[],
        &[(1, cells(0, 0, 98, 16)), (2, cells(0, 17, 98, 17))],
        1,
    );
    let layout = band_layout(&window, &info, &plain_config());
    let (_, top) = grid_origin((800, 600), (8.0, 4.0), 1.0, metrics(1.0));
    let prompt_top = top + 16.0;
    let chips = band_chips(&layout);
    assert_eq!(chips.len(), 6);
    let upper = chips[0].rect;
    let middle = upper.y + upper.h / 2.0;
    let centre = prompt_top + 8.0;
    assert!((middle - centre).abs() <= 0.5, "{upper:?} centre {centre}");
    assert!(chips[..3].iter().all(|chip| chip.rect.y == upper.y));
    let lower = chips[3].rect;
    assert_eq!(lower.h, upper.h);
    let row = top + 18.0 * 16.0;
    assert_eq!(lower.y, row);
    assert_eq!(lower.y + lower.h / 2.0, row + 8.0);
}

#[test]
fn no_chips_without_a_band_without_info_or_with_ps1() {
    let info = BTreeMap::from([(PaneId(1), prompt("/srv/one"))]);
    let panes = [(1, cells(0, 0, 98, 34))];
    assert!(
        band_chips(&band_layout(
            &window(&[], &panes, 1),
            &info,
            &plain_config()
        ))
        .is_empty()
    );
    let empty = BTreeMap::new();
    assert!(
        band_chips(&band_layout(
            &banded(&[], &panes, 1),
            &empty,
            &plain_config()
        ))
        .is_empty()
    );
    let ps1 = PromptConfig {
        style: PromptStyle::Ps1,
        ..plain_config()
    };
    assert!(band_chips(&band_layout(&banded(&[], &panes, 1), &info, &ps1)).is_empty());
    assert_eq!(
        band_chips(&band_layout(
            &banded(&[], &panes, 1),
            &info,
            &plain_config()
        ))
        .len(),
        3
    );
}

#[test]
fn the_active_accent_covers_the_band_row_of_a_banded_pane() {
    let theme = Config::default().theme();
    let outline = opaque(theme.surfaces().outline);
    let accent = opaque(theme.accent);
    let separators = [
        Separator {
            axis: Axis::Horizontal,
            rect: cells(49, 0, 1, 34),
        },
        Separator {
            axis: Axis::Vertical,
            rect: cells(50, 16, 48, 1),
        },
    ];
    let panes = [
        (1, cells(0, 0, 49, 34)),
        (2, cells(50, 0, 48, 16)),
        (3, cells(50, 17, 48, 17)),
    ];
    let vertical = rect(404.0, 35.0, 1.0, 565.0);
    let horizontal = rect(404.0, 316.0, 396.0, 1.0);
    let lower = rect(404.0, 316.0, 1.0, 284.0);
    assert_eq!(
        lines(run_layout(1.0, 800, Some(&banded(&separators, &panes, 3)))),
        [
            (vertical, outline),
            (horizontal, outline),
            (lower, accent),
            (horizontal, accent),
        ]
    );
    let upper = rect(404.0, 35.0, 1.0, 282.0);
    assert_eq!(
        lines(run_layout(1.0, 800, Some(&banded(&separators, &panes, 2)))),
        [
            (vertical, outline),
            (horizontal, outline),
            (upper, accent),
            (horizontal, accent),
        ]
    );
}
