use super::*;
use crate::{
    UiMetrics,
    overlay_layout::{Placement, Row, Sheet, layout},
    renderer::TextFace,
};
use flair_core::{
    config::Config,
    geom::{CellPoint, CellRect, GridSize},
    ids::{PaneId, SessionId, WindowId},
    proto::{Ink, PanePlacement, PanePreview, Separator},
    theme::Theme,
};

fn measure(text: &str, _: bool) -> f32 {
    text.chars().count() as f32 * 7.0
}

fn input(theme: &Theme, px: (u32, u32)) -> OverlayInput<'_> {
    OverlayInput {
        metrics: CellMetrics {
            width: 8.0,
            height: 16.0,
            baseline: 12.0,
        },
        px,
        scale: 1.0,
        ui: UiMetrics {
            height: 14.0,
            ascent: 11.0,
            small: 0.5,
            cap: 8.0,
            icon: CellMetrics {
                width: 7.0,
                height: 12.0,
                baseline: 9.0,
            },
        },
        theme,
        hover: None,
    }
}

fn window(id: u32, name: &str, active: bool) -> StatusWindow {
    StatusWindow {
        id: WindowId(id),
        index: u16::try_from(id).unwrap(),
        name: name.into(),
        program: String::new(),
        active,
        last: false,
    }
}

fn rect(x: f32, y: f32, w: f32, h: f32) -> PxRect {
    PxRect { x, y, w, h }
}

fn cells(col: u16, row: u16, cols: u16, rows: u16) -> CellRect {
    CellRect {
        origin: CellPoint { col, row },
        size: GridSize { cols, rows },
    }
}

fn shot(id: u32, rows: Vec<Vec<Ink>>, separators: Vec<Separator>) -> WindowPreview {
    let area = cells(0, 0, 97, 47);
    WindowPreview {
        window: WindowId(id),
        layout: WindowLayout {
            window: WindowId(id),
            area,
            panes: vec![PanePlacement {
                pane: PaneId(id),
                rect: area,
                band: 0,
            }],
            separators,
            active: PaneId(id),
            zoomed: false,
        },
        panes: vec![PanePreview {
            pane: PaneId(id),
            rows,
        }],
    }
}

fn draw(strip: &Strip, area: PxRect, px: (u32, u32)) -> OverlayScene {
    let theme = Config::default().theme();
    let mut scene = OverlayScene::default();
    self::strip(strip, area, &input(&theme, px), &mut scene, &mut measure);
    scene
}

fn frames(scene: &OverlayScene) -> Vec<&Quad> {
    scene
        .quads
        .iter()
        .filter(|quad| quad.border.is_some())
        .collect()
}

#[test]
fn one_thumbnail_per_window_up_to_nine_with_capped_width() {
    let strip = Strip {
        windows: (1..=12).map(|id| window(id, "zsh", id == 1)).collect(),
        preview: None,
    };
    let wide = draw(&strip, rect(0.0, 0.0, 4000.0, 120.0), (1000, 500));
    let thumbs: Vec<_> = wide
        .hits
        .windows
        .iter()
        .map(|(id, rect)| (*id, *rect))
        .collect();
    assert_eq!(thumbs.len(), 9);
    assert_eq!(thumbs[0], (WindowId(1), rect(0.0, 0.0, 200.0, 120.0)));
    assert_eq!(thumbs[1].1, rect(212.0, 0.0, 200.0, 120.0));
    assert_eq!(frames(&wide)[0].rect, rect(0.0, 0.0, 200.0, 100.0));
    let narrow = draw(
        &Strip {
            windows: strip.windows[..3].to_vec(),
            preview: None,
        },
        rect(10.0, 5.0, 300.0, 120.0),
        (1000, 500),
    );
    let rects: Vec<_> = frames(&narrow).iter().map(|quad| quad.rect).collect();
    assert_eq!(
        rects,
        [
            rect(10.0, 5.0, 92.0, 46.0),
            rect(114.0, 5.0, 92.0, 46.0),
            rect(218.0, 5.0, 92.0, 46.0),
        ]
    );
}

#[test]
fn the_current_window_gets_the_accent_border() {
    let theme = Config::default().theme();
    let strip = Strip {
        windows: vec![window(1, "a", false), window(2, "b", true)],
        preview: None,
    };
    let scene = draw(&strip, rect(0.0, 0.0, 600.0, 120.0), (1000, 500));
    let borders: Vec<_> = frames(&scene)
        .iter()
        .map(|quad| (quad.border.unwrap().color, quad.color, quad.radius))
        .collect();
    assert_eq!(
        borders,
        [
            (
                opaque(theme.surfaces().outline),
                opaque(theme.background),
                6.0
            ),
            (opaque(theme.accent), opaque(theme.background), 6.0),
        ]
    );
}

#[test]
fn labels_show_a_faint_index_and_a_muted_ellipsized_name() {
    let theme = Config::default().theme();
    let strip = Strip {
        windows: vec![window(3, "a-very-long-window-name", true)],
        preview: None,
    };
    let scene = draw(&strip, rect(0.0, 0.0, 60.0, 120.0), (1000, 500));
    let labels: Vec<_> = scene
        .text
        .iter()
        .map(|run| (run.text.as_str(), run.color, run.x))
        .collect();
    assert_eq!(labels[0], ("3", theme.surfaces().faint, 0.0));
    assert_eq!(labels[1].1, theme.surfaces().muted);
    assert_eq!(labels[1].2, 7.0);
    assert!(labels[1].0.ends_with(crate::ui::ELLIPSIS));
    assert!(measure(labels[1].0, false) * 0.5 <= 60.0 - 7.0);
}

#[test]
fn labels_put_a_faint_process_icon_between_the_index_and_the_name() {
    let theme = Config::default().theme();
    let strip = Strip {
        windows: vec![
            StatusWindow {
                program: "docker".into(),
                ..window(3, "docker compose", true)
            },
            window(4, "less", false),
        ],
        preview: None,
    };
    let scene = draw(&strip, rect(0.0, 0.0, 300.0, 120.0), (1000, 500));
    let labels: Vec<_> = scene
        .text
        .iter()
        .map(|run| (run.text.as_str(), run.color, run.x, run.face))
        .collect();
    let surfaces = theme.surfaces();
    assert_eq!(labels[0], ("3", surfaces.faint, 0.0, TextFace::Small));
    assert_eq!(
        labels[1],
        ("\u{f308}", surfaces.faint, 7.0, TextFace::PromptIcon)
    );
    assert_eq!(labels[2].0, "docker compose");
    assert_eq!((labels[2].1, labels[2].2), (surfaces.muted, 24.0));
    assert_eq!(labels[3].0, "4");
    assert_eq!(labels[4].0, "less");
    assert_eq!(labels[4].2 - labels[3].2, 7.0);
}

#[test]
fn ink_runs_become_thin_bars_and_backgrounds_become_cell_fills() {
    let theme = Config::default().theme();
    let rows = vec![
        vec![],
        vec![Ink {
            col: 2,
            len: 3,
            fg: Some(Color::Foreground),
            bg: Color::Background,
        }],
        vec![Ink {
            col: 10,
            len: 2,
            fg: None,
            bg: Color::Indexed(1),
        }],
    ];
    let separator = Separator {
        axis: Axis::Horizontal,
        rect: cells(48, 0, 1, 47),
    };
    let strip = Strip {
        windows: vec![window(1, "zsh", true)],
        preview: Some(SessionPreview {
            session: SessionId(1),
            windows: vec![shot(1, rows, vec![separator])],
        }),
    };
    let scene = draw(&strip, rect(0.0, 0.0, 2000.0, 1000.0), (1000, 500));
    let quads: Vec<_> = scene
        .quads
        .iter()
        .map(|quad| (quad.rect, quad.color))
        .collect();
    assert_eq!(quads[0].0, rect(0.0, 0.0, 200.0, 194.0));
    assert_eq!(
        quads[1..],
        [
            (
                rect(100.0, 3.0, 1.0, 188.0),
                opaque(theme.surfaces().outline)
            ),
            (
                rect(7.0, 8.0, 6.0, 2.0),
                opaque(blend(theme.background, theme.foreground, 55))
            ),
            (rect(23.0, 11.0, 4.0, 4.0), opaque(theme.normal[1])),
        ]
    );
}

#[test]
fn the_quad_count_is_capped() {
    let busy: Vec<Vec<Ink>> = (0..47)
        .map(|_| {
            (0..48)
                .map(|run| Ink {
                    col: run * 2,
                    len: 1,
                    fg: Some(Color::Foreground),
                    bg: Color::Background,
                })
                .collect()
        })
        .collect();
    let strip = Strip {
        windows: (1..=9).map(|id| window(id, "busy", false)).collect(),
        preview: Some(SessionPreview {
            session: SessionId(1),
            windows: (1..=9).map(|id| shot(id, busy.clone(), vec![])).collect(),
        }),
    };
    let scene = draw(&strip, rect(0.0, 0.0, 2000.0, 1000.0), (1000, 500));
    let ink = scene.quads.len() - frames(&scene).len();
    assert!(ink <= 4000, "{ink}");
    assert!(ink > 3900, "{ink}");
}

fn tree_sheet(rows: usize) -> Sheet {
    Sheet {
        placement: Placement::Centred,
        title: Some("Sessions"),
        field: None,
        count: (rows, rows),
        rows: (0..rows)
            .map(|index| Row {
                name: format!("s{index}"),
                ..Row::default()
            })
            .collect(),
        selected: 0,
        keys: vec![("esc", "close".into())],
        strip: Some(Strip {
            windows: vec![window(1, "zsh", true), window(2, "nvim", false)],
            preview: None,
        }),
    }
}

#[test]
fn the_sheet_reserves_the_strip_between_rows_and_footer() {
    let theme = Config::default().theme();
    let sheet = tree_sheet(4);
    let scene = layout(&sheet, &input(&theme, (1000, 700)), &mut measure);
    assert_eq!(scene.hits.windows.len(), 2);
    let last_row = scene.hits.rows.last().unwrap().1;
    let thumb = scene.hits.windows[0].1;
    assert_eq!(scene.hits.rows.len(), 4);
    assert!(thumb.y >= last_row.y + last_row.h + 25.0);
    let panel = scene.hits.panel;
    assert!(thumb.y + thumb.h <= panel.y + panel.h - 20.0 - 20.0 - 25.0);
    let without = layout(
        &Sheet {
            strip: None,
            ..tree_sheet(4)
        },
        &input(&theme, (1000, 700)),
        &mut measure,
    );
    assert_eq!(
        scene.hits.panel.h - without.hits.panel.h,
        12.0 * 2.0 + 1.0 + 120.0
    );
}

#[test]
fn the_strip_is_hidden_when_the_panel_is_too_short() {
    let theme = Config::default().theme();
    let scene = layout(&tree_sheet(10), &input(&theme, (1000, 330)), &mut measure);
    assert!(scene.hits.windows.is_empty());
    assert!(scene.hits.rows.len() >= 3);
}
