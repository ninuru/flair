use std::{cmp::Reverse, collections::BTreeSet};

use flair_core::{
    command::{Command, WindowRef},
    ids::SessionId,
    key::{Key, KeyAction, KeyInput, NamedKey},
    proto::{SessionNode, SessionPreview, Target},
    sessionizer,
};

use crate::{
    cursor::{Edit, TextCursor},
    overlay_layout::{Field, Placement, Row, Sheet},
    preview_layout::Strip,
    process_icon::process_icon,
    t3_style::SEARCH_ICON,
    ui::{CHEVRON_DOWN, CHEVRON_RIGHT, ENTER_KEY, ESCAPE_KEY, MOVE_KEY, SEARCH_KEY},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    Session(usize),
    Window(usize, usize),
    Group,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Search {
    query: TextCursor,
    selected: usize,
}

pub struct SessionTree {
    sessions: Vec<SessionNode>,
    current: SessionId,
    expanded: BTreeSet<SessionId>,
    group_open: bool,
    cursor: Spot,
    search: Option<Search>,
    watched: SessionId,
    preview: Option<SessionPreview>,
    armed: Option<SessionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOutcome {
    Pending,
    Ignored,
    Close,
    Run(Command),
    Send(Command),
    Watch(SessionId),
}

impl SessionTree {
    pub fn new(
        sessions: Vec<SessionNode>,
        current: SessionId,
        preview: SessionPreview,
    ) -> SessionTree {
        let index = sessions
            .iter()
            .position(|session| session.id == current)
            .unwrap_or(0);
        let mut tree = SessionTree {
            expanded: BTreeSet::new(),
            group_open: sessions.get(index).is_some_and(|session| session.temporary),
            sessions,
            current,
            cursor: Spot::Session(index),
            search: None,
            watched: current,
            preview: None,
            armed: None,
        };
        tree.set_preview(preview);
        tree
    }

    pub fn set_preview(&mut self, preview: SessionPreview) -> bool {
        let fresh = self
            .focus()
            .map_or(self.watched, |index| self.sessions[index].id)
            == preview.session;
        if fresh {
            self.preview = Some(preview);
        }
        fresh
    }

    pub fn is_searching(&self) -> bool {
        self.search.is_some()
    }

    pub fn key(&mut self, key: &KeyInput) -> TreeOutcome {
        if key.action == KeyAction::Release {
            return TreeOutcome::Ignored;
        }
        if let Some(session) = self.armed.take() {
            return match key.key {
                Key::Char('y') if !(key.mods.ctrl || key.mods.alt || key.mods.super_) => {
                    TreeOutcome::Send(Command::KillSession(session))
                }
                Key::Char(_) | Key::Named(_) => TreeOutcome::Pending,
            };
        }
        let outcome = match self.search {
            Some(_) => self.search_key(key),
            None if key.mods.ctrl || key.mods.alt || key.mods.super_ => TreeOutcome::Ignored,
            None => self.tree_key(&key.key),
        };
        self.settle(outcome)
    }

    pub fn insert(&mut self, text: &str) -> TreeOutcome {
        let Some(search) = &mut self.search else {
            return TreeOutcome::Ignored;
        };
        search.query.insert(text);
        search.selected = 0;
        self.settle(TreeOutcome::Pending)
    }

    pub fn step(&mut self, delta: isize) -> TreeOutcome {
        self.armed = None;
        let outcome = self.move_by(delta);
        self.settle(outcome)
    }

    fn settle(&mut self, outcome: TreeOutcome) -> TreeOutcome {
        let focus = self.focus().map(|index| self.sessions[index].id);
        match (outcome, focus) {
            (TreeOutcome::Pending, Some(session)) if session != self.watched => {
                self.watched = session;
                TreeOutcome::Watch(session)
            }
            (outcome, _) => outcome,
        }
    }

    fn focus(&self) -> Option<usize> {
        match &self.search {
            Some(search) => self.matches().get(search.selected).map(|(index, _)| *index),
            None => match self.cursor {
                Spot::Session(index) | Spot::Window(index, _) => Some(index),
                Spot::Group => None,
            },
        }
    }

    fn move_by(&mut self, delta: isize) -> TreeOutcome {
        let count = self.matches().len();
        let rows = self.visible();
        match &mut self.search {
            Some(search) => {
                search.selected = search
                    .selected
                    .saturating_add_signed(delta)
                    .min(count.saturating_sub(1));
            }
            None => {
                let at = rows
                    .iter()
                    .position(|spot| *spot == self.cursor)
                    .unwrap_or(0);
                if let Some(spot) = rows.get(
                    at.saturating_add_signed(delta)
                        .min(rows.len().saturating_sub(1)),
                ) {
                    self.cursor = *spot;
                }
            }
        }
        TreeOutcome::Pending
    }

    pub fn choose(&mut self, index: usize) -> TreeOutcome {
        self.armed = None;
        let matches = self.matches();
        let rows = self.visible();
        match &mut self.search {
            Some(search) => match matches.get(index) {
                Some((session, _)) => {
                    search.selected = index;
                    self.switch(*session)
                }
                None => TreeOutcome::Ignored,
            },
            None => match rows.get(index) {
                Some(spot) => {
                    self.cursor = *spot;
                    self.enter()
                }
                None => TreeOutcome::Ignored,
            },
        }
    }

    pub fn sheet(&self) -> Sheet {
        let total = self.sessions.len();
        match &self.search {
            Some(search) => {
                let rows: Vec<Row> = self
                    .matches()
                    .into_iter()
                    .map(|(index, matches)| Row {
                        name_matches: matches,
                        ..self.session_row(index)
                    })
                    .collect();
                Sheet {
                    placement: Placement::Centred,
                    title: None,
                    field: Some(Field {
                        text: search.query.text().to_owned(),
                        cursor: search.query.cursor(),
                        placeholder: "Search sessions",
                    }),
                    count: (rows.len(), total),
                    rows,
                    selected: search.selected,
                    keys: vec![
                        (MOVE_KEY, "Navigate".to_owned()),
                        (ENTER_KEY, "Switch".to_owned()),
                        (ESCAPE_KEY, "Back".to_owned()),
                    ],
                    strip: Some(self.strip()),
                    empty: Some("No matching sessions."),
                    glyph: SEARCH_ICON,
                }
            }
            None => {
                let visible = self.visible();
                Sheet {
                    placement: Placement::Centred,
                    title: Some("Sessions"),
                    field: None,
                    count: (total, total),
                    rows: visible.iter().map(|spot| self.row(*spot)).collect(),
                    selected: visible
                        .iter()
                        .position(|spot| *spot == self.cursor)
                        .unwrap_or(0),
                    keys: self.tree_keys(),
                    strip: Some(self.strip()),
                    empty: Some("No sessions."),
                    glyph: SEARCH_ICON,
                }
            }
        }
    }

    fn tree_keys(&self) -> Vec<(&'static str, String)> {
        match self
            .armed
            .and_then(|armed| self.sessions.iter().find(|session| session.id == armed))
        {
            Some(session) => vec![
                ("y", format!("Kill {}", session.name)),
                (ESCAPE_KEY, "Cancel".to_owned()),
            ],
            None => vec![
                ("j k", "Navigate".to_owned()),
                (ENTER_KEY, "Switch".to_owned()),
                ("h l", "Fold".to_owned()),
                ("0-9", "Jump".to_owned()),
                (SEARCH_KEY, "Search".to_owned()),
                ("x", "Kill".to_owned()),
                (ESCAPE_KEY, "Close".to_owned()),
            ],
        }
    }

    fn strip(&self) -> Strip {
        match self.focus().map(|index| &self.sessions[index]) {
            Some(session) => Strip {
                windows: session.windows.clone(),
                preview: self
                    .preview
                    .as_ref()
                    .filter(|preview| preview.session == session.id)
                    .cloned(),
            },
            None => Strip {
                windows: Vec::new(),
                preview: None,
            },
        }
    }

    fn tree_key(&mut self, key: &Key) -> TreeOutcome {
        match key {
            Key::Named(NamedKey::Escape) | Key::Char('q') => TreeOutcome::Close,
            Key::Named(NamedKey::Enter) => self.enter(),
            Key::Named(NamedKey::Down) | Key::Char('j') => self.move_by(1),
            Key::Named(NamedKey::Up) | Key::Char('k') => self.move_by(-1),
            Key::Named(NamedKey::Right) | Key::Char('l') => self.open_fold(),
            Key::Named(NamedKey::Left) | Key::Char('h') => self.close_fold(),
            Key::Char('/') => {
                let selected = match self.cursor {
                    Spot::Session(index) | Spot::Window(index, _) => index,
                    Spot::Group => self
                        .sessions
                        .iter()
                        .position(|session| session.temporary)
                        .unwrap_or(0),
                };
                self.search = Some(Search {
                    query: TextCursor::new(""),
                    selected,
                });
                TreeOutcome::Pending
            }
            Key::Char('x') => match self.focus() {
                Some(index) => {
                    self.armed = Some(self.sessions[index].id);
                    TreeOutcome::Pending
                }
                None => TreeOutcome::Ignored,
            },
            Key::Char(digit @ '0'..='9') => {
                let number = digit.to_digit(10).map_or(0, |digit| digit as usize);
                match self.numbers().get(number) {
                    Some(Some(index)) => self.switch(*index),
                    Some(None) => TreeOutcome::Run(Command::Switch(Target::Main)),
                    None => TreeOutcome::Ignored,
                }
            }
            Key::Char(_) | Key::Named(_) => TreeOutcome::Ignored,
        }
    }

    fn search_key(&mut self, key: &KeyInput) -> TreeOutcome {
        match key.key {
            Key::Named(NamedKey::Escape) => {
                let selected = self.matches().get(self.selected()).map(|(index, _)| *index);
                if let Some(index) = selected {
                    self.cursor = Spot::Session(index);
                    self.group_open |= self.sessions[index].temporary;
                }
                self.search = None;
                TreeOutcome::Pending
            }
            Key::Named(NamedKey::Enter) => match self.matches().get(self.selected()) {
                Some((index, _)) => self.switch(*index),
                None => TreeOutcome::Ignored,
            },
            Key::Named(NamedKey::Up) => self.move_by(-1),
            Key::Named(NamedKey::Down) => self.move_by(1),
            Key::Char(_) | Key::Named(_) => {
                let Some(search) = &mut self.search else {
                    return TreeOutcome::Ignored;
                };
                match search.query.edit(key) {
                    Edit::Changed => {
                        search.selected = 0;
                        TreeOutcome::Pending
                    }
                    Edit::Moved => TreeOutcome::Pending,
                    Edit::Ignored => TreeOutcome::Ignored,
                }
            }
        }
    }

    fn selected(&self) -> usize {
        self.search.as_ref().map_or(0, |search| search.selected)
    }

    fn open_fold(&mut self) -> TreeOutcome {
        let index = match self.cursor {
            Spot::Session(index) => index,
            Spot::Window(_, _) => return TreeOutcome::Ignored,
            Spot::Group if !self.group_open => return self.toggle(),
            Spot::Group => {
                let first = self.temporaries().into_iter().next();
                if let Some(index) = first {
                    self.cursor = Spot::Session(index);
                }
                return TreeOutcome::Pending;
            }
        };
        let session = &self.sessions[index];
        match self.expanded.insert(session.id) {
            true => TreeOutcome::Pending,
            false if session.windows.is_empty() => TreeOutcome::Ignored,
            false => {
                self.cursor = Spot::Window(index, 0);
                TreeOutcome::Pending
            }
        }
    }

    fn close_fold(&mut self) -> TreeOutcome {
        match self.cursor {
            Spot::Window(index, _) => {
                self.cursor = Spot::Session(index);
                TreeOutcome::Pending
            }
            Spot::Session(index) => match self.expanded.remove(&self.sessions[index].id) {
                true => TreeOutcome::Pending,
                false if self.sessions[index].temporary => {
                    self.cursor = Spot::Group;
                    TreeOutcome::Pending
                }
                false => TreeOutcome::Ignored,
            },
            Spot::Group if self.group_open => self.toggle(),
            Spot::Group => TreeOutcome::Ignored,
        }
    }

    fn toggle(&mut self) -> TreeOutcome {
        self.group_open = !self.group_open;
        TreeOutcome::Pending
    }

    fn enter(&mut self) -> TreeOutcome {
        match self.cursor {
            Spot::Group => self.toggle(),
            Spot::Session(index) => self.switch(index),
            Spot::Window(index, window) => self.sessions[index]
                .windows
                .get(window)
                .map_or(TreeOutcome::Ignored, |window| {
                    TreeOutcome::Run(Command::SelectWindow(WindowRef::Id(window.id)))
                }),
        }
    }

    fn switch(&self, index: usize) -> TreeOutcome {
        let session = &self.sessions[index];
        match session.id == self.current {
            true => TreeOutcome::Close,
            false => TreeOutcome::Run(Command::Switch(Target::Name(session.name.clone()))),
        }
    }

    fn visible(&self) -> Vec<Spot> {
        let temporaries = self.temporaries();
        let group = match (temporaries.is_empty(), self.group_open) {
            (true, _) => Vec::new(),
            (false, false) => vec![Spot::Group],
            (false, true) => std::iter::once(Spot::Group)
                .chain(self.branch(&temporaries))
                .collect(),
        };
        self.branch(&self.regular())
            .into_iter()
            .chain(group)
            .collect()
    }

    fn branch(&self, indices: &[usize]) -> Vec<Spot> {
        indices
            .iter()
            .flat_map(|index| {
                let index = *index;
                let session = &self.sessions[index];
                let windows = match self.expanded.contains(&session.id) {
                    true => session.windows.len(),
                    false => 0,
                };
                std::iter::once(Spot::Session(index))
                    .chain((0..windows).map(move |window| Spot::Window(index, window)))
            })
            .collect()
    }

    fn regular(&self) -> Vec<usize> {
        self.indices(false)
    }

    fn main(&self) -> Option<usize> {
        self.sessions
            .iter()
            .position(|session| !session.temporary && session.name == "main")
    }

    fn numbers(&self) -> Vec<Option<usize>> {
        let main = self.main();
        std::iter::once(main)
            .chain(
                self.regular()
                    .into_iter()
                    .filter(|index| Some(*index) != main)
                    .map(Some),
            )
            .collect()
    }

    fn temporaries(&self) -> Vec<usize> {
        self.indices(true)
    }

    fn indices(&self, temporary: bool) -> Vec<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| session.temporary == temporary)
            .map(|(index, _)| index)
            .collect()
    }

    fn matches(&self) -> Vec<(usize, Vec<usize>)> {
        let query = self
            .search
            .as_ref()
            .map_or("", |search| search.query.text());
        let mut scored = sessionizer::fuzzy(
            query,
            self.sessions.iter().map(|session| session.name.as_str()),
        );
        scored.sort_by_key(|(_, score, _)| Reverse(*score));
        scored
            .into_iter()
            .map(|(index, _, matches)| (index, matches))
            .collect()
    }

    fn row(&self, spot: Spot) -> Row {
        match spot {
            Spot::Session(index) => Row {
                depth: u8::from(self.sessions[index].temporary),
                ..self.session_row(index)
            },
            Spot::Window(index, window) => {
                let depth = 1 + u8::from(self.sessions[index].temporary);
                let window = &self.sessions[index].windows[window];
                Row {
                    depth,
                    number: Some(window.index),
                    icon: process_icon(&window.program),
                    name: window.name.clone(),
                    label: window.active.then_some("active"),
                    ..Row::default()
                }
            }
            Spot::Group => Row {
                icon: Some(match self.group_open {
                    true => CHEVRON_DOWN,
                    false => CHEVRON_RIGHT,
                }),
                name: "Temporary".to_owned(),
                detail: Some(match self.temporaries().len() {
                    1 => "1 session".to_owned(),
                    count => format!("{count} sessions"),
                }),
                muted: true,
                ..Row::default()
            },
        }
    }

    fn session_row(&self, index: usize) -> Row {
        let session = &self.sessions[index];
        let windows = match session.windows.len() {
            1 => "1 window".to_owned(),
            count => format!("{count} windows"),
        };
        let number = self
            .numbers()
            .iter()
            .position(|numbered| *numbered == Some(index))
            .map(|number| u16::try_from(number).unwrap_or(u16::MAX));
        Row {
            number,
            name: session.name.clone(),
            detail: Some(windows),
            current: session.id == self.current,
            monogram: !session.temporary,
            ..Row::default()
        }
    }
}

#[cfg(test)]
#[path = "session_tree_tests.rs"]
mod tests;
