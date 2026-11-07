use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct GridSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CellPoint {
    pub col: u16,
    pub row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct CellRect {
    pub origin: CellPoint,
    pub size: GridSize,
}

impl CellRect {
    pub fn contains(self, point: CellPoint) -> bool {
        point.col >= self.origin.col
            && point.row >= self.origin.row
            && u32::from(point.col) < u32::from(self.origin.col) + u32::from(self.size.cols)
            && u32::from(point.row) < u32::from(self.origin.row) + u32::from(self.size.rows)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[cfg(test)]
#[path = "geom_tests.rs"]
mod tests;
