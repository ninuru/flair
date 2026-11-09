use super::*;
use crate::config::Config;

#[test]
fn replaces_an_existing_top_level_theme() {
    assert_eq!(
        set_theme(
            "theme = \"flair-dark\"\n[font]\nsize = 13\n",
            "gruvbox-dark"
        ),
        "theme = \"gruvbox-dark\"\n[font]\nsize = 13\n"
    );
    assert_eq!(
        set_theme("shell = 1\n  theme   =  'x' \n", "mine"),
        "shell = 1\ntheme = \"mine\"\n"
    );
}

#[test]
fn inserts_into_empty_text() {
    assert_eq!(set_theme("", "mine"), "theme = \"mine\"\n");
}

#[test]
fn inserts_before_the_first_table() {
    assert_eq!(
        set_theme("[font]\nsize = 13\n", "mine"),
        "theme = \"mine\"\n[font]\nsize = 13\n"
    );
}

#[test]
fn theme_key_inside_a_table_is_untouched() {
    assert_eq!(
        set_theme("[colors]\ntheme = \"x\"\n", "mine"),
        "theme = \"mine\"\n[colors]\ntheme = \"x\"\n"
    );
}

#[test]
fn similar_keys_are_not_replaced() {
    assert_eq!(
        set_theme("themes = 1\n", "mine"),
        "theme = \"mine\"\nthemes = 1\n"
    );
}

#[test]
fn comments_and_crlf_are_kept() {
    assert_eq!(
        set_theme(
            "# my config\r\ntheme = \"a\" \r\n\r\n[font]\r\n# size\r\n",
            "b"
        ),
        "# my config\r\ntheme = \"b\"\r\n\r\n[font]\r\n# size\r\n"
    );
    assert_eq!(
        set_theme("# my config\r\n[font]\r\n", "b"),
        "theme = \"b\"\r\n# my config\r\n[font]\r\n"
    );
}

#[test]
fn missing_trailing_newline_is_kept() {
    assert_eq!(set_theme("theme = \"a\"", "b"), "theme = \"b\"");
    assert_eq!(
        set_theme("[font]\nsize = 13", "b"),
        "theme = \"b\"\n[font]\nsize = 13"
    );
}

#[test]
fn result_parses() {
    for text in [
        "",
        "theme = \"flair-dark\"\n",
        "[font]\nsize = 15\n",
        "# c\r\n[colors]\r\naccent = \"#ff0000\"\r\n",
    ] {
        let config = Config::from_toml(&set_theme(text, "gruvbox-dark")).unwrap();
        assert_eq!(config.theme, "gruvbox-dark", "{text:?}");
    }
}
