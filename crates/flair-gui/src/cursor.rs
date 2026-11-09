use flair_core::key::{Key, KeyAction, KeyInput, NamedKey};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextCursor {
    text: String,
    cursor: usize,
    limit: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    Changed,
    Moved,
    Ignored,
}

impl TextCursor {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.chars().filter(|c| !c.is_control()).collect(),
            cursor: text.chars().filter(|c| !c.is_control()).count(),
            limit: usize::MAX,
        }
    }

    pub fn with_limit(self, limit: usize) -> Self {
        let mut text = String::new();
        for c in self.text.chars() {
            if text.len() + c.len_utf8() > limit {
                break;
            }
            text.push(c);
        }
        let cursor = self.cursor.min(text.chars().count());
        Self {
            text,
            cursor,
            limit,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn insert(&mut self, s: &str) {
        for c in s.chars().filter(|c| !c.is_control()) {
            if self.text.len() + c.len_utf8() > self.limit {
                return;
            }
            let at = self.byte(self.cursor);
            self.text.insert(at, c);
            self.cursor += 1;
        }
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.remove(self.cursor - 1, self.cursor);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.len() {
            self.remove(self.cursor, self.cursor + 1);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.len());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.len();
    }

    pub fn kill_line(&mut self) {
        self.remove(self.cursor, self.len());
    }

    pub fn clear(&mut self) {
        self.remove(0, self.cursor);
    }

    pub fn word_left(&mut self) {
        self.cursor = self.word_start();
    }

    pub fn word_right(&mut self) {
        let chars: Vec<_> = self.text.chars().collect();
        let mut at = self.cursor;
        while at < chars.len() && !is_word(chars[at]) {
            at += 1;
        }
        while at < chars.len() && is_word(chars[at]) {
            at += 1;
        }
        self.cursor = at;
    }

    pub fn delete_word_back(&mut self) {
        let start = self.word_start();
        self.remove(start, self.cursor);
    }

    pub fn edit(&mut self, key: &KeyInput) -> Edit {
        if key.action == KeyAction::Release {
            return Edit::Ignored;
        }
        let before = (self.text.len(), self.cursor);
        let mods = key.mods;
        let plain = !mods.ctrl && !mods.alt && !mods.super_;
        match key.key {
            Key::Named(NamedKey::Left) if mods.ctrl || mods.alt => self.word_left(),
            Key::Named(NamedKey::Right) if mods.ctrl || mods.alt => self.word_right(),
            Key::Named(NamedKey::Left) => self.left(),
            Key::Named(NamedKey::Right) => self.right(),
            Key::Named(NamedKey::Home) => self.home(),
            Key::Named(NamedKey::End) => self.end(),
            Key::Named(NamedKey::Backspace) if mods.ctrl || mods.alt => self.delete_word_back(),
            Key::Named(NamedKey::Backspace) => self.backspace(),
            Key::Named(NamedKey::Delete) => self.delete(),
            Key::Char(c) if mods.ctrl && !mods.alt && !mods.super_ => {
                match c.to_ascii_lowercase() {
                    'a' => self.home(),
                    'e' => self.end(),
                    'b' => self.left(),
                    'f' => self.right(),
                    'u' => self.clear(),
                    'k' => self.kill_line(),
                    'w' => self.delete_word_back(),
                    _ => return Edit::Ignored,
                }
            }
            Key::Char(c) if mods.alt && !mods.ctrl && !mods.super_ => {
                match c.to_ascii_lowercase() {
                    'b' => self.word_left(),
                    'f' => self.word_right(),
                    _ => return Edit::Ignored,
                }
            }
            Key::Char(_) if plain => match &key.text {
                Some(text) => self.insert(text),
                None => return Edit::Ignored,
            },
            Key::Char(_) | Key::Named(_) => return Edit::Ignored,
        }
        if self.text.len() != before.0 {
            Edit::Changed
        } else if self.cursor != before.1 {
            Edit::Moved
        } else {
            Edit::Ignored
        }
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte(&self, index: usize) -> usize {
        self.text
            .char_indices()
            .nth(index)
            .map_or(self.text.len(), |(at, _)| at)
    }

    fn remove(&mut self, from: usize, to: usize) {
        let range = self.byte(from)..self.byte(to);
        self.text.replace_range(range, "");
        self.cursor = from;
    }

    fn word_start(&self) -> usize {
        let chars: Vec<_> = self.text.chars().collect();
        let mut at = self.cursor;
        while at > 0 && !is_word(chars[at - 1]) {
            at -= 1;
        }
        while at > 0 && is_word(chars[at - 1]) {
            at -= 1;
        }
        at
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
