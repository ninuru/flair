use flair_core::{
    config::Config,
    config_edit::apply,
    key::{Key, KeyAction, KeyInput, NamedKey},
    keys::Binding,
    settings::{Kind, SETTINGS, Section, Source, Value, position, sources},
    theme::Theme,
};

use crate::{
    cursor::{Edit, TextCursor},
    overlay_layout::Field,
    settings_layout::{Control, Entry, Group, SettingsHit, SettingsSheet},
    ui::{DELETE_KEY, ENTER_KEY, ESCAPE_KEY, SEARCH_KEY, SPACE_KEY, STEP_KEY},
};

const SEARCH_PLACEHOLDER: &str = "Search settings";
const EDITOR_PLACEHOLDER: &str = "unset";
const SEARCH_CRUMB: &str = "Search";
const BINDINGS: &str = "Key bindings";
const UNBOUND: &str = "unbound";

pub struct SettingsPage {
    config: Config,
    theme: Theme,
    file: String,
    sources: Vec<Source>,
    bindings: Vec<Binding>,
    themes: Vec<String>,
    section: usize,
    focus: Focus,
    item: usize,
    query: TextCursor,
    editor: Option<Editor>,
    pending: Option<(usize, Value)>,
    error: Option<(usize, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Sections,
    Items,
}

struct Editor {
    setting: usize,
    text: TextCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Setting(usize),
    Binding(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsOutcome {
    Pending,
    Preview(Box<Config>),
    Revert,
    Save {
        key: &'static str,
        value: Option<Value>,
    },
    Close,
}

impl SettingsPage {
    pub fn new(config: &Config, theme: Theme, text: String, themes: Vec<String>) -> SettingsPage {
        SettingsPage {
            config: config.clone(),
            theme,
            sources: sources(&text),
            file: text,
            bindings: config.keys.bindings(),
            themes,
            section: 0,
            focus: Focus::Items,
            item: 0,
            query: TextCursor::new(""),
            editor: None,
            pending: None,
            error: None,
        }
    }

    pub fn key(&mut self, key: &KeyInput) -> SettingsOutcome {
        if key.action == KeyAction::Release {
            return SettingsOutcome::Pending;
        }
        if self.editor.is_some() {
            return self.editor_key(key);
        }
        match self.focus {
            Focus::Sections => self.sections_key(key),
            Focus::Items => self.items_key(key),
        }
    }

    pub fn insert(&mut self, text: &str) -> SettingsOutcome {
        if let Some(editor) = &mut self.editor {
            let before = editor.text.text().len();
            editor.text.insert(text);
            return match editor.text.text().len() == before {
                true => SettingsOutcome::Pending,
                false => self.edited(),
            };
        }
        self.change_query(|query| query.insert(text))
    }

    pub fn click(&mut self, hit: SettingsHit) -> SettingsOutcome {
        match hit {
            SettingsHit::Section(index) => {
                let outcome = self.leave();
                self.query = TextCursor::new("");
                self.select_section(index);
                self.focus = Focus::Items;
                outcome
            }
            SettingsHit::Close => SettingsOutcome::Close,
            SettingsHit::Search => {
                let outcome = self.leave();
                self.focus = Focus::Items;
                outcome
            }
            SettingsHit::Toggle(item) => self.on_item(item, SettingsPage::toggle),
            SettingsHit::Decrease(item) => self.on_item(item, |page| page.step(-1)),
            SettingsHit::Increase(item) | SettingsHit::Cycle(item) => {
                self.on_item(item, |page| page.step(1))
            }
            SettingsHit::Edit(item) => self.on_item(item, SettingsPage::edit),
        }
    }

    pub fn focus(&mut self, item: usize) -> SettingsOutcome {
        self.on_item(item, |_| SettingsOutcome::Pending)
    }

    pub fn wheel(&mut self, rows: i32) -> SettingsOutcome {
        match self.editor {
            Some(_) => SettingsOutcome::Pending,
            None => self.move_by(rows),
        }
    }

    pub fn refresh(&mut self, config: &Config, theme: Theme, text: String) -> SettingsOutcome {
        self.config = config.clone();
        self.theme = theme;
        self.sources = sources(&text);
        self.file = text;
        self.bindings = config.keys.bindings();
        self.error = None;
        self.item = self.item.min(self.items().len().saturating_sub(1));
        if self.editor.is_some() {
            return self.preview();
        }
        match &self.pending {
            Some((index, value)) => self.preview_value(SETTINGS[*index].key, Some(value)),
            None => SettingsOutcome::Pending,
        }
    }

    pub fn fail(&mut self, key: &str, message: &str) {
        if let Some(index) = position(key) {
            self.error = Some((index, message.to_owned()));
        }
    }

    pub fn sheet(&self) -> SettingsSheet {
        let searching = !self.query.is_empty();
        let items = self.items();
        SettingsSheet {
            sections: Section::ALL.iter().map(|section| section.title()).collect(),
            section: (!searching).then_some(self.section),
            sections_focused: self.focus == Focus::Sections,
            crumb: match searching {
                true => SEARCH_CRUMB,
                false => Section::ALL[self.section].title(),
            },
            search: Field {
                text: self.query.text().to_owned(),
                cursor: self.query.cursor(),
                placeholder: SEARCH_PLACEHOLDER,
            },
            groups: self.groups(&items, searching),
            focus: (!items.is_empty()).then(|| self.item.min(items.len() - 1)),
            keys: self.footer(),
        }
    }

    fn editor_key(&mut self, key: &KeyInput) -> SettingsOutcome {
        match key.key {
            Key::Named(NamedKey::Enter) => self.accept(),
            Key::Named(NamedKey::Escape) => self.close_editor(),
            Key::Named(NamedKey::Tab) => {
                let outcome = self.close_editor();
                self.focus_sections();
                outcome
            }
            Key::Char(_) | Key::Named(_) => {
                let Some(editor) = &mut self.editor else {
                    return SettingsOutcome::Pending;
                };
                match editor.text.edit(key) {
                    Edit::Changed => self.edited(),
                    Edit::Moved | Edit::Ignored => SettingsOutcome::Pending,
                }
            }
        }
    }

    fn sections_key(&mut self, key: &KeyInput) -> SettingsOutcome {
        let ctrl = key.mods.ctrl;
        match key.key {
            Key::Named(NamedKey::Up) => self.select_section(self.section.saturating_sub(1)),
            Key::Char('p' | 'P') if ctrl => self.select_section(self.section.saturating_sub(1)),
            Key::Named(NamedKey::Down) => self.select_section(self.section + 1),
            Key::Char('n' | 'N') if ctrl => self.select_section(self.section + 1),
            Key::Named(NamedKey::Tab | NamedKey::Right | NamedKey::Enter) => {
                self.focus = Focus::Items;
            }
            Key::Named(NamedKey::Escape) => return SettingsOutcome::Close,
            Key::Char(_) if plain(key) => return self.type_query(key),
            Key::Char(_) | Key::Named(_) => {}
        }
        SettingsOutcome::Pending
    }

    fn items_key(&mut self, key: &KeyInput) -> SettingsOutcome {
        let ctrl = key.mods.ctrl;
        let empty = self.query.is_empty();
        match key.key {
            Key::Named(NamedKey::Up) => self.move_by(-1),
            Key::Char('p' | 'P') if ctrl => self.move_by(-1),
            Key::Named(NamedKey::Down) => self.move_by(1),
            Key::Char('n' | 'N') if ctrl => self.move_by(1),
            Key::Char('u' | 'U') if ctrl => self.change_query(|query| *query = TextCursor::new("")),
            Key::Named(NamedKey::Tab) if empty => {
                let outcome = self.leave();
                self.focus_sections();
                outcome
            }
            Key::Named(NamedKey::Enter) => self.activate(),
            Key::Named(NamedKey::Left) => self.step(-1),
            Key::Named(NamedKey::Right) => self.step(1),
            Key::Named(NamedKey::Delete) => self.reset(),
            Key::Named(NamedKey::Backspace) => self.change_query(TextCursor::backspace),
            Key::Named(NamedKey::Escape) => match self.pending.take() {
                Some(_) => SettingsOutcome::Revert,
                None => SettingsOutcome::Close,
            },
            Key::Char(' ') if plain(key) && empty => self.toggle(),
            Key::Char('/') if plain(key) && empty => SettingsOutcome::Pending,
            Key::Char(_) if plain(key) => self.type_query(key),
            Key::Char(_) | Key::Named(_) => SettingsOutcome::Pending,
        }
    }

    fn items(&self) -> Vec<Item> {
        let query = self.query.text();
        if query.is_empty() {
            let section = Section::ALL[self.section];
            let bindings = match section {
                Section::Keys => self.bindings.len(),
                Section::Appearance | Section::Font | Section::Window | Section::Terminal => 0,
            };
            return SETTINGS
                .iter()
                .enumerate()
                .filter(|(_, setting)| setting.section == section)
                .map(|(index, _)| Item::Setting(index))
                .chain((0..bindings).map(Item::Binding))
                .collect();
        }
        let lower = query.to_lowercase();
        SETTINGS
            .iter()
            .enumerate()
            .filter(|(_, setting)| setting.matches(query))
            .map(|(index, _)| Item::Setting(index))
            .chain(
                self.bindings
                    .iter()
                    .enumerate()
                    .filter(|(_, binding)| {
                        [
                            binding.chord.as_str(),
                            binding.command.as_deref().unwrap_or(UNBOUND),
                            binding.table.name(),
                        ]
                        .iter()
                        .any(|field| field.to_lowercase().contains(&lower))
                    })
                    .map(|(index, _)| Item::Binding(index)),
            )
            .collect()
    }

    fn selected(&self) -> Option<usize> {
        match self.items().get(self.item) {
            Some(Item::Setting(index)) => Some(*index),
            Some(Item::Binding(_)) | None => None,
        }
    }

    fn current(&self, index: usize) -> Value {
        SETTINGS[index].value(&self.config, &self.theme)
    }

    fn shown(&self, index: usize) -> Value {
        match &self.pending {
            Some((at, value)) if *at == index => value.clone(),
            Some(_) | None => self.current(index),
        }
    }

    fn activate(&mut self) -> SettingsOutcome {
        let Some(index) = self.selected() else {
            return SettingsOutcome::Pending;
        };
        if self.pending.is_some() {
            return self.leave();
        }
        match SETTINGS[index].kind {
            Kind::Toggle => self.toggle(),
            Kind::Choice(_) => self.step(1),
            Kind::Integer { .. }
            | Kind::Float { .. }
            | Kind::Text
            | Kind::Font
            | Kind::Words
            | Kind::Color { .. } => self.edit(),
        }
    }

    fn toggle(&mut self) -> SettingsOutcome {
        let Some(index) = self.selected() else {
            return SettingsOutcome::Pending;
        };
        let setting = &SETTINGS[index];
        match setting.kind {
            Kind::Toggle => setting.step(&self.current(index), 1, &self.themes).map_or(
                SettingsOutcome::Pending,
                |value| SettingsOutcome::Save {
                    key: setting.key,
                    value: Some(value),
                },
            ),
            Kind::Integer { .. }
            | Kind::Float { .. }
            | Kind::Choice(_)
            | Kind::Text
            | Kind::Font
            | Kind::Words
            | Kind::Color { .. } => SettingsOutcome::Pending,
        }
    }

    fn step(&mut self, delta: i32) -> SettingsOutcome {
        let Some(index) = self.selected() else {
            return SettingsOutcome::Pending;
        };
        let setting = &SETTINGS[index];
        match setting.kind {
            Kind::Integer { .. } | Kind::Float { .. } | Kind::Choice(_) => {}
            Kind::Toggle | Kind::Text | Kind::Font | Kind::Words | Kind::Color { .. } => {
                return SettingsOutcome::Pending;
            }
        }
        let Some(next) = setting.step(&self.shown(index), delta, &self.themes) else {
            return SettingsOutcome::Pending;
        };
        if next == self.current(index) {
            self.pending = None;
            return SettingsOutcome::Revert;
        }
        let outcome = self.preview_value(setting.key, Some(&next));
        self.pending = Some((index, next));
        outcome
    }

    fn edit(&mut self) -> SettingsOutcome {
        let Some(index) = self.selected() else {
            return SettingsOutcome::Pending;
        };
        let text = match (SETTINGS[index].kind, self.sources[index]) {
            (Kind::Color { .. }, Source::Default) => String::new(),
            (Kind::Color { .. }, Source::File)
            | (
                Kind::Toggle
                | Kind::Integer { .. }
                | Kind::Float { .. }
                | Kind::Choice(_)
                | Kind::Text
                | Kind::Font
                | Kind::Words,
                Source::Default | Source::File,
            ) => self.current(index).display(),
        };
        self.editor = Some(Editor {
            setting: index,
            text: TextCursor::new(&text),
        });
        SettingsOutcome::Pending
    }

    fn reset(&mut self) -> SettingsOutcome {
        let Some(index) = self.selected() else {
            return SettingsOutcome::Pending;
        };
        match self.sources[index] {
            Source::File => {
                self.pending = None;
                SettingsOutcome::Save {
                    key: SETTINGS[index].key,
                    value: None,
                }
            }
            Source::Default => SettingsOutcome::Pending,
        }
    }

    fn accept(&mut self) -> SettingsOutcome {
        let Some(editor) = &self.editor else {
            return SettingsOutcome::Pending;
        };
        let index = editor.setting;
        let setting = &SETTINGS[index];
        match setting.parse(editor.text.text()) {
            Err(message) => {
                self.error = Some((index, message));
                SettingsOutcome::Pending
            }
            Ok(value) => {
                self.editor = None;
                self.error = None;
                let unchanged = match &value {
                    Some(value) => *value == self.current(index),
                    None => self.sources[index] == Source::Default,
                };
                match unchanged {
                    true => SettingsOutcome::Revert,
                    false => SettingsOutcome::Save {
                        key: setting.key,
                        value,
                    },
                }
            }
        }
    }

    fn edited(&mut self) -> SettingsOutcome {
        self.error = None;
        self.preview()
    }

    fn preview(&self) -> SettingsOutcome {
        let Some(editor) = &self.editor else {
            return SettingsOutcome::Pending;
        };
        let setting = &SETTINGS[editor.setting];
        if matches!(setting.kind, Kind::Font) {
            return SettingsOutcome::Pending;
        }
        match setting.parse(editor.text.text()) {
            Ok(value) => self.preview_value(setting.key, value.as_ref()),
            Err(_) => SettingsOutcome::Pending,
        }
    }

    fn preview_value(&self, key: &str, value: Option<&Value>) -> SettingsOutcome {
        apply(&self.file, key, value).map_or(SettingsOutcome::Pending, |(_, config)| {
            SettingsOutcome::Preview(Box::new(config))
        })
    }

    fn close_editor(&mut self) -> SettingsOutcome {
        match self.editor.take() {
            Some(_) => {
                self.error = None;
                SettingsOutcome::Revert
            }
            None => SettingsOutcome::Pending,
        }
    }

    fn leave(&mut self) -> SettingsOutcome {
        if self.editor.is_some() {
            return self.close_editor();
        }
        match self.pending.take() {
            Some((index, value)) => SettingsOutcome::Save {
                key: SETTINGS[index].key,
                value: Some(value),
            },
            None => SettingsOutcome::Pending,
        }
    }

    fn go(&mut self, item: usize) -> SettingsOutcome {
        let moved = item != self.item;
        self.focus = Focus::Items;
        if !moved {
            return SettingsOutcome::Pending;
        }
        let outcome = self.leave();
        self.item = item;
        outcome
    }

    fn move_by(&mut self, delta: i32) -> SettingsOutcome {
        let last = self.items().len().saturating_sub(1);
        let next = self.item.saturating_add_signed(delta as isize).min(last);
        self.go(next)
    }

    fn on_item(
        &mut self,
        item: usize,
        action: impl FnOnce(&mut SettingsPage) -> SettingsOutcome,
    ) -> SettingsOutcome {
        if item >= self.items().len() {
            return SettingsOutcome::Pending;
        }
        let left = self.go(item);
        let acted = action(self);
        then(left, acted)
    }

    fn select_section(&mut self, index: usize) {
        self.section = index.min(Section::ALL.len() - 1);
        self.item = 0;
    }

    fn focus_sections(&mut self) {
        if self.query.is_empty() {
            self.focus = Focus::Sections;
        }
    }

    fn type_query(&mut self, key: &KeyInput) -> SettingsOutcome {
        match &key.text {
            Some(text) => self.change_query(|query| query.insert(text)),
            None => SettingsOutcome::Pending,
        }
    }

    fn change_query(&mut self, edit: impl FnOnce(&mut TextCursor)) -> SettingsOutcome {
        let mut query = self.query.clone();
        edit(&mut query);
        if query.text() == self.query.text() {
            return SettingsOutcome::Pending;
        }
        let outcome = self.leave();
        self.query = query;
        self.focus = Focus::Items;
        self.item = 0;
        outcome
    }

    fn groups(&self, items: &[Item], searching: bool) -> Vec<Group> {
        let mut groups: Vec<Group> = Vec::new();
        for item in items {
            let (heading, entry) = match *item {
                Item::Setting(index) => (
                    searching.then(|| SETTINGS[index].section.title()),
                    self.setting_entry(index),
                ),
                Item::Binding(index) => (Some(BINDINGS), self.binding_entry(index)),
            };
            match groups.last_mut().filter(|group| group.heading == heading) {
                Some(group) => group.entries.push(entry),
                None => groups.push(Group {
                    heading,
                    entries: vec![entry],
                }),
            }
        }
        groups
    }

    fn setting_entry(&self, index: usize) -> Entry {
        let setting = &SETTINGS[index];
        Entry::Setting {
            title: setting.title,
            description: setting.description,
            source: self.sources[index].label(),
            control: self.control(index),
            error: self
                .error
                .as_ref()
                .filter(|(at, _)| *at == index)
                .map(|(_, message)| message.clone()),
        }
    }

    fn control(&self, index: usize) -> Control {
        if let Some(editor) = self
            .editor
            .as_ref()
            .filter(|editor| editor.setting == index)
        {
            return Control::Editor(Field {
                text: editor.text.text().to_owned(),
                cursor: editor.text.cursor(),
                placeholder: EDITOR_PLACEHOLDER,
            });
        }
        let value = self.shown(index);
        match SETTINGS[index].kind {
            Kind::Toggle => Control::Toggle(value == Value::Bool(true)),
            Kind::Integer { .. } | Kind::Float { .. } => Control::Number(value.display()),
            Kind::Choice(_) => Control::Choice(value.display()),
            Kind::Text | Kind::Font | Kind::Words | Kind::Color { .. } => {
                Control::Text(value.display())
            }
        }
    }

    fn binding_entry(&self, index: usize) -> Entry {
        let binding = &self.bindings[index];
        Entry::Binding {
            chord: binding.chord.clone(),
            command: binding
                .command
                .clone()
                .unwrap_or_else(|| UNBOUND.to_owned()),
            unbound: binding.command.is_none(),
            table: binding.table.name(),
            source: binding.source.label(),
        }
    }

    fn footer(&self) -> Vec<(&'static str, String)> {
        let keys: &[(&'static str, &str)] = match (&self.editor, &self.pending) {
            (Some(_), _) => &[(ENTER_KEY, "save"), (ESCAPE_KEY, "revert")],
            (None, Some(_)) => &[
                (ENTER_KEY, "save"),
                (ESCAPE_KEY, "revert"),
                (STEP_KEY, "step"),
            ],
            (None, None) => &[
                (ENTER_KEY, "edit"),
                (SPACE_KEY, "toggle"),
                (STEP_KEY, "step"),
                (DELETE_KEY, "reset"),
                (ESCAPE_KEY, "close"),
                (SEARCH_KEY, "search"),
            ],
        };
        keys.iter()
            .map(|(cap, label)| (*cap, (*label).to_owned()))
            .collect()
    }
}

fn plain(key: &KeyInput) -> bool {
    !key.mods.ctrl && !key.mods.alt && !key.mods.super_
}

fn then(first: SettingsOutcome, second: SettingsOutcome) -> SettingsOutcome {
    match first {
        SettingsOutcome::Save { .. } | SettingsOutcome::Close => first,
        SettingsOutcome::Pending | SettingsOutcome::Preview(_) | SettingsOutcome::Revert => {
            match second {
                SettingsOutcome::Pending => first,
                SettingsOutcome::Preview(_)
                | SettingsOutcome::Revert
                | SettingsOutcome::Save { .. }
                | SettingsOutcome::Close => second,
            }
        }
    }
}

#[cfg(test)]
#[path = "settings_page_tests.rs"]
mod tests;
