use flair_core::{
    command::Command,
    config::Config,
    proto::ClientMsg,
    settings::{SETTINGS, Value, position},
};

use super::App;
use crate::{
    Fonts,
    notice::Notice,
    overlays::Preview,
    settings_page::SettingsOutcome,
    theme_file::{save, save_theme},
    theme_picker::ThemeOutcome,
};

impl App {
    pub(super) fn effective(&self) -> &Config {
        self.preview
            .as_ref()
            .map_or(&self.config, |preview| &preview.config)
    }

    pub(super) fn theme_outcome(&mut self, outcome: ThemeOutcome) {
        match outcome {
            ThemeOutcome::Pending => {}
            ThemeOutcome::Preview(name) => {
                self.set_preview(Some(self.config.with_theme(&name)));
            }
            ThemeOutcome::Cancel => self.set_preview(None),
            ThemeOutcome::Save(name) => match save_theme(&self.config_path, &name) {
                Ok(config) => {
                    self.overlays
                        .set_notice(Notice::info(format!("theme saved: {name}")));
                    self.reload(config);
                    self.send(ClientMsg::Command(Command::ReloadConfig));
                }
                Err(error) => {
                    log::warn!(
                        "could not save theme {name:?} to {}: {error}",
                        self.config_path.display()
                    );
                    self.overlays
                        .set_notice(Notice::error(format!("could not save theme: {error}")));
                    self.set_preview(None);
                }
            },
        }
        self.request_redraw();
    }

    pub(super) fn settings_outcome(&mut self, outcome: SettingsOutcome) {
        match outcome {
            SettingsOutcome::Pending => {}
            SettingsOutcome::Preview(config) => self.set_preview(Some(*config)),
            SettingsOutcome::Revert | SettingsOutcome::Close => self.set_preview(None),
            SettingsOutcome::Save { key, value } => self.save_setting(key, value),
        }
        self.request_redraw();
    }

    pub(super) fn refresh_overlays(&mut self, text: &str) {
        if let Some(picker) = self.overlays.theme_picker_mut() {
            let outcome = picker.refresh(&self.config.theme);
            self.theme_outcome(outcome);
        }
        if let Some(page) = self.overlays.settings_mut() {
            let outcome = page.refresh(&self.config, self.theme.clone(), text.to_owned());
            self.settings_outcome(outcome);
        }
    }

    fn set_preview(&mut self, preview: Option<Config>) {
        let font = self.effective().font.clone();
        self.preview = preview.map(|config| Preview::resolve(config, &self.config_path));
        let next = self.effective().font.clone();
        if next != font
            && let Some(surface) = &mut self.surface
        {
            match Fonts::load(&next, surface.window.scale_factor()) {
                Ok(fonts) => {
                    surface.renderer.set_metrics(fonts.metrics());
                    surface.fonts = fonts;
                }
                Err(error) => {
                    log::warn!("could not load the previewed font: {error}");
                    self.overlays
                        .set_notice(Notice::error(format!("font not loaded: {error}")));
                }
            }
        }
        self.send(ClientMsg::Resize(self.size()));
        self.request_redraw();
    }

    fn save_setting(&mut self, key: &'static str, value: Option<Value>) {
        match save(&self.config_path, key, value.as_ref()) {
            Ok((text, config)) => {
                let title = position(key).map_or(key, |index| SETTINGS[index].title);
                self.overlays
                    .set_notice(Notice::info(format!("saved {title}")));
                self.reload(config);
                self.send(ClientMsg::Command(Command::ReloadConfig));
                self.refresh_overlays(&text);
            }
            Err(error) => {
                log::warn!(
                    "could not save {key} to {}: {error}",
                    self.config_path.display()
                );
                self.set_preview(None);
                if let Some(page) = self.overlays.settings_mut() {
                    page.fail(key, &error.to_string());
                }
            }
        }
    }
}
