use super::*;
use crate::{CellMetrics, chrome_layout::frame};

fn config(position: TabsPosition) -> Config {
    let mut config = Config::default();
    config.tabs.position = position;
    config.tabs.width = 220;
    config
}

fn metrics() -> CellMetrics {
    CellMetrics {
        width: 8.0,
        height: 16.0,
        baseline: 12.0,
    }
}

fn left_frame(panels: &Panels) -> Frame {
    frame(
        (800, 600),
        1.0,
        false,
        (8.0, 4.0),
        metrics(),
        panels.dock(&config(TabsPosition::Left), false),
    )
}

#[test]
fn the_dock_follows_position_and_flip() {
    let mut panels = Panels::default();
    for (position, flipped, side) in [
        (TabsPosition::Top, false, None),
        (TabsPosition::Top, true, Some(Side::Left)),
        (TabsPosition::Left, false, Some(Side::Left)),
        (TabsPosition::Left, true, None),
        (TabsPosition::Right, false, Some(Side::Right)),
        (TabsPosition::Right, true, None),
    ] {
        panels.flipped = flipped.then_some(position);
        let dock = panels.dock(&config(position), false);
        assert_eq!(
            dock.tabs,
            side.map(|side| (side, 220.0)),
            "{position:?} {flipped}"
        );
        assert_eq!(dock.explorer, None);
        assert_eq!(panels.dock(&config(position), true).tabs, None);
    }
}

#[test]
fn toggling_tabs_flips_and_forgets_a_drag() {
    let mut panels = Panels {
        drag: Some(Drag::Edge {
            side: Side::Left,
            from: 10.0,
            width: 200.0,
        }),
        live_width: Some((PanelView::Tabs, 300.0)),
        ..Panels::default()
    };
    panels.toggle_tabs(TabsPosition::Left);
    assert_eq!(panels.flipped, Some(TabsPosition::Left));
    assert_eq!(panels.drag, None);
    assert_eq!(panels.live_width, None);
    panels.toggle_tabs(TabsPosition::Left);
    assert_eq!(panels.flipped, None);
}

#[test]
fn a_tab_toggle_is_forgotten_once_the_position_changes() {
    let mut panels = Panels::default();
    panels.toggle_tabs(TabsPosition::Left);
    panels.follow_position(TabsPosition::Left);
    assert_eq!(panels.flipped, Some(TabsPosition::Left));
    panels.follow_position(TabsPosition::Right);
    panels.follow_position(TabsPosition::Left);
    assert_eq!(panels.flipped, None);
}

#[test]
fn a_live_width_overrides_the_configured_one() {
    let panels = Panels {
        live_width: Some((PanelView::Tabs, 300.0)),
        ..Panels::default()
    };
    assert_eq!(
        panels.dock(&config(TabsPosition::Right), false).tabs,
        Some((Side::Right, 300.0))
    );
    let explorer = Panels {
        live_width: Some((PanelView::Explorer, 300.0)),
        ..Panels::default()
    };
    assert_eq!(
        explorer.dock(&config(TabsPosition::Right), false).tabs,
        Some((Side::Right, 220.0))
    );
}

#[test]
fn the_drop_index_is_the_row_under_the_pointer() {
    let panels = Panels::default();
    let frame = left_frame(&panels);
    for (y, index) in [
        (0.0, 1),
        (43.0, 1),
        (70.0, 1),
        (75.0, 2),
        (110.0, 3),
        (140.0, 4),
        (400.0, 4),
    ] {
        assert_eq!(panels.drop_index(&frame, y, 4, 1.0), index, "{y}");
    }
    let scrolled = Panels {
        tabs_scroll: 2,
        ..Panels::default()
    };
    assert_eq!(scrolled.drop_index(&frame, 50.0, 30, 1.0), 3);
    assert_eq!(scrolled.drop_index(&frame, 50.0, 6, 1.0), 1);
    let top = Panels::default();
    let bare = frame_without_panels();
    assert_eq!(top.drop_index(&bare, 50.0, 4, 1.0), 1);
}

fn frame_without_panels() -> Frame {
    frame(
        (800, 600),
        1.0,
        false,
        (8.0, 4.0),
        metrics(),
        Dock::default(),
    )
}

#[test]
fn scrolling_clamps_to_the_list_and_reveal_keeps_the_active_row_visible() {
    let mut panels = Panels::default();
    panels.place(Pages {
        tabs: 4,
        explorer: 0,
        agents: 0,
    });
    panels.scroll_tabs(-3, 10);
    assert_eq!(panels.tabs_scroll, 0);
    panels.scroll_tabs(5, 10);
    assert_eq!(panels.tabs_scroll, 5);
    panels.scroll_tabs(50, 10);
    assert_eq!(panels.tabs_scroll, 7);
    panels.reveal_tab(2);
    assert_eq!(panels.tabs_scroll, 2);
    panels.reveal_tab(9);
    assert_eq!(panels.tabs_scroll, 6);
    panels.reveal_tab(7);
    assert_eq!(panels.tabs_scroll, 6);
    panels.scroll_tabs(1, 3);
    assert_eq!(panels.tabs_scroll, 0);
}

#[test]
fn a_tab_drag_starts_after_the_threshold() {
    let mut panels = Panels {
        drag: Some(Drag::Tab {
            window: WindowId(3),
            from: 100.0,
            moved: false,
        }),
        ..Panels::default()
    };
    assert!(!panels.drag_tab(103.0, 1.0));
    assert!(!panels.drag_tab(96.0, 1.0));
    assert!(panels.drag_tab(105.0, 1.0));
    assert!(!panels.drag_tab(100.0, 1.0));
    assert_eq!(
        panels.drag,
        Some(Drag::Tab {
            window: WindowId(3),
            from: 100.0,
            moved: true,
        })
    );
}

#[test]
fn an_edge_drag_clamps_to_the_bounds_and_the_grid_limit() {
    let drag = |side, x: f32, limit: f32, scale: f32| {
        let mut panels = Panels {
            drag: Some(Drag::Edge {
                side,
                from: 400.0,
                width: 200.0,
            }),
            ..Panels::default()
        };
        assert!(panels.drag_edge(PanelView::Tabs, x, limit, scale));
        panels.live_width.unwrap()
    };
    assert_eq!(
        drag(Side::Left, 450.0, 1000.0, 1.0),
        (PanelView::Tabs, 250.0)
    );
    assert_eq!(
        drag(Side::Right, 450.0, 1000.0, 1.0),
        (PanelView::Tabs, 150.0)
    );
    assert_eq!(
        drag(Side::Left, 100.0, 1000.0, 1.0),
        (PanelView::Tabs, 120.0)
    );
    assert_eq!(
        drag(Side::Left, 900.0, 1000.0, 1.0),
        (PanelView::Tabs, 480.0)
    );
    assert_eq!(
        drag(Side::Left, 900.0, 300.0, 1.0),
        (PanelView::Tabs, 300.0)
    );
    assert_eq!(
        drag(Side::Left, 500.0, 1000.0, 2.0),
        (PanelView::Tabs, 250.0)
    );
    let mut idle = Panels::default();
    assert!(!idle.drag_edge(PanelView::Tabs, 10.0, 1000.0, 1.0));
    assert_eq!(idle.live_width, None);
    assert_eq!(bounds(PanelView::Explorer), (160.0, 640.0));
}

#[test]
fn a_flip_ends_when_the_configured_position_changes() {
    let mut panels = Panels::default();
    panels.toggle_tabs(TabsPosition::Left);
    assert_eq!(panels.dock(&config(TabsPosition::Left), false).tabs, None);
    assert_eq!(
        panels.dock(&config(TabsPosition::Right), false).tabs,
        Some((Side::Right, 220.0))
    );
    assert_eq!(panels.dock(&config(TabsPosition::Top), false).tabs, None);
    panels.toggle_tabs(TabsPosition::Right);
    assert_eq!(panels.dock(&config(TabsPosition::Right), false).tabs, None);
    assert_eq!(
        panels.dock(&config(TabsPosition::Left), false).tabs,
        Some((Side::Left, 220.0))
    );
}

#[test]
fn an_edge_drag_past_a_limit_below_the_minimum_keeps_no_width() {
    let mut panels = Panels {
        drag: Some(Drag::Edge {
            side: Side::Left,
            from: 400.0,
            width: 200.0,
        }),
        ..Panels::default()
    };
    assert!(panels.drag_edge(PanelView::Tabs, 420.0, 300.0, 1.0));
    assert_eq!(panels.live_width, Some((PanelView::Tabs, 220.0)));
    assert!(panels.drag_edge(PanelView::Tabs, 420.0, 100.0, 1.0));
    assert_eq!(panels.live_width, None);
    assert!(!panels.drag_edge(PanelView::Tabs, 300.0, 100.0, 1.0));
    assert_eq!(panels.live_width, None);
}

#[test]
fn a_reveal_before_the_panel_is_placed_applies_on_the_next_place() {
    let mut panels = Panels::default();
    panels.place(Pages {
        tabs: 4,
        explorer: 0,
        agents: 0,
    });
    panels.toggle_tabs(TabsPosition::Top);
    panels.reveal_tab(9);
    assert_eq!(panels.tabs_scroll, 0);
    assert!(!panels.place(Pages::default()));
    assert_eq!(panels.tabs_scroll, 0);
    assert!(panels.place(Pages {
        tabs: 4,
        explorer: 0,
        agents: 0,
    }));
    assert_eq!(panels.tabs_scroll, 6);
    panels.scroll_tabs(-6, 10);
    assert!(!panels.place(Pages {
        tabs: 4,
        explorer: 0,
        agents: 0,
    }));
    assert_eq!(panels.tabs_scroll, 0);
}

#[path = "panels_explorer_tests.rs"]
mod explorer_tests;

#[path = "panels_agents_tests.rs"]
mod agents_tests;

#[path = "panels_nav_tests.rs"]
mod nav_tests;
