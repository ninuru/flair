use super::*;
use flair_core::{
    config::Config,
    geom::{CellPoint, CellRect, GridSize},
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

fn full(size: GridSize) -> CellRect {
    CellRect {
        origin: CellPoint::default(),
        size,
    }
}

#[test]
fn chooser_draws_inside_the_area_and_returns_the_lifted_rect() {
    let theme = Config::default().theme();
    let surfaces = theme.surfaces();
    let mut cells = grid(80, 24);
    let before = cells.clone();
    let area = CellRect {
        origin: CellPoint { col: 4, row: 2 },
        size: GridSize { cols: 70, rows: 20 },
    };
    let lifted = Prompt::choose(&status())
        .draw(&mut cells, area, &theme)
        .unwrap();
    assert_eq!(
        lifted,
        CellRect {
            origin: CellPoint { col: 9, row: 9 },
            size: GridSize { cols: 60, rows: 5 },
        }
    );
    let lines = cells.to_lines();
    assert!(lines[10].contains(" 1: w1"));
    assert!(lines[11].contains(" 2: w2"));
    assert!(lines[12].contains(" 3: w3"));
    for row in 0..24 {
        for col in 0..80 {
            let index = usize::from(row) * 80 + usize::from(col);
            let cell = &cells.cells[index];
            if !lifted.contains(CellPoint { col, row }) {
                assert_eq!(*cell, before.cells[index]);
            } else if row == 11 {
                assert_eq!(cell.bg, surfaces.surface3);
                assert_eq!(cell.fg, theme.foreground);
            } else {
                assert_eq!(cell.bg, surfaces.surface1);
            }
        }
    }
}

#[test]
fn chooser_clips_to_a_small_area() {
    let mut cells = grid(12, 3);
    let area = full(cells.size);
    let lifted = Prompt::choose(&status())
        .draw(&mut cells, area, &Config::default().theme())
        .unwrap();
    assert_eq!(
        lifted,
        CellRect {
            origin: CellPoint::default(),
            size: GridSize { cols: 12, rows: 3 },
        }
    );
    assert_eq!(cells.cells.len(), 36);
    assert!(cells.to_lines()[1].contains(" 2: w2"));
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
fn rename_draws_nothing_and_gives_bar_text() {
    let mut cells = grid(30, 2);
    let before = cells.clone();
    let prompt = Prompt::rename("nvim".to_owned());
    assert_eq!(
        prompt.draw(&mut cells, full(before.size), &Config::default().theme()),
        None
    );
    assert_eq!(cells.cells, before.cells);
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
fn search_draws_nothing_and_gives_its_direction_as_bar_text() {
    let mut cells = grid(12, 2);
    let before = cells.clone();
    let mut prompt = Prompt::search(true);
    prompt.insert("err");
    assert_eq!(
        prompt.draw(&mut cells, full(before.size), &Config::default().theme()),
        None
    );
    assert_eq!(cells.cells, before.cells);
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
