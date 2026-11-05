use std::{collections::BTreeMap, path::PathBuf};

use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};

use crate::proto::{SessionInfo, Target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    pub root: PathBuf,
    pub open: bool,
}

impl Candidate {
    pub fn target(&self) -> Target {
        if self.open {
            Target::Name(self.name.clone())
        } else {
            Target::Path(self.root.clone())
        }
    }
}

pub fn candidates(
    sessions: &[SessionInfo],
    current: &str,
    directories: Vec<PathBuf>,
) -> Vec<Candidate> {
    let current_root = sessions
        .iter()
        .find(|session| session.name == current)
        .map(|session| &session.root);
    let mut entries = BTreeMap::<String, Candidate>::new();
    for root in directories {
        let Some(name) = root.file_name() else {
            continue;
        };
        let name = name.to_string_lossy().replace('.', "_");
        if name == current || current_root == Some(&root) {
            continue;
        }
        let candidate = Candidate {
            name: name.clone(),
            root,
            open: false,
        };
        if entries.get(&name).is_none_or(|existing| {
            candidate.root.as_os_str().len() < existing.root.as_os_str().len()
        }) {
            entries.insert(name, candidate);
        }
    }
    for session in sessions {
        if session.name != current {
            entries.insert(
                session.name.clone(),
                Candidate {
                    name: session.name.clone(),
                    root: session.root.clone(),
                    open: true,
                },
            );
        }
    }
    entries.into_values().collect()
}

pub fn rank<'a>(entries: &'a [Candidate], query: &str) -> Vec<&'a Candidate> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buffer = Vec::new();
    let mut scored: Vec<_> = entries
        .iter()
        .filter_map(|entry| {
            let label = format!("{} {}", entry.name, entry.root.display());
            pattern
                .score(Utf32Str::new(&label, &mut buffer), &mut matcher)
                .map(|score| (entry, score))
        })
        .collect();
    scored.sort_by(|(a, a_score), (b, b_score)| {
        b.open
            .cmp(&a.open)
            .then(b_score.cmp(a_score))
            .then(a.root.as_os_str().len().cmp(&b.root.as_os_str().len()))
            .then(a.name.cmp(&b.name))
    });
    scored.into_iter().map(|(entry, _)| entry).collect()
}

#[cfg(test)]
#[path = "sessionizer_tests.rs"]
mod tests;
