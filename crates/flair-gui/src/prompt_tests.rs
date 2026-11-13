use super::*;
use flair_core::{
    ids::WindowId,
    key::{Locks, Mods},
};

fn key(key: Key, text: Option<&str>) -> KeyInput {
    KeyInput {
        key,
        mods: Mods::default(),
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

fn status() -> StatusLine {
    StatusLine {
        session: "main".to_owned(),
        hidden: 0,
        running: false,
        finished: false,
        windows: (1..=3)
            .map(|index| StatusWindow {
                id: WindowId(u32::from(index) + 10),
                index,
                name: format!("w{index}"),
                program: String::new(),
                active: index == 2,
                last: false,
            })
            .collect(),
        pending: None,
        copy: false,
    }
}

#[test]
fn chooser_starts_on_the_active_window_and_selects_by_id() {
    let mut prompt = Prompt::choose(&status());
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Down), None)),
        Outcome::Pending
    );
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Down), None)),
        Outcome::Pending
    );
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Enter), None)),
        Outcome::Run(Command::SelectWindow(WindowRef::Id(WindowId(13))))
    );
}

#[test]
fn chooser_digit_selects_by_number_and_escape_closes() {
    let mut prompt = Prompt::choose(&status());
    assert_eq!(
        prompt.key(&key(Key::Char('1'), Some("1"))),
        Outcome::Run(Command::SelectWindow(WindowRef::Index(1)))
    );
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Escape), None)),
        Outcome::Close
    );
}

#[test]
fn chooser_ignores_releases() {
    let mut prompt = Prompt::choose(&status());
    let mut release = key(Key::Named(NamedKey::Escape), None);
    release.action = KeyAction::Release;
    assert_eq!(prompt.key(&release), Outcome::Pending);
}

#[test]
fn chooser_sheet_lists_windows_with_numbers_and_state_labels() {
    let mut status = status();
    status.windows[0].last = true;
    let sheet = Prompt::choose(&status).sheet().unwrap();
    assert_eq!(sheet.title, Some("Windows"));
    assert_eq!(sheet.count, (3, 3));
    assert_eq!(sheet.placement, Placement::Centred);
    assert_eq!(sheet.field, None);
    assert_eq!(sheet.selected, 1);
    let rows: Vec<_> = sheet
        .rows
        .iter()
        .map(|row| (row.number, row.name.as_str(), row.label))
        .collect();
    assert_eq!(
        rows,
        [
            (Some(1), "w1", Some("last")),
            (Some(2), "w2", Some("active")),
            (Some(3), "w3", None),
        ]
    );
    let keys: Vec<_> = sheet
        .keys
        .iter()
        .map(|(key, verb)| (*key, verb.as_str()))
        .collect();
    assert_eq!(
        keys,
        [
            ("↵", "select"),
            ("↑↓", "move"),
            ("1-9", "jump"),
            ("esc", "cancel")
        ]
    );
}

#[test]
fn chooser_moves_the_selection_within_bounds() {
    let mut prompt = Prompt::choose(&status());
    prompt.move_selection(5);
    assert_eq!(prompt.sheet().unwrap().selected, 2);
    prompt.move_selection(-9);
    assert_eq!(prompt.sheet().unwrap().selected, 0);
    let mut rename = Prompt::rename("x".to_owned());
    rename.move_selection(1);
    assert_eq!(rename.sheet(), None);
}

#[test]
fn rename_edits_text_and_submits_it() {
    let mut prompt = Prompt::rename("zsh".to_owned());
    prompt.key(&key(Key::Named(NamedKey::Backspace), None));
    prompt.key(&key(Key::Char('x'), Some("x")));
    prompt.insert("é\n");
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Enter), None)),
        Outcome::Run(Command::RenameWindow(Some("zsxé".to_owned())))
    );
}

#[test]
fn rename_has_no_sheet_and_gives_bar_text() {
    let prompt = Prompt::rename("nvim".to_owned());
    assert_eq!(prompt.sheet(), None);
    assert_eq!(prompt.bar_text().as_deref(), Some("(rename-window) nvim"));
    assert_eq!(Prompt::choose(&status()).bar_text(), None);
}

#[test]
fn rename_stops_at_the_servers_name_limit() {
    let mut prompt = Prompt::rename(String::new());
    prompt.insert(&"é".repeat(200));
    let Outcome::Run(Command::RenameWindow(Some(name))) =
        prompt.key(&key(Key::Named(NamedKey::Enter), None))
    else {
        panic!("rename did not submit");
    };
    assert_eq!(name.len(), 254);
}

#[test]
fn search_edits_text_and_submits_a_copy_search() {
    let mut prompt = Prompt::search(false);
    prompt.insert("fo");
    prompt.key(&key(Key::Char('x'), Some("x")));
    prompt.key(&key(Key::Named(NamedKey::Backspace), None));
    prompt.key(&key(Key::Char('o'), Some("o")));
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Enter), None)),
        Outcome::Run(Command::Copy(CopyCommand::Search {
            pattern: "foo".to_owned(),
            forward: false,
        }))
    );
    assert_eq!(
        Prompt::search(true).key(&key(Key::Named(NamedKey::Escape), None)),
        Outcome::Close
    );
}

#[test]
fn search_has_no_sheet_and_gives_its_direction_as_bar_text() {
    let mut prompt = Prompt::search(true);
    prompt.insert("err");
    assert_eq!(prompt.sheet(), None);
    assert_eq!(prompt.bar_text().as_deref(), Some("/err"));
    let mut prompt = Prompt::search(false);
    prompt.insert("x");
    assert_eq!(prompt.bar_text().as_deref(), Some("?x"));
}

#[test]
fn rename_moves_the_cursor_and_inserts_mid_text() {
    let mut prompt = Prompt::rename("vim".to_owned());
    assert_eq!(prompt.cursor(), Some(3));
    prompt.key(&key(Key::Named(NamedKey::Home), None));
    prompt.key(&key(Key::Named(NamedKey::Right), None));
    assert_eq!(prompt.cursor(), Some(1));
    prompt.key(&key(Key::Char('ü'), Some("ü")));
    prompt.insert("🙂");
    assert_eq!(prompt.cursor(), Some(3));
    assert_eq!(prompt.field_text(), Some("vü🙂im"));
    assert_eq!(prompt.field_prefix(), Some("(rename-window) "));
    assert_eq!(prompt.bar_text().as_deref(), Some("(rename-window) vü🙂im"));
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Enter), None)),
        Outcome::Run(Command::RenameWindow(Some("vü🙂im".to_owned())))
    );
}

#[test]
fn empty_rename_is_rejected_and_keeps_the_prompt() {
    let mut prompt = Prompt::rename("ab".to_owned());
    prompt.key(&KeyInput {
        mods: Mods {
            ctrl: true,
            ..Mods::default()
        },
        ..key(Key::Char('u'), None)
    });
    assert_eq!(prompt.field_text(), Some(""));
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Enter), None)),
        Outcome::Invalid
    );
    prompt.insert("c");
    assert_eq!(
        prompt.key(&key(Key::Named(NamedKey::Enter), None)),
        Outcome::Run(Command::RenameWindow(Some("c".to_owned())))
    );
}

#[test]
fn field_accessors_split_prefix_and_text() {
    let mut prompt = Prompt::search(false);
    prompt.insert("err");
    assert_eq!(prompt.field_prefix(), Some("?"));
    assert_eq!(prompt.field_text(), Some("err"));
    assert_eq!(prompt.cursor(), Some(3));
    assert_eq!(Prompt::search(true).field_prefix(), Some("/"));
    let chooser = Prompt::choose(&status());
    assert_eq!(chooser.field_prefix(), None);
    assert_eq!(chooser.field_text(), None);
    assert_eq!(chooser.cursor(), None);
}

#[test]
fn chooser_rows_carry_the_process_icon() {
    let mut status = status();
    status.windows[0].program = "ssh".into();
    let sheet = Prompt::choose(&status).sheet().unwrap();
    let icons: Vec<Option<char>> = sheet.rows.iter().map(|row| row.icon).collect();
    assert_eq!(icons, [Some('\u{ea7a}'), None, None]);
}
