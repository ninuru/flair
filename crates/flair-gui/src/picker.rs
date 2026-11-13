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
    overlay_layout::{Field, Placement, Row, Sheet},
    ui::{ENTER_KEY, ESCAPE_KEY, MOVE_KEY},
};

pub struct Picker {
    pub query: TextCursor,
    entries: Vec<Candidate>,
    selected: usize,
    last: Option<Target>,
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
            .map(|ranked| ranked.entry.target())
    }

    pub fn target(&self, index: usize) -> Option<Target> {
        sessionizer::rank(&self.entries, self.query.text())
            .get(index)
            .map(|ranked| ranked.entry.target())
    }

    pub fn sheet(&self, home: Option<&Path>) -> Sheet {
        let ranked = sessionizer::rank(&self.entries, self.query.text());
        let enter = match (&self.last, self.query.is_empty()) {
            (Some(Target::Name(name)), true) => format!("last: {name}"),
            (Some(_), true) => "last".to_owned(),
            (Some(_) | None, false) | (None, true) => match ranked.get(self.selected) {
                Some(ranked) if !ranked.entry.open => "open".to_owned(),
                Some(_) | None => "switch".to_owned(),
            },
        };
        let rows: Vec<Row> = ranked
            .into_iter()
            .map(|ranked| {
                let (detail, detail_matches) =
                    display_matches(&ranked.entry.root, home, &ranked.root);
                Row {
                    name: ranked.entry.name.clone(),
                    name_matches: ranked.name,
                    detail: Some(detail),
                    detail_matches,
                    label: ranked.entry.open.then_some("open"),
                    ..Row::default()
                }
            })
            .collect();
        Sheet {
            strip: None,
            placement: Placement::Centred,
            title: None,
            field: Some(Field {
                text: self.query.text().to_owned(),
                cursor: self.query.cursor(),
                placeholder: "Search projects",
            }),
            count: (rows.len(), self.entries.len()),
            rows,
            selected: self.selected,
            keys: vec![
                (ENTER_KEY, enter),
                (MOVE_KEY, "move".to_owned()),
                (ESCAPE_KEY, "close".to_owned()),
            ],
        }
    }
}

pub fn display_path(path: &Path, home: Option<&Path>) -> String {
    display_matches(path, home, &[]).0
}

fn display_matches(path: &Path, home: Option<&Path>, matches: &[usize]) -> (String, Vec<usize>) {
    let full: Vec<char> = path.display().to_string().chars().collect();
    let chars: Vec<(Option<usize>, char)> = match home.and_then(|home| path.strip_prefix(home).ok())
    {
        Some(rest) if rest.as_os_str().is_empty() => vec![(None, '~')],
        Some(rest) => {
            let start = full
                .len()
                .saturating_sub(rest.display().to_string().chars().count());
            [(None, '~'), (None, '/')]
                .into_iter()
                .chain(
                    full[start..]
                        .iter()
                        .enumerate()
                        .map(|(index, c)| (Some(start + index), *c)),
                )
                .collect()
        }
        None => full
            .iter()
            .enumerate()
            .map(|(index, c)| (Some(index), *c))
            .collect(),
    };
    let shown: Vec<(Option<usize>, char)> =
        chars.into_iter().filter(|(_, c)| !c.is_control()).collect();
    (
        shown.iter().map(|(_, c)| c).collect(),
        shown
            .iter()
            .enumerate()
            .filter(|(_, (source, _))| source.is_some_and(|source| matches.contains(&source)))
            .map(|(index, _)| index)
            .collect(),
    )
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
