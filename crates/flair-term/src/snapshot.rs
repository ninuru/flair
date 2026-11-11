use crate::Terminal;
use alacritty_terminal::{
    index::{Column, Line, Point},
    term::cell::Flags,
    vte::ansi::{Color as AnsiColor, CursorShape as AnsiCursor, NamedColor},
};
use flair_core::{
    geom::CellPoint,
    ids::PaneId,
    proto::{Cell, CellFlags, Color, Cursor, CursorShape, PaneFlags, PaneUpdate, SelectionSpan},
};

impl Terminal {
    fn selection_spans(&self) -> Vec<SelectionSpan> {
        let Some(range) = self
            .term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term))
        else {
            return Vec::new();
        };
        let offset = self.term.grid().display_offset() as i32;
        (0..self.size.rows)
            .filter_map(|row| {
                let line = Line(i32::from(row) - offset);
                if line < range.start.line || line > range.end.line || self.chrome_line(line) {
                    return None;
                }
                let start = if range.is_block || line == range.start.line {
                    range.start.column.0
                } else {
                    0
                };
                let end = if range.is_block || line == range.end.line {
                    range.end.column.0
                } else {
                    usize::from(self.size.cols - 1)
                };
                Some(SelectionSpan {
                    row,
                    start: start as u16,
                    end: end.min(usize::from(self.size.cols - 1)) as u16,
                })
            })
            .collect()
    }
    pub fn snapshot(&mut self, pane: PaneId, full: bool) -> Option<PaneUpdate> {
        let offset = self.term.grid().display_offset();
        let rows = (0..self.size.rows)
            .map(|row| {
                let cells = (0..self.size.cols)
                    .map(|col| {
                        let c = &self.term.grid()
                            [Point::new(Line(row as i32 - offset as i32), Column(col as usize))];
                        let mut flags = CellFlags::empty();
                        for (from, to) in [
                            (Flags::BOLD, CellFlags::BOLD),
                            (Flags::DIM, CellFlags::DIM),
                            (Flags::ITALIC, CellFlags::ITALIC),
                            (Flags::ALL_UNDERLINES, CellFlags::UNDERLINE),
                            (Flags::INVERSE, CellFlags::INVERSE),
                            (Flags::HIDDEN, CellFlags::HIDDEN),
                            (Flags::STRIKEOUT, CellFlags::STRIKE),
                            (Flags::WIDE_CHAR, CellFlags::WIDE),
                            (Flags::WIDE_CHAR_SPACER, CellFlags::WIDE_SPACER),
                        ] {
                            if c.flags.intersects(from) {
                                flags |= to;
                            }
                        }
                        Cell {
                            c: c.c,
                            fg: color(c.fg),
                            bg: color(c.bg),
                            flags,
                            extra: c
                                .zerowidth()
                                .filter(|v| !v.is_empty())
                                .map(|v| v.iter().collect()),
                        }
                    })
                    .collect();
                (row, cells)
            })
            .collect();
        let cursor = self.term.renderable_content().cursor;
        let shape = match cursor.shape {
            AnsiCursor::Block | AnsiCursor::Hidden => CursorShape::Block,
            AnsiCursor::Underline => CursorShape::Underline,
            AnsiCursor::Beam => CursorShape::Beam,
            AnsiCursor::HollowBlock => CursorShape::HollowBlock,
        };
        let mut frame = PaneUpdate {
            pane,
            size: self.size,
            full: true,
            rows,
            cursor: Cursor {
                point: CellPoint {
                    col: cursor.point.column.0.min(self.size.cols as usize - 1) as u16,
                    row: (cursor.point.line.0 + offset as i32).clamp(0, self.size.rows as i32 - 1)
                        as u16,
                },
                shape,
                visible: cursor.shape != AnsiCursor::Hidden
                    && cursor.point.line.0 + (offset as i32) < self.size.rows as i32,
            },
            selection: self.selection_spans(),
            flags: PaneFlags {
                prompt_row: self.current_prompt_row(),
                alt_screen: self.modes().alt_screen,
                display_offset: offset.min(u32::MAX as usize) as u32,
                mouse_reporting: self.modes().mouse != crate::terminal::MouseMode::Off,
                copy_mode: self.copy_mode(),
            },
        };
        let force = full
            || self.force_full
            || self
                .previous
                .as_ref()
                .is_none_or(|p| p.size != frame.size || p.flags != frame.flags);
        let previous = self.previous.replace(frame.clone());
        self.force_full = false;
        self.term.reset_damage();
        if !force {
            let previous = previous.unwrap();
            if frame == previous {
                return None;
            }
            frame.full = false;
            frame
                .rows
                .retain(|(row, cells)| previous.rows[*row as usize].1 != *cells);
        }
        Some(frame)
    }
}
fn color(color: AnsiColor) -> Color {
    match color {
        AnsiColor::Spec(c) => Color::Rgb(c.r, c.g, c.b),
        AnsiColor::Indexed(i) => Color::Indexed(i),
        AnsiColor::Named(c) => match c {
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => {
                Color::Foreground
            }
            NamedColor::Background => Color::Background,
            NamedColor::Cursor => Color::Cursor,
            c => {
                let n = c as usize;
                Color::Indexed(if n < 16 {
                    n as u8
                } else {
                    (n - NamedColor::DimBlack as usize) as u8
                })
            }
        },
    }
}
