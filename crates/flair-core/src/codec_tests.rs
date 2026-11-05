use std::path::PathBuf;

use super::*;
use crate::{
    command::Command,
    config::Config,
    geom::{CellPoint, GridSize},
    ids::{ClientId, PaneId},
    key::{InputEvent, Key, KeyInput, Mods, MouseAction, MouseButton, MouseInput, NamedKey},
    proto::{
        Cell, CellFlags, ClientKind, ClientMsg, Color, Cursor, CursorShape, ExitReason, PaneFlags,
        PaneUpdate, ServerMsg, Target,
    },
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
        })),
        ClientMsg::Input(InputEvent::Key(KeyInput {
            key: Key::Named(NamedKey::F(5)),
            mods: Mods::default(),
            text: Some("x".to_owned()),
            repeat: false,
            keypad: true,
        })),
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
                flags: PaneFlags {
                    alt_screen: true,
                    display_offset: 12,
                },
            }],
        },
        ServerMsg::Title("zsh".to_owned()),
        ServerMsg::Bell,
        ServerMsg::Error("boom".to_owned()),
        ServerMsg::Exit(ExitReason::Detached),
        ServerMsg::Exit(ExitReason::SessionEnded),
        ServerMsg::Exit(ExitReason::ServerExited),
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
