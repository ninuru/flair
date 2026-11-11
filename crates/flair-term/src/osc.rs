use crate::terminal::{MarkKind, TermEvent};
use flair_core::isles::{ShellFacts, label, plain};
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct OscSniffer {
    state: u8,
    bytes: Vec<u8>,
    events: Vec<TermEvent>,
    hostname: Option<String>,
    prompt: bool,
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
    pub fn take_prompt(&mut self) -> bool {
        std::mem::take(&mut self.prompt)
    }
    fn finish(&mut self) {
        let Ok(text) = std::str::from_utf8(&self.bytes) else {
            return;
        };
        if text == "6973;prompt" {
            self.prompt = true;
        } else if let Some(uri) = text.strip_prefix("7;file://") {
            let Some((host, path)) = uri.split_once('/') else {
                return;
            };
            let Some(host) = percent_decode(host) else {
                return;
            };
            if !host.is_empty() && host != "localhost" && host != hostname(&mut self.hostname) {
                return;
            }
            if let Some(path) = percent_decode(&format!("/{path}")).filter(|path| plain(path)) {
                self.events.push(TermEvent::Cwd(PathBuf::from(path)));
            }
        } else if let Some(facts) = text.strip_prefix("6973;flair") {
            if let Some(facts) = shell_facts(facts) {
                self.events.push(TermEvent::Facts(facts));
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

fn hostname(cache: &mut Option<String>) -> &str {
    cache.get_or_insert_with(|| {
        std::env::var("HOSTNAME")
            .or_else(|_| std::fs::read_to_string("/etc/hostname"))
            .unwrap_or_default()
            .trim()
            .to_owned()
    })
}

fn shell_facts(text: &str) -> Option<ShellFacts> {
    let mut facts = ShellFacts {
        user: String::new(),
        host: String::new(),
        venv: None,
    };
    if text.is_empty() {
        return Some(facts);
    }
    for part in text.strip_prefix(';')?.split(';') {
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let value = label(&percent_decode(value)?)?;
        match key {
            "u" => facts.user = value,
            "h" => facts.host = value,
            "v" => facts.venv = Some(value).filter(|venv| !venv.is_empty()),
            _ => (),
        }
    }
    Some(facts)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = match bytes[i] {
            b'%' => {
                let hex = bytes.get(i + 1..i + 3)?;
                if !hex.iter().all(u8::is_ascii_hexdigit) {
                    return None;
                }
                i += 3;
                u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?
            }
            b => {
                i += 1;
                b
            }
        };
        if b == 0 {
            return None;
        }
        decoded.push(b);
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
#[path = "osc_tests.rs"]
mod tests;
