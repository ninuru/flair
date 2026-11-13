use super::*;
use flair_core::config::Side;

const PAD: (f32, f32) = (8.0, 4.0);

fn docked(tabs: Option<(Side, f32)>, explorer: Option<(Side, f32)>) -> Dock {
    Dock {
        tabs,
        explorer,
        shown: PanelView::Tabs,
    }
}

fn at(width: u32, dock: Dock) -> Frame {
    frame((width, 600), 1.0, false, PAD, metrics(1.0), dock)
}

fn slot(side: Side, x: f32, w: f32, view: PanelView, shared: bool) -> Option<Slot> {
    Some(Slot {
        side,
        rect: rect(x, 35.0, w, 565.0),
        view,
        shared,
    })
}

fn framed(frame: Frame, window: Option<&WindowLayout>) -> ChromeLayout {
    let theme = Config::default().theme();
    let status = parts();
    let input = ChromeInput {
        px: (800, 600),
        scale: 1.0,
        padding: PAD,
        zen: false,
        metrics: metrics(1.0),
        ui: ui(1.0),
        theme: &theme,
        status: &status,
        prompt: None,
        layout: window,
        hover: None,
        overlays: None,
        notice: None,
        isles: None,
        frame,
        panels: None,
    };
    layout(&input, measure)
}

#[test]
fn the_top_position_keeps_the_whole_window_for_the_grid() {
    let frame = at(800, Dock::default());
    assert_eq!(frame.left, None);
    assert_eq!(frame.right, None);
    assert!(frame.bar_tabs);
    assert_eq!(frame.grid, rect(0.0, 35.0, 800.0, 565.0));
    assert_eq!(
        grid_size(&frame, PAD, metrics(1.0)),
        GridSize { cols: 98, rows: 34 }
    );
    assert_eq!(grid_origin(&frame, PAD, metrics(1.0)), (8.0, 52.0));
}

#[test]
fn a_left_panel_shifts_and_shrinks_the_grid() {
    let frame = at(800, docked(Some((Side::Left, 200.0)), None));
    assert_eq!(
        frame.left,
        slot(Side::Left, 0.0, 200.0, PanelView::Tabs, false)
    );
    assert_eq!(frame.right, None);
    assert!(!frame.bar_tabs);
    assert_eq!(frame.grid, rect(200.0, 35.0, 600.0, 565.0));
    assert_eq!(
        grid_size(&frame, PAD, metrics(1.0)),
        GridSize { cols: 73, rows: 34 }
    );
    let origin = grid_origin(&frame, PAD, metrics(1.0));
    assert_eq!(origin, (208.0, 52.0));
    let cell = crate::mouse::to_cell(
        (208.0, 52.0),
        &metrics(1.0),
        origin,
        GridSize { cols: 73, rows: 34 },
    );
    assert_eq!(cell, CellPoint { col: 0, row: 0 });
}

#[test]
fn a_right_panel_keeps_the_grid_at_the_left_edge() {
    let frame = at(800, docked(Some((Side::Right, 240.0)), None));
    assert_eq!(
        frame.right,
        slot(Side::Right, 560.0, 240.0, PanelView::Tabs, false)
    );
    assert_eq!(frame.grid, rect(0.0, 35.0, 560.0, 565.0));
    assert_eq!(grid_size(&frame, PAD, metrics(1.0)).cols, 68);
}

#[test]
fn tabs_and_explorer_on_different_sides_get_a_panel_each() {
    let frame = at(
        1200,
        docked(Some((Side::Left, 200.0)), Some((Side::Right, 280.0))),
    );
    assert_eq!(
        frame.left,
        slot(Side::Left, 0.0, 200.0, PanelView::Tabs, false)
    );
    assert_eq!(
        frame.right,
        slot(Side::Right, 920.0, 280.0, PanelView::Explorer, false)
    );
    assert_eq!(frame.grid, rect(200.0, 35.0, 720.0, 565.0));
}

#[test]
fn a_shared_side_uses_one_slot_with_the_explorer_width_and_the_shown_view() {
    let mut dock = docked(Some((Side::Left, 200.0)), Some((Side::Left, 280.0)));
    let frame = at(1200, dock);
    assert_eq!(
        frame.left,
        slot(Side::Left, 0.0, 280.0, PanelView::Tabs, true)
    );
    assert_eq!(frame.right, None);
    assert!(!frame.bar_tabs);
    dock.shown = PanelView::Explorer;
    let switched = at(1200, dock);
    assert_eq!(
        switched.left,
        slot(Side::Left, 0.0, 280.0, PanelView::Explorer, true)
    );
    assert_eq!(switched.grid, frame.grid);
}

#[test]
fn zen_hides_every_panel() {
    let dock = docked(Some((Side::Left, 200.0)), Some((Side::Right, 280.0)));
    let frame = frame((800, 600), 1.0, true, (0.0, 0.0), metrics(1.0), dock);
    assert_eq!(frame.left, None);
    assert_eq!(frame.right, None);
    assert_eq!(frame.grid, rect(0.0, 0.0, 800.0, 600.0));
}

#[test]
fn a_wide_panel_is_clamped_to_keep_the_minimum_grid_columns() {
    let frame = at(600, docked(Some((Side::Left, 480.0)), None));
    assert_eq!(frame.left.map(|slot| slot.rect.w), Some(424.0));
    assert_eq!(grid_size(&frame, PAD, metrics(1.0)).cols, MIN_GRID_COLS);
    assert_eq!(max_width(&frame, Side::Left, PAD, metrics(1.0), 1.0), 424.0);
    let roomy = at(1200, docked(Some((Side::Left, 200.0)), None));
    assert_eq!(
        max_width(&roomy, Side::Left, PAD, metrics(1.0), 1.0),
        1024.0
    );
    assert_eq!(
        max_width(&roomy, Side::Right, PAD, metrics(1.0), 1.0),
        824.0
    );
}

#[test]
fn a_narrow_window_drops_the_explorer_first_then_the_tabs() {
    let dock = docked(Some((Side::Left, 200.0)), Some((Side::Right, 280.0)));
    let narrow = at(400, dock);
    assert_eq!(
        narrow.left,
        slot(Side::Left, 0.0, 200.0, PanelView::Tabs, false)
    );
    assert_eq!(narrow.right, None);
    assert!(!narrow.bar_tabs);
    let tiny = at(250, dock);
    assert_eq!(tiny.left, None);
    assert_eq!(tiny.right, None);
    assert!(tiny.bar_tabs);
    assert_eq!(tiny.grid, rect(0.0, 35.0, 250.0, 565.0));
}

#[test]
fn extend_and_clip_follow_the_grid_rect() {
    let frame = at(800, docked(Some((Side::Left, 200.0)), None));
    let layout = framed(frame, None);
    assert_eq!(layout.chrome.extend, rect(200.0, 35.0, 600.0, 565.0));
    assert_eq!(layout.chrome.clip, rect(208.0, 39.0, 584.0, 557.0));
    assert_eq!(layout.chrome.origin, (208.0, 52.0));
    assert!(
        layout
            .chrome
            .text
            .iter()
            .all(|run| run.text != "editor" || run.x < 200.0)
    );
}

#[test]
fn the_bar_drops_its_tabs_while_a_panel_lists_them() {
    let layout = framed(at(800, docked(Some((Side::Right, 200.0)), None)), None);
    assert!(
        layout
            .hits
            .iter()
            .all(|hit| !matches!(hit.target, HitTarget::Window(_)))
    );
    assert!(
        layout
            .hits
            .iter()
            .any(|hit| hit.target == HitTarget::Tab(WindowId(2)))
    );
    assert_eq!(layout.pages.tabs, 17);
}

#[test]
fn separators_stay_inside_the_grid_beside_a_panel() {
    let separators = [Separator {
        axis: Axis::Horizontal,
        rect: cells(30, 0, 1, 34),
    }];
    let panes = [(1, cells(0, 0, 30, 34)), (2, cells(31, 0, 42, 34))];
    let window = window(&separators, &panes, 2);
    let frame = at(800, docked(Some((Side::Left, 200.0)), None));
    let layout = framed(frame, Some(&window));
    let outline = opaque(Config::default().theme().surfaces().outline);
    let accent = opaque(Config::default().theme().accent);
    let lines: Vec<PxRect> = layout
        .chrome
        .quads
        .iter()
        .filter(|quad| quad.color == accent && quad.rect.x >= 200.0)
        .map(|quad| quad.rect)
        .collect();
    assert_eq!(lines, [rect(452.0, 35.0, 1.0, 565.0)]);
    assert!(
        layout
            .chrome
            .quads
            .iter()
            .any(|quad| quad.color == outline && quad.rect == rect(199.0, 35.0, 1.0, 565.0))
    );
}
