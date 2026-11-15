use crate::terminal::{MarkKind, TermEvent};
use base64::{
    Engine,
    alphabet::STANDARD,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use flair_core::{
    agent::{AgentKind, AgentRecord, AgentReport, AgentState},
    isles::{ShellFacts, label, plain},
};
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct OscSniffer {
    state: u8,
    bytes: Vec<u8>,
    events: Vec<TermEvent>,
    hostname: Option<String>,
    prompt: bool,
    gap: bool,
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
                    if b == b'c' {
                        self.events.push(TermEvent::Agent(AgentReport::Clear(None)));
                    }
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
    pub fn take_gap(&mut self) -> bool {
        std::mem::take(&mut self.gap)
    }
    fn finish(&mut self) {
        let Ok(text) = std::str::from_utf8(&self.bytes) else {
            return;
        };
        if text == "6973;prompt" {
            self.prompt = true;
        } else if text == "6973;gap" {
            self.gap = true;
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
        } else if let Some(payload) = text.strip_prefix("7501;") {
            if payload == "?" {
                self.events.push(TermEvent::PtyWrite(AGENT_REPLY.to_vec()));
            } else if let Some(report) = agent_report(payload) {
                self.events.push(TermEvent::Agent(report));
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

const AGENT_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,blocked,done,error:kinds=permission,question,auth\x1b\\";

const BASE64: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

fn agent_report(text: &str) -> Option<AgentReport> {
    if text.len() > 4096 {
        return None;
    }
    let (mut state, mut id, mut kind, mut app, mut title, mut msg) =
        (None, None, None, None, None, None);
    for pair in text.split(':') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        let value = Some(value.trim());
        match key.trim() {
            "state" => state = value,
            "id" => id = value,
            "kind" => kind = value,
            "app" => app = value,
            "title" => title = value,
            "msg" => msg = value,
            _ => (),
        }
    }
    let id = match id.filter(|id| !id.is_empty()) {
        Some(id) if valid_id(id) => Some(id.to_owned()),
        Some(_) => return None,
        None => None,
    };
    let state = match state? {
        "idle" => AgentState::Idle,
        "working" => AgentState::Working,
        "blocked" => AgentState::Blocked,
        "done" => AgentState::Done,
        "error" => AgentState::Error,
        "clear" => return Some(AgentReport::Clear(id)),
        _ => return None,
    };
    let kind = match kind.filter(|_| state == AgentState::Blocked) {
        Some("permission") => Some(AgentKind::Permission),
        Some("question") => Some(AgentKind::Question),
        Some("auth") => Some(AgentKind::Auth),
        _ => None,
    };
    let app = app
        .filter(|app| !app.is_empty() && app.chars().count() <= 32)
        .filter(|app| plain(app))
        .map(str::to_owned);
    Some(AgentReport::Set {
        id: id.unwrap_or_default(),
        record: AgentRecord {
            state,
            kind,
            app,
            title: decode(title, 192)?,
            msg: decode(msg, 2048)?,
        },
    })
}

fn valid_id(id: &str) -> bool {
    id.len() <= 128
        && id.split('/').count() <= 8
        && id
            .split('/')
            .all(|segment| !segment.is_empty() && segment.len() <= 32)
        && id
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b':' && b != b'=')
}

fn decode(value: Option<&str>, limit: usize) -> Option<Option<String>> {
    let Some(value) = value else {
        return Some(None);
    };
    let text = String::from_utf8(BASE64.decode(value).ok()?).ok()?;
    if !plain(&text) {
        return None;
    }
    Some(Some(text).filter(|text| !text.is_empty() && text.len() <= limit))
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
