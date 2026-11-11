use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    io::{self, Read, Write},
    os::unix::{ffi::OsStrExt, net::UnixStream},
    path::{Path, PathBuf},
    time::Duration,
};

const ACCEPTING: u32 = 0x10000;
const DROP: &str = "vim.cmd.drop(vim.fn.fnameescape(...))";

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i128),
    Float(f64),
    Str(Vec<u8>),
    Bin(Vec<u8>),
    Array(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Ext(i8, Vec<u8>),
}

#[derive(Debug, PartialEq)]
pub enum Decode {
    Incomplete,
    Invalid,
}

pub fn open(proc: &Path, pids: &[u32], file: &Path) -> io::Result<()> {
    for socket in sockets(proc, pids) {
        let Ok(mut stream) = UnixStream::connect(&socket) else {
            continue;
        };
        return match call(
            &mut stream,
            "nvim_exec_lua",
            vec![
                Value::Str(DROP.into()),
                Value::Array(vec![Value::Str(file.as_os_str().as_bytes().to_vec())]),
            ],
        )? {
            Ok(_) => Ok(()),
            Err(message) => Err(io::Error::other(message)),
        };
    }
    Err(io::Error::other("no nvim runs in the active pane"))
}

pub fn call(
    stream: &mut UnixStream,
    method: &str,
    args: Vec<Value>,
) -> io::Result<Result<Value, String>> {
    let msgid = 1;
    let mut request = Vec::new();
    encode(
        &Value::Array(vec![
            Value::Int(0),
            Value::Int(msgid),
            Value::Str(method.into()),
            Value::Array(args),
        ]),
        &mut request,
    );
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.write_all(&request)?;
    let mut buf = Vec::new();
    loop {
        match decode(&buf) {
            Ok((message, used)) => {
                buf.drain(..used);
                if let Some(reply) = response(message, msgid) {
                    return Ok(reply);
                }
            }
            Err(Decode::Incomplete) => {
                let mut chunk = [0; 4096];
                let read = stream.read(&mut chunk)?;
                if read == 0 {
                    return Err(io::Error::other("nvim closed the connection"));
                }
                buf.extend_from_slice(&chunk[..read]);
            }
            Err(Decode::Invalid) => return Err(io::Error::other("nvim sent invalid msgpack")),
        }
    }
}

fn response(message: Value, msgid: i128) -> Option<Result<Value, String>> {
    let Value::Array(mut items) = message else {
        return None;
    };
    if items.len() != 4 || items[0] != Value::Int(1) || items[1] != Value::Int(msgid) {
        return None;
    }
    let result = items.pop()?;
    match items.pop()? {
        Value::Nil => Some(Ok(result)),
        error => Some(Err(error_message(error))),
    }
}

fn error_message(error: Value) -> String {
    let text = match error {
        Value::Str(text) => Some(text),
        Value::Array(items) => match items.into_iter().nth(1) {
            Some(Value::Str(text)) => Some(text),
            Some(_) | None => None,
        },
        Value::Nil
        | Value::Bool(_)
        | Value::Int(_)
        | Value::Float(_)
        | Value::Bin(_)
        | Value::Map(_)
        | Value::Ext(_, _) => None,
    };
    text.map_or_else(
        || "nvim reported an error".into(),
        |text| String::from_utf8_lossy(&text).into_owned(),
    )
}

pub fn encode(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Nil => out.push(0xc0),
        Value::Bool(value) => out.push(if *value { 0xc3 } else { 0xc2 }),
        Value::Int(value) => encode_int(*value, out),
        Value::Float(value) => {
            out.push(0xcb);
            out.extend_from_slice(&value.to_be_bytes());
        }
        Value::Str(bytes) => {
            match bytes.len() {
                len @ 0..32 => out.push(0xa0 | len as u8),
                len @ 32..256 => out.extend_from_slice(&[0xd9, len as u8]),
                len @ 256..65536 => {
                    out.push(0xda);
                    out.extend_from_slice(&(len as u16).to_be_bytes());
                }
                len => {
                    out.push(0xdb);
                    out.extend_from_slice(&(len as u32).to_be_bytes());
                }
            }
            out.extend_from_slice(bytes);
        }
        Value::Bin(bytes) => {
            out.push(0xc6);
            out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            out.extend_from_slice(bytes);
        }
        Value::Array(items) => {
            encode_len(items.len(), 0x90, 0xdd, out);
            for item in items {
                encode(item, out);
            }
        }
        Value::Map(entries) => {
            encode_len(entries.len(), 0x80, 0xdf, out);
            for (key, value) in entries {
                encode(key, out);
                encode(value, out);
            }
        }
        Value::Ext(kind, bytes) => {
            out.push(0xc9);
            out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            out.push(*kind as u8);
            out.extend_from_slice(bytes);
        }
    }
}

fn encode_int(value: i128, out: &mut Vec<u8>) {
    match value {
        0..128 => out.push(value as u8),
        -32..0 => out.push(value as i8 as u8),
        128..=0xffff_ffff => {
            out.push(0xce);
            out.extend_from_slice(&(value as u32).to_be_bytes());
        }
        0x1_0000_0000.. => {
            out.push(0xcf);
            out.extend_from_slice(&(value as u64).to_be_bytes());
        }
        ..-32 => {
            out.push(0xd3);
            out.extend_from_slice(&(value as i64).to_be_bytes());
        }
    }
}

fn encode_len(len: usize, fix: u8, wide: u8, out: &mut Vec<u8>) {
    if len < 16 {
        out.push(fix | len as u8);
    } else {
        out.push(wide);
        out.extend_from_slice(&(len as u32).to_be_bytes());
    }
}

pub fn decode(bytes: &[u8]) -> Result<(Value, usize), Decode> {
    let mut reader = Reader { bytes, at: 0 };
    let value = reader.value()?;
    Ok((value, reader.at))
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Result<&[u8], Decode> {
        let end = self.at.checked_add(len).ok_or(Decode::Invalid)?;
        let slice = self.bytes.get(self.at..end).ok_or(Decode::Incomplete)?;
        self.at = end;
        Ok(slice)
    }

    fn uint(&mut self, len: usize) -> Result<u64, Decode> {
        Ok(self
            .take(len)?
            .iter()
            .fold(0, |acc, byte| acc << 8 | u64::from(*byte)))
    }

    fn int(&mut self, len: usize) -> Result<i64, Decode> {
        let shift = 64 - 8 * len as u32;
        Ok(((self.uint(len)? << shift) as i64) >> shift)
    }

    fn len(&mut self, len: usize) -> Result<usize, Decode> {
        usize::try_from(self.uint(len)?).map_err(|_| Decode::Invalid)
    }

    fn bytes(&mut self, len: usize) -> Result<Vec<u8>, Decode> {
        Ok(self.take(len)?.to_vec())
    }

    fn array(&mut self, len: usize) -> Result<Value, Decode> {
        let mut items = Vec::new();
        for _ in 0..len {
            items.push(self.value()?);
        }
        Ok(Value::Array(items))
    }

    fn map(&mut self, len: usize) -> Result<Value, Decode> {
        let mut entries = Vec::new();
        for _ in 0..len {
            entries.push((self.value()?, self.value()?));
        }
        Ok(Value::Map(entries))
    }

    fn ext(&mut self, len: usize) -> Result<Value, Decode> {
        let kind = self.take(1)?[0] as i8;
        Ok(Value::Ext(kind, self.bytes(len)?))
    }

    fn value(&mut self) -> Result<Value, Decode> {
        let marker = self.take(1)?[0];
        match marker {
            0x00..=0x7f => Ok(Value::Int(i128::from(marker))),
            0x80..=0x8f => self.map(usize::from(marker & 0x0f)),
            0x90..=0x9f => self.array(usize::from(marker & 0x0f)),
            0xa0..=0xbf => Ok(Value::Str(self.bytes(usize::from(marker & 0x1f))?)),
            0xc0 => Ok(Value::Nil),
            0xc1 => Err(Decode::Invalid),
            0xc2 => Ok(Value::Bool(false)),
            0xc3 => Ok(Value::Bool(true)),
            0xc4..=0xc6 => {
                let len = self.len(1 << (marker - 0xc4))?;
                Ok(Value::Bin(self.bytes(len)?))
            }
            0xc7..=0xc9 => {
                let len = self.len(1 << (marker - 0xc7))?;
                self.ext(len)
            }
            0xca => Ok(Value::Float(f64::from(
                f32::from_bits(self.uint(4)? as u32),
            ))),
            0xcb => Ok(Value::Float(f64::from_bits(self.uint(8)?))),
            0xcc..=0xcf => Ok(Value::Int(i128::from(self.uint(1 << (marker - 0xcc))?))),
            0xd0..=0xd3 => Ok(Value::Int(i128::from(self.int(1 << (marker - 0xd0))?))),
            0xd4..=0xd8 => self.ext(1 << (marker - 0xd4)),
            0xd9..=0xdb => {
                let len = self.len(1 << (marker - 0xd9))?;
                Ok(Value::Str(self.bytes(len)?))
            }
            0xdc | 0xdd => {
                let len = self.len(2 << (marker - 0xdc))?;
                self.array(len)
            }
            0xde | 0xdf => {
                let len = self.len(2 << (marker - 0xde))?;
                self.map(len)
            }
            0xe0..=0xff => Ok(Value::Int(i128::from(marker as i8))),
        }
    }
}

pub fn sockets(proc: &Path, roots: &[u32]) -> Vec<PathBuf> {
    let listening = listening(proc);
    let mut sockets = Vec::new();
    for pid in descendants(proc, roots) {
        let comm = fs::read(proc.join(pid.to_string()).join("comm")).unwrap_or_default();
        if comm.trim_ascii_end() != b"nvim" {
            continue;
        }
        let inodes = socket_inodes(proc, pid);
        for (inode, path) in &listening {
            if inodes.contains(inode) && !sockets.contains(path) {
                sockets.push(path.clone());
            }
        }
    }
    sockets
}

fn descendants(proc: &Path, roots: &[u32]) -> Vec<u32> {
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for entry in fs::read_dir(proc).into_iter().flatten().flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse().ok())
        else {
            continue;
        };
        if let Some(parent) = fs::read(entry.path().join("stat"))
            .ok()
            .and_then(|stat| parent_pid(&stat))
        {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut found = Vec::new();
    let mut pending: Vec<u32> = roots.iter().rev().copied().collect();
    while let Some(pid) = pending.pop() {
        if found.contains(&pid) {
            continue;
        }
        found.push(pid);
        if let Some(kids) = children.get_mut(&pid) {
            kids.sort_unstable();
            pending.extend(kids.iter().rev());
        }
    }
    found
}

fn parent_pid(stat: &[u8]) -> Option<u32> {
    let close = stat.iter().rposition(|byte| *byte == b')')?;
    let rest = std::str::from_utf8(&stat[close + 1..]).ok()?;
    rest.split_ascii_whitespace().nth(1)?.parse().ok()
}

fn socket_inodes(proc: &Path, pid: u32) -> Vec<u64> {
    fs::read_dir(proc.join(pid.to_string()).join("fd"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .filter_map(|link| {
            link.to_str()?
                .strip_prefix("socket:[")?
                .strip_suffix(']')?
                .parse()
                .ok()
        })
        .collect()
}

fn listening(proc: &Path) -> Vec<(u64, PathBuf)> {
    fs::read(proc.join("net/unix"))
        .unwrap_or_default()
        .split(|byte| *byte == b'\n')
        .skip(1)
        .filter_map(unix_line)
        .collect()
}

fn unix_line(line: &[u8]) -> Option<(u64, PathBuf)> {
    let mut fields = Vec::new();
    let mut rest = line;
    for _ in 0..7 {
        rest = rest.trim_ascii_start();
        let end = rest
            .iter()
            .position(u8::is_ascii_whitespace)
            .unwrap_or(rest.len());
        fields.push(std::str::from_utf8(&rest[..end]).ok()?);
        rest = &rest[end..];
    }
    let path = rest.trim_ascii_start();
    let flags = u32::from_str_radix(fields[3], 16).ok()?;
    if flags & ACCEPTING == 0 || path.is_empty() || path[0] == b'@' {
        return None;
    }
    Some((
        fields[6].parse().ok()?,
        PathBuf::from(OsStr::from_bytes(path)),
    ))
}

#[cfg(test)]
#[path = "nvim_tests.rs"]
mod tests;
