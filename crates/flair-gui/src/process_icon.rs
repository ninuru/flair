use std::path::Path;

const FOLDER: char = '\u{ea83}';
const FOLDER_OPEN: char = '\u{eaf7}';
const FILE: char = '\u{ea7b}';

pub(crate) fn process_icon(program: &str) -> Option<char> {
    let family = program.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    match family {
        "docker" | "podman" | "docker-compose" => Some('\u{f308}'),
        "kubectl" | "k9s" | "helm" => Some('\u{f10fe}'),
        "ssh" | "mosh" | "mosh-client" | "scp" | "sftp" => Some('\u{ea7a}'),
        "git" | "lazygit" | "tig" => Some('\u{f418}'),
        "nvim" | "vim" | "vi" => Some('\u{e62b}'),
        "python" | "ipython" => Some('\u{e73c}'),
        "node" | "npm" | "npx" | "pnpm" | "yarn" | "bun" | "deno" => Some('\u{e718}'),
        "cargo" | "rustc" => Some('\u{e7a8}'),
        "go" => Some('\u{e627}'),
        "java" | "gradle" | "mvn" => Some('\u{e738}'),
        "perl" => Some('\u{e769}'),
        "ruby" => Some('\u{e739}'),
        "psql" | "mysql" | "sqlite" | "redis-cli" => Some('\u{f1c0}'),
        "htop" | "btop" | "top" => Some('\u{f0e4}'),
        "claude" => Some('\u{f06a9}'),
        "zsh" | "bash" | "fish" | "sh" | "dash" | "nu" => Some('\u{f120}'),
        _ => None,
    }
}

pub(crate) fn file_icon(name: &str, dir: bool, open: bool) -> char {
    match (dir, open) {
        (true, false) => return FOLDER,
        (true, true) => return FOLDER_OPEN,
        (false, _) => {}
    }
    let extension = Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    let family = match extension.as_deref() {
        Some("rs") => "cargo",
        Some("py") => "python",
        Some("js" | "ts" | "json") => "node",
        Some("go") => "go",
        Some("java") => "java",
        Some("rb") => "ruby",
        Some("pl") => "perl",
        Some("sh" | "zsh") => "zsh",
        Some("sql") => "psql",
        Some(_) | None => "",
    };
    process_icon(family).unwrap_or(FILE)
}

#[cfg(test)]
#[path = "process_icon_tests.rs"]
mod tests;
