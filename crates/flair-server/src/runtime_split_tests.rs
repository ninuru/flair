use super::*;
use flair_core::{command::Command, geom::Axis, ids::PaneId};
use std::{collections::BTreeMap, fs, os::unix::fs::PermissionsExt};

#[test]
fn real_splits_inherit_live_cwd_and_new_windows_use_session_root() {
    if !Path::new("/bin/sh").exists() {
        eprintln!("skipping split PTY test: /bin/sh unavailable");
        return;
    }
    let root = std::env::temp_dir().join(format!("flair-split-proof-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let child = root.join("child");
    fs::create_dir(&child).unwrap();
    let mut config = ServerConfig::from_config(&Config::default());
    config.program = "/bin/sh".into();
    config.cwd = root.clone();
    config.args = vec!["-c".into(), "printf 'CWD:%s\\r\\n' \"$PWD\"; while IFS= read -r line; do case \"$line\" in move) cd child; printf 'MOVED\\r\\n';; esac; done".into()];
    let (client, server) = channel_pair();
    let handle = spawn_in_process(config, server).unwrap();
    let proof = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client
            .tx
            .send(ClientMsg::Hello {
                protocol: PROTOCOL,
                kind: ClientKind::Headless,
                size: GridSize {
                    cols: 160,
                    rows: 20,
                },
                palette: Config::default().theme(),
                env: vec![],
                target: Target::Main,
            })
            .unwrap();
        let mut screens = BTreeMap::new();
        let first = wait_pane_text(
            &client,
            &mut screens,
            &format!("CWD:{}", root.display()),
            None,
        );
        client
            .tx
            .send(ClientMsg::Input(InputEvent::Text("move\r".into())))
            .unwrap();
        wait_pane_text(&client, &mut screens, "MOVED", Some(first));
        client
            .tx
            .send(ClientMsg::Command(Command::Split(Axis::Horizontal)))
            .unwrap();
        let split = wait_pane_text(
            &client,
            &mut screens,
            &format!("CWD:{}", child.display()),
            None,
        );
        assert_ne!(first, split);
        client
            .tx
            .send(ClientMsg::Command(Command::NewWindow))
            .unwrap();
        let window = wait_pane_text(
            &client,
            &mut screens,
            &format!("CWD:{}", root.display()),
            None,
        );
        assert_ne!(first, window);
        assert_ne!(split, window);
    }));
    drop(client);
    handle.join().unwrap();
    fs::remove_dir_all(root).unwrap();
    if let Err(error) = proof {
        std::panic::resume_unwind(error);
    }
}

fn wait_pane_text(
    client: &flair_core::link::ClientLink,
    screens: &mut BTreeMap<PaneId, Vec<String>>,
    expected: &str,
    pane: Option<PaneId>,
) -> PaneId {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let message = client
            .rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        if let ServerMsg::Frame { seq, panes } = message {
            let updated: Vec<_> = panes.iter().map(|update| update.pane).collect();
            for update in panes {
                if update.full {
                    screens.insert(
                        update.pane,
                        vec![String::new(); usize::from(update.size.rows)],
                    );
                }
                let rows = screens.get_mut(&update.pane).unwrap();
                for (index, cells) in update.rows {
                    rows[usize::from(index)] = cells.iter().map(|cell| cell.c).collect();
                }
            }
            client.tx.send(ClientMsg::FrameAck { seq }).unwrap();
            if let Some(found) = updated.into_iter().find(|id| {
                pane.is_none_or(|pane| pane == *id) && screens[id].join("\n").contains(expected)
            }) {
                return found;
            }
        } else if let ServerMsg::Error(error) = message {
            panic!("server error: {error}");
        }
    }
}
