use std::process;

use super::*;

const BLUEPRINT_EXAMPLE: &str = r##"
[shell]
program = "/bin/zsh"
args = ["-l"]

[env]
TERM = "xterm-256color"
COLORTERM = "truecolor"

[font]
family = "JetBrainsMono Nerd Font Mono"
size = 13.0
line_height = 1.0

[window]
padding_x = 16
padding_y = 8

[scrollback]
lines = 10000

[cursor]
style = "block"
blink = false

[bell]
audible = false

[colors]
background = "#030303"
foreground = "#d4d4d4"
cursor = "#d4d4d4"
selection = "#19aad840"
accent = "#19aad8"
normal = ["#1e1e1e", "#e06c75", "#98c379", "#e5c07b", "#61afef", "#c678dd", "#56b6c2", "#d4d4d4"]
bright = ["#5c6370", "#ff7a85", "#b5e890", "#ffd68a", "#7cc4ff", "#de8cff", "#6fd6e2", "#ffffff"]
"##;

#[test]
fn empty_text_gives_defaults() {
    assert_eq!(Config::from_toml("").unwrap(), Config::default());
}

#[test]
fn missing_file_gives_defaults_without_error() {
    let path = env::temp_dir()
        .join(format!("flair-missing-{}", process::id()))
        .join("flair.toml");
    let (config, error) = Config::load(&path);
    assert_eq!(config, Config::default());
    assert!(error.is_none());
}

#[test]
fn blueprint_example_equals_defaults() {
    assert_eq!(
        Config::from_toml(BLUEPRINT_EXAMPLE).unwrap(),
        Config::default()
    );
}

#[test]
fn partial_override_keeps_other_defaults() {
    let config = Config::from_toml("[font]\nsize = 15\n[colors]\naccent = \"#ff0000\"\n").unwrap();
    let defaults = Config::default();
    assert_eq!(config.font.size, 15.0);
    assert_eq!(config.font.family, defaults.font.family);
    assert_eq!(config.font.line_height, defaults.font.line_height);
    assert_eq!(config.colors.accent, Rgb { r: 255, g: 0, b: 0 });
    assert_eq!(config.colors.background, defaults.colors.background);
    assert_eq!(config.shell, defaults.shell);
}

#[test]
fn env_table_extends_default_env() {
    let config = Config::from_toml("[env]\nEDITOR = \"nvim\"\nCOLORTERM = \"24bit\"\n").unwrap();
    assert_eq!(config.env["EDITOR"], "nvim");
    assert_eq!(config.env["COLORTERM"], "24bit");
    assert_eq!(config.env["TERM"], "xterm-256color");
}

#[test]
fn bad_hex_names_the_key() {
    let error = Config::from_toml("[colors]\nbackground = \"#03\"\n").unwrap_err();
    let message = error.to_string();
    assert!(message.contains("background"), "{message}");
    assert!(message.contains("#03"), "{message}");
}

#[test]
fn unknown_keys_are_rejected() {
    assert!(Config::from_toml("[font]\nfamly = \"x\"\n").is_err());
    assert!(Config::from_toml("colours = 1\n").is_err());
}

#[test]
fn broken_file_falls_back_to_defaults_with_error() {
    let path = env::temp_dir().join(format!("flair-broken-{}.toml", process::id()));
    fs::write(&path, "[window\n").unwrap();
    let (config, error) = Config::load(&path);
    fs::remove_file(&path).unwrap();
    assert_eq!(config, Config::default());
    assert!(matches!(error, Some(ConfigError::Parse(_))));
}

#[test]
fn theme_copies_colors() {
    let config = Config::default();
    let theme = config.theme();
    assert_eq!(theme.background, Rgb { r: 3, g: 3, b: 3 });
    assert_eq!(theme.selection, config.colors.selection);
    assert_eq!(theme.bright, config.colors.bright);
}

#[test]
fn default_path_ends_in_flair_toml() {
    assert!(Config::default_path().ends_with("flair/flair.toml"));
}

#[test]
fn font_size_must_be_finite_and_positive() {
    for value in ["0", "-1", "nan", "inf"] {
        let error = Config::from_toml(&format!("[font]\nsize = {value}\n")).unwrap_err();
        assert!(
            matches!(
                error,
                ConfigError::NotPositive {
                    key: "font.size",
                    ..
                }
            ),
            "{error}"
        );
        assert!(error.to_string().contains("font.size"), "{error}");
    }
}

#[test]
fn ui_font_defaults_to_sans_serif_at_twelve() {
    let font = Config::default().font;
    assert_eq!(font.ui_family, "sans-serif");
    assert_eq!(font.ui_size, 12.0);
    let config = Config::from_toml("[font]\nui_family = \"Inter\"\nui_size = 11.5\n").unwrap();
    assert_eq!(config.font.ui_family, "Inter");
    assert_eq!(config.font.ui_size, 11.5);
}

#[test]
fn ui_size_must_be_finite_and_positive() {
    for value in ["0", "-2", "nan", "inf"] {
        let error = Config::from_toml(&format!("[font]\nui_size = {value}\n")).unwrap_err();
        assert!(
            matches!(
                error,
                ConfigError::NotPositive {
                    key: "font.ui_size",
                    ..
                }
            ),
            "{error}"
        );
        assert!(error.to_string().contains("font.ui_size"), "{error}");
    }
}

#[test]
fn line_height_must_be_finite_and_positive() {
    for value in ["0.0", "-0.5", "nan", "-inf"] {
        let error = Config::from_toml(&format!("[font]\nline_height = {value}\n")).unwrap_err();
        assert!(
            matches!(
                error,
                ConfigError::NotPositive {
                    key: "font.line_height",
                    ..
                }
            ),
            "{error}"
        );
        assert!(error.to_string().contains("font.line_height"), "{error}");
    }
}

#[test]
fn key_tables_accept_configured_prefix_and_unbind_default_chords() {
    let config = Config::from_toml(
        r#"
[keys]
key = "C-b"
timeout_ms = 750
[keys.prefix]
f = "last-session"
[keys.root]
"C-S-Enter" = "none"
"#,
    );
    assert!(config.is_ok(), "{config:?}");
}

#[test]
fn invalid_key_configuration_names_the_binding() {
    for text in [
        "[keys]\nkey = 'C-C-a'",
        "[keys.prefix]\n'unknown-key' = 'last-session'",
        "[keys.root]\n'A-x' = 'typo-command'",
        "[keys.prefix]\n'&' = 'close-window'\n'S-&' = 'close-pane'",
    ] {
        assert!(Config::from_toml(text).is_err(), "{text}");
    }
}
