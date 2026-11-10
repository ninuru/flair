use std::{error::Error, fmt};

use toml_edit::{DocumentMut, Item, TableLike, TomlError, table};

use crate::{
    config::{Config, ConfigError},
    settings::Value,
};

const BOM: &str = "\u{feff}";

#[derive(Debug)]
pub enum EditError {
    Parse(TomlError),
    NotATable(String),
    Invalid(ConfigError),
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::Parse(error) => {
                let line = first_line(error.message());
                write!(f, "flair.toml does not parse: {line}")
            }
            EditError::NotATable(name) => write!(f, "[{name}] in flair.toml is not a table"),
            EditError::Invalid(ConfigError::Parse(error)) => {
                let line = first_line(error.message());
                write!(f, "invalid config: {line}")
            }
            EditError::Invalid(error) => {
                let message = error.to_string();
                let line = first_line(&message);
                write!(f, "{line}")
            }
        }
    }
}

impl Error for EditError {}

pub fn apply(text: &str, key: &str, value: Option<&Value>) -> Result<(String, Config), EditError> {
    let (bom, body) = match text.strip_prefix(BOM) {
        Some(rest) => (BOM, rest),
        None => ("", text),
    };
    let mut doc = document(body)?;
    match value {
        Some(value) => set(&mut doc, key, value.to_toml())?,
        None => remove(&mut doc, key),
    }
    let rendered = doc.to_string();
    let lines = if body.contains("\r\n") {
        rendered.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        rendered
    };
    let ending = if body.is_empty() || body.ends_with('\n') {
        lines.as_str()
    } else {
        lines
            .strip_suffix("\r\n")
            .or_else(|| lines.strip_suffix('\n'))
            .unwrap_or(&lines)
    };
    let out = format!("{bom}{ending}");
    let config = Config::from_toml(&out).map_err(EditError::Invalid)?;
    Ok((out, config))
}

pub(crate) fn document(text: &str) -> Result<DocumentMut, EditError> {
    text.strip_prefix(BOM)
        .unwrap_or(text)
        .parse()
        .map_err(EditError::Parse)
}

pub(crate) fn contains(doc: &DocumentMut, key: &str) -> bool {
    match key.split_once('.') {
        Some((name, leaf)) => doc
            .get(name)
            .and_then(Item::as_table_like)
            .is_some_and(|table| table.contains_key(leaf)),
        None => doc.contains_key(key),
    }
}

fn set(doc: &mut DocumentMut, key: &str, value: toml_edit::Value) -> Result<(), EditError> {
    let (table, leaf): (&mut dyn TableLike, &str) = match key.split_once('.') {
        Some((name, leaf)) => (
            doc.entry(name)
                .or_insert(table())
                .as_table_like_mut()
                .ok_or_else(|| EditError::NotATable(name.to_owned()))?,
            leaf,
        ),
        None => (doc.as_table_mut(), key),
    };
    match table.get_mut(leaf) {
        Some(Item::Value(old)) => {
            let decor = old.decor().clone();
            *old = value;
            *old.decor_mut() = decor;
        }
        Some(Item::None | Item::Table(_) | Item::ArrayOfTables(_)) | None => {
            table.insert(leaf, Item::Value(value));
        }
    }
    Ok(())
}

fn remove(doc: &mut DocumentMut, key: &str) {
    match key.split_once('.') {
        Some((name, leaf)) => {
            if let Some(table) = doc.get_mut(name).and_then(Item::as_table_like_mut) {
                table.remove(leaf);
            }
        }
        None => {
            doc.remove(key);
        }
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

#[cfg(test)]
#[path = "config_edit_tests.rs"]
mod tests;
