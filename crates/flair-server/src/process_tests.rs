use super::*;

fn name(command: &str) -> (String, String) {
    let argv: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
    process_name(&argv)
}

#[test]
fn argv_names_the_program_and_its_label() {
    let cases = [
        ("", "", ""),
        ("zsh", "zsh", "zsh"),
        ("-zsh", "zsh", "zsh"),
        ("/usr/bin/bash --login", "bash", "bash"),
        ("fish -l", "fish", "fish"),
        ("sh -c true", "sh", "sh"),
        ("dash", "dash", "dash"),
        ("nu", "nu", "nu"),
        ("bash ./deploy.sh", "bash", "bash"),
        ("htop", "htop", "htop"),
        ("/usr/bin/sleep 60", "sleep", "sleep"),
        ("sudo htop", "htop", "htop"),
        ("sudo -u root -E apt upgrade", "apt", "apt"),
        ("sudo -iu root", "sudo", "sudo"),
        ("sudo", "sudo", "sudo"),
        ("env FOO=1 BAR=2 cargo test", "cargo", "cargo test"),
        ("env -i -u HOME PATH=/bin make -j4 all", "make", "make all"),
        ("env", "env", "env"),
        ("env A=1", "env", "env"),
        ("sudo env A=1 docker compose up", "docker", "docker compose"),
        ("ssh build-host", "ssh", "build-host"),
        ("ssh -p 22 deploy@build-host", "ssh", "build-host"),
        ("ssh -p22 deploy@build-host uptime", "ssh", "build-host"),
        (
            "ssh -i ~/.ssh/key -o StrictHostKeyChecking=no -J jump me@db",
            "ssh",
            "db",
        ),
        ("ssh -tt -l admin web-1", "ssh", "web-1"),
        ("ssh -- host", "ssh", "host"),
        (
            "ssh ssh://me@git.example.com:2222",
            "ssh",
            "git.example.com",
        ),
        ("ssh -v", "ssh", "ssh"),
        ("mosh deploy@build-host", "mosh", "build-host"),
        ("mosh -p 60001 --ssh=ssh -p 22 box", "mosh", "box"),
        ("perl /usr/bin/mosh me@box", "mosh", "box"),
        ("docker compose up", "docker", "docker compose"),
        ("docker -H tcp://x ps", "docker", "docker ps"),
        ("docker", "docker", "docker"),
        ("podman run --rm alpine", "podman", "podman run"),
        ("kubectl -n prod get pods", "kubectl", "kubectl get"),
        ("git log --oneline", "git", "git log"),
        ("git -C repo status", "git", "git status"),
        ("cargo +nightly test -p flair", "cargo", "cargo test"),
        ("npm run dev", "npm", "npm run"),
        ("pnpm install", "pnpm", "pnpm install"),
        ("yarn", "yarn", "yarn"),
        ("systemctl status nginx", "systemctl", "systemctl status"),
        ("journalctl -u nginx -f", "journalctl", "journalctl"),
        ("make -C build test", "make", "make test"),
        ("just build", "just", "just build"),
        ("nvim src/main.rs", "nvim", "nvim main.rs"),
        ("vim +10 -c :set notes.md", "vim", "vim notes.md"),
        ("vi", "vi", "vi"),
        ("nano /etc/hosts", "nano", "nano hosts"),
        ("hx Cargo.toml", "hx", "hx Cargo.toml"),
        ("less /etc/os-release", "less", "less os-release"),
        ("less +F /var/log/syslog", "less", "less syslog"),
        ("bat -l rust x.rs", "bat", "bat x.rs"),
        ("tail -n 20 -f /var/log/app.log", "tail", "tail app.log"),
        ("tail -f", "tail", "tail"),
        ("python3 manage.py runserver", "python3", "manage.py"),
        (
            "/usr/bin/python3.12 -u tools/run.py",
            "python3.12",
            "run.py",
        ),
        ("python3 -c pass", "python3", "python3"),
        ("python3 -m http.server", "python3", "python3"),
        ("python", "python", "python"),
        ("node server.js", "node", "server.js"),
        ("node /usr/bin/claude --resume", "claude", "claude"),
        ("node -e 1", "node", "node"),
        ("bun run dev.ts", "bun", "dev.ts"),
        ("deno run -A main.ts", "deno", "main.ts"),
        ("perl script.pl", "perl", "script.pl"),
        ("ruby -e 1", "ruby", "ruby"),
        ("ruby app.rb", "ruby", "app.rb"),
        ("sudo python3 /opt/tool.py", "python3", "tool.py"),
    ];
    for (command, program, label) in cases {
        assert_eq!(
            name(command),
            (program.to_owned(), label.to_owned()),
            "{command}"
        );
    }
}

#[test]
fn empty_argv_has_no_name() {
    assert_eq!(process_name(&[]), (String::new(), String::new()));
}

#[test]
fn control_characters_are_stripped() {
    let argv = ["less".to_owned(), "/tmp/a\u{1b}[31mb\u{7}".to_owned()];
    assert_eq!(
        process_name(&argv),
        ("less".to_owned(), "less a[31mb".to_owned())
    );
    let argv = ["ss\u{8}h".to_owned(), "host".to_owned()];
    assert_eq!(process_name(&argv), ("ssh".to_owned(), "host".to_owned()));
}

#[test]
fn long_labels_are_capped() {
    let host = "a".repeat(80);
    let (_, label) = process_name(&["ssh".to_owned(), host]);
    assert_eq!(label.chars().count(), LABEL_MAX);
    assert!(label.ends_with(ELLIPSIS));
    let (_, label) = process_name(&["ssh".to_owned(), "b".repeat(LABEL_MAX)]);
    assert_eq!(label, "b".repeat(LABEL_MAX));
}
