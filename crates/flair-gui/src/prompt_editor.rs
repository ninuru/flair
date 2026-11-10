use crate::{
    CellMetrics,
    chrome_bar::{opaque, rounded, ui_text},
    overlay_layout::{OverlayInput, OverlayScene},
    renderer::{Border, PxRect, TextFace, TextRun},
    settings_layout::{self, SettingsHit, SettingsSheet},
};
use flair_core::{
    config::{
        AnsiColour, PromptAppearance, PromptConfig, PromptDateFormat, PromptOrder, PromptPath,
        PromptSeparator, PromptStyle, PromptTimeFormat, SegmentConfig,
    },
    isles::{AheadBehind, Changes, Chip, GitInfo, Head, LastCommand, PromptInfo, ShellFacts},
    settings::Value,
    time::Millis,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptAction {
    Select(Chip),
    Add(Chip),
    Move(i8),
    Toggle,
    Icon,
    Colour,
    Appearance,
    Path,
    Time,
    Date,
    Separator,
    GlobalAppearance,
    GlobalIcons,
    Style,
    Duration(i8),
    Reset,
    Save,
    Cancel,
}

pub(crate) struct PromptEditor {
    pub(crate) draft: PromptConfig,
    original: PromptConfig,
    selected: Chip,
    scroll: usize,
}

impl PromptEditor {
    pub(crate) fn new(config: &PromptConfig) -> Self {
        Self {
            draft: config.clone(),
            original: config.clone(),
            selected: Chip::Cwd,
            scroll: 0,
        }
    }

    pub(crate) fn wheel(&mut self, rows: i32) {
        self.scroll = (self.scroll as i32 + rows).clamp(0, 7) as usize;
    }

    pub(crate) fn selected(&self) -> Chip {
        self.selected
    }

    pub(crate) fn dirty(&self) -> bool {
        self.draft != self.original
    }

    pub(crate) fn sync(&mut self, config: &PromptConfig) {
        if !self.dirty() || config == &self.draft {
            self.draft = config.clone();
            self.original = config.clone();
        }
    }

    pub(crate) fn cancel(&mut self) {
        self.draft = self.original.clone();
    }

    pub(crate) fn ordered(&self) -> Vec<Chip> {
        let mut order = self.draft.order.chips().to_vec();
        order.extend(
            Chip::ALL
                .into_iter()
                .filter(|chip| !self.draft.order.chips().contains(chip)),
        );
        order
    }

    pub(crate) fn act(&mut self, action: PromptAction) {
        let chip = self.selected;
        match action {
            PromptAction::Select(chip) => self.selected = chip,
            PromptAction::Add(chip) => {
                self.selected = chip;
                if !self.draft.enabled(chip) {
                    self.toggle(chip);
                }
            }
            PromptAction::Move(delta) => {
                let mut order = self.ordered();
                if let Some(index) = order.iter().position(|item| *item == chip) {
                    let target =
                        (index as i32 + i32::from(delta)).clamp(0, order.len() as i32 - 1) as usize;
                    order.swap(index, target);
                    if let Ok(order) = PromptOrder::from_chips(order) {
                        self.draft.order = order;
                    }
                }
            }
            PromptAction::Toggle => self.toggle(chip),
            PromptAction::Icon => {
                let inherited = self.draft.icons;
                let segment = self.draft.segments.entry(chip).or_default();
                segment.icons = match segment.icons {
                    None => Some(!inherited),
                    Some(false) => Some(true),
                    Some(true) => None,
                };
            }
            PromptAction::Colour => {
                let segment = self.draft.segments.entry(chip).or_default();
                segment.colour = match segment.colour {
                    None => Some(AnsiColour::Black),
                    Some(colour) => match colour.index() {
                        7 => None,
                        index => colour_from_index(index + 1),
                    },
                };
            }
            PromptAction::Appearance => {
                let segment = self.draft.segments.entry(chip).or_default();
                segment.appearance = match segment.appearance {
                    None => Some(PromptAppearance::Outlined),
                    Some(PromptAppearance::Outlined) => Some(PromptAppearance::Filled),
                    Some(PromptAppearance::Filled) => Some(PromptAppearance::Plain),
                    Some(PromptAppearance::Plain) => None,
                };
            }
            PromptAction::Path => {
                self.draft.path = match self.draft.path {
                    PromptPath::Full => PromptPath::Abbreviated,
                    PromptPath::Abbreviated => PromptPath::Folder,
                    PromptPath::Folder => PromptPath::Full,
                }
            }
            PromptAction::Time => {
                self.draft.time_format = match self.draft.time_format {
                    PromptTimeFormat::TwentyFour => PromptTimeFormat::Twelve,
                    PromptTimeFormat::Twelve => PromptTimeFormat::TwentyFour,
                }
            }
            PromptAction::Date => {
                self.draft.date_format = match self.draft.date_format {
                    PromptDateFormat::Iso => PromptDateFormat::Short,
                    PromptDateFormat::Short => PromptDateFormat::Long,
                    PromptDateFormat::Long => PromptDateFormat::Iso,
                }
            }
            PromptAction::Separator => {
                self.draft.separator = match self.draft.separator {
                    PromptSeparator::None => PromptSeparator::Chevron,
                    PromptSeparator::Chevron => PromptSeparator::Dollar,
                    PromptSeparator::Dollar => PromptSeparator::Percent,
                    PromptSeparator::Percent => PromptSeparator::None,
                }
            }
            PromptAction::GlobalAppearance => {
                self.draft.appearance = match self.draft.appearance {
                    PromptAppearance::Outlined => PromptAppearance::Filled,
                    PromptAppearance::Filled => PromptAppearance::Plain,
                    PromptAppearance::Plain => PromptAppearance::Outlined,
                }
            }
            PromptAction::GlobalIcons => self.draft.icons = !self.draft.icons,
            PromptAction::Style => {
                self.draft.style = match self.draft.style {
                    PromptStyle::Chips => PromptStyle::Ps1,
                    PromptStyle::Ps1 => PromptStyle::Chips,
                }
            }
            PromptAction::Duration(delta) => {
                self.draft.duration_ms = (self.draft.duration_ms as i128 + i128::from(delta) * 500)
                    .clamp(0, 600_000) as u64
            }
            PromptAction::Reset => self.draft = PromptConfig::default(),
            PromptAction::Cancel => self.cancel(),
            PromptAction::Save => {}
        }
        self.draft
            .segments
            .retain(|_, segment| *segment != SegmentConfig::default());
    }

    fn toggle(&mut self, chip: Chip) {
        let value = !self.draft.enabled(chip);
        match chip {
            Chip::User => self.draft.user = value,
            Chip::Host => self.draft.host = value,
            Chip::Cwd => self.draft.cwd = value,
            Chip::GitBranch => self.draft.git_branch = value,
            Chip::GitAheadBehind => self.draft.git_ahead_behind = value,
            Chip::GitChanges => self.draft.git_changes = value,
            Chip::Venv => self.draft.venv = value,
            Chip::Docker => self.draft.docker = value,
            Chip::Kube => self.draft.kube = value,
            Chip::Exit => self.draft.exit = value,
            Chip::Time => self.draft.time = value,
            Chip::Date => self.draft.date = value,
        }
    }

    pub(crate) fn changes(&self) -> Vec<(String, Option<Value>)> {
        let config = &self.draft;
        let mut changes = vec![("prompt".into(), None)];
        for (key, value) in [
            ("style", config.style.name()),
            ("path", config.path.name()),
            ("appearance", config.appearance.name()),
            ("time_format", config.time_format.name()),
            ("date_format", config.date_format.name()),
            ("separator", config.separator.name()),
        ] {
            changes.push((format!("prompt.{key}"), Some(Value::Text(value.into()))));
        }
        changes.push((
            "prompt.order".into(),
            Some(Value::Text(config.order.to_string())),
        ));
        changes.push(("prompt.icons".into(), Some(Value::Bool(config.icons))));
        changes.push((
            "prompt.duration_ms".into(),
            Some(Value::Integer(
                config.duration_ms.min(i64::MAX as u64) as i64
            )),
        ));
        for chip in Chip::ALL {
            changes.push((
                format!("prompt.{}", chip.name()),
                Some(Value::Bool(config.enabled(chip))),
            ));
        }
        for (chip, segment) in &config.segments {
            let prefix = format!("prompt.segments.{}", chip.name());
            if let Some(colour) = segment.colour {
                changes.push((
                    format!("{prefix}.colour"),
                    Some(Value::Text(colour.name().into())),
                ));
            }
            if let Some(appearance) = segment.appearance {
                changes.push((
                    format!("{prefix}.appearance"),
                    Some(Value::Text(appearance.name().into())),
                ));
            }
            if let Some(icons) = segment.icons {
                changes.push((format!("{prefix}.icons"), Some(Value::Bool(icons))));
            }
        }
        changes
    }

    pub(crate) fn layout(
        &self,
        sheet: &SettingsSheet,
        input: &OverlayInput,
        metrics: CellMetrics,
        measure: &mut impl FnMut(&str, bool) -> f32,
    ) -> OverlayScene {
        let mut sheet = sheet.clone();
        sheet.groups.clear();
        sheet.focus = None;
        sheet.keys = vec![
            ("Space", "toggle".into()),
            ("Shift ←/→", "reorder".into()),
            ("Enter", "save".into()),
            ("Esc", "cancel".into()),
        ];
        let mut scene = settings_layout::layout(&sheet, input, measure);
        let rect = settings_layout::editor_rect(input);
        if rect.w < 160.0 * input.scale || rect.h < 100.0 * input.scale {
            return scene;
        }
        let scale = input.scale;
        let mut y = rect.y;
        let preview = PxRect {
            x: rect.x,
            y,
            w: rect.w,
            h: 76.0 * scale,
        };
        scene.quads.push(rounded(
            preview,
            input.theme.surfaces().surface1,
            4.0,
            scale,
        ));
        let info = sample();
        let hits = crate::isles::preview(
            input,
            metrics,
            &self.draft,
            &info,
            PxRect {
                x: preview.x + 8.0 * scale,
                y: preview.y + 8.0 * scale,
                w: (preview.w - 16.0 * scale).max(0.0),
                h: metrics.height + 4.0 * scale,
            },
            &mut scene,
        );
        scene.hits.controls.extend(
            hits.into_iter()
                .map(|(chip, rect)| (SettingsHit::PromptEditor(PromptAction::Select(chip)), rect)),
        );
        scene.text.push(TextRun {
            face: TextFace::Mono,
            ..ui_text(
                preview.x + 8.0 * scale,
                preview.y + preview.h - 14.0 * scale,
                format!(
                    "{}cargo build",
                    match self.draft.separator {
                        PromptSeparator::None => "",
                        PromptSeparator::Chevron => "❯ ",
                        PromptSeparator::Dollar => "$ ",
                        PromptSeparator::Percent => "% ",
                    }
                ),
                input.theme.foreground,
                false,
            )
        });
        y += preview.h + 12.0 * scale;
        let gap = 6.0 * scale;
        let mut x = rect.x;
        let mut row_y = y;
        for chip in self.ordered() {
            let text = format!(
                "{} {}",
                if self.draft.enabled(chip) { "✓" } else { "+" },
                title(chip)
            );
            let width = (measure(&text, false) + 16.0 * scale).min(rect.w);
            if x > rect.x && x + width > rect.x + rect.w {
                x = rect.x;
                row_y += 32.0 * scale;
            }
            let at = PxRect {
                x,
                y: row_y,
                w: width,
                h: 26.0 * scale,
            };
            button(
                &text,
                if self.draft.enabled(chip) {
                    PromptAction::Select(chip)
                } else {
                    PromptAction::Add(chip)
                },
                at,
                chip == self.selected,
                input,
                &mut scene,
                measure,
            );
            x += width + gap;
        }
        y = row_y + 38.0 * scale;
        let segment = self
            .draft
            .segments
            .get(&self.selected)
            .copied()
            .unwrap_or_default();
        let controls = [
            (
                format!(
                    "{} {}",
                    if self.draft.enabled(self.selected) {
                        "Remove"
                    } else {
                        "Add"
                    },
                    title(self.selected)
                ),
                PromptAction::Toggle,
            ),
            (
                format!(
                    "Colour: {}",
                    segment.colour.map_or("theme", AnsiColour::name)
                ),
                PromptAction::Colour,
            ),
            (
                format!(
                    "Shape: {}",
                    segment.appearance.map_or("inherit", PromptAppearance::name)
                ),
                PromptAction::Appearance,
            ),
            (
                format!(
                    "Icon: {}",
                    segment
                        .icons
                        .map_or("inherit", |icons| if icons { "on" } else { "off" })
                ),
                PromptAction::Icon,
            ),
            ("Move left".into(), PromptAction::Move(-1)),
            ("Move right".into(), PromptAction::Move(1)),
            (
                format!("Path: {}", self.draft.path.name()),
                PromptAction::Path,
            ),
            (
                format!("Time: {}", self.draft.time_format.name()),
                PromptAction::Time,
            ),
            (
                format!("Date: {}", self.draft.date_format.name()),
                PromptAction::Date,
            ),
            (
                format!("Input: {}", self.draft.separator.name()),
                PromptAction::Separator,
            ),
            (
                format!("Style: {}", self.draft.appearance.name()),
                PromptAction::GlobalAppearance,
            ),
            (
                format!("Icons: {}", if self.draft.icons { "on" } else { "off" }),
                PromptAction::GlobalIcons,
            ),
            (
                format!("Prompt: {}", self.draft.style.name()),
                PromptAction::Style,
            ),
            (
                format!("Duration − ({} ms)", self.draft.duration_ms),
                PromptAction::Duration(-1),
            ),
            ("Duration +500 ms".into(), PromptAction::Duration(1)),
        ];
        let control_columns = if rect.w >= 520.0 * scale { 3 } else { 2 };
        let control_width = (rect.w - gap * (control_columns - 1) as f32) / control_columns as f32;
        let bottom = rect.y + rect.h - 56.0 * scale;
        let visible = ((bottom - y) / (34.0 * scale)).floor().max(1.0) as usize;
        let scroll = self.scroll.min(
            controls
                .len()
                .div_ceil(control_columns)
                .saturating_sub(visible),
        );
        for (index, (text, action)) in controls.iter().enumerate() {
            let at = PxRect {
                x: rect.x + (index % control_columns) as f32 * (control_width + gap),
                y: y + ((index / control_columns) as f32 - scroll as f32) * 34.0 * scale,
                w: control_width,
                h: 28.0 * scale,
            };
            if at.y >= y && at.y + at.h <= bottom {
                button(text, *action, at, false, input, &mut scene, measure);
            }
        }
        scene.text.push(ui_text(
            rect.x,
            bottom + 8.0 * scale,
            "Scroll for more options · Input separator applies to new shells".into(),
            input.theme.surfaces().muted,
            false,
        ));
        let w = (rect.w - gap * 2.0) / 3.0;
        for (index, (text, action)) in [
            ("Reset", PromptAction::Reset),
            ("Cancel", PromptAction::Cancel),
            (
                if self.dirty() {
                    "Save changes"
                } else {
                    "Saved"
                },
                PromptAction::Save,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            button(
                text,
                action,
                PxRect {
                    x: rect.x + index as f32 * (w + gap),
                    y: bottom + 24.0 * scale,
                    w,
                    h: 28.0 * scale,
                },
                action == PromptAction::Save && self.dirty(),
                input,
                &mut scene,
                measure,
            );
        }
        scene
    }
}

fn colour_from_index(index: usize) -> Option<AnsiColour> {
    [
        AnsiColour::Black,
        AnsiColour::Red,
        AnsiColour::Green,
        AnsiColour::Yellow,
        AnsiColour::Blue,
        AnsiColour::Magenta,
        AnsiColour::Cyan,
        AnsiColour::White,
    ]
    .get(index)
    .copied()
}

fn title(chip: Chip) -> &'static str {
    match chip {
        Chip::User => "User",
        Chip::Host => "Host",
        Chip::Cwd => "Directory",
        Chip::GitBranch => "Branch",
        Chip::GitAheadBehind => "Ahead/behind",
        Chip::GitChanges => "Changes",
        Chip::Venv => "Environment",
        Chip::Docker => "Docker",
        Chip::Kube => "Kubernetes",
        Chip::Exit => "Exit / duration",
        Chip::Time => "Time",
        Chip::Date => "Date",
    }
}

fn button(
    text: &str,
    action: PromptAction,
    rect: PxRect,
    selected: bool,
    input: &OverlayInput,
    scene: &mut OverlayScene,
    measure: &mut impl FnMut(&str, bool) -> f32,
) {
    if rect.y < 0.0 || rect.y + rect.h > input.px.1 as f32 || rect.w <= 0.0 {
        return;
    }
    let surfaces = input.theme.surfaces();
    scene.quads.push(crate::renderer::Quad {
        border: Some(Border {
            color: opaque(if selected {
                input.theme.accent
            } else {
                surfaces.outline
            }),
            width: input.scale,
        }),
        ..rounded(
            rect,
            if selected {
                surfaces.surface2
            } else {
                surfaces.surface1
            },
            3.0,
            input.scale,
        )
    });
    let text = crate::chrome_bar::ellipsize(text, rect.w - 12.0 * input.scale, false, measure);
    scene.text.push(ui_text(
        rect.x + 6.0 * input.scale,
        rect.y + rect.h / 2.0 + input.ui.ascent / 2.0 - 2.0 * input.scale,
        text,
        if selected {
            input.theme.foreground
        } else {
            surfaces.muted
        },
        false,
    ));
    scene
        .hits
        .controls
        .push((SettingsHit::PromptEditor(action), rect));
}

fn sample() -> PromptInfo {
    PromptInfo {
        cwd: Some("/home/developer/projects/flair".into()),
        git: Some(GitInfo {
            head: Head::Branch("main".into()),
            upstream: Some(AheadBehind {
                ahead: 1,
                behind: 2,
            }),
            changes: Some(Changes {
                files: 2,
                added: 12,
                removed: 3,
            }),
        }),
        last: Some(LastCommand {
            exit: Some(0),
            duration: Millis(2400),
        }),
        docker: Some("skynetx-prod".into()),
        kube: Some("production".into()),
        shell: Some(ShellFacts {
            user: "developer".into(),
            host: "workstation".into(),
            venv: Some(".venv".into()),
        }),
    }
}

#[cfg(test)]
#[path = "prompt_editor_tests.rs"]
mod tests;
