use super::*;

#[test]
fn info_and_error_notices_carry_their_kind() {
    assert_eq!(
        Notice::info("config reloaded"),
        Notice {
            text: "config reloaded".into(),
            error: false,
        }
    );
    assert!(Notice::error("could not save theme: denied").error);
}

#[test]
fn a_missing_theme_falls_back_to_flair_dark() {
    assert_eq!(
        Notice::theme_fallback(&ConfigError::UnknownTheme("nord".into())),
        Notice::error("theme nord not found, using flair-dark")
    );
}

#[test]
fn other_theme_errors_use_the_config_error_text() {
    let notice = Notice::theme_fallback(&ConfigError::ThemeName("../x".into()));
    assert!(notice.error);
    assert!(notice.text.starts_with("invalid theme name \"../x\""));
    assert!(notice.text.ends_with(", using flair-dark"));
}
