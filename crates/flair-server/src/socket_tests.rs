use super::*;
use std::os::unix::net::UnixStream;

#[test]
fn socket_codec_round_trips_and_rejects_oversized_headers() {
    let (mut a, mut b) = UnixStream::pair().unwrap();
    write_message(
        &mut a,
        &ClientMsg::Control(flair_core::proto::Control::List),
    )
    .unwrap();
    assert_eq!(
        read_message::<ClientMsg>(&mut b).unwrap(),
        ClientMsg::Control(flair_core::proto::Control::List)
    );
    a.write_all(&((MAX_FRAME + 1) as u32).to_le_bytes())
        .unwrap();
    assert!(read_message::<ClientMsg>(&mut b).is_err());
}

#[test]
fn handshake_checks_version_before_decoding_messages() {
    let (mut a, mut b) = UnixStream::pair().unwrap();
    let peer = thread::spawn(move || {
        let mut version = [0; 4];
        b.read_exact(&mut version).unwrap();
        assert_eq!(u32::from_le_bytes(version), PROTOCOL);
        b.write_all(&(PROTOCOL + 1).to_le_bytes()).unwrap();
    });
    let error = handshake(&mut a).unwrap_err().to_string();
    assert!(error.contains("run flair kill-server"));
    peer.join().unwrap();
}

#[test]
fn peer_credentials_are_from_the_kernel() {
    let (a, _) = UnixStream::pair().unwrap();
    assert_eq!(peer_pid(&a).unwrap(), std::process::id() as i32);
}

#[test]
fn private_socket_directory_rejects_public_permissions_and_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = std::env::temp_dir().join(format!("flair-socket-security-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(private_parent(&root.join("test.sock")).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(private_parent(&root.join("test.sock")).is_ok());
    let linked = root.join("linked");
    symlink(&root, &linked).unwrap();
    assert!(private_parent(&linked.join("test.sock")).is_err());
    let lock_path = root.join("test.sock.lock");
    symlink(root.join("untouched"), &lock_path).unwrap();
    assert!(lock(&root.join("test.sock")).is_err());
    assert!(!root.join("untouched").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn explicit_socket_name_overrides_inherited_pane_routing() {
    assert_eq!(
        resolve_socket_path(
            None,
            Some("dev".into()),
            Some("/tmp/regular/default.sock,1234".into()),
            Some("/run/user/test".into())
        )
        .unwrap(),
        PathBuf::from("/run/user/test/flair/dev.sock")
    );
    assert_eq!(
        resolve_socket_path(
            Some("/tmp/explicit.sock".into()),
            Some("dev".into()),
            Some("/tmp/regular/default.sock,1234".into()),
            None
        )
        .unwrap(),
        PathBuf::from("/tmp/explicit.sock")
    );
    assert_eq!(
        resolve_socket_path(
            None,
            None,
            Some("/tmp/regular/default.sock,1234".into()),
            None
        )
        .unwrap(),
        PathBuf::from("/tmp/regular/default.sock")
    );
}
