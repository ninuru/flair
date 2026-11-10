use std::{
    fs, io,
    path::{Path, PathBuf},
};

use flair_core::{
    command::{Command, WindowRef},
    config::{Config, theme_names},
    key::{Key, KeyAction, KeyInput, NamedKey},
    proto::{ClientMsg, ServerMsg},
    status::StatusLine,
    theme::Theme,
};

use crate::{
    chrome_layout::BarField,
    input::{self, LocalAction},
    notice::Notice,
    overlay_layout::{self, OverlayHit, OverlayHits, OverlayInput, OverlayScene, Sheet},
    picker::{self, Picker},
    prompt::{Outcome, Prompt},
    settings_layout,
    settings_page::{SettingsOutcome, SettingsPage},
    theme_picker::{ThemeOutcome, ThemePicker},
};

#[derive(Default)]
pub struct Overlays {
    picker: Option<Picker>,
    prompt: Option<Prompt>,
    theme_picker: Option<ThemePicker>,
    settings: Option<SettingsPage>,
    home: Option<PathBuf>,
    hits: Option<OverlayHits>,
    notice: Option<Notice>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayEvent {
    Ignored,
    Redraw,
    Send(ClientMsg),
    Theme(ThemeOutcome),
    Settings(SettingsOutcome),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub config: Config,
    pub theme: Theme,
}

impl Preview {
    pub fn resolve(config: Config, config_path: &Path) -> Preview {
        let theme = config.load_theme(&Config::themes_dir(config_path)).0;
        Preview { config, theme }
    }
}

impl Overlays {
    pub fn new(home: Option<PathBuf>) -> Overlays {
        Overlays {
            home,
            ..Overlays::default()
        }
    }

    pub fn open(
        &mut self,
        message: &ServerMsg,
        status: &StatusLine,
        config: &Config,
        config_path: &Path,
    ) -> bool {
        match message {
            ServerMsg::Sessionizer {
                sessions,
                current,
                last,
            } => {
                self.picker = Some(Picker::new(
                    sessions.clone(),
                    current,
                    last.clone(),
                    picker::directories(),
                ));
            }
            ServerMsg::ChooseSession { sessions, current } => {
                self.picker = Some(Picker::sessions(sessions.clone(), current));
            }
            ServerMsg::ChooseWindow => self.prompt = Some(Prompt::choose(status)),
            ServerMsg::PromptRename { current } => {
                let current = if current.is_empty() {
                    status
                        .windows
                        .iter()
                        .find(|window| window.active)
                        .map(|window| window.name.clone())
                        .unwrap_or_default()
                } else {
                    current.clone()
                };
                self.prompt = Some(Prompt::rename(current));
            }
            ServerMsg::PromptSearch { forward } => self.prompt = Some(Prompt::search(*forward)),
            ServerMsg::ChooseTheme => {
                self.theme_picker = Some(ThemePicker::new(
                    theme_names(&Config::themes_dir(config_path)),
                    &config.theme,
                ));
            }
            ServerMsg::Settings => {
                let text = match fs::read_to_string(config_path) {
                    Ok(text) => text,
                    Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
                    Err(error) => {
                        self.notice = Some(Notice::error(format!("settings: {error}")));
                        return true;
                    }
                };
                let dir = Config::themes_dir(config_path);
                self.settings = Some(SettingsPage::new(
                    config,
                    config.load_theme(&dir).0,
                    text,
                    theme_names(&dir),
                ));
            }
            ServerMsg::Welcome { .. }
            | ServerMsg::Frame { .. }
            | ServerMsg::Title(_)
            | ServerMsg::Bell
            | ServerMsg::SetClipboard { .. }
            | ServerMsg::Layout(_)
            | ServerMsg::Status(_)
            | ServerMsg::PasteClipboard
            | ServerMsg::ReloadConfig { .. }
            | ServerMsg::Sessions(_)
            | ServerMsg::Done
            | ServerMsg::Error(_)
            | ServerMsg::Exit(_) => return false,
        }
        self.notice = None;
        true
    }

    pub fn set_notice(&mut self, notice: Notice) {
        self.notice = Some(notice);
    }

    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
    }

    pub fn clear_notice(&mut self) {
        self.notice = None;
    }

    pub fn is_open(&self) -> bool {
        self.settings.is_some()
            || self.picker.is_some()
            || self.prompt.is_some()
            || self.theme_picker.is_some()
    }

    pub fn settings_mut(&mut self) -> Option<&mut SettingsPage> {
        self.settings.as_mut()
    }

    pub fn theme_picker_mut(&mut self) -> Option<&mut ThemePicker> {
        self.theme_picker.as_mut()
    }

    pub fn key(
        &mut self,
        key: &KeyInput,
        clipboard: impl FnOnce() -> Option<String>,
    ) -> Option<OverlayEvent> {
        if key.action == KeyAction::Press {
            self.notice = None;
        }
        let direction = match key.key {
            Key::Named(NamedKey::PageUp) => -1,
            Key::Named(NamedKey::PageDown) => 1,
            Key::Char(_) | Key::Named(_) => 0,
        };
        if direction != 0 && key.action == KeyAction::Press && self.panel_open() {
            let page = self.hits.as_ref().map_or(1, OverlayHits::page) as i32;
            return Some(self.wheel(direction * page));
        }
        if let Some(page) = &mut self.settings {
            let outcome = match input::local_action(key) == Some(LocalAction::Paste) {
                true => match clipboard() {
                    Some(text) => page.insert(&text),
                    None => return Some(OverlayEvent::Ignored),
                },
                false => page.key(key),
            };
            return Some(self.settings(outcome));
        }
        if let Some(picker) = &mut self.theme_picker {
            let outcome = picker.key(key);
            return Some(self.theme(outcome));
        }
        if let Some(picker) = &mut self.picker {
            if key.action == KeyAction::Release {
                return Some(OverlayEvent::Ignored);
            }
            if input::local_action(key) == Some(LocalAction::Paste) {
                return Some(match clipboard() {
                    Some(text) => {
                        picker.insert(&text);
                        OverlayEvent::Redraw
                    }
                    None => OverlayEvent::Ignored,
                });
            }
            return Some(self.picker_key(key));
        }
        let prompt = self.prompt.as_mut()?;
        if input::local_action(key) == Some(LocalAction::Paste) {
            return Some(match clipboard() {
                Some(text) => {
                    prompt.insert(&text);
                    OverlayEvent::Redraw
                }
                None => OverlayEvent::Ignored,
            });
        }
        Some(match prompt.key(key) {
            Outcome::Pending => OverlayEvent::Redraw,
            Outcome::Invalid => {
                self.notice = Some(Notice::error("window name cannot be empty"));
                OverlayEvent::Redraw
            }
            Outcome::Close => {
                self.prompt = None;
                OverlayEvent::Redraw
            }
            Outcome::Run(command) => {
                self.prompt = None;
                OverlayEvent::Send(ClientMsg::Command(command))
            }
        })
    }

    pub fn text(&mut self, text: &str) -> Option<OverlayEvent> {
        if let Some(page) = &mut self.settings {
            let outcome = page.insert(text);
            return Some(self.settings(outcome));
        }
        if let Some(picker) = &mut self.theme_picker {
            let outcome = picker.insert(text);
            return Some(self.theme(outcome));
        }
        if let Some(picker) = &mut self.picker {
            picker.insert(text);
            return Some(OverlayEvent::Redraw);
        }
        let prompt = self.prompt.as_mut()?;
        prompt.insert(text);
        Some(OverlayEvent::Redraw)
    }

    pub fn layout(
        &self,
        input: &OverlayInput,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) -> Option<OverlayScene> {
        if let Some(page) = &self.settings {
            return Some(settings_layout::layout(&page.sheet(), input, measure));
        }
        let sheet = self.sheet()?;
        Some(overlay_layout::layout(&sheet, input, measure))
    }

    pub fn place(&mut self, hits: Option<OverlayHits>) {
        self.hits = hits;
    }

    pub fn hit_test(&self, pos: (f64, f64)) -> Option<OverlayHit> {
        match self.panel_open() {
            true => self.hits.as_ref().map(|hits| hits.hit_test(pos)),
            false => None,
        }
    }

    pub fn click(&mut self, hit: OverlayHit) -> OverlayEvent {
        if let Some(page) = &mut self.settings {
            let outcome = match hit {
                OverlayHit::Panel | OverlayHit::Outside => return OverlayEvent::Ignored,
                OverlayHit::Row(index) => page.focus(index),
                OverlayHit::Settings(hit) => page.click(hit),
            };
            return self.settings(outcome);
        }
        match hit {
            OverlayHit::Panel | OverlayHit::Settings(_) => OverlayEvent::Ignored,
            OverlayHit::Outside => self.dismiss(),
            OverlayHit::Row(index) => self.choose(index),
        }
    }

    pub fn wheel(&mut self, rows: i32) -> OverlayEvent {
        if let Some(page) = &mut self.settings {
            let outcome = page.wheel(rows);
            return self.settings(outcome);
        }
        let delta = rows as isize;
        if let Some(picker) = &mut self.theme_picker {
            let outcome = picker.step(delta);
            return self.theme(outcome);
        }
        if let Some(picker) = &mut self.picker {
            picker.move_selection(rows);
            return OverlayEvent::Redraw;
        }
        match &mut self.prompt {
            Some(prompt @ Prompt::Choose { .. }) => {
                prompt.move_selection(delta);
                OverlayEvent::Redraw
            }
            Some(Prompt::Rename { .. } | Prompt::Search { .. }) | None => OverlayEvent::Ignored,
        }
    }

    pub fn bar_text(&self) -> Option<String> {
        self.prompt.as_ref().and_then(Prompt::bar_text)
    }

    pub fn bar_field(&self) -> Option<BarField<'_>> {
        let prompt = self.prompt.as_ref()?;
        Some(BarField {
            label: prompt.field_prefix()?,
            text: prompt.field_text()?,
            cursor: prompt.cursor()?,
        })
    }

    fn sheet(&self) -> Option<Sheet> {
        if let Some(picker) = &self.theme_picker {
            return Some(picker.sheet());
        }
        if let Some(picker) = &self.picker {
            return Some(picker.sheet(self.home.as_deref()));
        }
        self.prompt.as_ref().and_then(Prompt::sheet)
    }

    fn panel_open(&self) -> bool {
        self.settings.is_some()
            || self.theme_picker.is_some()
            || self.picker.is_some()
            || matches!(self.prompt, Some(Prompt::Choose { .. }))
    }

    fn dismiss(&mut self) -> OverlayEvent {
        if self.settings.is_some() {
            return self.settings(SettingsOutcome::Close);
        }
        if self.theme_picker.is_some() {
            return self.theme(ThemeOutcome::Cancel);
        }
        if self.picker.take().is_none() {
            self.prompt = None;
        }
        OverlayEvent::Redraw
    }

    fn choose(&mut self, index: usize) -> OverlayEvent {
        if let Some(picker) = &mut self.theme_picker {
            let outcome = picker.select(index);
            return self.theme(outcome);
        }
        if let Some(picker) = self.picker.take() {
            return match picker.target(index) {
                Some(target) => OverlayEvent::Send(ClientMsg::Command(Command::Switch(target))),
                None => {
                    self.picker = Some(picker);
                    OverlayEvent::Ignored
                }
            };
        }
        let window = match &self.prompt {
            Some(Prompt::Choose { windows, .. }) => windows.get(index).map(|window| window.id),
            Some(Prompt::Rename { .. } | Prompt::Search { .. }) | None => None,
        };
        match window {
            Some(id) => {
                self.prompt = None;
                OverlayEvent::Send(ClientMsg::Command(Command::SelectWindow(WindowRef::Id(id))))
            }
            None => OverlayEvent::Ignored,
        }
    }

    fn theme(&mut self, outcome: ThemeOutcome) -> OverlayEvent {
        match outcome {
            ThemeOutcome::Cancel | ThemeOutcome::Save(_) => self.theme_picker = None,
            ThemeOutcome::Pending | ThemeOutcome::Preview(_) => {}
        }
        OverlayEvent::Theme(outcome)
    }

    fn settings(&mut self, outcome: SettingsOutcome) -> OverlayEvent {
        match outcome {
            SettingsOutcome::Close => self.settings = None,
            SettingsOutcome::Pending
            | SettingsOutcome::Preview(_)
            | SettingsOutcome::Revert
            | SettingsOutcome::Save { .. } => {}
        }
        OverlayEvent::Settings(outcome)
    }

    fn picker_key(&mut self, key: &KeyInput) -> OverlayEvent {
        let Some(picker) = &mut self.picker else {
            return OverlayEvent::Ignored;
        };
        let mut close = false;
        let mut target = None;
        match key.key {
            Key::Named(NamedKey::Escape) => close = true,
            Key::Named(NamedKey::Enter) => {
                target = picker.accept();
                close = target.is_some();
            }
            Key::Named(NamedKey::Up) => picker.move_selection(-1),
            Key::Named(NamedKey::Down) => picker.move_selection(1),
            Key::Char(_) | Key::Named(_) => {
                picker.edit(key);
            }
        }
        if close {
            self.picker = None;
        }
        match target {
            Some(target) => OverlayEvent::Send(ClientMsg::Command(Command::Switch(target))),
            None => OverlayEvent::Redraw,
        }
    }
}

#[cfg(test)]
#[path = "overlays_tests.rs"]
mod tests;
