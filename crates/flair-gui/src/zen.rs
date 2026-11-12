#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Zen {
    on: bool,
    saved: Option<bool>,
}

impl Zen {
    pub fn on(self) -> bool {
        self.on
    }

    pub fn toggled(self) -> Self {
        Self {
            on: !self.on,
            ..self
        }
    }

    pub fn fullscreen(self, fullscreen: bool) -> Self {
        match (fullscreen, self.saved) {
            (true, None) => Self {
                on: true,
                saved: Some(self.on),
            },
            (false, Some(on)) => Self { on, saved: None },
            (true, Some(_)) | (false, None) => self,
        }
    }
}

#[cfg(test)]
#[path = "zen_tests.rs"]
mod tests;
