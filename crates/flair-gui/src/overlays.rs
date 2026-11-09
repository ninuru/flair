use std::path::Path;

use flair_core::{
    command::Command,
    config::{Config, theme_names},
    key::{Key, KeyAction, KeyInput, NamedKey},
    proto::{ClientMsg, ServerMsg},
    status::StatusLine,
    theme::Theme,
};

use crate::{
    CellMetrics,
    cellgrid::CellGrid,
    chrome_layout::{self, Modal},
    input::{self, LocalAction},
    picker::{self, Picker},
    prompt::{Outcome, Prompt},
    theme_picker::{ThemeOutcome, ThemePicker},
};

#[derive(Default)]
pub struct Overlays {
    picker: Option<Picker>,
    prompt: Option<Prompt>,
    theme_picker: Option<ThemePicker>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OverlayEvent {
    Ignored,
    Redraw,
    Send(ClientMsg),
    Theme(ThemeOutcome),
}

impl Overlays {
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
        true
    }

    pub fn is_open(&self) -> bool {
        self.picker.is_some() || self.prompt.is_some() || self.theme_picker.is_some()
    }

    pub fn theme_picker_mut(&mut self) -> Option<&mut ThemePicker> {
        self.theme_picker.as_mut()
    }

    pub fn key(
        &mut self,
        key: &KeyInput,
        clipboard: impl FnOnce() -> Option<String>,
    ) -> Option<OverlayEvent> {
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
            Outcome::Pending | Outcome::Invalid => OverlayEvent::Redraw,
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

    pub fn draw(
        &self,
        grid: &mut CellGrid,
        metrics: CellMetrics,
        scale: f32,
        theme: &Theme,
    ) -> Option<Modal> {
        let area = chrome_layout::modal_area(grid.size, metrics, scale);
        let mut lifted = None;
        if let Some(picker) = &self.picker {
            lifted = Some(Modal::Centered(picker.draw(grid, area, theme)));
        }
        if let Some(prompt) = &self.prompt {
            lifted = prompt
                .draw(grid, area, theme)
                .map(Modal::Centered)
                .or(lifted);
        }
        if let Some(picker) = &self.theme_picker {
            let area = ThemePicker::area(grid.size, metrics, scale);
            lifted = Some(Modal::Docked(picker.draw(grid, area, theme)));
        }
        lifted
    }

    pub fn bar_text(&self) -> Option<String> {
        self.prompt.as_ref().and_then(Prompt::bar_text)
    }

    fn theme(&mut self, outcome: ThemeOutcome) -> OverlayEvent {
        match outcome {
            ThemeOutcome::Cancel | ThemeOutcome::Save(_) => self.theme_picker = None,
            ThemeOutcome::Pending | ThemeOutcome::Preview(_) => {}
        }
        OverlayEvent::Theme(outcome)
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
