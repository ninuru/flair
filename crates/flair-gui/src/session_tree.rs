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
    ui::{ENTER_KEY, ESCAPE_KEY, MOVE_KEY, SEARCH_KEY},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    Session(usize),
    Window(usize, usize),
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
    cursor: Spot,
    search: Option<Search>,
    watched: SessionId,
    preview: Option<SessionPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeOutcome {
    Pending,
    Ignored,
    Close,
    Run(Command),
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
            sessions,
            current,
            cursor: Spot::Session(index),
            search: None,
            watched: current,
            preview: None,
        };
        tree.set_preview(preview);
        tree
    }

    pub fn set_preview(&mut self, preview: SessionPreview) -> bool {
        let fresh = self.focus().map(|index| self.sessions[index].id) == Some(preview.session);
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
                        (ENTER_KEY, "switch".to_owned()),
                        (MOVE_KEY, "move".to_owned()),
                        (ESCAPE_KEY, "back".to_owned()),
                    ],
                    strip: Some(self.strip()),
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
                    keys: vec![
                        (ENTER_KEY, "switch".to_owned()),
                        ("j k", "move".to_owned()),
                        ("h l", "fold".to_owned()),
                        ("1-9", "jump".to_owned()),
                        (SEARCH_KEY, "search".to_owned()),
                        (ESCAPE_KEY, "close".to_owned()),
                    ],
                    strip: Some(self.strip()),
                }
            }
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
                };
                self.search = Some(Search {
                    query: TextCursor::new(""),
                    selected,
                });
                TreeOutcome::Pending
            }
            Key::Char(digit @ '1'..='9') => {
                let index = digit.to_digit(10).map_or(0, |digit| digit as usize - 1);
                match index < self.sessions.len() {
                    true => self.switch(index),
                    false => TreeOutcome::Ignored,
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
                false => TreeOutcome::Ignored,
            },
        }
    }

    fn enter(&self) -> TreeOutcome {
        match self.cursor {
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
        self.sessions
            .iter()
            .enumerate()
            .flat_map(|(index, session)| {
                let windows = match self.expanded.contains(&session.id) {
                    true => session.windows.len(),
                    false => 0,
                };
                std::iter::once(Spot::Session(index))
                    .chain((0..windows).map(move |window| Spot::Window(index, window)))
            })
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
            Spot::Session(index) => self.session_row(index),
            Spot::Window(index, window) => {
                let window = &self.sessions[index].windows[window];
                Row {
                    depth: 1,
                    number: Some(window.index),
                    icon: process_icon(&window.program),
                    name: window.name.clone(),
                    label: window.active.then_some("active"),
                    ..Row::default()
                }
            }
        }
    }

    fn session_row(&self, index: usize) -> Row {
        let session = &self.sessions[index];
        let windows = match session.windows.len() {
            1 => "1 window".to_owned(),
            count => format!("{count} windows"),
        };
        Row {
            number: Some(u16::try_from(index + 1).unwrap_or(u16::MAX)),
            name: session.name.clone(),
            detail: Some(windows),
            label: (session.id == self.current).then_some("current"),
            ..Row::default()
        }
    }
}

#[cfg(test)]
#[path = "session_tree_tests.rs"]
mod tests;
