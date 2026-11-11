use super::*;

impl App {
    fn open_prompt_actions(&mut self, target: HitTarget) -> bool {
        let HitTarget::Prompt { pane, row, chip } = target else {
            return false;
        };
        let Some(hit) = self.hits.iter().find(|hit| hit.target == target) else {
            return false;
        };
        let snapshot = self
            .mirror
            .history(pane)
            .and_then(|history| history.iter().find(|snapshot| snapshot.row == row));
        let info = snapshot
            .map(|snapshot| &snapshot.info)
            .or_else(|| self.mirror.prompts().get(&pane));
        let Some(info) = info else { return false };
        let calendar = snapshot.and_then(|snapshot| crate::isles::calendar_at(snapshot.timestamp));
        let value = crate::isles::labels_at(
            info,
            &self.effective().prompt,
            calendar.map(|calendar| calendar.clock).or_else(local_clock),
            calendar
                .map(|calendar| calendar.date)
                .or_else(crate::isles::local_date),
            self.home.as_deref(),
        )
        .into_iter()
        .find(|label| label.chip == chip)
        .map(|label| label.text)
        .unwrap_or_default();
        let menu = PromptMenu::new(pane, chip, info, hit.rect, value);
        let generation = self.overlays.open_prompt(menu);
        if chip == flair_core::isles::Chip::Docker {
            let proxy = self.proxy.clone();
            thread::spawn(move || {
                let result = prompt_actions::docker_contexts();
                let _ = proxy.send_event(UserEvent::PromptContexts { generation, result });
            });
        }
        self.request_redraw();
        true
    }

    pub(super) fn prompt_action(&mut self, action: PromptAction) {
        match action {
            PromptAction::Copy(text) => {
                if let Some(surface) = &self.surface {
                    surface.clipboard.store(text, false);
                }
                self.overlays.set_notice(Notice::info("Copied"));
            }
            PromptAction::Insert { pane, text } => {
                let active = self
                    .layout
                    .as_ref()
                    .map(|layout| layout.active)
                    .or(self.pane);
                if prompt_actions::can_insert(pane, active, &self.mirror) {
                    self.send(ClientMsg::Input(InputEvent::Paste(text)));
                } else {
                    self.overlays.set_notice(Notice::error(
                        "Use an empty prompt in this pane before inserting a command",
                    ));
                }
            }
            PromptAction::DockerContext(name) => {
                let proxy = self.proxy.clone();
                thread::spawn(move || {
                    let result = prompt_actions::switch_docker_context(&name);
                    let _ = proxy.send_event(UserEvent::PromptActionFinished(result));
                });
            }
        }
        self.request_redraw();
    }

    pub(super) fn prompt_action_finished(&mut self, result: Result<(), String>) {
        self.overlays.set_notice(match result {
            Ok(()) => Notice::info("Docker context switched"),
            Err(error) => Notice::error(error),
        });
        self.request_redraw();
    }

    pub(super) fn mouse(&mut self, button: MouseButton, action: MouseAction) {
        let Some(point) = self.cell() else { return };
        if self.capture == Capture::Idle
            && button == MouseButton::Left
            && action == MouseAction::Press
            && !self.mods.shift_key()
            && let Some(target @ HitTarget::Prompt { .. }) =
                chrome_layout::hit_test(&self.hits, (self.pointer.x, self.pointer.y))
            && self.open_prompt_actions(target)
        {
            self.capture = Capture::Overlay;
            return;
        }
        let route = route_mouse(self.in_bar(), button, action, self.capture);
        self.capture = capture_after(self.capture, button, action, route);
        match route {
            MouseRoute::Bar => {
                if let Some(target) =
                    chrome_layout::hit_test(&self.hits, (self.pointer.x, self.pointer.y))
                    && let Some(command) = bar_command(target)
                {
                    self.send(ClientMsg::Command(command));
                }
            }
            MouseRoute::Drop => {}
            MouseRoute::Server => {
                let mods = flair_core::key::Mods {
                    ctrl: self.mods.control_key(),
                    shift: self.mods.shift_key(),
                    alt: self.mods.alt_key(),
                    super_: self.mods.super_key(),
                };
                self.send(ClientMsg::Input(InputEvent::Mouse(MouseInput {
                    point,
                    button,
                    action,
                    mods,
                })));
            }
        }
    }
}
