use flair_core::{
    ids::PaneId,
    isles::{Chip, Head, PromptInfo},
    key::{Key, KeyAction, KeyInput, NamedKey},
};

use crate::renderer::PxRect;

#[path = "prompt_actions_layout.rs"]
mod layout;
#[path = "prompt_actions_worker.rs"]
mod worker;

pub(crate) use worker::{docker_contexts, switch_docker_context};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAction {
    Copy(String),
    Insert { pane: PaneId, text: String },
    DockerContext(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuOutcome {
    Pending,
    Close,
    Action(PromptAction),
}

pub struct PromptMenu {
    pub(crate) pane: PaneId,
    pub(crate) anchor: PxRect,
    pub(crate) chip: Chip,
    pub(crate) query: String,
    pub(crate) selected: usize,
    pub(crate) items: Vec<MenuItem>,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    current: String,
}

pub(crate) struct MenuItem {
    pub name: String,
    pub current: bool,
    pub action: PromptAction,
}

impl PromptMenu {
    pub fn new(pane: PaneId, chip: Chip, info: &PromptInfo, anchor: PxRect, value: String) -> Self {
        let mut menu = Self {
            pane,
            anchor,
            chip,
            query: String::new(),
            selected: 0,
            items: Vec::new(),
            loading: chip == Chip::Docker,
            error: None,
            current: info.docker.clone().unwrap_or_else(|| "default".into()),
        };
        match chip {
            Chip::Docker => {}
            Chip::Cwd => {
                if let Some(path) = &info.cwd {
                    let path = path.display().to_string();
                    menu.add("Copy full path", PromptAction::Copy(path.clone()));
                    menu.add(
                        "Insert cd command",
                        menu.command(format!("cd -- {}", quote(&path))),
                    );
                }
            }
            Chip::GitBranch => {
                if let Some(git) = &info.git {
                    let (Head::Branch(branch) | Head::Detached(branch)) = &git.head;
                    menu.add("Copy branch", PromptAction::Copy(branch.clone()));
                }
                menu.add("Insert git status", menu.command("git status".into()));
                menu.add("Insert git diff", menu.command("git diff".into()));
            }
            Chip::GitAheadBehind | Chip::GitChanges => {
                menu.add("Copy value", PromptAction::Copy(value));
                menu.add("Insert git status", menu.command("git status".into()));
                menu.add("Insert git diff", menu.command("git diff".into()));
            }
            Chip::User
            | Chip::Host
            | Chip::Venv
            | Chip::Kube
            | Chip::Exit
            | Chip::Time
            | Chip::Date => {
                menu.add("Copy value", PromptAction::Copy(value));
            }
        }
        menu
    }

    pub fn contexts(&mut self, result: Result<Vec<String>, String>) {
        self.loading = false;
        self.error = result.as_ref().err().cloned();
        self.items = match result {
            Ok(names) => names
                .into_iter()
                .map(|name| MenuItem {
                    current: name == self.current,
                    action: PromptAction::DockerContext(name.clone()),
                    name,
                })
                .collect(),
            Err(_) => vec![MenuItem {
                name: "Copy current context".into(),
                current: false,
                action: PromptAction::Copy(self.current.clone()),
            }],
        };
        self.selected = 0;
    }

    pub fn insert(&mut self, text: &str) {
        self.query.extend(
            text.chars()
                .filter(|c| !c.is_control())
                .take(256usize.saturating_sub(self.query.chars().count())),
        );
        self.selected = 0;
    }

    pub fn key(&mut self, key: &KeyInput) -> MenuOutcome {
        if key.action == KeyAction::Release {
            return MenuOutcome::Pending;
        }
        match key.key {
            Key::Named(NamedKey::Escape) => return MenuOutcome::Close,
            Key::Named(NamedKey::Enter) => return self.choose(self.selected),
            Key::Named(NamedKey::Up) => self.step(-1),
            Key::Named(NamedKey::Down | NamedKey::Tab) => self.step(1),
            Key::Named(NamedKey::Backspace) => {
                self.query.pop();
                self.selected = 0;
            }
            Key::Char(_) if !key.mods.ctrl && !key.mods.alt && !key.mods.super_ => {
                if let Some(text) = &key.text {
                    self.insert(text);
                }
            }
            Key::Char(_) | Key::Named(_) => {}
        }
        MenuOutcome::Pending
    }

    pub fn step(&mut self, delta: i32) {
        let count = self.filtered().len();
        if count > 0 {
            self.selected =
                (self.selected as i64 + i64::from(delta)).rem_euclid(count as i64) as usize;
        }
    }

    pub fn choose(&self, index: usize) -> MenuOutcome {
        self.filtered()
            .get(index)
            .map_or(MenuOutcome::Pending, |item| {
                MenuOutcome::Action(item.action.clone())
            })
    }

    pub(crate) fn filtered(&self) -> Vec<&MenuItem> {
        let query = self.query.to_lowercase();
        self.items
            .iter()
            .filter(|item| item.name.to_lowercase().contains(&query))
            .collect()
    }

    fn add(&mut self, name: &str, action: PromptAction) {
        self.items.push(MenuItem {
            name: name.into(),
            current: false,
            action,
        });
    }

    fn command(&self, text: String) -> PromptAction {
        PromptAction::Insert {
            pane: self.pane,
            text,
        }
    }
}

pub(crate) fn can_insert(
    pane: PaneId,
    active: Option<PaneId>,
    mirror: &crate::mirror::Mirror,
) -> bool {
    active == Some(pane)
        && mirror.pane(pane).is_some_and(|pane| {
            !pane.flags.alt_screen
                && !pane.flags.copy_mode
                && pane.flags.display_offset == 0
                && pane
                    .flags
                    .prompt_row
                    .is_some_and(|row| row.checked_add(1) == Some(pane.cursor.point.row))
                && pane.cursor.point.col == 0
                && pane
                    .rows
                    .get(usize::from(pane.cursor.point.row))
                    .is_some_and(|row| row.iter().all(|cell| cell.c == ' ' && cell.extra.is_none()))
        })
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn context_names(output: &str) -> Vec<String> {
    let mut names: Vec<_> = output
        .lines()
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 256
                && !name.starts_with('-')
                && name.chars().all(|c| {
                    !c.is_control()
                        && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
                })
        })
        .take(256)
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
#[path = "prompt_actions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prompt_actions_render_tests.rs"]
mod render_tests;
