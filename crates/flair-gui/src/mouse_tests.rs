use super::*;
#[test]
fn coordinates_start_at_the_grid_origin_below_the_bar() {
    let metrics = CellMetrics {
        width: 10.0,
        height: 20.0,
        baseline: 15.0,
    };
    let size = GridSize { cols: 80, rows: 24 };
    let origin = (16.0, 43.0);
    assert_eq!(
        to_cell((16.0, 43.0), &metrics, origin, size),
        CellPoint { col: 0, row: 0 }
    );
    assert_eq!(
        to_cell((35.0, 84.0), &metrics, origin, size),
        CellPoint { col: 1, row: 2 }
    );
    assert_eq!(
        to_cell((35.0, 62.9), &metrics, origin, size),
        CellPoint { col: 1, row: 0 }
    );
    assert_eq!(
        to_cell((35.0, 10.0), &metrics, origin, size),
        CellPoint { col: 1, row: 0 }
    );
}
#[test]
fn coordinates_clamp_and_account_for_padding() {
    let metrics = CellMetrics {
        width: 10.0,
        height: 20.0,
        baseline: 15.0,
    };
    let size = GridSize { cols: 80, rows: 24 };
    assert_eq!(
        to_cell((35.0, 49.0), &metrics, (16.0, 8.0), size),
        CellPoint { col: 1, row: 2 }
    );
    assert_eq!(
        to_cell((-1.0, 9999.0), &metrics, (16.0, 8.0), size),
        CellPoint { col: 0, row: 23 }
    );
}
#[test]
fn fractional_wheel_accumulates() {
    let mut wheel = WheelAccumulator::default();
    assert_eq!(
        wheel.push(
            MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0.0, 6.0)),
            10.0
        ),
        0
    );
    assert_eq!(
        wheel.push(
            MouseScrollDelta::PixelDelta(winit::dpi::PhysicalPosition::new(0.0, 6.0)),
            10.0
        ),
        1
    );
}
