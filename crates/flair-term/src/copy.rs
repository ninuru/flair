use crate::Terminal;
use alacritty_terminal::{
    grid::{Dimensions, Scroll},
    index::{Boundary, Column, Direction, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::search::RegexSearch,
    vi_mode::ViMotion,
};
use flair_core::command::{CopyMotion, SelectionKind};

impl Terminal {
    pub fn copy_move(&mut self, motion: CopyMotion) {
        if !self.copy_mode() {
            return;
        }
        let vi = match motion {
            CopyMotion::Left => ViMotion::Left,
            CopyMotion::Down => ViMotion::Down,
            CopyMotion::Up => ViMotion::Up,
            CopyMotion::Right => ViMotion::Right,
            CopyMotion::NextWord => ViMotion::SemanticRight,
            CopyMotion::PreviousWord => ViMotion::SemanticLeft,
            CopyMotion::NextWordEnd => ViMotion::SemanticRightEnd,
            CopyMotion::NextSpace => ViMotion::WordRight,
            CopyMotion::PreviousSpace => ViMotion::WordLeft,
            CopyMotion::NextSpaceEnd => ViMotion::WordRightEnd,
            CopyMotion::StartOfLine => ViMotion::First,
            CopyMotion::BackToIndentation => ViMotion::FirstOccupied,
            CopyMotion::EndOfLine => ViMotion::Last,
            CopyMotion::TopLine => ViMotion::High,
            CopyMotion::MiddleLine => ViMotion::Middle,
            CopyMotion::BottomLine => ViMotion::Low,
            CopyMotion::HistoryTop => return self.jump_to_line(self.term.topmost_line()),
            CopyMotion::HistoryBottom => return self.jump_to_line(self.term.bottommost_line()),
            CopyMotion::HalfPageUp => return self.half_page(1),
            CopyMotion::HalfPageDown => return self.half_page(-1),
        };
        self.term.vi_motion(vi);
    }

    fn jump_to_line(&mut self, line: Line) {
        self.term.vi_goto_point(Point::new(line, Column(0)));
        self.term.vi_motion(ViMotion::FirstOccupied);
    }

    fn half_page(&mut self, direction: i32) {
        let lines = direction * (self.term.screen_lines() as i32 / 2).max(1);
        let target = self.term.vi_mode_cursor.scroll(&self.term, lines);
        self.term.scroll_display(Scroll::Delta(lines));
        self.term.vi_goto_point(target.point);
    }

    pub fn copy_select(&mut self, kind: SelectionKind) {
        if !self.copy_mode() {
            return;
        }
        let ty = match kind {
            SelectionKind::Simple => SelectionType::Simple,
            SelectionKind::Lines => SelectionType::Lines,
            SelectionKind::Block => SelectionType::Block,
        };
        let current = self
            .term
            .selection
            .as_ref()
            .filter(|selection| !selection.is_empty())
            .map(|selection| selection.ty);
        match (current, &mut self.term.selection) {
            (Some(current), _) if current == ty => {
                self.term.selection = None;
                self.copy_anchor = None;
            }
            (Some(_), Some(selection)) => {
                selection.ty = ty;
                selection.include_all();
            }
            (Some(_), None) | (None, _) => {
                let point = self.term.vi_mode_cursor.point;
                self.start_copy_selection(ty, point);
                self.copy_anchor = Some(point);
            }
        }
    }

    pub fn copy_other_end(&mut self) {
        if !self.copy_mode() {
            return;
        }
        let Some(anchor) = self.copy_anchor else {
            return;
        };
        let Some((ty, range)) = self.term.selection.as_ref().and_then(|selection| {
            selection
                .to_range(&self.term)
                .map(|range| (selection.ty, range))
        }) else {
            return;
        };
        let line = if anchor.line == range.start.line {
            range.end.line
        } else {
            range.start.line
        };
        let moving = match ty {
            SelectionType::Lines => Point::new(line, self.term.vi_mode_cursor.point.column),
            SelectionType::Block => Point::new(
                line,
                if anchor.column == range.start.column {
                    range.end.column
                } else {
                    range.start.column
                },
            ),
            SelectionType::Simple | SelectionType::Semantic => {
                if anchor == range.start {
                    range.end
                } else {
                    range.start
                }
            }
        };
        self.start_copy_selection(ty, moving);
        self.copy_anchor = Some(moving);
        self.term.vi_goto_point(anchor);
    }

    pub(crate) fn selection_end_line(&self) -> Option<Line> {
        self.term
            .selection
            .as_ref()
            .and_then(|selection| selection.to_range(&self.term))
            .map(|range| range.end.line)
    }

    fn start_copy_selection(&mut self, ty: SelectionType, point: Point) {
        let mut selection = Selection::new(ty, point, Side::Left);
        selection.include_all();
        self.term.selection = Some(selection);
    }

    pub fn copy_search(&mut self, pattern: &str, forward: bool) -> Result<(), String> {
        if !self.copy_mode() || pattern.is_empty() {
            return Ok(());
        }
        let regex = RegexSearch::new(pattern).map_err(|error| error.to_string())?;
        self.search = Some((regex, forward));
        self.search_from_cursor(forward);
        Ok(())
    }

    pub fn copy_search_again(&mut self, reverse: bool) {
        if !self.copy_mode() {
            return;
        }
        if let Some((_, forward)) = &self.search {
            let forward = *forward != reverse;
            self.search_from_cursor(forward);
        }
    }

    fn search_from_cursor(&mut self, forward: bool) {
        let Some((regex, _)) = &mut self.search else {
            return;
        };
        let point = self.term.vi_mode_cursor.point;
        let (direction, origin) = if forward {
            (Direction::Right, point.add(&self.term, Boundary::None, 1))
        } else {
            (Direction::Left, point.sub(&self.term, Boundary::None, 1))
        };
        if let Some(found) = self
            .term
            .search_next(regex, origin, direction, Side::Left, None)
        {
            self.term.vi_goto_point(*found.start());
        }
    }
}

#[cfg(test)]
#[path = "copy_tests.rs"]
mod tests;
