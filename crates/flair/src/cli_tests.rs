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
