use super::*;
use crate::{
    Fonts,
    renderer::{HeadlessContext, Renderer},
};
use flair_core::{config::Config, status::StatusWindow};
use std::{fs, io::Write};

#[test]
fn status_row_preserves_format_and_theme_on_black() {
    let theme = Config::default().theme();
    let status = fixture();
    let row = compose_status(&status, 40, &theme);
    assert_eq!(
        row.grid.to_lines(),
        [" 1:shell 2:editor                | main"]
    );
    assert_eq!(row.grid.cells.len(), 40);
    assert!(row.grid.cursor.is_none());
    assert!(row.grid.cells.iter().all(|cell| cell.bg == Rgb::default()));
    assert!(
        row.grid
            .cells
            .iter()
            .all(|cell| cell.fg == theme.foreground)
    );
    assert_eq!(row.grid.cells[39].c, ' ');
}

#[test]
fn only_visible_window_entries_are_clickable() {
    let theme = Config::default().theme();
    let row = compose_status(&fixture(), 40, &theme);
    for col in 0..8 {
        assert_eq!(row.hit_test(CellPoint { col, row: 0 }), Some(WindowId(1)));
    }
    for col in 8..17 {
        assert_eq!(row.hit_test(CellPoint { col, row: 0 }), Some(WindowId(2)));
    }
    for col in 17..=40 {
        assert_eq!(row.hit_test(CellPoint { col, row: 0 }), None);
    }
    assert_eq!(row.hit_test(CellPoint { col: 2, row: 1 }), None);
}

#[test]
fn active_entry_and_prefix_are_per_view() {
    let theme = Config::default().theme();
    let first = fixture();
    let second = second_view(&first);
    let before = compose_status(&first, 40, &theme);
    let other = compose_status(&second, 40, &theme);
    assert!(before.grid.cells[1].flags.contains(CellFlags::BOLD));
    assert!(before.grid.cells[1].flags.contains(CellFlags::UNDERLINE));
    assert!(!before.grid.cells[9].flags.contains(CellFlags::BOLD));
    assert!(!other.grid.cells[1].flags.contains(CellFlags::BOLD));
    assert!(other.grid.cells[9].flags.contains(CellFlags::BOLD));
    assert!(other.grid.to_lines()[0].ends_with("^A"));
    assert!(!before.grid.to_lines()[0].contains("^A"));
    assert_eq!(compose_status(&first, 40, &theme).grid, before.grid);
}

#[test]
fn narrow_grids_keep_cells_and_targets_in_bounds() {
    let theme = Config::default().theme();
    for status in [fixture(), second_view(&fixture())] {
        for cols in 0..=40 {
            let row = compose_status(&status, cols, &theme);
            assert_eq!(row.grid.size, GridSize { cols, rows: 1 });
            assert_eq!(row.grid.cells.len(), usize::from(cols));
            assert_eq!(row.hit_test(CellPoint { col: cols, row: 0 }), None);
            for target in &row.windows {
                assert!(target.start < target.end);
                assert!(target.end <= cols);
            }
            for (index, cell) in row.grid.cells.iter().enumerate() {
                if cell.flags.contains(CellFlags::WIDE) {
                    assert!(
                        row.grid.cells[index + 1]
                            .flags
                            .contains(CellFlags::WIDE_SPACER)
                    );
                }
                if cell.flags.contains(CellFlags::WIDE_SPACER) {
                    assert!(index > 0 && row.grid.cells[index - 1].flags.contains(CellFlags::WIDE));
                }
            }
        }
    }
}

#[test]
fn gpu_renders_status_with_real_font_and_black_surface() {
    let (ctx, gl) = match HeadlessContext::new((800, 120)) {
        Ok(pair) => pair,
        Err(error) => {
            if std::env::var_os("FLAIR_REQUIRE_EGL").is_some() {
                panic!("required EGL unavailable: {error}");
            }
            eprintln!("skipped status EGL smoke: {error}");
            return;
        }
    };
    let config = Config::default();
    let theme = config.theme();
    let mut fonts = Fonts::load(&config.font, 1.0).unwrap();
    let metrics = fonts.metrics();
    let mut renderer = Renderer::new(gl, metrics).unwrap();
    renderer.resize((800, 120));
    let cols = (768.0 / metrics.width).floor() as u16;
    let row = compose_status(&second_view(&fixture()), cols, &theme);
    renderer
        .draw(&row.grid, &mut fonts, (16.0, 8.0), theme.background)
        .unwrap();
    let pixels = renderer.read_pixels();
    let pixel =
        |x: usize, y: usize| &pixels[((119 - y) * 800 + x) * 4..((119 - y) * 800 + x) * 4 + 3];
    assert_eq!(pixel(16, 8), &[0, 0, 0]);
    assert_eq!(pixel(16, 8 + metrics.height as usize + 2), &[3, 3, 3]);
    for (start, end) in [(1, 8), (usize::from(cols) - 2, usize::from(cols))] {
        let lit = (8..8 + metrics.height as usize).any(|y| {
            ((16.0 + start as f32 * metrics.width) as usize
                ..(16.0 + end as f32 * metrics.width) as usize)
                .any(|x| pixel(x, y).iter().any(|value| *value > 30))
        });
        assert!(lit, "status glyphs missing in columns {start}..{end}");
    }
    if let Some(path) = std::env::var_os("FLAIR_STATUS_PPM") {
        let mut file = fs::File::create(path).unwrap();
        write!(file, "P6\n800 120\n255\n").unwrap();
        for y in 0..120 {
            for x in 0..800 {
                file.write_all(pixel(x, y)).unwrap();
            }
        }
    }
    drop(renderer);
    drop(ctx);
}

fn fixture() -> StatusLine {
    StatusLine {
        session: "main".into(),
        hidden: 0,
        running: false,
        finished: false,
        windows: vec![
            StatusWindow {
                id: WindowId(1),
                index: 1,
                name: "shell".into(),
                active: true,
                last: false,
            },
            StatusWindow {
                id: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: false,
                last: true,
            },
        ],
        pending: None,
        copy: false,
    }
}

fn second_view(status: &StatusLine) -> StatusLine {
    StatusLine {
        windows: status
            .windows
            .iter()
            .map(|window| StatusWindow {
                active: window.id == WindowId(2),
                ..window.clone()
            })
            .collect(),
        pending: Some("^A".into()),
        ..status.clone()
    }
}

#[test]
fn overflow_keeps_the_active_window_visible_and_hides_other_targets() {
    let theme = Config::default().theme();
    let status = StatusLine {
        windows: (1..=30)
            .map(|index| StatusWindow {
                id: WindowId(u32::from(index)),
                index,
                name: "shell".into(),
                active: index == 30,
                last: false,
            })
            .collect(),
        ..fixture()
    };
    let row = compose_status(&status, 24, &theme);
    assert!(row.grid.to_lines()[0].starts_with(" 30:shell"));
    assert_eq!(row.windows[0].window, WindowId(30));
    assert!(
        row.windows
            .iter()
            .all(|target| target.window != WindowId(1))
    );
}

#[test]
fn unicode_names_use_display_cells_and_complete_wide_glyphs() {
    let theme = Config::default().theme();
    let status = StatusLine {
        session: "会話".into(),
        windows: vec![StatusWindow {
            name: "界e\u{301}🙂long".into(),
            ..fixture().windows[0].clone()
        }],
        ..fixture()
    };
    let row = compose_status(&status, 30, &theme);
    assert!(row.grid.to_lines()[0].contains(" 1:界e\u{301}🙂long"));
    assert!(row.grid.to_lines()[0].ends_with("| 会話"));
    assert!(row.grid.cells[3].flags.contains(CellFlags::WIDE));
    assert!(row.grid.cells[4].flags.contains(CellFlags::WIDE_SPACER));
    assert_eq!(row.grid.cells[5].extra.as_deref(), Some("\u{301}"));
    assert_eq!(
        row.hit_test(CellPoint { col: 4, row: 0 }),
        Some(WindowId(1))
    );
    for cols in 0..=30 {
        let clipped = compose_status(&status, cols, &theme);
        for (index, cell) in clipped.grid.cells.iter().enumerate() {
            if cell.flags.contains(CellFlags::WIDE) {
                assert!(
                    clipped
                        .grid
                        .cells
                        .get(index + 1)
                        .is_some_and(|next| next.flags.contains(CellFlags::WIDE_SPACER))
                );
            }
        }
    }
}

#[test]
fn long_session_names_leave_room_for_active_entry_and_prefix() {
    let theme = Config::default().theme();
    let status = StatusLine {
        session: "long-session".repeat(30),
        ..second_view(&fixture())
    };
    let row = compose_status(&status, 24, &theme);
    assert!(row.grid.to_lines()[0].starts_with(" 2:editor"));
    assert!(row.grid.to_lines()[0].ends_with("^A"));
    assert_eq!(row.windows[0].window, WindowId(2));
    assert_eq!(row.hit_test(CellPoint { col: 23, row: 0 }), None);
}

#[test]
fn tiny_grids_prioritize_pending_and_never_offer_an_unlabelled_target() {
    let theme = Config::default().theme();
    for cols in 0..=5 {
        let row = compose_status(&second_view(&fixture()), cols, &theme);
        assert!(row.windows.is_empty());
        if cols >= 2 {
            assert!(row.grid.to_lines()[0].ends_with("^A"));
        }
    }
    for cols in 0..=3 {
        assert!(compose_status(&fixture(), cols, &theme).windows.is_empty());
    }
}

#[test]
fn copy_badge_is_bold_at_the_right_next_to_the_prefix_badge() {
    let theme = Config::default().theme();
    let copy = StatusLine {
        copy: true,
        ..fixture()
    };
    let row = compose_status(&copy, 40, &theme);
    let line = &row.grid.to_lines()[0];
    assert!(line.ends_with("| main  COPY"), "{line:?}");
    assert!(
        row.grid.cells[36..]
            .iter()
            .all(|cell| cell.flags.contains(CellFlags::BOLD))
    );
    let both = StatusLine {
        copy: true,
        ..second_view(&fixture())
    };
    let line = &compose_status(&both, 40, &theme).grid.to_lines()[0];
    assert!(line.ends_with(" COPY ^A"), "{line:?}");
    assert!(line.starts_with(" 1:shell 2:editor"));
    let line = &compose_status(&copy, 4, &theme).grid.to_lines()[0];
    assert_eq!(line, "COPY");
}

#[test]
fn status_parts_split_chip_tabs_and_badges() {
    let status = StatusLine {
        hidden: 2,
        running: true,
        finished: false,
        copy: true,
        ..second_view(&fixture())
    };
    let parts = status_parts(&status);
    assert_eq!(
        parts.chip,
        ChipParts {
            session: "main".into(),
            hidden: 2,
            running: true,
            finished: false,
        }
    );
    assert_eq!(
        parts.tabs,
        [
            TabParts {
                window: WindowId(1),
                index: 1,
                name: "shell".into(),
                active: false,
            },
            TabParts {
                window: WindowId(2),
                index: 2,
                name: "editor".into(),
                active: true,
            },
        ]
    );
    assert_eq!(
        parts.badges,
        [
            BadgeParts {
                kind: BadgeKind::Copy,
                text: "COPY".into(),
            },
            BadgeParts {
                kind: BadgeKind::Prefix,
                text: "^A".into(),
            },
        ]
    );
    assert!(status_parts(&fixture()).badges.is_empty());
}

#[test]
fn status_text_keeps_the_old_row_and_adds_chip_marks() {
    let text = status_text(&status_parts(&fixture()), 40);
    assert_eq!(text, " 1:shell 2:editor                | main ");
    let chip = StatusLine {
        hidden: 2,
        running: true,
        finished: true,
        ..fixture()
    };
    let text = status_text(&status_parts(&chip), 40);
    assert_eq!(text.chars().count(), 40);
    assert!(text.starts_with(" 1:shell 2:editor "), "{text:?}");
    assert!(text.ends_with("| main +2 * ! "), "{text:?}");
    let both = StatusLine {
        copy: true,
        ..second_view(&fixture())
    };
    let text = status_text(&status_parts(&both), 40);
    assert!(text.ends_with(" COPY ^A"), "{text:?}");
    for cols in 0..=40 {
        assert_eq!(
            status_text(&status_parts(&chip), cols).chars().count(),
            usize::from(cols)
        );
    }
}
