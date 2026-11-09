use flair_core::config::ConfigError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub error: bool,
}

impl Notice {
    pub fn info(text: impl Into<String>) -> Notice {
        Notice {
            text: text.into(),
            error: false,
        }
    }

    pub fn error(text: impl Into<String>) -> Notice {
        Notice {
            text: text.into(),
            error: true,
        }
    }

    pub fn theme_fallback(error: &ConfigError) -> Notice {
        match error {
            ConfigError::UnknownTheme(name) => {
                Notice::error(format!("theme {name} not found, using flair-dark"))
            }
            ConfigError::Read { .. }
            | ConfigError::Parse(_)
            | ConfigError::Keys(_)
            | ConfigError::NotPositive { .. }
            | ConfigError::ThemeName(_)
            | ConfigError::Theme { .. } => Notice::error(format!("{error}, using flair-dark")),
        }
    }
}

#[cfg(test)]
#[path = "notice_tests.rs"]
mod tests;
