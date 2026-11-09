pub fn set_theme(text: &str, name: &str) -> String {
    let entry = format!("theme = \"{name}\"");
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if content.trim_start().starts_with('[') {
            break;
        }
        if is_theme_key(content) {
            let ending = &line[content.len()..];
            let (before, after) = (&text[..offset], &text[offset + line.len()..]);
            return format!("{before}{entry}{ending}{after}");
        }
        offset += line.len();
    }
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    format!("{entry}{ending}{text}")
}

fn is_theme_key(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("theme")
        .is_some_and(|rest| rest.trim_start().starts_with('='))
}

#[cfg(test)]
#[path = "config_edit_tests.rs"]
mod tests;
