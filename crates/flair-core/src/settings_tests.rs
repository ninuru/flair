use super::*;
use crate::{config_edit::apply, theme::BUILTIN_THEMES};

fn setting(key: &str) -> &'static Setting {
    &SETTINGS[position(key).unwrap()]
}

fn default_value(setting: &Setting) -> Value {
    let config = Config::default();
    setting.value(&config, &config.theme())
}

fn builtin_themes() -> Vec<String> {
    BUILTIN_THEMES
        .iter()
        .map(|(name, _)| (*name).to_owned())
        .collect()
}

fn other_value(setting: &Setting) -> Value {
    let current = default_value(setting);
    match setting.kind {
        Kind::Toggle | Kind::Integer { .. } | Kind::Float { .. } | Kind::Choice(_) => {
            setting.step(&current, 1, &builtin_themes()).unwrap()
        }
        Kind::Text => Value::Text("C-b".to_owned()),
        Kind::Font => Value::Text("Iosevka".to_owned()),
        Kind::Words => Value::Words(vec!["-i".to_owned()]),
        Kind::Color { alpha: false } => Value::Text("#123456".to_owned()),
        Kind::Color { alpha: true } => Value::Text("#12345678".to_owned()),
    }
}

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

#[test]
fn the_table_lists_every_editable_key_in_display_order() {
    let keys: Vec<_> = SETTINGS.iter().map(|setting| setting.key).collect();
    assert_eq!(
        keys,
        [
            "theme",
            "colors.background",
            "colors.foreground",
            "colors.cursor",
            "colors.accent",
            "colors.selection",
            "font.family",
            "font.size",
            "font.line_height",
            "font.ui_family",
            "font.ui_size",
            "window.padding_x",
            "window.padding_y",
            "shell.program",
            "shell.args",
            "scrollback.lines",
            "keys.enabled",
            "keys.key",
            "keys.timeout_ms",
        ]
    );
}

#[test]
fn every_section_has_an_entry_and_entries_are_grouped() {
    for section in Section::ALL {
        assert!(
            SETTINGS.iter().any(|setting| setting.section == section),
            "{section:?}"
        );
    }
    let sections: Vec<_> = SETTINGS.iter().map(|setting| setting.section).collect();
    let mut sorted = sections.clone();
    sorted.sort_by_key(|section| Section::ALL.iter().position(|s| s == section));
    assert_eq!(sections, sorted);
    assert_eq!(
        Section::ALL.map(Section::title),
        ["Appearance", "Font", "Window", "Terminal", "Keys"]
    );
    assert_eq!(setting("shell.program").section, Section::Terminal);
    assert_eq!(setting("scrollback.lines").section, Section::Terminal);
}

#[test]
fn source_labels() {
    assert_eq!(Source::Default.label(), "default");
    assert_eq!(Source::File.label(), "flair.toml");
}

#[test]
fn keys_enabled_warns_about_the_lock_out() {
    assert_eq!(
        setting("keys.enabled").description,
        "Prefix and root bindings. Off also disables prefix S."
    );
    assert_eq!(setting("keys.enabled").kind, Kind::Toggle);
}

#[test]
fn position_finds_keys() {
    assert_eq!(position("theme"), Some(0));
    assert_eq!(SETTINGS[position("font.size").unwrap()].key, "font.size");
    assert_eq!(position("cursor.blink"), None);
    assert_eq!(position(""), None);
}

#[test]
fn every_default_round_trips_through_the_file() {
    for setting in SETTINGS {
        let default = default_value(setting);
        let (out, config) = apply("", setting.key, Some(&default)).unwrap();
        assert_eq!(
            setting.value(&config, &config.theme()),
            default,
            "{} {out:?}",
            setting.key
        );
    }
}

#[test]
fn every_changed_value_round_trips_and_removing_it_restores_the_default() {
    for setting in SETTINGS {
        let changed = other_value(setting);
        assert_ne!(changed, default_value(setting), "{}", setting.key);
        let (out, config) = apply("", setting.key, Some(&changed)).unwrap();
        assert_eq!(
            setting.value(&config, &config.theme()),
            changed,
            "{}",
            setting.key
        );
        let (_, restored) = apply(&out, setting.key, None).unwrap();
        assert_eq!(restored, Config::default(), "{}", setting.key);
        assert_eq!(
            setting.value(&restored, &restored.theme()),
            default_value(setting)
        );
    }
}

#[test]
fn colours_read_the_effective_theme() {
    let config = Config::default().with_theme("gruvbox-dark");
    let theme = config.theme();
    assert_eq!(
        setting("colors.background").value(&config, &theme),
        Value::Text(theme.background.hex())
    );
    assert_eq!(
        setting("colors.selection").value(&config, &theme),
        Value::Text(theme.selection.hex())
    );
    assert_ne!(
        setting("colors.background").value(&config, &theme),
        default_value(setting("colors.background"))
    );
}

#[test]
fn floats_read_as_their_shortest_decimal() {
    let mut config = Config::default();
    config.font.line_height = 1.05;
    assert_eq!(
        setting("font.line_height").value(&config, &config.theme()),
        Value::Float(1.05)
    );
}

#[test]
fn values_display_plainly() {
    assert_eq!(Value::Float(1.05).display(), "1.05");
    assert_eq!(Value::Float(13.0).display(), "13");
    assert_eq!(Value::Integer(10_000).display(), "10000");
    assert_eq!(Value::Bool(true).display(), "on");
    assert_eq!(Value::Bool(false).display(), "off");
    assert_eq!(text("C-a").display(), "C-a");
    assert_eq!(
        Value::Words(vec!["-l".to_owned(), "-i".to_owned()]).display(),
        "-l -i"
    );
    assert_eq!(Value::Words(Vec::new()).display(), "");
}

#[test]
fn float_steps_land_on_the_grid() {
    let line_height = setting("font.line_height");
    let next = line_height.step(&Value::Float(1.0), 1, &[]).unwrap();
    assert_eq!(next, Value::Float(1.05));
    assert_eq!(next.display(), "1.05");
    assert_eq!(
        line_height.step(&Value::Float(1.03), 1, &[]),
        Some(Value::Float(1.1))
    );
    let size = setting("font.size");
    assert_eq!(
        size.step(&Value::Float(13.0), 1, &[]),
        Some(Value::Float(13.5))
    );
    assert_eq!(
        size.step(&Value::Float(13.0), -3, &[]),
        Some(Value::Float(11.5))
    );
}

#[test]
fn steps_clamp_and_report_no_change_at_a_bound() {
    let size = setting("font.size");
    assert_eq!(size.step(&Value::Float(0.5), -1, &[]), None);
    assert_eq!(
        size.step(&Value::Float(0.7), -1, &[]),
        Some(Value::Float(0.5))
    );
    let padding = setting("window.padding_x");
    assert_eq!(padding.step(&Value::Integer(65535), 1, &[]), None);
    assert_eq!(
        padding.step(&Value::Integer(65534), 5, &[]),
        Some(Value::Integer(65535))
    );
    assert_eq!(padding.step(&Value::Integer(0), -1, &[]), None);
    assert_eq!(
        setting("scrollback.lines").step(&Value::Integer(10_000), 1, &[]),
        Some(Value::Integer(11_000))
    );
    assert_eq!(
        setting("keys.timeout_ms").step(&Value::Integer(50), -1, &[]),
        Some(Value::Integer(0))
    );
}

#[test]
fn toggles_flip_and_text_kinds_do_not_step() {
    let enabled = setting("keys.enabled");
    assert_eq!(
        enabled.step(&Value::Bool(true), 1, &[]),
        Some(Value::Bool(false))
    );
    assert_eq!(
        enabled.step(&Value::Bool(false), -1, &[]),
        Some(Value::Bool(true))
    );
    for key in [
        "shell.program",
        "font.family",
        "shell.args",
        "colors.accent",
    ] {
        let setting = setting(key);
        assert_eq!(setting.step(&default_value(setting), 1, &[]), None, "{key}");
    }
}

#[test]
fn theme_cycles_with_wrap_around() {
    let theme = setting("theme");
    let names = ["a", "b", "c"].map(str::to_owned);
    assert_eq!(theme.step(&text("a"), 1, &names), Some(text("b")));
    assert_eq!(theme.step(&text("c"), 1, &names), Some(text("a")));
    assert_eq!(theme.step(&text("a"), -1, &names), Some(text("c")));
    assert_eq!(theme.step(&text("z"), 1, &names), Some(text("a")));
    assert_eq!(theme.step(&text("a"), 1, &[]), None);
    assert_eq!(theme.step(&text("a"), 1, &names[..1]), None);
}

#[test]
fn fixed_choices_cycle() {
    let style = Setting {
        key: "cursor.style",
        title: "Cursor style",
        section: Section::Terminal,
        kind: Kind::Choice(Choices::Fixed(&["block", "underline", "beam"])),
        description: "Shape of the terminal cursor.",
        read: |_, _| Value::Text("block".to_owned()),
    };
    assert_eq!(style.step(&text("block"), 1, &[]), Some(text("underline")));
    assert_eq!(style.step(&text("underline"), 1, &[]), Some(text("beam")));
    assert_eq!(style.step(&text("beam"), 1, &[]), Some(text("block")));
    assert_eq!(style.step(&text("block"), -1, &[]), Some(text("beam")));
}

#[test]
fn parsing_integers() {
    let padding = setting("window.padding_x");
    assert_eq!(padding.parse(" 12 "), Ok(Some(Value::Integer(12))));
    for bad in ["abc", "", "-1", "70000", "1.5"] {
        assert_eq!(
            padding.parse(bad),
            Err("enter a whole number from 0 to 65535".to_owned()),
            "{bad:?}"
        );
    }
    assert_eq!(
        setting("scrollback.lines").parse("-5"),
        Err("enter a whole number of at least 0".to_owned())
    );
}

#[test]
fn parsing_floats() {
    let size = setting("font.size");
    assert_eq!(size.parse(" 14.5 "), Ok(Some(Value::Float(14.5))));
    for bad in ["0", "-1", "nan", "inf", "big", ""] {
        assert_eq!(
            size.parse(bad),
            Err("enter a number greater than 0".to_owned()),
            "{bad:?}"
        );
    }
}

#[test]
fn parsing_text_words_and_toggles() {
    assert_eq!(
        setting("shell.program").parse("/bin/fish"),
        Ok(Some(text("/bin/fish")))
    );
    assert_eq!(
        setting("font.family").parse("Iosevka Term"),
        Ok(Some(text("Iosevka Term")))
    );
    assert_eq!(
        setting("shell.args").parse("  -l   -i "),
        Ok(Some(Value::Words(vec!["-l".to_owned(), "-i".to_owned()])))
    );
    assert_eq!(
        setting("shell.args").parse(""),
        Ok(Some(Value::Words(Vec::new())))
    );
    assert_eq!(
        setting("keys.enabled").parse("on"),
        Err("use space or ← →".to_owned())
    );
    assert_eq!(
        setting("theme").parse("x"),
        Err("use space or ← →".to_owned())
    );
}

#[test]
fn parsing_colours() {
    let accent = setting("colors.accent");
    assert_eq!(accent.parse(""), Ok(None));
    assert_eq!(accent.parse("  "), Ok(None));
    assert_eq!(accent.parse("#19AAD8"), Ok(Some(text("#19aad8"))));
    let error = accent.parse("#19aad840").unwrap_err();
    assert_eq!(
        error,
        "invalid colour \"#19aad840\", expected a hex colour like #rrggbb"
    );
    assert!(!error.contains('\n'));
    let selection = setting("colors.selection");
    assert_eq!(selection.parse("#19aad840"), Ok(Some(text("#19aad840"))));
    assert_eq!(selection.parse("#19aad8"), Ok(Some(text("#19aad8ff"))));
    assert!(selection.parse("red").is_err());
}

#[test]
fn sources_follow_the_file() {
    let file =
        "theme = \"a\"\nfont.size = 14.0\nkeys = { enabled = false }\n[window]\npadding_x = 3\n";
    let sources = sources(file);
    assert_eq!(sources.len(), SETTINGS.len());
    for (setting, source) in SETTINGS.iter().zip(&sources) {
        let expected = match setting.key {
            "theme" | "font.size" | "keys.enabled" | "window.padding_x" => Source::File,
            _ => Source::Default,
        };
        assert_eq!(*source, expected, "{}", setting.key);
    }
}

#[test]
fn an_unparseable_file_has_only_defaults() {
    for file in ["", "[font\n", "font = 3\n"] {
        let sources = sources(file);
        assert_eq!(sources.len(), SETTINGS.len());
        assert!(
            sources.iter().all(|source| *source == Source::Default),
            "{file:?}"
        );
    }
}

#[test]
fn matching_looks_at_title_key_description_and_section() {
    assert!(setting("theme").matches("THEME"));
    assert!(setting("window.padding_x").matches("padding_x"));
    assert!(setting("scrollback.lines").matches("history"));
    assert!(setting("shell.program").matches("termin"));
    assert!(setting("font.size").matches(""));
    assert!(!setting("font.size").matches("zzz"));
    assert!(!setting("theme").matches("padding"));
}
