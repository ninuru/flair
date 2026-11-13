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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranked<'a> {
    pub entry: &'a Candidate,
    pub name: Vec<usize>,
    pub root: Vec<usize>,
}

pub fn fuzzy<'a>(
    query: &str,
    labels: impl IntoIterator<Item = &'a str>,
) -> Vec<(usize, u32, Vec<usize>)> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buffer = Vec::new();
    labels
        .into_iter()
        .enumerate()
        .filter_map(|(index, label)| {
            let score = pattern.score(Utf32Str::new(label, &mut buffer), &mut matcher)?;
            let chars: Vec<char> = label.chars().collect();
            let mut indices = Vec::new();
            pattern.indices(Utf32Str::Unicode(&chars), &mut matcher, &mut indices);
            indices.sort_unstable();
            indices.dedup();
            Some((
                index,
                score,
                indices.into_iter().map(|index| index as usize).collect(),
            ))
        })
        .collect()
}

pub fn rank<'a>(entries: &'a [Candidate], query: &str) -> Vec<Ranked<'a>> {
    let labels: Vec<String> = entries
        .iter()
        .map(|entry| format!("{} {}", entry.name, entry.root.display()))
        .collect();
    let mut scored: Vec<_> = fuzzy(query, labels.iter().map(String::as_str))
        .into_iter()
        .map(|(index, score, indices)| {
            let entry = &entries[index];
            let split = entry.name.chars().count();
            let (name, root): (Vec<usize>, Vec<usize>) =
                indices.into_iter().partition(|index| *index < split);
            let ranked = Ranked {
                entry,
                name,
                root: root
                    .into_iter()
                    .filter_map(|index| index.checked_sub(split + 1))
                    .collect(),
            };
            (ranked, score)
        })
        .collect();
    scored.sort_by(|(a, a_score), (b, b_score)| {
        b.entry
            .open
            .cmp(&a.entry.open)
            .then(b_score.cmp(a_score))
            .then(
                a.entry
                    .root
                    .as_os_str()
                    .len()
                    .cmp(&b.entry.root.as_os_str().len()),
            )
            .then(a.entry.name.cmp(&b.entry.name))
    });
    scored.into_iter().map(|(ranked, _)| ranked).collect()
}

#[cfg(test)]
#[path = "sessionizer_tests.rs"]
mod tests;
