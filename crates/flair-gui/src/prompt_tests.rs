use super::*;
use flair_core::{
    config::Config,
    geom::GridSize,
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
        windows: (1..=3)
            .map(|index| StatusWindow {
                id: WindowId(u32::from(index) + 10),
                index,
                name: format!("w{index}"),
                active: index == 2,
                last: false,
            })
            .collect(),
        pending: None,
        copy: false,
    }
}

fn grid(cols: u16, rows: u16) -> CellGrid {
    CellGrid {
        size: GridSize { cols, rows },
        cells: vec![
            DrawCell {
                c: '.',
                extra: None,
                fg: Rgb::default(),
                bg: Rgb::default(),
                flags: CellFlags::empty(),
            };
            usize::from(cols) * usize::from(rows)
        ],
        cursor: None,
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
fn chooser_draws_entries_below_the_status_row() {
    let mut cells = grid(12, 3);
    Prompt::choose(&status()).draw(&mut cells, &Config::default().theme());
    let lines = cells.to_lines();
    assert_eq!(lines[0], "............");
    assert_eq!(lines[1], " 1: w1");
    assert_eq!(lines[2], " 2: w2");
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
fn rename_draws_on_the_status_row() {
    let mut cells = grid(30, 2);
    Prompt::rename("nvim".to_owned()).draw(&mut cells, &Config::default().theme());
    assert_eq!(cells.to_lines()[0], "(rename-window) nvim");
    assert_eq!(cells.to_lines()[1], "..............................");
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
fn search_draws_its_direction_on_the_status_row() {
    let mut cells = grid(12, 2);
    let mut prompt = Prompt::search(true);
    prompt.insert("err");
    prompt.draw(&mut cells, &Config::default().theme());
    assert_eq!(cells.to_lines()[0], "/err");
    let mut cells = grid(12, 2);
    let mut prompt = Prompt::search(false);
    prompt.insert("x");
    prompt.draw(&mut cells, &Config::default().theme());
    assert_eq!(cells.to_lines()[0], "?x");
    assert_eq!(cells.to_lines()[1], "............");
}
