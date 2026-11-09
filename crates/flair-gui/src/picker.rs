use std::{
    env, fs,
    path::{Path, PathBuf},
};

use flair_core::{
    key::KeyInput,
    proto::{SessionInfo, Target},
    sessionizer::{self, Candidate},
};

use crate::{
    cursor::{Edit, TextCursor},
    overlay_layout::{Field, Mark, Placement, Row, Sheet},
    ui::{ENTER_KEY, ESCAPE_KEY, MOVE_KEY},
};

pub struct Picker {
    pub query: TextCursor,
    entries: Vec<Candidate>,
    selected: usize,
    last: Option<Target>,
    current: Option<String>,
}

impl Picker {
    pub fn new(
        sessions: Vec<SessionInfo>,
        current: &str,
        last: Option<Target>,
        directories: Vec<PathBuf>,
    ) -> Self {
        Self {
            query: TextCursor::new(""),
            entries: sessionizer::candidates(&sessions, current, directories),
            selected: 0,
            last,
            current: None,
        }
    }

    pub fn sessions(sessions: Vec<SessionInfo>, current: &str) -> Self {
        let entries: Vec<_> = sessions
            .into_iter()
            .map(|session| Candidate {
                name: session.name,
                root: session.root,
                open: true,
            })
            .collect();
        let selected = sessionizer::rank(&entries, "")
            .iter()
            .position(|entry| entry.name == current)
            .unwrap_or(0);
        Self {
            query: TextCursor::new(""),
            entries,
            selected,
            last: None,
            current: Some(current.to_owned()),
        }
    }

    pub fn insert(&mut self, text: &str) {
        self.query.insert(text);
        self.selected = 0;
    }

    pub fn edit(&mut self, key: &KeyInput) -> Edit {
        let edit = self.query.edit(key);
        match edit {
            Edit::Changed => self.selected = 0,
            Edit::Moved | Edit::Ignored => {}
        }
        edit
    }

    pub fn backspace(&mut self) {
        self.query.backspace();
        self.selected = 0;
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.selected = 0;
    }

    pub fn move_selection(&mut self, delta: i32) {
        let count = sessionizer::rank(&self.entries, self.query.text()).len();
        self.selected = self
            .selected
            .saturating_add_signed(delta as isize)
            .min(count.saturating_sub(1));
    }

    pub fn accept(&self) -> Option<Target> {
        if self.query.is_empty()
            && let Some(last) = &self.last
        {
            return Some(last.clone());
        }
        sessionizer::rank(&self.entries, self.query.text())
            .get(self.selected)
            .map(|entry| entry.target())
    }

    pub fn target(&self, index: usize) -> Option<Target> {
        sessionizer::rank(&self.entries, self.query.text())
            .get(index)
            .map(|entry| entry.target())
    }

    pub fn sheet(&self, home: Option<&Path>) -> Sheet {
        let rows = sessionizer::rank(&self.entries, self.query.text())
            .into_iter()
            .map(|entry| Row {
                mark: Mark::Monogram(entry.name.clone()),
                name: entry.name.clone(),
                detail: Some(display_path(&entry.root, home)),
                label: match &self.current {
                    Some(current) => (entry.name == *current).then_some("current"),
                    None => entry.open.then_some("open"),
                },
            })
            .collect();
        let (title, placeholder) = match self.current {
            Some(_) => ("Sessions", "Search sessions"),
            None => ("Sessionizer", "Search projects"),
        };
        let enter = match (&self.last, self.query.is_empty()) {
            (Some(Target::Name(name)), true) => format!("last: {name}"),
            (Some(_), true) => "last".to_owned(),
            (Some(_) | None, false) | (None, true) => "switch".to_owned(),
        };
        Sheet {
            placement: Placement::Centred,
            title,
            field: Some(Field {
                text: self.query.text().to_owned(),
                cursor: self.query.cursor(),
                placeholder,
            }),
            rows,
            selected: self.selected,
            keys: vec![
                (ENTER_KEY, enter),
                (MOVE_KEY, "move".to_owned()),
                (ESCAPE_KEY, "cancel".to_owned()),
            ],
        }
    }
}

pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    let text = match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    };
    text.chars().filter(|c| !c.is_control()).collect()
}

pub fn directories() -> Vec<PathBuf> {
    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let roots = [
        home.clone(),
        home.join("projects"),
        home.join("IdeaProjects"),
        home.join("projects/workspaces"),
        home.join("IdeaProjects/workspaces"),
    ];
    roots
        .into_iter()
        .flat_map(|root| match fs::read_dir(&root) {
            Ok(entries) => entries
                .filter_map(|entry| match entry {
                    Ok(entry) => match entry.path().is_dir() {
                        true => Some(entry.path()),
                        false => None,
                    },
                    Err(error) => {
                        log::debug!("could not read sessionizer entry: {error}");
                        None
                    }
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                log::debug!(
                    "could not scan sessionizer root {}: {error}",
                    root.display()
                );
                Vec::new()
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
