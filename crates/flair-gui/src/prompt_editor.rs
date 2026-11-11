use std::cmp::Ordering;

use flair_core::{
    config::{
        AnsiColour, PromptAppearance, PromptConfig, PromptDateFormat, PromptOrder, PromptPath,
        PromptSeparator, PromptStyle, PromptTimeFormat, SegmentConfig,
    },
    isles::Chip,
    settings::Value,
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
    scroll_limit: usize,
}

impl PromptEditor {
    pub(crate) fn new(config: &PromptConfig) -> Self {
        Self {
            draft: config.clone(),
            original: config.clone(),
            selected: Chip::Cwd,
            scroll: 0,
            scroll_limit: 0,
        }
    }

    pub(crate) fn wheel(&mut self, rows: i32) {
        self.scroll =
            (self.scroll as i64 + i64::from(rows)).clamp(0, self.scroll_limit as i64) as usize;
    }

    pub(crate) fn set_scroll_limit(&mut self, limit: usize) {
        self.scroll_limit = limit;
        self.scroll = self.scroll.min(limit);
    }

    pub(crate) fn selected(&self) -> Chip {
        self.selected
    }

    pub(crate) fn dirty(&self) -> bool {
        self.draft != self.original
    }

    pub(crate) fn available(&self, action: PromptAction) -> bool {
        match action {
            PromptAction::Save | PromptAction::Cancel => self.dirty(),
            PromptAction::Reset => self.draft != PromptConfig::default(),
            PromptAction::Move(delta) => {
                let order = self.ordered();
                order
                    .iter()
                    .position(|chip| *chip == self.selected)
                    .is_some_and(|index| {
                        let target = index as i32 + i32::from(delta);
                        delta != 0 && target >= 0 && target < order.len() as i32
                    })
            }
            PromptAction::Duration(delta) => match delta.cmp(&0) {
                Ordering::Less => self.draft.duration_ms > 0,
                Ordering::Greater => self.draft.duration_ms < 600_000,
                Ordering::Equal => false,
            },
            PromptAction::Select(_)
            | PromptAction::Add(_)
            | PromptAction::Toggle
            | PromptAction::Icon
            | PromptAction::Colour
            | PromptAction::Appearance
            | PromptAction::Path
            | PromptAction::Time
            | PromptAction::Date
            | PromptAction::Separator
            | PromptAction::GlobalAppearance
            | PromptAction::GlobalIcons
            | PromptAction::Style => true,
        }
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

#[cfg(test)]
#[path = "prompt_editor_tests.rs"]
mod tests;

#[path = "prompt_editor_layout.rs"]
mod layout;
