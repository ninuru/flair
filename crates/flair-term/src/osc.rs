use crate::terminal::{MarkKind, TermEvent};
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct OscSniffer {
    state: u8,
    bytes: Vec<u8>,
    events: Vec<TermEvent>,
}
impl OscSniffer {
    pub fn advance(&mut self, bytes: &[u8]) {
        for &b in bytes {
            match self.state {
                0 => {
                    if b == 27 {
                        self.state = 1;
                    }
                }
                1 => {
                    self.state = if b == b']' { 2 } else { 0 };
                    self.bytes.clear();
                }
                2 => match b {
                    7 => {
                        self.finish();
                        self.state = 0;
                    }
                    27 => self.state = 3,
                    b if self.bytes.len() < 8192 => self.bytes.push(b),
                    _ => {
                        self.bytes.clear();
                        self.state = 0;
                    }
                },
                3 => {
                    if b == b'\\' {
                        self.finish();
                    }
                    self.state = 0;
                }
                _ => unreachable!(),
            }
        }
    }
    pub fn take(&mut self) -> Vec<TermEvent> {
        std::mem::take(&mut self.events)
    }
    fn finish(&mut self) {
        let Ok(text) = std::str::from_utf8(&self.bytes) else {
            return;
        };
        if let Some(uri) = text.strip_prefix("7;file://") {
            let Some((host, path)) = uri.split_once('/') else {
                return;
            };
            let local = std::env::var("HOSTNAME")
                .or_else(|_| std::fs::read_to_string("/etc/hostname"))
                .unwrap_or_default();
            if !host.is_empty() && host != "localhost" && host != local.trim() {
                return;
            }
            let bytes = format!("/{path}").into_bytes();
            let mut decoded = Vec::new();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'%' {
                    if i + 2 >= bytes.len() {
                        return;
                    }
                    let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) else {
                        return;
                    };
                    let Ok(b) = u8::from_str_radix(hex, 16) else {
                        return;
                    };
                    if b == 0 {
                        return;
                    }
                    decoded.push(b);
                    i += 3;
                } else {
                    decoded.push(bytes[i]);
                    i += 1;
                }
            }
            if let Ok(path) = String::from_utf8(decoded) {
                self.events.push(TermEvent::Cwd(PathBuf::from(path)));
            }
        } else if let Some(mark) = text.strip_prefix("133;") {
            let mut parts = mark.split(';');
            let kind = match parts.next() {
                Some("A") => MarkKind::PromptStart,
                Some("B") => MarkKind::PromptEnd,
                Some("C") => MarkKind::CommandStart,
                Some("D") => MarkKind::CommandEnd,
                _ => return,
            };
            let code = if kind == MarkKind::CommandEnd {
                parts.next().and_then(|v| v.parse().ok())
            } else {
                None
            };
            self.events.push(TermEvent::Mark(kind, code));
        }
    }
}

#[cfg(test)]
#[path = "osc_tests.rs"]
mod tests;
