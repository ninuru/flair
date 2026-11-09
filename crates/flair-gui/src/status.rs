use flair_core::{ids::WindowId, status::StatusLine};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusParts {
    pub chip: ChipParts,
    pub tabs: Vec<TabParts>,
    pub badges: Vec<BadgeParts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipParts {
    pub session: String,
    pub hidden: u16,
    pub running: bool,
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabParts {
    pub window: WindowId,
    pub index: u16,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    Prefix,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadgeParts {
    pub kind: BadgeKind,
    pub text: String,
}

pub fn status_parts(status: &StatusLine) -> StatusParts {
    StatusParts {
        chip: ChipParts {
            session: status.session.clone(),
            hidden: status.hidden,
            running: status.running,
            finished: status.finished,
        },
        tabs: status
            .windows
            .iter()
            .map(|window| TabParts {
                window: window.id,
                index: window.index,
                name: window.name.clone(),
                active: window.active,
            })
            .collect(),
        badges: [
            status.copy.then(|| BadgeParts {
                kind: BadgeKind::Copy,
                text: "COPY".into(),
            }),
            status.pending.clone().map(|text| BadgeParts {
                kind: BadgeKind::Prefix,
                text,
            }),
        ]
        .into_iter()
        .flatten()
        .collect(),
    }
}

pub fn status_text(parts: &StatusParts, cols: u16) -> String {
    let width = usize::from(cols);
    let badges = parts
        .badges
        .iter()
        .map(|badge| badge.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let (pending, pending_width) = fit(&badges, width);
    let pending_space = usize::from(pending_width > 0 && pending_width < width);
    let remaining = width - pending_width - pending_space;
    let suffix_width = if parts.tabs.is_empty() {
        remaining
    } else {
        remaining / 2
    };
    let (suffix, suffix_used) = session_text(&chip_label(&parts.chip), suffix_width);
    let left_width = remaining - suffix_used;
    let (windows, windows_used) = windows_text(&parts.tabs, left_width);
    let pad = " ".repeat(left_width - windows_used);
    let space = " ".repeat(pending_space);
    format!("{windows}{pad}{suffix}{space}{pending}")
}

fn chip_label(chip: &ChipParts) -> String {
    let session = &chip.session;
    let hidden = if chip.hidden > 0 {
        format!(" +{}", chip.hidden)
    } else {
        String::new()
    };
    let running = if chip.running { " *" } else { "" };
    let finished = if chip.finished { " !" } else { "" };
    format!("{session}{hidden}{running}{finished}")
}

fn windows_text(tabs: &[TabParts], width: usize) -> (String, usize) {
    let entries: Vec<String> = tabs
        .iter()
        .map(|tab| format!(" {}:{}", tab.index, tab.name))
        .collect();
    let widths: Vec<usize> = entries.iter().map(|entry| fit(entry, width).1).collect();
    let active = tabs.iter().position(|tab| tab.active).unwrap_or(0);
    let mut first = 0;
    let mut used: usize = widths.iter().take(active + 1).sum();
    while used > width && first < active {
        used -= widths[first];
        first += 1;
    }
    let mut text = String::new();
    let mut col = 0;
    for (tab, entry) in tabs.iter().zip(entries).skip(first) {
        let (part, count) = fit(&entry, width - col);
        if count < tab.index.to_string().len() + 2 {
            break;
        }
        text.push_str(&part);
        col += count;
        if col == width {
            break;
        }
    }
    (text, col)
}

fn session_text(session: &str, width: usize) -> (String, usize) {
    let (bar, bar_width) = fit("| ", width);
    if width < 3 {
        return (bar, bar_width);
    }
    let (name, name_width) = fit(session, width - 3);
    (format!("{bar}{name} "), bar_width + name_width + 1)
}

fn fit(text: &str, limit: usize) -> (String, usize) {
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let Some(width) = c.width() else {
            continue;
        };
        if width == 0 {
            if !out.is_empty() {
                out.push(c);
            }
            continue;
        }
        if used + width > limit {
            break;
        }
        out.push(c);
        used += width;
    }
    (out, used)
}

#[cfg(test)]
#[path = "status_tests.rs"]
mod tests;
