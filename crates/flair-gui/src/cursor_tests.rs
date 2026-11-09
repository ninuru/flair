use flair_core::key::{Locks, Mods};

use super::*;

fn press(key: Key, mods: Mods, text: Option<&str>) -> KeyInput {
    KeyInput {
        key,
        mods,
        text: text.map(str::to_owned),
        repeat: false,
        keypad: false,
        action: KeyAction::Press,
        unshifted: None,
        shifted: None,
        base: None,
        locks: Locks::default(),
    }
}

fn plain(key: Key) -> KeyInput {
    press(key, Mods::default(), None)
}

fn ctrl(c: char) -> KeyInput {
    press(
        Key::Char(c),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
        None,
    )
}

fn alt(key: Key) -> KeyInput {
    press(
        key,
        Mods {
            alt: true,
            ..Mods::default()
        },
        None,
    )
}

fn at(text: &str, cursor: usize) -> TextCursor {
    let mut field = TextCursor::new(text);
    field.home();
    for _ in 0..cursor {
        field.right();
    }
    field
}

#[test]
fn new_puts_the_cursor_at_the_end_in_chars() {
    let field = TextCursor::new("äö🙂");
    assert_eq!(field.text(), "äö🙂");
    assert_eq!(field.cursor(), 3);
}

#[test]
fn insert_goes_in_at_the_cursor_and_filters_controls() {
    let mut field = at("aü", 1);
    field.insert("ß\n🙂\t");
    assert_eq!(field.text(), "aß🙂ü");
    assert_eq!(field.cursor(), 3);
}

#[test]
fn insert_stops_at_the_byte_limit_without_splitting_chars() {
    let mut field = TextCursor::new("").with_limit(5);
    field.insert("ääö");
    assert_eq!(field.text(), "ää");
    field.insert("x");
    assert_eq!(field.text(), "ääx");
    field.insert("y");
    assert_eq!(field.text(), "ääx");
    assert_eq!(TextCursor::new("äää").with_limit(4).text(), "ää");
}

#[test]
fn backspace_and_delete_remove_one_char_around_the_cursor() {
    let mut field = at("a🙂ü", 2);
    field.backspace();
    assert_eq!((field.text(), field.cursor()), ("aü", 1));
    field.delete();
    assert_eq!((field.text(), field.cursor()), ("a", 1));
    field.delete();
    assert_eq!(field.text(), "a");
    field.home();
    field.backspace();
    assert_eq!((field.text(), field.cursor()), ("a", 0));
}

#[test]
fn movement_stays_inside_the_text() {
    let mut field = TextCursor::new("äb");
    field.right();
    assert_eq!(field.cursor(), 2);
    field.left();
    field.left();
    field.left();
    assert_eq!(field.cursor(), 0);
    field.end();
    assert_eq!(field.cursor(), 2);
    field.home();
    assert_eq!(field.cursor(), 0);
}

#[test]
fn kill_line_deletes_to_the_end_and_clear_deletes_to_the_start() {
    let mut field = at("grüße welt", 5);
    field.kill_line();
    assert_eq!((field.text(), field.cursor()), ("grüße", 5));
    let mut field = at("grüße welt", 5);
    field.clear();
    assert_eq!((field.text(), field.cursor()), (" welt", 0));
}

#[test]
fn word_motion_skips_separators_then_words() {
    let mut field = TextCursor::new("über die-brücke 🙂");
    field.word_left();
    assert_eq!(field.cursor(), 9);
    field.word_left();
    assert_eq!(field.cursor(), 5);
    field.word_left();
    assert_eq!(field.cursor(), 0);
    field.word_right();
    assert_eq!(field.cursor(), 4);
    field.word_right();
    assert_eq!(field.cursor(), 8);
    field.word_right();
    field.word_right();
    assert_eq!(field.cursor(), 17);
}

#[test]
fn delete_word_back_removes_the_previous_word() {
    let mut field = at("foo bär baz", 7);
    field.delete_word_back();
    assert_eq!((field.text(), field.cursor()), ("foo  baz", 4));
    field.delete_word_back();
    assert_eq!((field.text(), field.cursor()), (" baz", 0));
}

#[test]
fn edit_routes_the_shared_key_table() {
    let mut field = TextCursor::new("ab cd");
    assert_eq!(field.edit(&ctrl('a')), Edit::Moved);
    assert_eq!(field.cursor(), 0);
    assert_eq!(field.edit(&ctrl('e')), Edit::Moved);
    assert_eq!(field.cursor(), 5);
    assert_eq!(field.edit(&ctrl('b')), Edit::Moved);
    assert_eq!(field.edit(&ctrl('f')), Edit::Moved);
    assert_eq!(field.edit(&plain(Key::Named(NamedKey::Home))), Edit::Moved);
    assert_eq!(field.edit(&plain(Key::Named(NamedKey::End))), Edit::Moved);
    assert_eq!(field.edit(&plain(Key::Named(NamedKey::Left))), Edit::Moved);
    assert_eq!(field.edit(&plain(Key::Named(NamedKey::Right))), Edit::Moved);
    assert_eq!(field.edit(&alt(Key::Char('b'))), Edit::Moved);
    assert_eq!(field.cursor(), 3);
    assert_eq!(field.edit(&alt(Key::Char('f'))), Edit::Moved);
    assert_eq!(field.cursor(), 5);
    let ctrl_left = press(
        Key::Named(NamedKey::Left),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
        None,
    );
    assert_eq!(field.edit(&ctrl_left), Edit::Moved);
    assert_eq!(field.cursor(), 3);
    assert_eq!(
        field.edit(&press(Key::Char('x'), Mods::default(), Some("x"))),
        Edit::Changed
    );
    assert_eq!(field.text(), "ab xcd");
    assert_eq!(
        field.edit(&plain(Key::Named(NamedKey::Delete))),
        Edit::Changed
    );
    assert_eq!(
        field.edit(&plain(Key::Named(NamedKey::Backspace))),
        Edit::Changed
    );
    assert_eq!(field.text(), "ab d");
    assert_eq!(field.edit(&ctrl('k')), Edit::Changed);
    assert_eq!(field.text(), "ab ");
    assert_eq!(field.edit(&ctrl('w')), Edit::Changed);
    assert_eq!(field.text(), "");
    field.insert("one two");
    assert_eq!(
        field.edit(&alt(Key::Named(NamedKey::Backspace))),
        Edit::Changed
    );
    assert_eq!(field.text(), "one ");
    assert_eq!(field.edit(&ctrl('u')), Edit::Changed);
    assert_eq!(field.text(), "");
}

#[test]
fn edit_ignores_releases_unknown_keys_and_no_ops() {
    let mut field = TextCursor::new("a");
    let mut release = press(Key::Char('b'), Mods::default(), Some("b"));
    release.action = KeyAction::Release;
    assert_eq!(field.edit(&release), Edit::Ignored);
    assert_eq!(field.edit(&ctrl('z')), Edit::Ignored);
    assert_eq!(field.edit(&plain(Key::Named(NamedKey::Up))), Edit::Ignored);
    assert_eq!(field.edit(&plain(Key::Named(NamedKey::End))), Edit::Ignored);
    assert_eq!(field.edit(&plain(Key::Char('q'))), Edit::Ignored);
    let super_q = press(
        Key::Char('q'),
        Mods {
            super_: true,
            ..Mods::default()
        },
        Some("q"),
    );
    assert_eq!(field.edit(&super_q), Edit::Ignored);
    assert_eq!(field.text(), "a");
}
