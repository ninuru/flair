use super::*;
#[test]
fn help_and_headless_options_parse() {
    assert!(matches!(Args::parse_from(["--help"]).unwrap(), Args::Help));
    assert!(matches!(
        Args::parse_from([
            "test-headless",
            "--scenario",
            "a.toml",
            "--output",
            "/tmp/proof"
        ])
        .unwrap(),
        Args::Headless { .. }
    ));
    assert!(
        Args::parse_from([
            "test-headless",
            "--scenario",
            "a.toml",
            "--output",
            "/tmp/proof",
            "--timeout-ms",
            "0"
        ])
        .is_err()
    );
    assert!(Args::parse_from(["--typo"]).is_err());
}

#[test]
fn headless_pixel_options_parse() {
    assert_eq!(
        Args::parse_from([
            "test-headless",
            "--scenario",
            "a.toml",
            "--output",
            "/tmp/proof",
            "--pixels",
            "640x400",
            "--scale",
            "2",
            "--config",
            "c.toml"
        ])
        .unwrap(),
        Args::Headless {
            scenario: "a.toml".into(),
            output: "/tmp/proof".into(),
            timeout_ms: 5000,
            config: Some("c.toml".into()),
            pixels: Some((640, 400)),
            scale: 2.0,
        }
    );
    for bad in [["--pixels", "640"], ["--pixels", "0x400"], ["--scale", "0"]] {
        assert!(
            Args::parse_from([
                "test-headless",
                "--scenario",
                "a",
                "--output",
                "b",
                bad[0],
                bad[1]
            ])
            .is_err()
        );
    }
}

#[test]
fn daemon_commands_parse_and_reject_missing_targets() {
    assert_eq!(Args::parse_from(["ls"]).unwrap(), Args::List);
    assert_eq!(Args::parse_from(["kill-server"]).unwrap(), Args::KillServer);
    assert!(
        matches!(Args::parse_from(["new", "-d", "-s", "H", "-c", "/tmp"]).unwrap(), Args::Create { target: Target::Create { name: Some(name), .. }, .. } if name == "H")
    );
    assert!(
        matches!(Args::parse_from(["attach", "-t", "main"]).unwrap(), Args::Gui { target: Target::Name(name), standalone: false, .. } if name == "main")
    );
    assert!(Args::parse_from(["attach"]).is_err());
    assert!(Args::parse_from(["switch"]).is_err());
    assert!(Args::parse_from(["--standalone", "new"]).is_err());
    assert!(Args::parse_from(["ls", "-d"]).is_err());
}
