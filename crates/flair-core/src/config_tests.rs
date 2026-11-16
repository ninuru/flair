use std::process;

use super::*;
use crate::isles::Chip;

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
padding_x = 8
padding_y = 0

[scrollback]
lines = 10000

[cursor]
style = "block"
blink = false

[bell]
audible = false

[prompt]
style = "chips"
icons = true
cwd = true
git_branch = true
git_ahead_behind = true
git_changes = true
exit = true
duration_ms = 2000
time = false
user = false
host = false
venv = false
docker = false
kube = false

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
    let config = Config::from_toml(BLUEPRINT_EXAMPLE).unwrap();
    assert_eq!(config.theme(), Config::default().theme());
    assert_eq!(
        Config {
            colors: ColorsConfig::default(),
            ..config
        },
        Config::default()
    );
}

#[test]
fn window_padding_defaults_to_eight_by_zero() {
    let window = Config::default().window;
    assert_eq!((window.padding_x, window.padding_y), (8, 0));
}

#[test]
fn partial_override_keeps_other_defaults() {
    let config = Config::from_toml("[font]\nsize = 15\n[colors]\naccent = \"#ff0000\"\n").unwrap();
    let defaults = Config::default();
    assert_eq!(config.font.size, 15.0);
    assert_eq!(config.font.family, defaults.font.family);
    assert_eq!(config.font.line_height, defaults.font.line_height);
    assert_eq!(config.colors.accent, Some(Rgb { r: 255, g: 0, b: 0 }));
    assert_eq!(config.colors.background, None);
    assert_eq!(config.theme().background, defaults.theme().background);
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
fn prompt_defaults_to_chips_with_directory_git_and_exit() {
    let prompt = Config::default().prompt;
    assert_eq!(
        prompt,
        PromptConfig {
            style: PromptStyle::Chips,
            icons: true,
            cwd: true,
            git_branch: true,
            git_ahead_behind: true,
            git_changes: true,
            exit: true,
            duration_ms: 2000,
            time: false,
            user: false,
            host: false,
            venv: false,
            docker: false,
            kube: false,
            ..PromptConfig::default()
        }
    );
    assert_eq!(prompt.band(), 1);
}

#[test]
fn prompt_table_overrides_defaults() {
    let config = Config::from_toml(
        "[prompt]\nstyle = \"ps1\"\nicons = false\ncwd = false\nduration_ms = 500\nkube = true\n",
    )
    .unwrap();
    let prompt = config.prompt;
    assert_eq!(prompt.style, PromptStyle::Ps1);
    assert!(!prompt.icons);
    assert!(!prompt.cwd);
    assert!(prompt.git_branch);
    assert_eq!(prompt.duration_ms, 500);
    assert!(prompt.kube);
    assert_eq!(prompt.band(), 0);
}

#[test]
fn prompt_rejects_unknown_keys_and_styles() {
    assert!(Config::from_toml("[prompt]\nchips = []\n").is_err());
    assert!(Config::from_toml("[prompt]\nstyle = \"fancy\"\n").is_err());
    assert!(Config::from_toml("[prompt]\nstyle = \"Chips\"\n").is_err());
    assert!(Config::from_toml("[prompt]\nduration_ms = -1\n").is_err());
}

#[test]
fn prompt_style_names() {
    assert_eq!(PromptStyle::Chips.name(), "chips");
    assert_eq!(PromptStyle::Ps1.name(), "ps1");
    assert_eq!(PromptStyle::NAMES, ["chips", "ps1"]);
}

#[test]
fn prompt_enabled_reads_exactly_the_matching_field() {
    let off = PromptConfig {
        cwd: false,
        git_branch: false,
        git_ahead_behind: false,
        git_changes: false,
        exit: false,
        ..PromptConfig::default()
    };
    for chip in Chip::ALL {
        assert!(!off.enabled(chip), "{chip:?}");
    }
    let only = |chip| match chip {
        Chip::User => PromptConfig {
            user: true,
            ..off.clone()
        },
        Chip::Host => PromptConfig {
            host: true,
            ..off.clone()
        },
        Chip::Cwd => PromptConfig {
            cwd: true,
            ..off.clone()
        },
        Chip::GitBranch => PromptConfig {
            git_branch: true,
            ..off.clone()
        },
        Chip::GitAheadBehind => PromptConfig {
            git_ahead_behind: true,
            ..off.clone()
        },
        Chip::GitChanges => PromptConfig {
            git_changes: true,
            ..off.clone()
        },
        Chip::Venv => PromptConfig {
            venv: true,
            ..off.clone()
        },
        Chip::Docker => PromptConfig {
            docker: true,
            ..off.clone()
        },
        Chip::Kube => PromptConfig {
            kube: true,
            ..off.clone()
        },
        Chip::Exit => PromptConfig {
            exit: true,
            ..off.clone()
        },
        Chip::Time => PromptConfig {
            time: true,
            ..off.clone()
        },
        Chip::Date => PromptConfig {
            date: true,
            ..off.clone()
        },
    };
    for chip in Chip::ALL {
        let config = only(chip);
        for other in Chip::ALL {
            assert_eq!(config.enabled(other), other == chip, "{chip:?} {other:?}");
        }
    }
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
    let config = Config::from_toml(BLUEPRINT_EXAMPLE).unwrap();
    let theme = config.theme();
    assert_eq!(theme.background, Rgb { r: 3, g: 3, b: 3 });
    assert_eq!(Some(theme.selection), config.colors.selection);
    assert_eq!(Some(theme.bright), config.colors.bright);
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
fn ligatures_are_off_by_default_and_parse_as_a_bool() {
    assert!(!Config::default().font.ligatures);
    let config = Config::from_toml("[font]\nligatures = true\n").unwrap();
    assert!(config.font.ligatures);
    assert!(Config::from_toml("[font]\nligatures = \"yes\"\n").is_err());
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

fn temp_dir(tag: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("flair-{tag}-{}", process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn hex(value: &str) -> Rgb {
    Rgb::from_hex(value).unwrap()
}

fn flair_dark() -> Theme {
    Config::default().theme()
}

#[test]
fn theme_defaults_to_flair_dark_and_parses() {
    assert_eq!(Config::default().theme, "flair-dark");
    let config = Config::from_toml("theme = \"gruvbox-dark\"\n").unwrap();
    assert_eq!(config.theme, "gruvbox-dark");
    assert_eq!(config.theme().background, hex("#282828"));
}

#[test]
fn with_theme_changes_only_the_name() {
    let config = Config::from_toml("[font]\nsize = 15\n").unwrap();
    let changed = config.with_theme("catppuccin-mocha");
    assert_eq!(changed.theme, "catppuccin-mocha");
    assert_eq!(changed.font, config.font);
}

#[test]
fn themes_dir_sits_next_to_the_config_file() {
    assert_eq!(
        Config::themes_dir(Path::new("/home/u/.config/flair/flair.toml")),
        PathBuf::from("/home/u/.config/flair/themes")
    );
    assert!(Config::themes_dir(&Config::default_path()).ends_with("flair/themes"));
}

#[test]
fn load_theme_uses_builtin_without_file() {
    let dir = temp_dir("themes-builtin");
    let config = Config::default().with_theme("catppuccin-mocha");
    let (theme, error) = config.load_theme(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(theme, config.theme());
    assert_eq!(theme.cursor, hex("#f5e0dc"));
}

#[test]
fn theme_file_with_builtin_name_wins() {
    let dir = temp_dir("themes-wins");
    fs::write(dir.join("gruvbox-dark.toml"), "background = \"#123456\"\n").unwrap();
    let (theme, error) = Config::default()
        .with_theme("gruvbox-dark")
        .load_theme(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(theme.background, hex("#123456"));
    assert_eq!(theme.foreground, flair_dark().foreground);
}

#[test]
fn partial_theme_file_falls_back_to_flair_dark_and_derives() {
    let dir = temp_dir("themes-partial");
    fs::write(
        dir.join("mine.toml"),
        "foreground = \"#eeeeee\"\naccent = \"#ff0000\"\n",
    )
    .unwrap();
    let (theme, error) = Config::default().with_theme("mine").load_theme(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert!(error.is_none(), "{error:?}");
    let base = flair_dark();
    assert_eq!(theme.background, base.background);
    assert_eq!(theme.normal, base.normal);
    assert_eq!(theme.bright, base.bright);
    assert_eq!(theme.foreground, hex("#eeeeee"));
    assert_eq!(theme.cursor, hex("#eeeeee"));
    assert_eq!(
        theme.selection,
        Rgba {
            r: 0xff,
            g: 0,
            b: 0,
            a: 0x40
        }
    );
}

#[test]
fn colors_accent_overrides_theme_and_rederives_selection() {
    let config =
        Config::from_toml("theme = \"gruvbox-dark\"\n[colors]\naccent = \"#00ff00\"\n").unwrap();
    let theme = config.theme();
    assert_eq!(theme.background, hex("#282828"));
    assert_eq!(theme.accent, hex("#00ff00"));
    assert_eq!(
        theme.selection,
        Rgba {
            r: 0,
            g: 0xff,
            b: 0,
            a: 0x40
        }
    );
    let dir = temp_dir("themes-colors");
    let (loaded, error) = config.load_theme(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert!(error.is_none(), "{error:?}");
    assert_eq!(loaded, theme);
}

#[test]
fn colors_selection_is_kept_when_set() {
    let config = Config::from_toml(
        "theme = \"gruvbox-dark\"\n[colors]\naccent = \"#00ff00\"\nselection = \"#01020304\"\n",
    )
    .unwrap();
    assert_eq!(
        config.theme().selection,
        Rgba {
            r: 1,
            g: 2,
            b: 3,
            a: 4
        }
    );
}

#[test]
fn unknown_theme_falls_back_to_flair_dark_with_error() {
    let dir = temp_dir("themes-unknown");
    let (theme, error) = Config::default().with_theme("nope").load_theme(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(theme, flair_dark());
    assert!(
        matches!(&error, Some(ConfigError::UnknownTheme(name)) if name == "nope"),
        "{error:?}"
    );
    assert!(error.unwrap().to_string().contains("nope"));
}

#[test]
fn invalid_theme_names_fall_back_to_flair_dark_with_error() {
    let dir = temp_dir("themes-invalid");
    let inner = dir.join("inner");
    fs::create_dir_all(&inner).unwrap();
    fs::write(dir.join("outside.toml"), "background = \"#123456\"\n").unwrap();
    fs::write(inner.join(".hidden.toml"), "background = \"#123456\"\n").unwrap();
    let long = "a".repeat(65);
    for name in [
        "",
        ".hidden",
        "../outside",
        "a/b",
        "sp ace",
        "ü",
        long.as_str(),
    ] {
        let (theme, error) = Config::default().with_theme(name).load_theme(&inner);
        assert_eq!(theme, flair_dark(), "{name}");
        assert!(
            matches!(&error, Some(ConfigError::ThemeName(bad)) if bad == name),
            "{name}: {error:?}"
        );
    }
    let (_, error) = Config::default()
        .with_theme(&"a".repeat(64))
        .load_theme(&inner);
    fs::remove_dir_all(&dir).unwrap();
    assert!(
        matches!(error, Some(ConfigError::UnknownTheme(_))),
        "{error:?}"
    );
}

#[test]
fn broken_theme_file_falls_back_to_flair_dark_with_error() {
    let dir = temp_dir("themes-broken");
    fs::write(dir.join("bad.toml"), "background = \"#12\"\n").unwrap();
    fs::write(dir.join("typo.toml"), "backgrund = \"#123456\"\n").unwrap();
    for name in ["bad", "typo"] {
        let (theme, error) = Config::default().with_theme(name).load_theme(&dir);
        assert_eq!(theme, flair_dark(), "{name}");
        let error = error.unwrap();
        assert!(
            matches!(&error, ConfigError::Theme { name: bad, .. } if bad == name),
            "{error:?}"
        );
        assert!(error.to_string().contains(name), "{error}");
    }
    let (_, error) = Config::default().with_theme("bad").load_theme(&dir);
    fs::remove_dir_all(&dir).unwrap();
    let message = error.unwrap().to_string();
    assert!(message.contains("background"), "{message}");
}

#[test]
fn colors_config_from_toml_reads_top_level_keys() {
    let colors = ColorsConfig::from_toml("accent = \"#010203\"\n").unwrap();
    assert_eq!(colors.accent, Some(Rgb { r: 1, g: 2, b: 3 }));
    assert_eq!(colors.background, None);
    assert!(ColorsConfig::from_toml("[colors]\naccent = \"#010203\"\n").is_err());
}

#[test]
fn colors_over_prefers_self() {
    let top = ColorsConfig::from_toml("accent = \"#010203\"\n").unwrap();
    let base = ColorsConfig::from_toml("accent = \"#040506\"\nbackground = \"#070809\"\n").unwrap();
    let merged = top.over(&base);
    assert_eq!(merged.accent, top.accent);
    assert_eq!(merged.background, base.background);
    assert_eq!(merged.foreground, None);
}

#[test]
fn theme_names_lists_builtins_and_valid_files() {
    let dir = temp_dir("themes-names");
    for file in [
        "mine.toml",
        "gruvbox-dark.toml",
        "a.b_c-1.toml",
        ".hidden.toml",
        "sp ace.toml",
        "notes.txt",
        "README",
    ] {
        fs::write(dir.join(file), "").unwrap();
    }
    let names = theme_names(&dir);
    fs::remove_dir_all(&dir).unwrap();
    assert_eq!(
        names,
        [
            "a.b_c-1",
            "catppuccin-mocha",
            "flair-dark",
            "gruvbox-dark",
            "mine"
        ]
    );
}

#[test]
fn theme_names_without_dir_lists_builtins() {
    let dir = env::temp_dir().join(format!("flair-themes-missing-{}", process::id()));
    assert_eq!(
        theme_names(&dir),
        ["catppuccin-mocha", "flair-dark", "gruvbox-dark"]
    );
}

#[test]
fn prompt_editor_configuration_reads_order_formats_and_segment_overrides() {
    let config = Config::from_toml(
        r#"
[prompt]
order = "docker, cwd, date"
path = "folder"
appearance = "plain"
time_format = "12h"
date_format = "long"
separator = "dollar"
date = true
[prompt.segments.cwd]
colour = "cyan"
appearance = "outlined"
icons = false
"#,
    )
    .unwrap();
    assert_eq!(
        config.prompt.order.chips(),
        &[Chip::Docker, Chip::Cwd, Chip::Date]
    );
    assert_eq!(config.prompt.path, PromptPath::Folder);
    assert_eq!(config.prompt.time_format, PromptTimeFormat::Twelve);
    assert_eq!(config.prompt.date_format, PromptDateFormat::Long);
    assert_eq!(config.prompt.separator, PromptSeparator::Dollar);
    assert!(config.prompt.enabled(Chip::Date));
    let cwd = &config.prompt.segments[&Chip::Cwd];
    assert_eq!(cwd.colour, Some(AnsiColour::Cyan));
    assert_eq!(cwd.appearance, Some(PromptAppearance::Outlined));
    assert_eq!(cwd.icons, Some(false));
}

#[test]
fn prompt_editor_rejects_invalid_orders_and_segment_options() {
    for text in [
        "[prompt]\norder = \"cwd,cwd\"",
        "[prompt]\norder = \"cwd,unknown\"",
        "[prompt]\norder = \"\"",
        "[prompt.segments.unknown]\nicons = false",
        "[prompt.segments.cwd]\ncolour = \"#123456\"",
        "[prompt.segments.cwd]\nappearance = \"neon\"",
    ] {
        assert!(Config::from_toml(text).is_err(), "{text}");
    }
}

#[test]
fn panes_dim_inactive_by_thirty_percent_by_default() {
    let panes = Config::default().panes;
    assert!(panes.dim_inactive);
    assert_eq!(panes.dim_amount, 30);
    assert_eq!(PANES_DIM, (0, 80));
}

#[test]
fn panes_table_reads_dimming() {
    let config = Config::from_toml("[panes]\ndim_inactive = false\ndim_amount = 55\n").unwrap();
    assert!(!config.panes.dim_inactive);
    assert_eq!(config.panes.dim_amount, 55);
    let config = Config::from_toml("[panes]\ndim_amount = 0\n").unwrap();
    assert!(config.panes.dim_inactive);
    assert_eq!(config.panes.dim_amount, 0);
    assert!(Config::from_toml("[panes]\nfade = 3\n").is_err());
}

#[test]
fn panes_dim_amount_outside_the_bounds_is_rejected_with_the_range() {
    for amount in [81, 100, 500] {
        let error = Config::from_toml(&format!("[panes]\ndim_amount = {amount}\n")).unwrap_err();
        assert!(
            matches!(
                error,
                ConfigError::Range {
                    key: "panes.dim_amount",
                    value,
                    min: 0,
                    max: 80,
                } if value == amount
            ),
            "{error}"
        );
    }
    assert!(Config::from_toml("[panes]\ndim_amount = 80\n").is_ok());
}

#[test]
fn tabs_default_to_the_top_bar_two_hundred_wide() {
    let tabs = Config::default().tabs;
    assert_eq!(tabs.position, TabsPosition::Top);
    assert_eq!(tabs.width, 200);
    assert_eq!(TABS_WIDTH, (120, 480));
}

#[test]
fn tabs_table_reads_position_and_width() {
    let config = Config::from_toml("[tabs]\nposition = \"right\"\nwidth = 240\n").unwrap();
    assert_eq!(config.tabs.position, TabsPosition::Right);
    assert_eq!(config.tabs.width, 240);
    let config = Config::from_toml("[tabs]\nposition = \"left\"\n").unwrap();
    assert_eq!(config.tabs.position, TabsPosition::Left);
    assert_eq!(config.tabs.width, 200);
    assert!(Config::from_toml("[tabs]\nposition = \"bottom\"\n").is_err());
    assert!(Config::from_toml("[tabs]\nheight = 3\n").is_err());
}

#[test]
fn tabs_width_bounds_are_inclusive() {
    for width in [120, 480] {
        let config = Config::from_toml(&format!("[tabs]\nwidth = {width}\n")).unwrap();
        assert_eq!(config.tabs.width, width);
    }
}

#[test]
fn tabs_width_outside_the_bounds_is_rejected_with_the_range() {
    for width in [0, 90, 119, 481, 2000] {
        let error = Config::from_toml(&format!("[tabs]\nwidth = {width}\n")).unwrap_err();
        assert!(
            matches!(
                error,
                ConfigError::Range {
                    key: "tabs.width",
                    value,
                    min: 120,
                    max: 480,
                } if value == width
            ),
            "{error}"
        );
    }
    let error = Config::from_toml("[tabs]\nwidth = 90\n").unwrap_err();
    assert_eq!(
        error.to_string(),
        "invalid config: tabs.width must be from 120 to 480, got 90"
    );
    assert!(error.source().is_none());
}

#[test]
fn panel_position_names() {
    assert_eq!(TabsPosition::NAMES, ["top", "left", "right"]);
    assert_eq!(TabsPosition::Top.name(), "top");
    assert_eq!(TabsPosition::Left.name(), "left");
    assert_eq!(TabsPosition::Right.name(), "right");
    assert_eq!(Side::NAMES, ["left", "right"]);
    assert_eq!(Side::Left.name(), "left");
    assert_eq!(Side::Right.name(), "right");
}

#[test]
fn explorer_defaults_to_the_left_two_hundred_eighty_wide_with_hidden_files() {
    let explorer = Config::default().explorer;
    assert_eq!(explorer.side, Side::Left);
    assert_eq!(explorer.width, 280);
    assert!(explorer.show_hidden);
    assert!(explorer.editor.is_empty());
    assert_eq!(EXPLORER_WIDTH, (160, 640));
}

#[test]
fn explorer_table_reads_side_width_and_hidden_files() {
    let config =
        Config::from_toml("[explorer]\nside = \"right\"\nwidth = 320\nshow_hidden = false\n")
            .unwrap();
    assert_eq!(config.explorer.side, Side::Right);
    assert_eq!(config.explorer.width, 320);
    assert!(!config.explorer.show_hidden);
    let config = Config::from_toml("[explorer]\neditor = \"nvim\"\n").unwrap();
    assert_eq!(config.explorer.editor, "nvim");
    let config = Config::from_toml("[explorer]\nside = \"left\"\n").unwrap();
    assert_eq!(config.explorer, ExplorerConfig::default());
    assert!(Config::from_toml("[explorer]\nside = \"top\"\n").is_err());
    assert!(Config::from_toml("[explorer]\nside = \"Left\"\n").is_err());
    assert!(Config::from_toml("[explorer]\nhidden = true\n").is_err());
}

#[test]
fn explorer_width_bounds_are_inclusive_and_checked() {
    for width in [160, 640] {
        let config = Config::from_toml(&format!("[explorer]\nwidth = {width}\n")).unwrap();
        assert_eq!(config.explorer.width, width);
    }
    for width in [0, 159, 641] {
        let error = Config::from_toml(&format!("[explorer]\nwidth = {width}\n")).unwrap_err();
        assert!(
            matches!(
                error,
                ConfigError::Range {
                    key: "explorer.width",
                    value,
                    min: 160,
                    max: 640,
                } if value == width
            ),
            "{error}"
        );
    }
    assert_eq!(
        Config::from_toml("[explorer]\nwidth = 100\n")
            .unwrap_err()
            .to_string(),
        "invalid config: explorer.width must be from 160 to 640, got 100"
    );
}

#[test]
fn notifications_default_to_enabled_and_read_from_the_table() {
    assert!(Config::default().notifications.enabled);
    let config = Config::from_toml("[notifications]\nenabled = false\n").unwrap();
    assert!(!config.notifications.enabled);
    assert!(Config::from_toml("[notifications]\nsound = true\n").is_err());
}

#[test]
fn tab_icons_default_on_after_the_name_and_read_from_the_tabs_table() {
    let tabs = Config::default().tabs;
    assert!(tabs.icons);
    assert_eq!(tabs.icon_side, IconSide::After);
    let config = Config::from_toml("[tabs]\nicons = false\nicon_side = \"before\"\n").unwrap();
    assert!(!config.tabs.icons);
    assert_eq!(config.tabs.icon_side, IconSide::Before);
    assert_eq!(IconSide::Before.name(), "before");
    assert_eq!(IconSide::After.name(), "after");
    assert!(Config::from_toml("[tabs]\nicon_side = \"left\"\n").is_err());
    assert!(Config::from_toml("[tabs]\nicons = \"yes\"\n").is_err());
}
