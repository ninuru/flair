use crate::{
    geom::{Axis, CellPoint, CellRect, Direction, GridSize},
    ids::PaneId,
    proto::{PanePlacement, Separator},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Layout {
    Leaf(PaneId),
    Split {
        axis: Axis,
        ratio: u16,
        first: Box<Layout>,
        second: Box<Layout>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Geometry {
    pub panes: Vec<PanePlacement>,
    pub separators: Vec<Separator>,
}

impl Layout {
    pub fn contains(&self, pane: PaneId) -> bool {
        match self {
            Self::Leaf(id) => *id == pane,
            Self::Split { first, second, .. } => first.contains(pane) || second.contains(pane),
        }
    }

    pub fn first_pane(&self) -> PaneId {
        match self {
            Self::Leaf(pane) => *pane,
            Self::Split { first, .. } => first.first_pane(),
        }
    }

    pub fn split(&mut self, pane: PaneId, new: PaneId, axis: Axis, area: CellRect) -> bool {
        if self.contains(new) {
            return false;
        }
        let Some(placement) = self
            .geometry(area)
            .panes
            .into_iter()
            .find(|item| item.pane == pane)
        else {
            return false;
        };
        let size = placement.rect.size;
        let fits = match axis {
            Axis::Horizontal => size.cols >= 3 && size.rows >= 1,
            Axis::Vertical => size.rows >= 3 && size.cols >= 1,
        };
        fits && self.split_leaf(pane, new, axis)
    }

    fn split_leaf(&mut self, pane: PaneId, new: PaneId, axis: Axis) -> bool {
        match self {
            Self::Leaf(id) => {
                if *id != pane {
                    return false;
                }
                *self = Self::Split {
                    axis,
                    ratio: 32768,
                    first: Box::new(Self::Leaf(pane)),
                    second: Box::new(Self::Leaf(new)),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.split_leaf(pane, new, axis) || second.split_leaf(pane, new, axis)
            }
        }
    }

    pub fn remove(self, pane: PaneId) -> Option<Layout> {
        match self {
            Self::Leaf(id) => (id != pane).then_some(Self::Leaf(id)),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => match (first.remove(pane), second.remove(pane)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    axis,
                    ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (Some(remaining), None) | (None, Some(remaining)) => Some(remaining),
                (None, None) => None,
            },
        }
    }

    pub fn geometry(&self, area: CellRect) -> Geometry {
        let mut geometry = Geometry::default();
        self.place(area, &mut geometry);
        geometry
    }

    pub fn hit_test(&self, area: CellRect, point: CellPoint) -> Option<PaneId> {
        self.geometry(area)
            .panes
            .into_iter()
            .find(|placement| placement.rect.contains(point))
            .map(|placement| placement.pane)
    }

    pub fn separator_at(&self, area: CellRect, point: CellPoint) -> Option<Vec<bool>> {
        if !area.contains(point) {
            return None;
        }
        let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        else {
            return None;
        };
        let (first_area, separator, second_area) = split_rect(
            area,
            *axis,
            *ratio,
            first.minimum_size(),
            second.minimum_size(),
        );
        if separator.contains(point) {
            return Some(Vec::new());
        }
        if let Some(mut path) = first.separator_at(first_area, point) {
            path.insert(0, false);
            return Some(path);
        }
        let mut path = second.separator_at(second_area, point)?;
        path.insert(0, true);
        Some(path)
    }

    pub fn resize_separator(&mut self, path: &[bool], area: CellRect, point: CellPoint) -> bool {
        let minimum = self.minimum_size();
        if area.size.cols < minimum.cols || area.size.rows < minimum.rows {
            return false;
        }
        let Self::Split {
            axis,
            ratio,
            first,
            second,
        } = self
        else {
            return false;
        };
        let first_minimum = first.minimum_size();
        let second_minimum = second.minimum_size();
        if let Some((branch, remaining)) = path.split_first() {
            let (first_area, _, second_area) =
                split_rect(area, *axis, *ratio, first_minimum, second_minimum);
            return if *branch {
                second.resize_separator(remaining, second_area, point)
            } else {
                first.resize_separator(remaining, first_area, point)
            };
        }
        let (length, offset, first_minimum, second_minimum) = match axis {
            Axis::Horizontal => (
                area.size.cols,
                point.col.saturating_sub(area.origin.col),
                first_minimum.cols,
                second_minimum.cols,
            ),
            Axis::Vertical => (
                area.size.rows,
                point.row.saturating_sub(area.origin.row),
                first_minimum.rows,
                second_minimum.rows,
            ),
        };
        let available = length - 1;
        let desired = offset.clamp(first_minimum, available - second_minimum);
        let current = ((u32::from(available) * u32::from(*ratio) / 65536) as u16)
            .clamp(first_minimum, available - second_minimum);
        if desired == current {
            return false;
        }
        let next = (u32::from(desired) * 65536).div_ceil(u32::from(available)) as u16;
        if next == *ratio {
            return false;
        }
        *ratio = next;
        true
    }

    fn place(&self, area: CellRect, geometry: &mut Geometry) {
        match self {
            Self::Leaf(pane) => geometry.panes.push(PanePlacement {
                pane: *pane,
                rect: area,
            }),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let (first_area, separator, second_area) = split_rect(
                    area,
                    *axis,
                    *ratio,
                    first.minimum_size(),
                    second.minimum_size(),
                );
                first.place(first_area, geometry);
                geometry.separators.push(Separator {
                    axis: *axis,
                    rect: separator,
                });
                second.place(second_area, geometry);
            }
        }
    }

    fn minimum_size(&self) -> GridSize {
        match self {
            Self::Leaf(_) => GridSize { cols: 1, rows: 1 },
            Self::Split {
                axis,
                first,
                second,
                ..
            } => {
                let first = first.minimum_size();
                let second = second.minimum_size();
                match axis {
                    Axis::Horizontal => GridSize {
                        cols: first.cols.saturating_add(second.cols).saturating_add(1),
                        rows: first.rows.max(second.rows),
                    },
                    Axis::Vertical => GridSize {
                        cols: first.cols.max(second.cols),
                        rows: first.rows.saturating_add(second.rows).saturating_add(1),
                    },
                }
            }
        }
    }

    pub fn neighbor(&self, pane: PaneId, direction: Direction, area: CellRect) -> Option<PaneId> {
        let geometry = self.geometry(area);
        let source = geometry.panes.iter().find(|item| item.pane == pane)?.rect;
        geometry
            .panes
            .iter()
            .filter(|candidate| candidate.pane != pane)
            .filter_map(|candidate| {
                neighbor_distance(source, candidate.rect, direction)
                    .map(|distance| ((distance, candidate.pane), candidate.pane))
            })
            .min_by_key(|(score, _)| *score)
            .map(|(_, pane)| pane)
    }
}

fn split_rect(
    area: CellRect,
    axis: Axis,
    ratio: u16,
    first_minimum: GridSize,
    second_minimum: GridSize,
) -> (CellRect, CellRect, CellRect) {
    let (length, first_minimum, second_minimum) = match axis {
        Axis::Horizontal => (area.size.cols, first_minimum.cols, second_minimum.cols),
        Axis::Vertical => (area.size.rows, first_minimum.rows, second_minimum.rows),
    };
    let separator = length.min(1);
    let available = length - separator;
    let ideal = (u32::from(available) * u32::from(ratio) / 65536) as u16;
    let first_length =
        if u32::from(first_minimum) + u32::from(second_minimum) <= u32::from(available) {
            ideal.clamp(first_minimum, available - second_minimum)
        } else {
            ideal
        };
    let second_length = available - first_length;
    match axis {
        Axis::Horizontal => (
            CellRect {
                size: GridSize {
                    cols: first_length,
                    ..area.size
                },
                ..area
            },
            CellRect {
                origin: CellPoint {
                    col: area.origin.col.saturating_add(first_length),
                    ..area.origin
                },
                size: GridSize {
                    cols: separator,
                    ..area.size
                },
            },
            CellRect {
                origin: CellPoint {
                    col: area
                        .origin
                        .col
                        .saturating_add(first_length)
                        .saturating_add(separator),
                    ..area.origin
                },
                size: GridSize {
                    cols: second_length,
                    ..area.size
                },
            },
        ),
        Axis::Vertical => (
            CellRect {
                size: GridSize {
                    rows: first_length,
                    ..area.size
                },
                ..area
            },
            CellRect {
                origin: CellPoint {
                    row: area.origin.row.saturating_add(first_length),
                    ..area.origin
                },
                size: GridSize {
                    rows: separator,
                    ..area.size
                },
            },
            CellRect {
                origin: CellPoint {
                    row: area
                        .origin
                        .row
                        .saturating_add(first_length)
                        .saturating_add(separator),
                    ..area.origin
                },
                size: GridSize {
                    rows: second_length,
                    ..area.size
                },
            },
        ),
    }
}

fn neighbor_distance(
    source: CellRect,
    target: CellRect,
    direction: Direction,
) -> Option<(u32, u32)> {
    if source.size.cols == 0
        || source.size.rows == 0
        || target.size.cols == 0
        || target.size.rows == 0
    {
        return None;
    }
    let x = u32::from(source.origin.col);
    let y = u32::from(source.origin.row);
    let right = x + u32::from(source.size.cols);
    let bottom = y + u32::from(source.size.rows);
    let tx = u32::from(target.origin.col);
    let ty = u32::from(target.origin.row);
    let tright = tx + u32::from(target.size.cols);
    let tbottom = ty + u32::from(target.size.rows);
    match direction {
        Direction::Left if tright <= x && ty < bottom && y < tbottom => {
            Some((x - tright, (y + bottom).abs_diff(ty + tbottom)))
        }
        Direction::Right if tx >= right && ty < bottom && y < tbottom => {
            Some((tx - right, (y + bottom).abs_diff(ty + tbottom)))
        }
        Direction::Up if tbottom <= y && tx < right && x < tright => {
            Some((y - tbottom, (x + right).abs_diff(tx + tright)))
        }
        Direction::Down if ty >= bottom && tx < right && x < tright => {
            Some((ty - bottom, (x + right).abs_diff(tx + tright)))
        }
        Direction::Left | Direction::Right | Direction::Up | Direction::Down => None,
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
