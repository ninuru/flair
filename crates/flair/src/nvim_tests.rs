use super::*;
use std::{
    os::unix::fs::{PermissionsExt, symlink},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> TempDir {
        let path = std::env::temp_dir().join(format!(
            "fo-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn bytes(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    encode(value, &mut out);
    out
}

#[test]
fn encodes_a_request_in_compact_msgpack() {
    let request = Value::Array(vec![
        Value::Int(0),
        Value::Int(1),
        Value::Str(b"ab".to_vec()),
        Value::Array(vec![Value::Nil, Value::Bool(true), Value::Int(-1)]),
    ]);
    assert_eq!(
        bytes(&request),
        [0x94, 0x00, 0x01, 0xa2, b'a', b'b', 0x93, 0xc0, 0xc3, 0xff]
    );
}

#[test]
fn every_value_round_trips() {
    let values = vec![
        Value::Nil,
        Value::Bool(false),
        Value::Int(127),
        Value::Int(-32),
        Value::Int(-33),
        Value::Int(300),
        Value::Int(1 << 40),
        Value::Int(-(1 << 40)),
        Value::Float(1.5),
        Value::Str(vec![b'x'; 31]),
        Value::Str(vec![b'x'; 200]),
        Value::Str(vec![b'x'; 70_000]),
        Value::Str(b"caf\xe9".to_vec()),
        Value::Bin(vec![1, 2, 3]),
        Value::Array((0..20).map(Value::Int).collect()),
        Value::Map(vec![(Value::Str(b"k".to_vec()), Value::Int(1)); 17]),
        Value::Ext(2, vec![9, 9]),
    ];
    for value in values {
        let encoded = bytes(&value);
        assert_eq!(decode(&encoded), Ok((value, encoded.len())));
    }
}

#[test]
fn decodes_every_wire_width() {
    let cases: Vec<(Vec<u8>, Value)> = vec![
        (vec![0xc2], Value::Bool(false)),
        (vec![0xcc, 0xff], Value::Int(255)),
        (vec![0xcd, 0x01, 0x00], Value::Int(256)),
        (vec![0xce, 0, 1, 0, 0], Value::Int(65536)),
        (
            vec![0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            Value::Int(u64::MAX.into()),
        ),
        (vec![0xd0, 0xff], Value::Int(-1)),
        (vec![0xd1, 0xff, 0x00], Value::Int(-256)),
        (vec![0xd2, 0xff, 0xff, 0xff, 0xfe], Value::Int(-2)),
        (
            vec![0xd3, 0x80, 0, 0, 0, 0, 0, 0, 0],
            Value::Int(i64::MIN.into()),
        ),
        (vec![0xe0], Value::Int(-32)),
        (vec![0xca, 0x3f, 0xc0, 0, 0], Value::Float(1.5)),
        (vec![0xd9, 1, b'a'], Value::Str(b"a".to_vec())),
        (vec![0xda, 0, 1, b'a'], Value::Str(b"a".to_vec())),
        (vec![0xdb, 0, 0, 0, 1, b'a'], Value::Str(b"a".to_vec())),
        (vec![0xc4, 1, 7], Value::Bin(vec![7])),
        (vec![0xc5, 0, 1, 7], Value::Bin(vec![7])),
        (vec![0xd4, 3, 7], Value::Ext(3, vec![7])),
        (vec![0xd5, 3, 7, 8], Value::Ext(3, vec![7, 8])),
        (vec![0xc7, 1, 3, 7], Value::Ext(3, vec![7])),
        (vec![0xc8, 0, 1, 3, 7], Value::Ext(3, vec![7])),
        (vec![0xdc, 0, 1, 0xc0], Value::Array(vec![Value::Nil])),
        (
            vec![0xde, 0, 1, 0xc0, 0xc3],
            Value::Map(vec![(Value::Nil, Value::Bool(true))]),
        ),
        (
            vec![0x81, 0x01, 0x02],
            Value::Map(vec![(Value::Int(1), Value::Int(2))]),
        ),
    ];
    for (wire, value) in cases {
        assert_eq!(decode(&wire), Ok((value, wire.len())), "{wire:02x?}");
    }
}

#[test]
fn truncated_input_is_incomplete_and_c1_is_invalid() {
    let full = bytes(&Value::Array(vec![
        Value::Str(b"hello".to_vec()),
        Value::Int(1000),
    ]));
    for len in 0..full.len() {
        assert_eq!(decode(&full[..len]), Err(Decode::Incomplete));
    }
    assert_eq!(decode(&[0xc1]), Err(Decode::Invalid));
    assert_eq!(decode(&[0x91, 0xc1]), Err(Decode::Invalid));
}

#[test]
fn responses_match_the_msgid_and_carry_the_error_message() {
    let notification = Value::Array(vec![
        Value::Int(2),
        Value::Str(b"nvim_buf_lines_event".to_vec()),
        Value::Array(vec![]),
    ]);
    assert_eq!(response(notification, 1), None);
    let other = Value::Array(vec![Value::Int(1), Value::Int(7), Value::Nil, Value::Nil]);
    assert_eq!(response(other, 1), None);
    let ok = Value::Array(vec![
        Value::Int(1),
        Value::Int(1),
        Value::Nil,
        Value::Int(5),
    ]);
    assert_eq!(response(ok, 1), Some(Ok(Value::Int(5))));
    let failed = Value::Array(vec![
        Value::Int(1),
        Value::Int(1),
        Value::Array(vec![Value::Int(0), Value::Str(b"E37: no write".to_vec())]),
        Value::Nil,
    ]);
    assert_eq!(response(failed, 1), Some(Err("E37: no write".into())));
    assert_eq!(error_message(Value::Str(b"plain".to_vec())), "plain");
    assert_eq!(error_message(Value::Int(3)), "nvim reported an error");
}

#[test]
fn call_skips_notifications_and_reads_a_split_reply() {
    let (mut client, mut server) = UnixStream::pair().unwrap();
    let peer = std::thread::spawn(move || {
        let mut request = vec![0; 64];
        let read = server.read(&mut request).unwrap();
        let (value, _) = decode(&request[..read]).unwrap();
        let mut reply = bytes(&Value::Array(vec![
            Value::Int(2),
            Value::Str(b"redraw".to_vec()),
            Value::Array(vec![]),
        ]));
        reply.extend(bytes(&Value::Array(vec![
            Value::Int(1),
            Value::Int(1),
            Value::Array(vec![Value::Int(1), Value::Str(b"boom".to_vec())]),
            Value::Nil,
        ])));
        let (first, second) = reply.split_at(reply.len() - 3);
        server.write_all(first).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        server.write_all(second).unwrap();
        value
    });
    let reply = call(
        &mut client,
        "nvim_exec_lua",
        vec![Value::Str(b"x".to_vec())],
    )
    .unwrap();
    assert_eq!(reply, Err("boom".into()));
    assert_eq!(
        peer.join().unwrap(),
        Value::Array(vec![
            Value::Int(0),
            Value::Int(1),
            Value::Str(b"nvim_exec_lua".to_vec()),
            Value::Array(vec![Value::Str(b"x".to_vec())]),
        ])
    );
}

fn fake_process(proc: &Path, pid: u32, stat: &str, comm: &str, inodes: &[u64]) {
    let dir = proc.join(pid.to_string());
    fs::create_dir_all(dir.join("fd")).unwrap();
    fs::write(dir.join("stat"), stat).unwrap();
    fs::write(dir.join("comm"), format!("{comm}\n")).unwrap();
    for (fd, inode) in inodes.iter().enumerate() {
        symlink(
            format!("socket:[{inode}]"),
            dir.join("fd").join(fd.to_string()),
        )
        .unwrap();
    }
    symlink("pipe:[1]", dir.join("fd").join("99")).unwrap();
}

fn unix_table(rows: &[(&str, u64, &[u8])]) -> Vec<u8> {
    let mut table = b"Num       RefCount Protocol Flags    Type St Inode Path\n".to_vec();
    for (flags, inode, path) in rows {
        table.extend(
            format!("0000000000000000: 00000002 00000000 {flags} 0001 01 {inode:5}").bytes(),
        );
        if !path.is_empty() {
            table.push(b' ');
            table.extend_from_slice(path);
        }
        table.push(b'\n');
    }
    table
}

#[test]
fn finds_listening_sockets_of_nvim_descendants_only() {
    let root = TempDir::new("proc");
    let proc = root.0.as_path();
    fs::create_dir_all(proc.join("net")).unwrap();
    fake_process(proc, 100, "100 (zsh) S 1 100 100 0", "zsh", &[]);
    fake_process(proc, 120, "120 (nvim) S 100 120 100 0", "nvim", &[11]);
    fake_process(
        proc,
        121,
        "121 (a) (b) S 120 120 100 0",
        "nvim",
        &[12, 13, 14, 15],
    );
    fake_process(proc, 130, "130 (nvim) S 100 130 100 0", "nvim", &[16]);
    fake_process(proc, 200, "200 (bash) S 1 200 200 0", "bash", &[]);
    fake_process(proc, 210, "210 (nvim) S 200 210 200 0", "nvim", &[17]);
    fake_process(proc, 140, "140 (lua) S 100 140 100 0", "lua", &[18]);
    let odd: &[u8] = b"a b.\xffsock";
    let rows: [(&str, u64, &[u8]); 8] = [
        ("00000000", 11, b""),
        ("00010000", 12, b"nvim.121.0"),
        ("00010000", 13, b"@abstract"),
        ("00000000", 14, b"connected"),
        ("00010000", 15, odd),
        ("00010000", 16, b"second"),
        ("00010000", 17, b"elsewhere"),
        ("00010000", 18, b"not-nvim"),
    ];
    fs::write(proc.join("net").join("unix"), unix_table(&rows)).unwrap();
    assert_eq!(
        sockets(proc, &[100]),
        vec![
            PathBuf::from("nvim.121.0"),
            PathBuf::from(OsStr::from_bytes(odd)),
            PathBuf::from("second"),
        ]
    );
    assert_eq!(
        sockets(proc, &[130, 100]).first(),
        Some(&PathBuf::from("second"))
    );
    assert_eq!(sockets(proc, &[999]), Vec::<PathBuf>::new());
    assert_eq!(parent_pid(b"5 (x) y) z) S 42 5"), Some(42));
    for pids in [&[200, 210][..], &[100]] {
        let error = open(proc, pids, Path::new("x")).unwrap_err();
        assert_eq!(error.to_string(), "no nvim runs in the active pane");
    }
}

fn nvim_on_path() -> bool {
    Command::new("nvim")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

struct Nvim(Child);

impl Drop for Nvim {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn exec_lua(socket: &Path, code: &str) -> Result<Value, String> {
    let mut stream = UnixStream::connect(socket).unwrap();
    call(
        &mut stream,
        "nvim_exec_lua",
        vec![Value::Str(code.into()), Value::Array(vec![])],
    )
    .unwrap()
}

#[test]
fn opens_the_file_in_a_real_nvim_through_its_default_socket() {
    if !nvim_on_path() {
        eprintln!("skipping real nvim test: nvim is not on PATH");
        return;
    }
    let root = TempDir::new("nvim");
    let runtime = root.0.join("run");
    fs::create_dir(&runtime).unwrap();
    fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    let nvim = Nvim(
        Command::new("nvim")
            .args(["--headless", "--clean"])
            .env("XDG_RUNTIME_DIR", &runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let proc = Path::new("/proc");
    let pid = nvim.0.id();
    let deadline = Instant::now() + Duration::from_secs(10);
    let socket = loop {
        if let Some(socket) = sockets(proc, &[pid]).into_iter().next() {
            break socket;
        }
        assert!(Instant::now() < deadline, "nvim never listened");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(socket.starts_with(&runtime), "{}", socket.display());
    for name in [&b"plain file.txt"[..], b"caf\xe9 %#.txt"] {
        let file = root.0.join(OsStr::from_bytes(name));
        fs::write(&file, "hello\n").unwrap();
        open(proc, &[pid], &file).unwrap();
        assert_eq!(
            exec_lua(&socket, "return vim.fn.expand('%:p')"),
            Ok(Value::Str(file.as_os_str().as_bytes().to_vec()))
        );
    }
    let error = exec_lua(&socket, "error('flair boom')").unwrap_err();
    assert!(error.contains("flair boom"), "{error}");
}
