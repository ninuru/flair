use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    path::{Path, PathBuf},
};

use flair_core::{
    key::{Key, KeyAction, KeyInput, NamedKey},
    sessionizer,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub name: OsString,
    pub dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub more: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowKind {
    Dir { open: bool },
    File,
    More(usize),
    Loading,
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    pub path: PathBuf,
    pub name: String,
    pub depth: u16,
    pub kind: RowKind,
    pub matches: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerOutcome {
    Ignored,
    Redraw,
    Load(Vec<PathBuf>),
    Insert(PathBuf),
    Cd(PathBuf),
    Edit(PathBuf),
    Copy(PathBuf),
    Blur,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Explorer {
    root: Option<PathBuf>,
    listings: BTreeMap<PathBuf, Listing>,
    pending: BTreeSet<PathBuf>,
    expanded: BTreeSet<PathBuf>,
    selected: Option<PathBuf>,
    scroll: usize,
    page: usize,
    show_hidden: bool,
    filter: Option<String>,
    editing: bool,
    rows: Vec<TreeRow>,
}

impl Explorer {
    pub fn new(show_hidden: bool) -> Explorer {
        Explorer {
            show_hidden,
            ..Explorer::default()
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn set_root(&mut self, cwd: &Path) -> Vec<PathBuf> {
        self.root = Some(cwd.to_path_buf());
        self.selected = Some(cwd.to_path_buf());
        self.scroll = 0;
        self.listings.retain(|dir, _| dir.starts_with(cwd));
        self.pending.retain(|dir| dir.starts_with(cwd));
        let mut wanted = vec![cwd.to_path_buf()];
        wanted.extend(self.open_under(cwd));
        wanted.retain(|dir| !self.listings.contains_key(dir));
        let loads = self.request(wanted);
        self.rebuild();
        loads
    }

    pub fn reload(&mut self) -> Vec<PathBuf> {
        let Some(root) = self.root.clone() else {
            return Vec::new();
        };
        let mut wanted = vec![root.clone()];
        wanted.extend(self.open_under(&root));
        let loads = self.request(wanted);
        self.rebuild();
        loads
    }

    pub fn insert(&mut self, listing: Listing) -> bool {
        if !self.pending.remove(&listing.dir) {
            return false;
        }
        self.listings.insert(listing.dir.clone(), listing);
        self.rebuild();
        true
    }

    pub fn set_show_hidden(&mut self, show: bool) {
        self.show_hidden = show;
        self.rebuild();
    }

    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    pub fn selected(&self) -> Option<usize> {
        let selected = self.selected.as_ref()?;
        self.rows
            .iter()
            .position(|row| entry(&row.kind) && row.path == *selected)
    }

    pub fn set_page(&mut self, page: usize) {
        self.page = page;
        self.scroll = self.scroll.min(self.rows.len().saturating_sub(page.max(1)));
    }

    pub fn scroll(&self) -> usize {
        self.scroll
    }

    pub fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }

    pub fn editing(&self) -> bool {
        self.editing
    }

    pub fn key(&mut self, key: &KeyInput, at_prompt: bool, page: usize) -> ExplorerOutcome {
        if key.action == KeyAction::Release {
            return ExplorerOutcome::Ignored;
        }
        if self.editing {
            return self.field_key(key);
        }
        if key.mods.ctrl || key.mods.alt || key.mods.super_ {
            return ExplorerOutcome::Ignored;
        }
        let step = isize::try_from(page.max(1)).unwrap_or(isize::MAX);
        match key.key {
            Key::Named(NamedKey::Up) | Key::Char('k') => self.step(-1, page),
            Key::Named(NamedKey::Down) | Key::Char('j') => self.step(1, page),
            Key::Named(NamedKey::PageUp) => self.step(-step, page),
            Key::Named(NamedKey::PageDown) => self.step(step, page),
            Key::Named(NamedKey::Home) => self.step(isize::MIN, page),
            Key::Named(NamedKey::End) => self.step(isize::MAX, page),
            Key::Named(NamedKey::Right) | Key::Char('l') => self.right(page),
            Key::Named(NamedKey::Left) | Key::Char('h') => self.left(page),
            Key::Named(NamedKey::Enter) => self.enter(),
            Key::Named(NamedKey::Escape) => ExplorerOutcome::Blur,
            Key::Char('o') => match self.current() {
                Some((_, RowKind::File, path)) => ExplorerOutcome::Edit(path),
                Some(_) | None => ExplorerOutcome::Ignored,
            },
            Key::Char('c') => match self.current() {
                Some((_, RowKind::Dir { .. }, path)) if at_prompt => ExplorerOutcome::Cd(path),
                Some(_) | None => ExplorerOutcome::Ignored,
            },
            Key::Char('i') => self
                .current()
                .map_or(ExplorerOutcome::Ignored, |(_, _, path)| {
                    ExplorerOutcome::Insert(path)
                }),
            Key::Char('y') => self
                .current()
                .map_or(ExplorerOutcome::Ignored, |(_, _, path)| {
                    ExplorerOutcome::Copy(path)
                }),
            Key::Char('.') if self.root.is_some() => {
                self.set_show_hidden(!self.show_hidden);
                ExplorerOutcome::Redraw
            }
            Key::Char('/') if self.root.is_some() => {
                self.editing = true;
                self.filter = Some(self.filter.take().unwrap_or_default());
                ExplorerOutcome::Redraw
            }
            Key::Char(_) | Key::Named(_) => ExplorerOutcome::Ignored,
        }
    }

    pub fn text(&mut self, text: &str) -> ExplorerOutcome {
        if !self.editing {
            return ExplorerOutcome::Ignored;
        }
        let Some(filter) = &mut self.filter else {
            return ExplorerOutcome::Ignored;
        };
        let before = filter.len();
        filter.extend(text.chars().filter(|c| !c.is_control()));
        if filter.len() == before {
            return ExplorerOutcome::Ignored;
        }
        self.refilter();
        ExplorerOutcome::Redraw
    }

    pub fn click(&mut self, row: usize, double: bool) -> ExplorerOutcome {
        let Some(target) = self.rows.get(row) else {
            return ExplorerOutcome::Ignored;
        };
        let (kind, path) = (target.kind.clone(), target.path.clone());
        match kind {
            RowKind::Dir { .. } if row == 0 || double => {
                self.selected = Some(path);
                ExplorerOutcome::Redraw
            }
            RowKind::Dir { .. } => {
                self.selected = Some(path.clone());
                self.toggle(path)
            }
            RowKind::File if double => {
                self.selected = Some(path.clone());
                ExplorerOutcome::Edit(path)
            }
            RowKind::File => {
                self.selected = Some(path);
                ExplorerOutcome::Redraw
            }
            RowKind::More(_) | RowKind::Loading | RowKind::Error(_) => ExplorerOutcome::Ignored,
        }
    }

    pub fn wheel(&mut self, rows: i32, page: usize) {
        let last = self.rows.len().saturating_sub(page);
        let moved = match usize::try_from(rows) {
            Ok(down) => self.scroll.saturating_add(down),
            Err(_) => self
                .scroll
                .saturating_sub(usize::try_from(rows.unsigned_abs()).unwrap_or(usize::MAX)),
        };
        self.scroll = moved.min(last);
    }

    fn field_key(&mut self, key: &KeyInput) -> ExplorerOutcome {
        let plain = !key.mods.ctrl && !key.mods.alt && !key.mods.super_;
        match key.key {
            Key::Named(NamedKey::Escape) => {
                self.editing = false;
                self.filter = None;
                self.rebuild();
                ExplorerOutcome::Redraw
            }
            Key::Named(NamedKey::Enter | NamedKey::Down) => {
                self.editing = false;
                if self.filter.as_deref() == Some("") {
                    self.filter = None;
                }
                ExplorerOutcome::Redraw
            }
            Key::Named(NamedKey::Backspace) => match self.filter.as_mut().and_then(String::pop) {
                Some(_) => {
                    self.refilter();
                    ExplorerOutcome::Redraw
                }
                None => ExplorerOutcome::Ignored,
            },
            Key::Char(_) if plain => match &key.text {
                Some(text) => self.text(text),
                None => ExplorerOutcome::Ignored,
            },
            Key::Char(_) | Key::Named(_) => ExplorerOutcome::Ignored,
        }
    }

    fn refilter(&mut self) {
        self.rebuild();
        let first = self.rows.iter().find(|row| !row.matches.is_empty());
        self.selected = first.map(|row| row.path.clone()).or(self.root.clone());
        self.scroll = 0;
    }

    fn current(&self) -> Option<(usize, RowKind, PathBuf)> {
        let index = self.selected()?;
        let row = &self.rows[index];
        Some((index, row.kind.clone(), row.path.clone()))
    }

    fn step(&mut self, delta: isize, page: usize) -> ExplorerOutcome {
        let Some(current) = self.selected() else {
            return ExplorerOutcome::Ignored;
        };
        let selectable: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| entry(&row.kind))
            .map(|(index, _)| index)
            .collect();
        let at = selectable
            .iter()
            .position(|index| *index == current)
            .unwrap_or(0);
        let target = at
            .saturating_add_signed(delta)
            .min(selectable.len().saturating_sub(1));
        self.select(selectable[target], page)
    }

    fn select(&mut self, index: usize, page: usize) -> ExplorerOutcome {
        if self.selected() == Some(index) {
            return ExplorerOutcome::Ignored;
        }
        self.selected = Some(self.rows[index].path.clone());
        let page = page.max(1);
        if index < self.scroll {
            self.scroll = index;
        } else if index >= self.scroll + page {
            self.scroll = index + 1 - page;
        }
        ExplorerOutcome::Redraw
    }

    fn right(&mut self, page: usize) -> ExplorerOutcome {
        let Some((index, kind, path)) = self.current() else {
            return ExplorerOutcome::Ignored;
        };
        match kind {
            RowKind::Dir { open: false } => self.expand(path),
            RowKind::Dir { open: true } => match self.rows.get(index + 1) {
                Some(next) if next.depth == self.rows[index].depth + 1 && entry(&next.kind) => {
                    self.select(index + 1, page)
                }
                Some(_) | None => ExplorerOutcome::Ignored,
            },
            RowKind::File | RowKind::More(_) | RowKind::Loading | RowKind::Error(_) => {
                ExplorerOutcome::Ignored
            }
        }
    }

    fn left(&mut self, page: usize) -> ExplorerOutcome {
        let Some((index, kind, path)) = self.current() else {
            return ExplorerOutcome::Ignored;
        };
        match kind {
            RowKind::Dir { open: true } if self.expanded.remove(&path) => {
                self.rebuild();
                return ExplorerOutcome::Redraw;
            }
            RowKind::Dir { .. }
            | RowKind::File
            | RowKind::More(_)
            | RowKind::Loading
            | RowKind::Error(_) => {}
        }
        let Some(depth) = self.rows[index].depth.checked_sub(1) else {
            return ExplorerOutcome::Ignored;
        };
        match self.rows[..index]
            .iter()
            .rposition(|row| row.depth == depth)
        {
            Some(parent) => self.select(parent, page),
            None => ExplorerOutcome::Ignored,
        }
    }

    fn enter(&mut self) -> ExplorerOutcome {
        match self.current() {
            Some((0, _, _)) | None => ExplorerOutcome::Ignored,
            Some((_, RowKind::Dir { .. }, path)) => self.toggle(path),
            Some((_, RowKind::File, path)) => ExplorerOutcome::Edit(path),
            Some((_, RowKind::More(_) | RowKind::Loading | RowKind::Error(_), _)) => {
                ExplorerOutcome::Ignored
            }
        }
    }

    fn toggle(&mut self, path: PathBuf) -> ExplorerOutcome {
        if self.expanded.remove(&path) {
            self.rebuild();
            return ExplorerOutcome::Redraw;
        }
        self.expand(path)
    }

    fn expand(&mut self, path: PathBuf) -> ExplorerOutcome {
        let mut wanted = vec![path.clone()];
        wanted.extend(
            self.open_under(&path)
                .into_iter()
                .filter(|dir| !self.listings.contains_key(dir)),
        );
        self.expanded.insert(path);
        let loads = self.request(wanted);
        self.rebuild();
        match loads.is_empty() {
            true => ExplorerOutcome::Redraw,
            false => ExplorerOutcome::Load(loads),
        }
    }

    fn open_under(&self, base: &Path) -> Vec<PathBuf> {
        self.expanded
            .iter()
            .filter(|dir| *dir != base && dir.starts_with(base))
            .filter(|dir| {
                dir.ancestors()
                    .skip(1)
                    .take_while(|ancestor| *ancestor != base)
                    .all(|ancestor| self.expanded.contains(ancestor))
            })
            .cloned()
            .collect()
    }

    fn request(&mut self, wanted: Vec<PathBuf>) -> Vec<PathBuf> {
        wanted
            .into_iter()
            .filter(|dir| self.pending.insert(dir.clone()))
            .collect()
    }

    fn rebuild(&mut self) {
        let Some(root) = &self.root else {
            self.rows = Vec::new();
            return;
        };
        let top = TreeRow {
            path: root.clone(),
            name: root.file_name().map_or_else(
                || "/".to_string(),
                |name| name.to_string_lossy().into_owned(),
            ),
            depth: 0,
            kind: RowKind::Dir { open: true },
            matches: Vec::new(),
        };
        let mut rows = vec![top];
        match self.filter.as_deref() {
            Some(query) if !query.is_empty() => self.filtered(root, query, &mut rows),
            Some(_) | None => self.children(root, 1, &mut rows),
        }
        let found = |path: &Path| {
            rows.iter()
                .any(|row| entry(&row.kind) && row.path.as_path() == path)
        };
        let selected = self
            .selected
            .as_ref()
            .and_then(|selected| selected.ancestors().find(|path| found(path)))
            .unwrap_or(root)
            .to_path_buf();
        self.scroll = self.scroll.min(rows.len().saturating_sub(self.page.max(1)));
        self.selected = Some(selected);
        self.rows = rows;
    }

    fn children(&self, dir: &Path, depth: u16, rows: &mut Vec<TreeRow>) {
        let filler = |kind| TreeRow {
            path: dir.to_path_buf(),
            name: String::new(),
            depth,
            kind,
            matches: Vec::new(),
        };
        let Some(listing) = self.listings.get(dir) else {
            if self.pending.contains(dir) {
                rows.push(filler(RowKind::Loading));
            }
            return;
        };
        for item in listing.entries.iter().filter(|item| self.shown(item)) {
            let path = dir.join(&item.name);
            let open = item.dir && self.expanded.contains(&path);
            rows.push(TreeRow {
                name: item.name.to_string_lossy().into_owned(),
                depth,
                kind: match item.dir {
                    true => RowKind::Dir { open },
                    false => RowKind::File,
                },
                matches: Vec::new(),
                path: path.clone(),
            });
            if open {
                self.children(&path, depth + 1, rows);
            }
        }
        if listing.more > 0 {
            rows.push(filler(RowKind::More(listing.more)));
        }
        if let Some(error) = &listing.error {
            rows.push(filler(RowKind::Error(error.clone())));
        }
    }

    fn filtered(&self, root: &Path, query: &str, rows: &mut Vec<TreeRow>) {
        let mut nodes = Vec::new();
        self.collect(root, 0, 1, &mut nodes);
        let hits = sessionizer::fuzzy(query, nodes.iter().map(|(row, _)| row.name.as_str()));
        let mut keep = vec![false; nodes.len() + 1];
        keep[0] = true;
        for (index, _, indices) in hits {
            nodes[index].0.matches = indices;
            let mut at = index + 1;
            while !keep[at] {
                keep[at] = true;
                at = nodes[at - 1].1;
            }
        }
        let mut parents = vec![false; nodes.len() + 1];
        for (index, (_, parent)) in nodes.iter().enumerate() {
            if keep[index + 1] {
                parents[*parent] = true;
            }
        }
        for (index, (mut row, _)) in nodes.into_iter().enumerate() {
            if !keep[index + 1] {
                continue;
            }
            if let RowKind::Dir { open } = &mut row.kind {
                *open = parents[index + 1];
            }
            rows.push(row);
        }
    }

    fn collect(&self, dir: &Path, parent: usize, depth: u16, nodes: &mut Vec<(TreeRow, usize)>) {
        let Some(listing) = self.listings.get(dir) else {
            return;
        };
        for item in listing.entries.iter().filter(|item| self.shown(item)) {
            let path = dir.join(&item.name);
            nodes.push((
                TreeRow {
                    path: path.clone(),
                    name: item.name.to_string_lossy().into_owned(),
                    depth,
                    kind: match item.dir {
                        true => RowKind::Dir { open: false },
                        false => RowKind::File,
                    },
                    matches: Vec::new(),
                },
                parent,
            ));
            if item.dir {
                let index = nodes.len();
                self.collect(&path, index, depth + 1, nodes);
            }
        }
    }

    fn shown(&self, item: &Entry) -> bool {
        self.show_hidden || item.name.as_encoded_bytes().first() != Some(&b'.')
    }
}

pub fn quote(path: &str) -> String {
    let text = path.replace('\'', r"'\''");
    format!("'{text}'")
}

fn entry(kind: &RowKind) -> bool {
    match kind {
        RowKind::Dir { .. } | RowKind::File => true,
        RowKind::More(_) | RowKind::Loading | RowKind::Error(_) => false,
    }
}

#[cfg(test)]
#[path = "explorer_tests.rs"]
mod tests;
