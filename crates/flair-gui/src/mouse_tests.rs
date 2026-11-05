use super::*;
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
