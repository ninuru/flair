use std::path::PathBuf;

use super::*;
use crate::{
    command::{Command, CopyCommand, CopyMotion, PaneRef, SelectionKind, WindowRef},
    config::Config,
    geom::{Axis, CellPoint, CellRect, Direction, GridSize},
    ids::{ClientId, PaneId, WindowId},
    key::{
        InputEvent, Key, KeyAction, KeyInput, Locks, Mods, MouseAction, MouseButton, MouseInput,
        NamedKey,
    },
    proto::{
        Cell, CellFlags, ClientKind, ClientMsg, Color, Cursor, CursorShape, ExitReason, PaneFlags,
        PanePlacement, PaneUpdate, Separator, ServerMsg, SessionInfo, Target, WindowLayout,
    },
    status::{StatusLine, StatusWindow},
};

fn size() -> GridSize {
    GridSize { cols: 80, rows: 24 }
}

fn client_messages() -> Vec<ClientMsg> {
    let ctrl = Mods {
        ctrl: true,
        ..Mods::default()
    };
    vec![
        ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: size(),
            palette: Config::default().theme(),
            env: vec![("WAYLAND_DISPLAY".to_owned(), "wayland-1".to_owned())],
            target: Target::Main,
        },
        ClientMsg::Hello {
            protocol: PROTOCOL,
            kind: ClientKind::Gui,
            size: size(),
            palette: Config::default().theme(),
            env: Vec::new(),
            target: Target::Path(PathBuf::from("/home/u/project")),
        },
        ClientMsg::Resize(size()),
        ClientMsg::Input(InputEvent::Key(KeyInput {
            key: Key::Char('a'),
            mods: ctrl,
            text: None,
            repeat: true,
            keypad: false,
            action: KeyAction::Press,
            unshifted: None,
            shifted: None,
            base: None,
            locks: Locks::default(),
        })),
        ClientMsg::Input(InputEvent::Key(KeyInput {
            key: Key::Named(NamedKey::F(5)),
            mods: Mods::default(),
            text: Some("x".to_owned()),
            repeat: false,
            keypad: true,
            action: KeyAction::Press,
            unshifted: None,
            shifted: None,
            base: None,
            locks: Locks::default(),
        })),
        ClientMsg::Input(InputEvent::Key(KeyInput {
            key: Key::Char('&'),
            mods: Mods {
                shift: true,
                ..Mods::default()
            },
            text: None,
            repeat: false,
            keypad: false,
            action: KeyAction::Release,
            unshifted: Some('7'),
            shifted: Some('&'),
            base: Some('7'),
            locks: Locks {
                caps: true,
                num: true,
            },
        })),
        ClientMsg::Input(InputEvent::Key(KeyInput {
            key: Key::Named(NamedKey::ShiftLeft),
            mods: Mods::default(),
            text: None,
            repeat: true,
            keypad: false,
            action: KeyAction::Press,
            unshifted: None,
            shifted: None,
            base: None,
            locks: Locks::default(),
        })),
        ClientMsg::Input(InputEvent::Ime(true)),
        ClientMsg::Input(InputEvent::Text("ä".to_owned())),
        ClientMsg::Input(InputEvent::Paste("a\nb".to_owned())),
        ClientMsg::Input(InputEvent::Mouse(MouseInput {
            point: CellPoint { col: 3, row: 4 },
            button: MouseButton::WheelUp,
            action: MouseAction::Press,
            mods: ctrl,
        })),
        ClientMsg::Focus(true),
        ClientMsg::FrameAck { seq: u64::MAX },
        ClientMsg::Command(Command::ScrollLines(-3)),
        ClientMsg::Command(Command::ScrollPages(2)),
        ClientMsg::Command(Command::ScrollToBottom),
        ClientMsg::Command(Command::PasteClipboard),
        ClientMsg::Command(Command::Detach),
        ClientMsg::Command(Command::NewWindow),
        ClientMsg::Command(Command::NextWindow),
        ClientMsg::Command(Command::PreviousWindow),
        ClientMsg::Command(Command::LastWindow),
        ClientMsg::Command(Command::SelectWindow(WindowRef::Index(9))),
        ClientMsg::Command(Command::SelectWindow(WindowRef::Id(WindowId(4)))),
        ClientMsg::Command(Command::ChooseWindow),
        ClientMsg::Command(Command::RenameWindow(None)),
        ClientMsg::Command(Command::RenameWindow(Some("editor".to_owned()))),
        ClientMsg::Command(Command::CloseWindow),
        ClientMsg::Command(Command::Split(Axis::Horizontal)),
        ClientMsg::Command(Command::Split(Axis::Vertical)),
        ClientMsg::Command(Command::SelectPane(PaneRef::Direction(Direction::Left))),
        ClientMsg::Command(Command::SelectPane(PaneRef::Id(PaneId(3)))),
        ClientMsg::Command(Command::ClosePane),
        ClientMsg::Command(Command::ToggleZoom),
        ClientMsg::Command(Command::ReloadConfig),
        ClientMsg::Command(Command::ChooseSession),
        ClientMsg::Command(Command::ChooseTheme),
        ClientMsg::Command(Command::Settings),
        ClientMsg::Palette(Config::default().theme()),
        ClientMsg::Command(Command::CopyMode),
        ClientMsg::Command(Command::Copy(CopyCommand::Cancel)),
        ClientMsg::Command(Command::Copy(CopyCommand::Yank)),
        ClientMsg::Command(Command::Copy(CopyCommand::Move(CopyMotion::HalfPageUp))),
        ClientMsg::Command(Command::Copy(CopyCommand::Select(SelectionKind::Block))),
        ClientMsg::Command(Command::Copy(CopyCommand::Search {
            pattern: "err(or)?".to_owned(),
            forward: false,
        })),
        ClientMsg::Detach,
    ]
}

fn server_messages() -> Vec<ServerMsg> {
    let cell = Cell {
        c: '世',
        fg: Color::Indexed(196),
        bg: Color::Rgb(1, 2, 3),
        flags: CellFlags::BOLD | CellFlags::WIDE,
        extra: Some("\u{301}".to_owned()),
    };
    vec![
        ServerMsg::Welcome {
            client: ClientId(7),
        },
        ServerMsg::Frame {
            seq: 42,
            panes: vec![PaneUpdate {
                pane: PaneId(1),
                size: size(),
                full: true,
                rows: vec![(0, vec![cell, Cell::default()]), (23, Vec::new())],
                cursor: Cursor {
                    point: CellPoint { col: 2, row: 0 },
                    shape: CursorShape::Beam,
                    visible: true,
                },
                selection: Vec::new(),
                flags: PaneFlags {
                    alt_screen: true,
                    display_offset: 12,
                    mouse_reporting: false,
                    copy_mode: true,
                },
            }],
        },
        ServerMsg::Layout(WindowLayout {
            window: WindowId(2),
            area: CellRect {
                origin: CellPoint { col: 0, row: 1 },
                size: GridSize { cols: 80, rows: 23 },
            },
            panes: vec![
                PanePlacement {
                    pane: PaneId(1),
                    rect: CellRect {
                        origin: CellPoint { col: 0, row: 1 },
                        size: GridSize { cols: 40, rows: 23 },
                    },
                },
                PanePlacement {
                    pane: PaneId(2),
                    rect: CellRect {
                        origin: CellPoint { col: 41, row: 1 },
                        size: GridSize { cols: 39, rows: 23 },
                    },
                },
            ],
            separators: vec![Separator {
                axis: Axis::Horizontal,
                rect: CellRect {
                    origin: CellPoint { col: 40, row: 1 },
                    size: GridSize { cols: 1, rows: 23 },
                },
            }],
            active: PaneId(2),
            zoomed: false,
        }),
        ServerMsg::Status(StatusLine {
            session: "main".to_owned(),
            hidden: 2,
            running: true,
            finished: true,
            windows: vec![StatusWindow {
                id: WindowId(2),
                index: 1,
                name: "nvim 世界".to_owned(),
                active: true,
                last: false,
            }],
            pending: Some("^A".to_owned()),
            copy: true,
        }),
        ServerMsg::ChooseWindow,
        ServerMsg::ChooseTheme,
        ServerMsg::Settings,
        ServerMsg::PasteClipboard,
        ServerMsg::PromptRename {
            current: "zsh".to_owned(),
        },
        ServerMsg::PromptSearch { forward: false },
        ServerMsg::ReloadConfig {
            toml: "[keys]\n".to_owned(),
        },
        ServerMsg::Title("zsh".to_owned()),
        ServerMsg::Bell,
        ServerMsg::Error("boom".to_owned()),
        ServerMsg::Exit(ExitReason::Detached),
        ServerMsg::Exit(ExitReason::SessionEnded),
        ServerMsg::Exit(ExitReason::ServerExited),
        ServerMsg::ChooseSession {
            sessions: vec![SessionInfo {
                name: "main".to_owned(),
                root: "/home/u".into(),
                windows: 2,
            }],
            current: "main".to_owned(),
        },
    ]
}

fn round_trip<T: Serialize + DeserializeOwned + PartialEq + fmt::Debug>(msg: &T) {
    let mut out = Vec::new();
    encode(msg, &mut out).unwrap();
    let mut reader = FrameReader::default();
    reader.push(&out).unwrap();
    assert_eq!(&reader.next_frame::<T>().unwrap().unwrap(), msg);
    assert!(reader.next_frame::<T>().is_none());
}

#[test]
fn every_client_message_round_trips() {
    for msg in client_messages() {
        round_trip(&msg);
    }
}

#[test]
fn every_server_message_round_trips() {
    for msg in server_messages() {
        round_trip(&msg);
    }
}

#[test]
fn byte_by_byte_feeding_yields_frame_once_complete() {
    let mut out = Vec::new();
    encode(&ServerMsg::Title("hello".to_owned()), &mut out).unwrap();
    let mut reader = FrameReader::default();
    let (last, head) = out.split_last().unwrap();
    for byte in head {
        reader.push(&[*byte]).unwrap();
        assert!(reader.next_frame::<ServerMsg>().is_none());
    }
    reader.push(&[*last]).unwrap();
    assert_eq!(
        reader.next_frame::<ServerMsg>().unwrap().unwrap(),
        ServerMsg::Title("hello".to_owned())
    );
}

#[test]
fn two_frames_in_one_push() {
    let mut out = Vec::new();
    encode(&ServerMsg::Bell, &mut out).unwrap();
    encode(&ServerMsg::Title("t".to_owned()), &mut out).unwrap();
    let mut reader = FrameReader::default();
    reader.push(&out).unwrap();
    assert_eq!(
        reader.next_frame::<ServerMsg>().unwrap().unwrap(),
        ServerMsg::Bell
    );
    assert_eq!(
        reader.next_frame::<ServerMsg>().unwrap().unwrap(),
        ServerMsg::Title("t".to_owned())
    );
    assert!(reader.next_frame::<ServerMsg>().is_none());
}

#[test]
fn oversize_frame_is_rejected_on_encode() {
    let mut out = Vec::new();
    let result = encode(&ServerMsg::Error("x".repeat(MAX_FRAME)), &mut out);
    assert!(matches!(result, Err(CodecError::Oversize(_))));
    assert!(out.is_empty());
}

fn poisoned_reader() -> (FrameReader, usize) {
    let mut reader = FrameReader::default();
    let len = MAX_FRAME + 1;
    reader
        .push(&u32::try_from(len).unwrap().to_le_bytes())
        .unwrap();
    (reader, len)
}

#[test]
fn oversize_header_is_rejected_on_decode() {
    let (mut reader, len) = poisoned_reader();
    assert!(matches!(
        reader.next_frame::<ServerMsg>(),
        Some(Err(CodecError::Oversize(got))) if got == len
    ));
    assert!(matches!(
        reader.next_frame::<ServerMsg>(),
        Some(Err(CodecError::Oversize(got))) if got == len
    ));
    assert!(matches!(
        reader.next_version(),
        Some(Err(CodecError::Oversize(got))) if got == len
    ));
}

#[test]
fn oversize_poisons_the_reader_for_later_bytes() {
    let (mut reader, len) = poisoned_reader();
    assert!(reader.next_frame::<ServerMsg>().unwrap().is_err());
    let mut out = Vec::new();
    encode(&ServerMsg::Bell, &mut out).unwrap();
    assert!(matches!(
        reader.push(&out),
        Err(CodecError::Oversize(got)) if got == len
    ));
    assert!(matches!(
        reader.next_frame::<ServerMsg>(),
        Some(Err(CodecError::Oversize(got))) if got == len
    ));
}

#[test]
fn malformed_payload_is_an_error() {
    let mut reader = FrameReader::default();
    reader.push(&[1, 0, 0, 0, 0xff]).unwrap();
    assert!(matches!(
        reader.next_frame::<ServerMsg>(),
        Some(Err(CodecError::Postcard(_)))
    ));
}

#[test]
fn version_frame_round_trips() {
    let mut out = Vec::new();
    encode_version(&mut out);
    assert_eq!(out.len(), 4);
    assert_eq!(decode_version(&out).unwrap(), PROTOCOL);
}

#[test]
fn version_frame_with_wrong_length_is_rejected() {
    assert!(matches!(
        decode_version(&[1, 0, 0]),
        Err(CodecError::VersionFrame(3))
    ));
}

#[test]
fn version_frame_and_messages_in_one_push() {
    let mut out = Vec::new();
    encode_version(&mut out);
    encode(&ServerMsg::Bell, &mut out).unwrap();
    encode(&ServerMsg::Title("t".to_owned()), &mut out).unwrap();
    let mut reader = FrameReader::default();
    reader.push(&out).unwrap();
    assert_eq!(reader.next_version().unwrap().unwrap(), PROTOCOL);
    assert_eq!(
        reader.next_frame::<ServerMsg>().unwrap().unwrap(),
        ServerMsg::Bell
    );
    assert_eq!(
        reader.next_frame::<ServerMsg>().unwrap().unwrap(),
        ServerMsg::Title("t".to_owned())
    );
    assert!(reader.next_frame::<ServerMsg>().is_none());
}

#[test]
fn next_version_waits_for_four_bytes() {
    let mut reader = FrameReader::default();
    reader.push(&PROTOCOL.to_le_bytes()[..3]).unwrap();
    assert!(reader.next_version().is_none());
    reader.push(&PROTOCOL.to_le_bytes()[3..]).unwrap();
    assert_eq!(reader.next_version().unwrap().unwrap(), PROTOCOL);
    assert!(reader.next_version().is_none());
}

#[test]
fn headless_client_kind_round_trips() {
    let mut bytes = Vec::new();
    encode(&ClientKind::Headless, &mut bytes).unwrap();
    let mut reader = FrameReader::default();
    reader.push(&bytes).unwrap();
    assert_eq!(
        reader.next_frame::<ClientKind>().unwrap().unwrap(),
        ClientKind::Headless
    );
}
