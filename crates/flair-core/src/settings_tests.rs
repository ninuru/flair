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
        Kind::Text if setting.key == "prompt.order" => text("cwd, docker"),
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
            "font.ligatures",
            "window.padding_x",
            "window.padding_y",
            "shell.program",
            "shell.args",
            "scrollback.lines",
            "prompt.style",
            "prompt.icons",
            "prompt.user",
            "prompt.host",
            "prompt.cwd",
            "prompt.git_branch",
            "prompt.git_ahead_behind",
            "prompt.git_changes",
            "prompt.venv",
            "prompt.docker",
            "prompt.kube",
            "prompt.exit",
            "prompt.time",
            "prompt.appearance",
            "prompt.path",
            "prompt.time_format",
            "prompt.date_format",
            "prompt.separator",
            "prompt.order",
            "prompt.date",
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
        ["Appearance", "Font", "Window", "Terminal", "Prompt", "Keys"]
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
        "Off also disables prefix S; re-enable it in flair.toml."
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

fn words(words: &[&str]) -> Value {
    Value::Words(words.iter().map(|word| (*word).to_owned()).collect())
}

#[test]
fn words_quote_what_would_not_split_back() {
    assert_eq!(words(&["-c", "echo hi"]).display(), "-c \"echo hi\"");
    assert_eq!(words(&["", "a"]).display(), "\"\" a");
    assert_eq!(words(&["say \"hi\""]).display(), "\"say \\\"hi\\\"\"");
    assert_eq!(words(&["a\\b"]).display(), "\"a\\\\b\"");
    assert_eq!(words(&["tab\there"]).display(), "\"tab\there\"");
}

#[test]
fn words_round_trip_through_display_and_parse() {
    let args = setting("shell.args");
    for case in [
        words(&["-c", "echo hi"]),
        words(&["", "a"]),
        words(&["say \"hi\"", "a\\b"]),
        words(&["-l"]),
        words(&[]),
    ] {
        assert_eq!(
            args.parse(&case.display()),
            Ok(Some(case.clone())),
            "{case:?}"
        );
    }
}

#[test]
fn words_parse_quotes_and_escapes() {
    let args = setting("shell.args");
    assert_eq!(
        args.parse("-c \"echo hi\""),
        Ok(Some(words(&["-c", "echo hi"])))
    );
    assert_eq!(args.parse("a\"b c\"d"), Ok(Some(words(&["ab cd"]))));
    assert_eq!(args.parse("\"\\q\\x\""), Ok(Some(words(&["\\q\\x"]))));
    assert_eq!(args.parse("plain\\path"), Ok(Some(words(&["plain\\path"]))));
    for bad in ["\"open", "-c \"echo", "\"ends\\\""] {
        assert_eq!(
            args.parse(bad),
            Err("close the quote".to_owned()),
            "{bad:?}"
        );
    }
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
fn the_prompt_section_lists_style_icons_and_one_toggle_per_chip() {
    let prompt: Vec<_> = SETTINGS
        .iter()
        .filter(|setting| setting.section == Section::Prompt)
        .collect();
    assert_eq!(prompt.len(), 20);
    assert_eq!(prompt[0].key, "prompt.style");
    assert_eq!(prompt[0].title, "Style");
    assert_eq!(prompt[0].kind, Kind::Choice(Choices::Of(&["chips", "ps1"])));
    assert_eq!(prompt[1].key, "prompt.icons");
    assert_eq!(prompt[1].title, "Icons");
    for setting in &prompt[1..13] {
        assert_eq!(setting.kind, Kind::Toggle, "{}", setting.key);
    }
    assert_eq!(prompt[13].key, "prompt.appearance");
    assert!(
        prompt
            .iter()
            .all(|setting| setting.key != "prompt.duration_ms")
    );
    assert_eq!(setting("theme").kind, Kind::Choice(Choices::Themes));
}

#[test]
fn prompt_defaults_read_from_the_config() {
    assert_eq!(default_value(setting("prompt.style")), text("chips"));
    assert_eq!(default_value(setting("prompt.icons")), Value::Bool(true));
    assert_eq!(default_value(setting("prompt.cwd")), Value::Bool(true));
    assert_eq!(default_value(setting("prompt.time")), Value::Bool(false));
}

#[test]
fn ligatures_is_an_off_toggle_in_the_font_section() {
    let ligatures = setting("font.ligatures");
    assert_eq!(ligatures.section, Section::Font);
    assert_eq!(ligatures.kind, Kind::Toggle);
    assert_eq!(default_value(ligatures), Value::Bool(false));
    let mut config = Config::default();
    config.font.ligatures = true;
    assert_eq!(ligatures.value(&config, &config.theme()), Value::Bool(true));
}

#[test]
fn a_fixed_choice_steps_through_its_own_list_both_ways() {
    let style = setting("prompt.style");
    let themes = ["a", "b", "c"].map(str::to_owned);
    assert_eq!(style.step(&text("chips"), 1, &themes), Some(text("ps1")));
    assert_eq!(style.step(&text("ps1"), 1, &themes), Some(text("chips")));
    assert_eq!(style.step(&text("chips"), -1, &[]), Some(text("ps1")));
    assert_eq!(style.step(&text("ps1"), -1, &[]), Some(text("chips")));
    assert_eq!(style.step(&text("other"), 1, &[]), Some(text("chips")));
    assert_eq!(style.step(&text("chips"), 2, &[]), None);
    assert_eq!(style.step(&Value::Bool(true), 1, &[]), None);
    assert_eq!(style.parse("ps1"), Err("use space or ← →".to_owned()));
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
    assert_eq!(size.parse("128"), Ok(Some(Value::Float(128.0))));
    assert_eq!(size.parse("0.5"), Ok(Some(Value::Float(0.5))));
    for bad in ["0", "0.4", "-1", "128.5", "nan", "inf", "big", ""] {
        assert_eq!(
            size.parse(bad),
            Err("enter a number from 0.5 to 128".to_owned()),
            "{bad:?}"
        );
    }
    assert_eq!(
        setting("font.line_height").parse("5"),
        Err("enter a number from 0.5 to 4".to_owned())
    );
    assert_eq!(
        setting("font.ui_size").parse("65"),
        Err("enter a number from 0.5 to 64".to_owned())
    );
}

#[test]
fn float_steps_clamp_to_the_maximum() {
    let size = setting("font.size");
    assert_eq!(size.step(&Value::Float(128.0), 1, &[]), None);
    assert_eq!(
        size.step(&Value::Float(127.8), 1, &[]),
        Some(Value::Float(128.0))
    );
    assert_eq!(
        setting("font.line_height").step(&Value::Float(4.0), 1, &[]),
        None
    );
    assert_eq!(
        setting("font.ui_size").step(&Value::Float(70.0), 1, &[]),
        Some(Value::Float(64.0))
    );
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
    for key in ["shell.program", "keys.key", "font.family", "font.ui_family"] {
        assert_eq!(setting(key).parse(""), Ok(None), "{key}");
        assert_eq!(setting(key).parse("   "), Ok(None), "{key}");
    }
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

#[test]
fn prompt_order_editing_validates_and_normalizes_segment_names() {
    let order = setting("prompt.order");
    assert_eq!(
        order.parse(" docker,cwd, date "),
        Ok(Some(text("docker, cwd, date")))
    );
    assert!(order.parse("cwd,cwd").is_err());
    assert!(order.parse("cwd,unknown").is_err());
    assert_eq!(order.parse(""), Ok(None));
    assert_eq!(
        default_value(setting("prompt.appearance")),
        text("outlined")
    );
    assert_eq!(default_value(setting("prompt.path")), text("abbreviated"));
    assert_eq!(default_value(setting("prompt.separator")), text("none"));
    assert_eq!(default_value(setting("prompt.time_format")), text("24h"));
    assert_eq!(default_value(setting("prompt.date_format")), text("iso"));
    assert_eq!(default_value(setting("prompt.date")), Value::Bool(false));
}
