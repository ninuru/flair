const BOM: &str = "\u{feff}";

pub fn set_theme(text: &str, name: &str) -> String {
    match text.strip_prefix(BOM) {
        Some(rest) => format!("{BOM}{}", set_theme_in(rest, name)),
        None => set_theme_in(text, name),
    }
}

fn set_theme_in(text: &str, name: &str) -> String {
    let entry = format!("theme = \"{name}\"");
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if content.trim_start().starts_with('[') {
            break;
        }
        if let Some(value) = theme_value(content) {
            let comment = trailing_comment(value).map_or(String::new(), |c| format!(" {c}"));
            let ending = &line[content.len()..];
            let (before, after) = (&text[..offset], &text[offset + line.len()..]);
            return format!("{before}{entry}{comment}{ending}{after}");
        }
        offset += line.len();
    }
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    format!("{entry}{ending}{text}")
}

fn theme_value(line: &str) -> Option<&str> {
    let line = line.trim_start();
    ["theme", "\"theme\"", "'theme'"]
        .into_iter()
        .find_map(|key| line.strip_prefix(key))
        .and_then(|rest| rest.trim_start().strip_prefix('='))
}

fn trailing_comment(value: &str) -> Option<&str> {
    let mut quote = None;
    let mut escaped = false;
    for (index, c) in value.char_indices() {
        match (quote, c) {
            (Some('"'), _) if escaped => escaped = false,
            (Some('"'), '\\') => escaped = true,
            (Some(open), _) if c == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '#') => return Some(value[index..].trim_end()),
            (None, _) => {}
        }
    }
    None
}

#[cfg(test)]
#[path = "config_edit_tests.rs"]
mod tests;
