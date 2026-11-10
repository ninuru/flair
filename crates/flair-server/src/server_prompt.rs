use super::*;
use crate::{event::Timer, facts::Contexts};
use flair_core::isles::{Chip, GitInfo, LastCommand, PromptInfo};
use std::path::{Path, PathBuf};

const REFRESH: Millis = Millis(5000);

pub(super) struct GitEntry {
    info: Option<GitInfo>,
    pending: Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pending {
    Idle,
    Running,
    Again,
}

impl Pending {
    fn request(&mut self) -> bool {
        let (next, send) = match self {
            Pending::Idle => (Pending::Running, true),
            Pending::Running | Pending::Again => (Pending::Again, false),
        };
        *self = next;
        send
    }

    fn finish(&mut self) -> bool {
        let (next, again) = match self {
            Pending::Again => (Pending::Running, true),
            Pending::Idle | Pending::Running => (Pending::Idle, false),
        };
        *self = next;
        again
    }
}

impl Server {
    fn git_on(&self) -> bool {
        let prompt = &self.config.prompt;
        self.config.prompt.band() > 0
            && [Chip::GitBranch, Chip::GitAheadBehind, Chip::GitChanges]
                .into_iter()
                .any(|chip| prompt.enabled(chip))
    }

    fn contexts_on(&self) -> bool {
        let prompt = &self.config.prompt;
        self.config.prompt.band() > 0
            && (prompt.enabled(Chip::Docker) || prompt.enabled(Chip::Kube))
    }

    pub(super) fn pane_cwd(&self, pane: PaneId) -> Option<PathBuf> {
        let pane = self.panes.get(&pane)?;
        Some(
            pane.terminal
                .cwd()
                .map_or_else(|| pane.cwd.clone(), Path::to_path_buf),
        )
    }

    pub(super) fn prompt_marks(
        &mut self,
        now: Millis,
        pane: PaneId,
        kind: MarkKind,
        code: Option<i32>,
    ) {
        match kind {
            MarkKind::CommandStart => {
                self.set_started(pane, Some(now));
                if let Some(state) = self.panes.get_mut(&pane) {
                    state.last = None;
                }
            }
            MarkKind::CommandEnd => {
                if let Some(state) = self.panes.get_mut(&pane)
                    && let Some(start) = state.started
                {
                    state.last = Some(LastCommand {
                        exit: code,
                        duration: now.saturating_sub(start),
                    });
                }
                self.set_started(pane, None);
            }
            MarkKind::PromptStart => self.set_started(pane, None),
            MarkKind::PromptEnd => {}
        }
    }

    fn read_git(&self, cwd: PathBuf) -> Effect {
        let panes = self
            .panes
            .keys()
            .copied()
            .filter(|pane| self.pane_cwd(*pane).as_ref() == Some(&cwd))
            .collect();
        Effect::ReadGit {
            cwd,
            changes: self.config.prompt.git_changes,
            panes,
        }
    }

    pub(super) fn want_git(&mut self, cwd: PathBuf, out: &mut Vec<Effect>) {
        if !self.git_on() {
            return;
        }
        let entry = self.git.entry(cwd.clone()).or_insert(GitEntry {
            info: None,
            pending: Pending::Idle,
        });
        if entry.pending.request() {
            out.push(self.read_git(cwd));
        }
    }

    pub(super) fn git_read(&mut self, cwd: PathBuf, info: Option<GitInfo>, out: &mut Vec<Effect>) {
        let Some(entry) = self.git.get_mut(&cwd) else {
            return;
        };
        entry.info = info;
        if entry.pending.finish() {
            out.push(self.read_git(cwd));
        }
    }

    pub(super) fn want_contexts(&mut self, out: &mut Vec<Effect>) {
        if self.contexts_on() && self.contexts.1.request() {
            out.push(Effect::ReadContexts);
        }
    }

    pub(super) fn contexts_read(&mut self, contexts: Contexts, out: &mut Vec<Effect>) {
        self.contexts.0 = contexts;
        if self.contexts.1.finish() {
            out.push(Effect::ReadContexts);
        }
    }

    pub(super) fn prompt_info(&self, pane: PaneId, cwd: PathBuf) -> PromptInfo {
        let prompt = &self.config.prompt;
        let state = &self.panes[&pane];
        let git = self
            .git
            .get(&cwd)
            .and_then(|entry| entry.info.clone())
            .filter(|_| self.git_on())
            .map(|info| GitInfo {
                upstream: info
                    .upstream
                    .filter(|_| prompt.enabled(Chip::GitAheadBehind)),
                changes: info.changes.filter(|_| prompt.enabled(Chip::GitChanges)),
                head: info.head,
            });
        PromptInfo {
            cwd: prompt
                .enabled(Chip::Cwd)
                .then_some(cwd)
                .filter(|cwd| cwd.to_str().is_some()),
            git,
            last: state.last.filter(|_| prompt.enabled(Chip::Exit)),
            shell: state.shell.clone().filter(|_| {
                [Chip::User, Chip::Host, Chip::Venv]
                    .into_iter()
                    .any(|chip| prompt.enabled(chip))
            }),
            docker: self
                .contexts
                .0
                .docker
                .clone()
                .filter(|_| prompt.enabled(Chip::Docker)),
            kube: self
                .contexts
                .0
                .kube
                .clone()
                .filter(|_| prompt.enabled(Chip::Kube)),
        }
    }

    fn prompt_panes(&self, window: WindowId) -> Vec<PaneId> {
        let band = self.config.prompt.band();
        if band == 0 {
            return Vec::new();
        }
        self.tree.window(window).map_or_else(Vec::new, |window| {
            window
                .geometry(0)
                .panes
                .into_iter()
                .map(|placement| placement.pane)
                .collect()
        })
    }

    pub(super) fn flush_prompts(&mut self, ready: &BTreeSet<ClientId>, out: &mut Vec<Effect>) {
        if !std::mem::take(&mut self.prompts_dirty) {
            return;
        }
        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
            let panes = self.prompt_panes(self.clients[&id].view.window);
            for pane in &panes {
                let Some(cwd) = self.pane_cwd(*pane) else {
                    continue;
                };
                if !self.git.contains_key(&cwd) {
                    self.want_git(cwd.clone(), out);
                }
                let info = self.prompt_info(*pane, cwd);
                let state = self.panes.get_mut(pane).unwrap();
                state.terminal.set_prompt_info(info.clone());
                let prompts = state.terminal.prompt_history();
                if let Some(client) = self.clients.get_mut(&id)
                    && ready.contains(&id)
                    && client.prompt_history.get(pane) != Some(&prompts)
                {
                    client.prompt_history.insert(*pane, prompts.clone());
                    out.push(Effect::Send(
                        id,
                        ServerMsg::PromptHistory {
                            pane: *pane,
                            prompts,
                        },
                    ));
                }
                if let Some(client) = self.clients.get_mut(&id)
                    && client.prompts.get(pane) != Some(&info)
                {
                    client.prompts.insert(*pane, info.clone());
                    out.push(Effect::Send(
                        id,
                        ServerMsg::PromptInfo { pane: *pane, info },
                    ));
                }
            }
            if let Some(client) = self.clients.get_mut(&id) {
                client.prompts.retain(|pane, _| panes.contains(pane));
                client.prompt_history.retain(|pane, _| panes.contains(pane));
            }
        }
    }

    pub(super) fn arm_refresh(&mut self, now: Millis, out: &mut Vec<Effect>) {
        if !self.refresh_armed && !self.clients.is_empty() && (self.git_on() || self.contexts_on())
        {
            self.refresh_armed = true;
            out.push(Effect::SetTimer(Timer::Prompt, now + REFRESH));
        }
    }

    pub(super) fn prompt_timer(&mut self, out: &mut Vec<Effect>) {
        self.refresh_armed = false;
        let visible: BTreeSet<PathBuf> = self
            .clients
            .values()
            .flat_map(|client| self.prompt_panes(client.view.window))
            .filter_map(|pane| self.pane_cwd(pane))
            .collect();
        for cwd in visible {
            self.want_git(cwd, out);
        }
        self.want_contexts(out);
        let used: BTreeSet<PathBuf> = self
            .panes
            .keys()
            .filter_map(|pane| self.pane_cwd(*pane))
            .collect();
        self.git
            .retain(|cwd, entry| used.contains(cwd) || entry.pending != Pending::Idle);
    }
}

#[cfg(test)]
#[path = "server_prompt_tests.rs"]
mod tests;
