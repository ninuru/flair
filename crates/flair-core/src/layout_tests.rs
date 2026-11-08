use super::*;

fn rect(cols: u16, rows: u16) -> CellRect {
    CellRect {
        origin: CellPoint { col: 0, row: 1 },
        size: GridSize { cols, rows },
    }
}

#[test]
fn side_by_side_split_reserves_one_separator_cell() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, rect(21, 8)));
    let geometry = layout.geometry(rect(21, 8));
    assert_eq!(geometry.panes.len(), 2);
    assert_eq!(geometry.panes[0].rect, rect(10, 8));
    assert_eq!(geometry.panes[1].rect.origin, CellPoint { col: 11, row: 1 });
    assert_eq!(geometry.panes[1].rect.size, GridSize { cols: 10, rows: 8 });
    assert_eq!(geometry.separators.len(), 1);
}

#[test]
fn stacked_split_keeps_global_content_offset() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Vertical, rect(20, 9)));
    let geometry = layout.geometry(rect(20, 9));
    assert_eq!(geometry.panes[0].rect, rect(20, 4));
    assert_eq!(geometry.panes[1].rect.origin, CellPoint { col: 0, row: 6 });
    assert_eq!(geometry.panes[1].rect.size, GridSize { cols: 20, rows: 4 });
}

#[test]
fn nested_removal_collapses_only_the_removed_branch() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, rect(41, 9)));
    assert!(layout.split(PaneId(2), PaneId(3), Axis::Vertical, rect(41, 9)));
    let layout = layout.remove(PaneId(2)).unwrap();
    let geometry = layout.geometry(rect(41, 9));
    assert_eq!(geometry.panes.len(), 2);
    assert_eq!(geometry.panes[1].pane, PaneId(3));
    assert_eq!(geometry.panes[1].rect.size, GridSize { cols: 20, rows: 9 });
}

#[test]
fn split_rejects_geometry_that_cannot_fit_two_leaves() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(!layout.split(PaneId(1), PaneId(2), Axis::Horizontal, rect(2, 10)));
    assert_eq!(layout, Layout::Leaf(PaneId(1)));
}

#[test]
fn directional_navigation_uses_adjacent_pane_geometry() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, rect(41, 9)));
    assert!(layout.split(PaneId(2), PaneId(3), Axis::Vertical, rect(41, 9)));
    assert_eq!(
        layout.neighbor(PaneId(1), Direction::Right, rect(41, 9)),
        Some(PaneId(2))
    );
    assert_eq!(
        layout.neighbor(PaneId(2), Direction::Down, rect(41, 9)),
        Some(PaneId(3))
    );
    assert_eq!(
        layout.neighbor(PaneId(3), Direction::Left, rect(41, 9)),
        Some(PaneId(1))
    );
    assert_eq!(
        layout.neighbor(PaneId(1), Direction::Left, rect(41, 9)),
        None
    );
}

#[test]
fn nested_rectangles_stay_disjoint_and_within_available_area() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, rect(41, 9)));
    assert!(layout.split(PaneId(2), PaneId(3), Axis::Vertical, rect(41, 9)));
    for cols in 0..50 {
        for rows in 0..20 {
            let area = rect(cols, rows);
            let geometry = layout.geometry(area);
            let mut occupied = std::collections::BTreeSet::new();
            for placement in geometry.panes {
                for row in
                    placement.rect.origin.row..placement.rect.origin.row + placement.rect.size.rows
                {
                    for col in placement.rect.origin.col
                        ..placement.rect.origin.col + placement.rect.size.cols
                    {
                        let point = CellPoint { col, row };
                        assert!(area.contains(point));
                        assert!(occupied.insert((col, row)));
                    }
                }
            }
            for separator in geometry.separators {
                for row in
                    separator.rect.origin.row..separator.rect.origin.row + separator.rect.size.rows
                {
                    for col in separator.rect.origin.col
                        ..separator.rect.origin.col + separator.rect.size.cols
                    {
                        assert!(area.contains(CellPoint { col, row }));
                        assert!(occupied.insert((col, row)));
                    }
                }
            }
            assert_eq!(occupied.len(), usize::from(cols) * usize::from(rows));
        }
    }
}

#[test]
fn split_rejects_unknown_targets_and_duplicate_panes_without_changes() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(!layout.split(PaneId(9), PaneId(2), Axis::Horizontal, rect(21, 8)));
    assert!(!layout.split(PaneId(1), PaneId(1), Axis::Horizontal, rect(21, 8)));
    assert_eq!(layout, Layout::Leaf(PaneId(1)));
}

#[test]
fn removing_unknown_pane_preserves_layout_and_last_leaf_ends_it() {
    let layout = Layout::Leaf(PaneId(1));
    assert_eq!(layout.clone().remove(PaneId(9)), Some(layout.clone()));
    assert_eq!(layout.remove(PaneId(1)), None);
}

#[test]
fn extreme_ratios_preserve_nested_leaf_minimum_sizes_when_they_fit() {
    let layout = Layout::Split {
        axis: Axis::Horizontal,
        ratio: 0,
        first: Box::new(Layout::Split {
            axis: Axis::Horizontal,
            ratio: u16::MAX,
            first: Box::new(Layout::Leaf(PaneId(1))),
            second: Box::new(Layout::Leaf(PaneId(2))),
        }),
        second: Box::new(Layout::Leaf(PaneId(3))),
    };
    let geometry = layout.geometry(rect(5, 1));
    assert_eq!(
        geometry
            .panes
            .iter()
            .map(|pane| pane.rect.size.cols)
            .collect::<Vec<_>>(),
        vec![1, 1, 1]
    );
    assert_eq!(layout.first_pane(), PaneId(1));
    assert!(layout.contains(PaneId(3)));
    assert!(!layout.contains(PaneId(4)));
}

#[test]
fn hit_testing_distinguishes_panes_separators_and_status_row() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, rect(21, 8)));
    assert_eq!(
        layout.hit_test(rect(21, 8), CellPoint { col: 9, row: 1 }),
        Some(PaneId(1))
    );
    assert_eq!(
        layout.hit_test(rect(21, 8), CellPoint { col: 11, row: 8 }),
        Some(PaneId(2))
    );
    assert_eq!(
        layout.hit_test(rect(21, 8), CellPoint { col: 10, row: 4 }),
        None
    );
    assert_eq!(
        layout.hit_test(rect(21, 8), CellPoint { col: 5, row: 0 }),
        None
    );
    assert_eq!(
        layout.separator_at(rect(21, 8), CellPoint { col: 10, row: 4 }),
        Some(vec![])
    );
    assert_eq!(
        layout.separator_at(rect(21, 8), CellPoint { col: 9, row: 4 }),
        None
    );
    assert_eq!(
        layout.separator_at(rect(21, 8), CellPoint { col: 10, row: 0 }),
        None
    );
}

#[test]
fn horizontal_separator_resize_reaches_exact_requested_column_and_clamps_edges() {
    let area = rect(21, 8);
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, area));
    assert!(layout.resize_separator(&[], area, CellPoint { col: 7, row: 4 }));
    assert_eq!(layout.geometry(area).separators[0].rect.origin.col, 7);
    assert!(!layout.resize_separator(&[], area, CellPoint { col: 7, row: 4 }));
    assert!(layout.resize_separator(&[], area, CellPoint { col: 0, row: 4 }));
    assert_eq!(layout.geometry(area).panes[0].rect.size.cols, 1);
    assert!(layout.resize_separator(
        &[],
        area,
        CellPoint {
            col: u16::MAX,
            row: 4
        }
    ));
    assert_eq!(layout.geometry(area).panes[1].rect.size.cols, 1);
}

#[test]
fn vertical_separator_resize_uses_global_row_offset() {
    let area = CellRect {
        origin: CellPoint { col: 3, row: 4 },
        size: GridSize { cols: 20, rows: 11 },
    };
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Vertical, area));
    assert!(layout.resize_separator(&[], area, CellPoint { col: 5, row: 7 }));
    let geometry = layout.geometry(area);
    assert_eq!(geometry.panes[0].rect.size.rows, 3);
    assert_eq!(geometry.separators[0].rect.origin.row, 7);
    assert_eq!(geometry.panes[1].rect.size.rows, 7);
}

#[test]
fn nested_separator_path_resizes_only_its_subtree() {
    let area = rect(41, 11);
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, area));
    assert!(layout.split(PaneId(2), PaneId(3), Axis::Vertical, area));
    let before = layout.geometry(area);
    let point = CellPoint { col: 30, row: 6 };
    assert_eq!(layout.separator_at(area, point), Some(vec![true]));
    assert!(layout.resize_separator(&[true], area, CellPoint { col: 30, row: 3 }));
    let after = layout.geometry(area);
    assert_eq!(after.panes[0], before.panes[0]);
    assert_eq!(after.separators[0], before.separators[0]);
    assert_eq!(after.panes[1].rect.size.rows, 2);
    assert_eq!(after.panes[2].rect.origin.row, 4);
}

#[test]
fn separator_resize_preserves_recursive_minimum_sizes() {
    let area = rect(21, 8);
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, area));
    assert!(layout.split(PaneId(1), PaneId(3), Axis::Horizontal, area));
    assert!(layout.resize_separator(&[], area, CellPoint { col: 0, row: 2 }));
    let geometry = layout.geometry(area);
    assert_eq!(geometry.panes[0].rect.size.cols, 1);
    assert_eq!(geometry.panes[1].rect.size.cols, 1);
    assert_eq!(geometry.separators[1].rect.origin.col, 3);
}

#[test]
fn separator_resize_rejects_invalid_paths_and_undersized_areas_without_changes() {
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(!layout.resize_separator(&[], rect(21, 8), CellPoint { col: 5, row: 2 }));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, rect(21, 8)));
    let before = layout.clone();
    assert!(!layout.resize_separator(&[false], rect(21, 8), CellPoint { col: 5, row: 2 }));
    assert!(!layout.resize_separator(&[], rect(2, 8), CellPoint { col: 0, row: 2 }));
    assert_eq!(layout, before);
}

#[test]
fn resizing_at_current_separator_preserves_fractional_ratio() {
    let area = rect(20, 8);
    let mut layout = Layout::Leaf(PaneId(1));
    assert!(layout.split(PaneId(1), PaneId(2), Axis::Horizontal, area));
    let before = layout.clone();
    let separator = layout.geometry(area).separators[0].rect.origin;
    assert!(!layout.resize_separator(&[], area, separator));
    assert_eq!(layout, before);
}
