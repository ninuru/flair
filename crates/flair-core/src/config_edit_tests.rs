use super::*;
use crate::config::ConfigError;

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn set(source: &str, key: &str, value: Value) -> String {
    apply(source, key, Some(&value)).unwrap().0
}

fn theme(source: &str, name: &str) -> String {
    set(source, "theme", text(name))
}

fn remove(source: &str, key: &str) -> String {
    apply(source, key, None).unwrap().0
}

#[test]
fn replaces_an_existing_top_level_theme() {
    assert_eq!(
        theme(
            "theme = \"flair-dark\"\n[font]\nsize = 13\n",
            "gruvbox-dark"
        ),
        "theme = \"gruvbox-dark\"\n[font]\nsize = 13\n"
    );
    assert_eq!(
        theme("  theme   =  'x' \n[font]\n", "mine"),
        "  theme   =  \"mine\" \n[font]\n"
    );
}

#[test]
fn inserts_into_empty_text() {
    assert_eq!(theme("", "mine"), "theme = \"mine\"\n");
}

#[test]
fn inserts_before_the_first_table() {
    assert_eq!(
        theme("[font]\nsize = 13\n", "mine"),
        "theme = \"mine\"\n[font]\nsize = 13\n"
    );
}

#[test]
fn theme_key_inside_a_table_is_untouched() {
    let (out, config) = apply("[keys]\nkey = \"C-b\"\n", "theme", Some(&text("mine"))).unwrap();
    assert_eq!(out, "theme = \"mine\"\n[keys]\nkey = \"C-b\"\n");
    assert_eq!(config.keys.key, "C-b");
    assert!(matches!(
        apply("[colors]\ntheme = \"x\"\n", "theme", Some(&text("mine"))),
        Err(EditError::Invalid(ConfigError::Parse(_)))
    ));
}

#[test]
fn similar_keys_are_not_replaced() {
    assert_eq!(
        set("[font]\nui_size = 12.0\n", "font.size", Value::Float(14.0)),
        "[font]\nui_size = 12.0\nsize = 14.0\n"
    );
}

#[test]
fn comments_and_crlf_are_kept() {
    assert_eq!(
        theme(
            "# my config\r\ntheme = \"a\"\r\n\r\n[font]\r\n# size\r\n",
            "b"
        ),
        "# my config\r\ntheme = \"b\"\r\n\r\n[font]\r\n# size\r\n"
    );
    assert_eq!(
        theme("# my config\r\n[font]\r\n", "b"),
        "theme = \"b\"\r\n# my config\r\n[font]\r\n"
    );
    assert_eq!(
        set("[font]\r\nsize = 13.0\r\n", "font.family", text("Iosevka")),
        "[font]\r\nsize = 13.0\r\nfamily = \"Iosevka\"\r\n"
    );
}

#[test]
fn missing_trailing_newline_is_kept() {
    assert_eq!(theme("theme = \"a\"", "b"), "theme = \"b\"");
    assert_eq!(
        theme("[font]\nsize = 13", "b"),
        "theme = \"b\"\n[font]\nsize = 13"
    );
    assert_eq!(
        theme("[font]\r\nsize = 13", "b"),
        "theme = \"b\"\r\n[font]\r\nsize = 13"
    );
}

#[test]
fn result_parses() {
    for source in [
        "",
        "theme = \"flair-dark\"\n",
        "[font]\nsize = 15\n",
        "# c\r\n[colors]\r\naccent = \"#ff0000\"\r\n",
        "\u{feff}[font]\nsize = 15",
        "font.size = 15.0\n",
        "font = { size = 15.0 }\n",
    ] {
        let (out, config) = apply(source, "theme", Some(&text("gruvbox-dark"))).unwrap();
        assert_eq!(config.theme, "gruvbox-dark", "{source:?}");
        assert_eq!(Config::from_toml(&out).unwrap(), config, "{source:?}");
    }
}

#[test]
fn a_leading_bom_stays_first() {
    let (replaced, config) =
        apply("\u{feff}theme = \"a\"\n[font]\n", "theme", Some(&text("b"))).unwrap();
    assert_eq!(replaced, "\u{feff}theme = \"b\"\n[font]\n");
    assert_eq!(config.theme, "b");
    let (inserted, config) =
        apply("\u{feff}[font]\nsize = 13\n", "theme", Some(&text("b"))).unwrap();
    assert_eq!(inserted, "\u{feff}theme = \"b\"\n[font]\nsize = 13\n");
    assert_eq!(config.theme, "b");
}

#[test]
fn quoted_theme_keys_are_replaced() {
    for (source, expected) in [
        ("\"theme\" = \"a\"\n", "\"theme\" = \"b\"\n"),
        ("'theme' = \"a\"\n", "'theme' = \"b\"\n"),
        ("  \"theme\"= 'a'\n", "  \"theme\"= \"b\"\n"),
    ] {
        let (out, config) = apply(source, "theme", Some(&text("b"))).unwrap();
        assert_eq!(out, expected, "{source:?}");
        assert_eq!(config.theme, "b");
    }
}

#[test]
fn a_trailing_comment_is_kept() {
    let (replaced, config) = apply(
        "theme = \"a\" # mine\r\n[font]\r\n",
        "theme",
        Some(&text("b")),
    )
    .unwrap();
    assert_eq!(replaced, "theme = \"b\" # mine\r\n[font]\r\n");
    assert_eq!(config.theme, "b");
    assert_eq!(
        theme("theme = \"a#b\"  # note\n", "c"),
        "theme = \"c\"  # note\n"
    );
    assert_eq!(
        set(
            "[font]\n# points\nsize = 13.0 # big\nfamily = \"x\"\n",
            "font.size",
            Value::Float(14.5)
        ),
        "[font]\n# points\nsize = 14.5 # big\nfamily = \"x\"\n"
    );
}

#[test]
fn a_new_table_is_appended() {
    assert_eq!(
        set(
            "theme = \"a\"\n\n[keys]\nkey = \"C-b\"\n",
            "font.size",
            Value::Float(15.0)
        ),
        "theme = \"a\"\n\n[keys]\nkey = \"C-b\"\n\n[font]\nsize = 15.0\n"
    );
}

#[test]
fn dotted_and_inline_tables_are_edited_in_place() {
    assert_eq!(
        set("font.size = 13.0\n", "font.size", Value::Float(14.0)),
        "font.size = 14.0\n"
    );
    assert_eq!(
        set("font.size = 13.0\n", "font.family", text("x")),
        "font.size = 13.0\nfont.family = \"x\"\n"
    );
    let (inline, config) =
        apply("font = { size = 13.0 }\n", "font.family", Some(&text("x"))).unwrap();
    assert!(inline.starts_with("font = {"), "{inline:?}");
    assert!(!inline.contains("[font]"), "{inline:?}");
    assert_eq!(config.font.family, "x");
    assert_eq!(config.font.size, 13.0);
}

#[test]
fn values_of_every_kind_are_written() {
    let (out, config) = apply(
        "",
        "shell.args",
        Some(&Value::Words(vec!["-l".to_owned(), "-i".to_owned()])),
    )
    .unwrap();
    assert_eq!(out, "[shell]\nargs = [\"-l\", \"-i\"]\n");
    assert_eq!(config.shell.args, ["-l", "-i"]);
    let (out, config) = apply("", "keys.enabled", Some(&Value::Bool(false))).unwrap();
    assert_eq!(out, "[keys]\nenabled = false\n");
    assert!(!config.keys.enabled);
    let (out, config) = apply("", "window.padding_x", Some(&Value::Integer(12))).unwrap();
    assert_eq!(out, "[window]\npadding_x = 12\n");
    assert_eq!(config.window.padding_x, 12);
}

#[test]
fn removing_a_key_keeps_its_neighbours() {
    assert_eq!(
        remove(
            "theme = \"a\" # t\n[font]\nsize = 15.0 # s\nfamily = \"x\" # f\n",
            "font.size"
        ),
        "theme = \"a\" # t\n[font]\nfamily = \"x\" # f\n"
    );
    assert_eq!(
        remove("theme = \"a\"\n[font]\nsize = 15.0\n", "theme"),
        "[font]\nsize = 15.0\n"
    );
    let (out, config) = apply("font.size = 15.0\n", "font.size", None).unwrap();
    assert!(!out.contains("size"), "{out:?}");
    assert_eq!(config, Config::default());
}

#[test]
fn removing_a_missing_key_is_a_no_op() {
    for source in ["", "# c\n", "[font]\nfamily = \"x\"\n", "theme = \"a\"\r\n"] {
        assert_eq!(remove(source, "font.size"), source, "{source:?}");
    }
}

#[test]
fn a_scalar_where_a_table_belongs_is_rejected() {
    let error = apply("font = 3\n", "font.size", Some(&Value::Float(14.0))).unwrap_err();
    assert!(matches!(&error, EditError::NotATable(name) if name == "font"));
    assert_eq!(error.to_string(), "[font] in flair.toml is not a table");
}

#[test]
fn a_broken_file_is_a_parse_error() {
    let error = apply("[font\nsize = 13\n", "theme", Some(&text("a"))).unwrap_err();
    assert!(matches!(error, EditError::Parse(_)));
    let message = error.to_string();
    assert!(
        message.starts_with("flair.toml does not parse: "),
        "{message}"
    );
    assert!(!message.contains('\n'), "{message}");
}

#[test]
fn an_invalid_result_is_rejected() {
    let error = apply("", "font.size", Some(&Value::Float(0.0))).unwrap_err();
    assert!(matches!(
        error,
        EditError::Invalid(ConfigError::NotPositive {
            key: "font.size",
            ..
        })
    ));
    assert!(!error.to_string().contains('\n'));
    let error = apply("", "keys.key", Some(&text(""))).unwrap_err();
    assert!(matches!(error, EditError::Invalid(ConfigError::Keys(_))));
    let error = apply("", "font.size", Some(&text("big"))).unwrap_err();
    assert!(matches!(&error, EditError::Invalid(ConfigError::Parse(_))));
    assert!(!error.to_string().contains('\n'), "{error}");
}

#[test]
fn contains_sees_root_table_dotted_and_inline_keys() {
    let doc = document(
        "theme = \"a\"\nfont.size = 13.0\nkeys = { key = \"C-b\" }\n[window]\npadding_x = 3\n",
    )
    .unwrap();
    for key in ["theme", "font.size", "keys.key", "window.padding_x"] {
        assert!(contains(&doc, key), "{key}");
    }
    for key in [
        "font.family",
        "keys.enabled",
        "window.padding_y",
        "shell.program",
        "colors.accent",
    ] {
        assert!(!contains(&doc, key), "{key}");
    }
    assert!(contains(
        &document("\u{feff}theme = \"a\"\n").unwrap(),
        "theme"
    ));
}
