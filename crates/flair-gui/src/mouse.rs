use crate::CellMetrics;
use flair_core::geom::{CellPoint, GridSize};
use winit::event::MouseScrollDelta;

pub fn to_cell(
    pos: (f64, f64),
    metrics: &CellMetrics,
    origin: (f32, f32),
    size: GridSize,
) -> CellPoint {
    CellPoint {
        col: (((pos.0 - f64::from(origin.0)) / f64::from(metrics.width))
            .floor()
            .max(0.0) as u16)
            .min(size.cols.saturating_sub(1)),
        row: (((pos.1 - f64::from(origin.1)) / f64::from(metrics.height))
            .floor()
            .max(0.0) as u16)
            .min(size.rows.saturating_sub(1)),
    }
}

#[derive(Default)]
pub struct WheelAccumulator {
    lines: f64,
}
impl WheelAccumulator {
    pub fn push(&mut self, delta: MouseScrollDelta, line_px: f32) -> i32 {
        self.lines += match delta {
            MouseScrollDelta::LineDelta(_, y) => f64::from(y),
            MouseScrollDelta::PixelDelta(pos) => pos.y / f64::from(line_px.max(1.0)),
        };
        let whole = self.lines.trunc() as i32;
        self.lines -= f64::from(whole);
        whole
    }
}

#[cfg(test)]
#[path = "mouse_tests.rs"]
mod tests;
