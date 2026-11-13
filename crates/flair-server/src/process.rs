const LABEL_MAX: usize = 32;
const ELLIPSIS: char = '\u{2026}';

const SHELLS: &[&str] = &["zsh", "bash", "fish", "sh", "dash", "nu"];
const INTERPRETERS: &[&str] = &["python", "node", "bun", "deno", "perl", "ruby"];
const SUBCOMMANDS: &[&str] = &[
    "docker",
    "podman",
    "kubectl",
    "git",
    "cargo",
    "npm",
    "pnpm",
    "yarn",
    "systemctl",
    "journalctl",
    "make",
    "just",
];
const FILES: &[&str] = &["nvim", "vim", "vi", "nano", "hx", "less", "bat", "tail"];

const SUDO_VALUES: &[&str] = &[
    "-u", "-g", "-C", "-D", "-R", "-T", "-p", "-U", "-h", "--user", "--group", "--chdir",
    "--prompt",
];
const ENV_VALUES: &[&str] = &["-u", "-C", "-S", "--unset", "--chdir", "--split-string"];
const SSH_VALUES: &[&str] = &[
    "-B", "-b", "-c", "-D", "-E", "-e", "-F", "-I", "-i", "-J", "-L", "-l", "-m", "-O", "-o", "-p",
    "-Q", "-R", "-S", "-W", "-w",
];
const MOSH_VALUES: &[&str] = &["-p", "--port", "--ssh", "--predict", "--family", "--server"];

pub(crate) fn process_name(argv: &[String]) -> (String, String) {
    let argv = command(argv);
    let Some(first) = argv.first() else {
        return (String::new(), String::new());
    };
    let program = clean(base(first).strip_prefix('-').unwrap_or(base(first)));
    let label = label(&program, &argv[1..]).unwrap_or_else(|| program.clone());
    (program, clean(&label))
}

fn command(argv: &[String]) -> &[String] {
    let Some(first) = argv.first() else {
        return argv;
    };
    let name = base(first);
    let rest = &argv[1..];
    let inner = match name {
        "sudo" => operand(rest, SUDO_VALUES).map(|index| &rest[index..]),
        "env" => env_command(rest),
        _ if INTERPRETERS.contains(&family(name)) => script(name, rest)
            .filter(|index| !base(&rest[*index]).contains('.'))
            .map(|index| &rest[index..]),
        _ => None,
    };
    inner.map_or(argv, command)
}

fn env_command(args: &[String]) -> Option<&[String]> {
    let mut rest = args;
    loop {
        let index = operand(rest, ENV_VALUES)?;
        rest = &rest[index..];
        match rest.first() {
            Some(arg) if arg.contains('=') => rest = &rest[1..],
            Some(_) => return Some(rest),
            None => return None,
        }
    }
}

fn label(program: &str, args: &[String]) -> Option<String> {
    let family = family(program);
    if SHELLS.contains(&program) {
        None
    } else if program == "ssh" {
        operand(args, SSH_VALUES).and_then(|index| host(&args[index]))
    } else if program == "mosh" {
        operand(args, MOSH_VALUES).and_then(|index| host(&args[index]))
    } else if SUBCOMMANDS.contains(&program) {
        operand(args, subcommand_values(program)).map(|index| format!("{program} {}", args[index]))
    } else if FILES.contains(&program) {
        operand(args, file_values(program)).map(|index| format!("{program} {}", base(&args[index])))
    } else if INTERPRETERS.contains(&family) {
        script(program, args).map(|index| base(&args[index]).to_owned())
    } else {
        None
    }
}

fn script(program: &str, args: &[String]) -> Option<usize> {
    let values: &[&str] = match family(program) {
        "python" => &["-c", "-m", "-W", "-X"],
        "node" => &[
            "-e",
            "-p",
            "-r",
            "--eval",
            "--print",
            "--require",
            "--import",
        ],
        "perl" => &["-e", "-E"],
        "ruby" => &["-e", "-r", "-I", "-C"],
        _ => &[],
    };
    let index = operand(args, values)?;
    match (family(program), args[index].as_str()) {
        ("deno" | "bun", "run") => operand(&args[index + 1..], values).map(|next| index + 1 + next),
        _ => Some(index),
    }
}

fn subcommand_values(program: &str) -> &'static [&'static str] {
    match program {
        "git" => &["-C", "-c", "--git-dir", "--work-tree", "--namespace"],
        "docker" | "podman" => &[
            "-H",
            "-c",
            "-l",
            "--host",
            "--context",
            "--config",
            "--log-level",
        ],
        "kubectl" => &[
            "-n",
            "-s",
            "--namespace",
            "--context",
            "--cluster",
            "--server",
            "--kubeconfig",
            "--user",
        ],
        "make" => &["-C", "-f", "-I", "-o", "-W", "--directory", "--file"],
        "just" => &["-f", "-d", "--justfile", "--working-directory"],
        "systemctl" => &[
            "-t",
            "-p",
            "-H",
            "-M",
            "-n",
            "--type",
            "--property",
            "--host",
        ],
        "journalctl" => &[
            "-u", "-n", "-p", "-t", "-S", "-U", "-o", "-M", "-D", "-g", "--unit", "--lines",
            "--since", "--until", "--output",
        ],
        "npm" | "pnpm" | "yarn" => &["-C", "--prefix", "--dir", "--cwd", "--filter"],
        _ => &[],
    }
}

fn file_values(program: &str) -> &'static [&'static str] {
    match program {
        "nvim" | "vim" | "vi" => &[
            "-c", "-S", "-u", "-i", "-t", "-q", "-T", "-w", "-W", "--cmd",
        ],
        "tail" => &["-n", "-c", "-s", "--pid", "--lines", "--bytes"],
        "less" => &[
            "-b", "-h", "-j", "-p", "-P", "-o", "-O", "-t", "-T", "-x", "-y", "-z",
        ],
        "bat" => &[
            "-l",
            "-r",
            "-H",
            "--language",
            "--line-range",
            "--highlight-line",
        ],
        _ => &[],
    }
}

fn operand(args: &[String], values: &[&str]) -> Option<usize> {
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        if arg == "--" {
            return (index + 1 < args.len()).then_some(index + 1);
        }
        let option = arg.len() > 1 && arg.starts_with(['-', '+']);
        if !option && arg != "-" {
            return Some(index);
        }
        index += 1 + usize::from(arg.starts_with('-') && takes_value(arg, values));
    }
    None
}

fn takes_value(arg: &str, values: &[&str]) -> bool {
    if arg.starts_with("--") {
        return !arg.contains('=') && values.contains(&arg);
    }
    let cluster = &arg[1..];
    cluster
        .char_indices()
        .find(|(_, c)| {
            values
                .iter()
                .any(|value| value.len() == 2 && value.ends_with(*c))
        })
        .is_some_and(|(at, c)| at + c.len_utf8() == cluster.len())
}

fn host(target: &str) -> Option<String> {
    let (target, uri) = match target.strip_prefix("ssh://") {
        Some(rest) => (rest, true),
        None => (target, false),
    };
    let host = target.rsplit('@').next().unwrap_or(target);
    let host = match uri {
        true => host.split(':').next().unwrap_or(host),
        false => host,
    };
    (!host.is_empty()).then(|| host.to_owned())
}

fn base(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

fn family(name: &str) -> &str {
    name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.')
}

fn clean(text: &str) -> String {
    let text: String = text.chars().filter(|c| !c.is_control()).collect();
    match text.chars().nth(LABEL_MAX) {
        Some(_) => text.chars().take(LABEL_MAX - 1).chain([ELLIPSIS]).collect(),
        None => text,
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
