use super::*;

#[test]
fn cwd_bel_st_and_byte_by_byte_percent_decoding() {
    for terminator in ["\x07", "\x1b\\"] {
        let mut osc = OscSniffer::default();
        let sequence = format!("\x1b]7;file://localhost/tmp/a%20b/%E7%95%8C{terminator}");
        for byte in sequence.bytes() {
            osc.advance(&[byte]);
        }
        assert_eq!(
            osc.take(),
            vec![TermEvent::Cwd(PathBuf::from("/tmp/a b/界"))]
        );
        assert!(osc.take().is_empty());
    }
}

#[test]
fn foreign_invalid_and_unrelated_osc_sequences_are_ignored() {
    let mut osc = OscSniffer::default();
    for sequence in [
        "\x1b]7;file://foreign.invalid/tmp\x07",
        "\x1b]7;https://localhost/tmp\x07",
        "\x1b]7;file://localhost/tmp/%XX\x07",
        "\x1b]7;file://localhost/tmp/%0\x07",
        "\x1b]7;file://localhost/tmp/%00\x07",
        "\x1b]2;title\x07",
        "\x1b]133;Z\x07",
    ] {
        osc.advance(sequence.as_bytes());
        assert!(osc.take().is_empty(), "{sequence:?}");
    }
    osc.advance(b"\x1b]7;file:///tmp/valid\x07");
    assert_eq!(
        osc.take(),
        vec![TermEvent::Cwd(PathBuf::from("/tmp/valid"))]
    );
}

#[test]
fn all_prompt_marks_are_reported_in_order() {
    let mut osc = OscSniffer::default();
    osc.advance(b"\x1b]133;A\x07\x1b]133;B\x1b\\\x1b]133;C\x07\x1b]133;D;17\x07\x1b]133;D\x07");
    assert_eq!(
        osc.take(),
        vec![
            TermEvent::Mark(MarkKind::PromptStart, None),
            TermEvent::Mark(MarkKind::PromptEnd, None),
            TermEvent::Mark(MarkKind::CommandStart, None),
            TermEvent::Mark(MarkKind::CommandEnd, Some(17)),
            TermEvent::Mark(MarkKind::CommandEnd, None),
        ]
    );
}

fn facts(user: &str, host: &str, venv: Option<&str>) -> TermEvent {
    TermEvent::Facts(ShellFacts {
        user: user.to_string(),
        host: host.to_string(),
        venv: venv.map(str::to_string),
    })
}

#[test]
fn shell_facts_with_bel_and_st() {
    for terminator in ["\x07", "\x1b\\"] {
        let mut osc = OscSniffer::default();
        osc.advance(format!("\x1b]6973;flair;u=ada;h=box;v=env{terminator}").as_bytes());
        assert_eq!(osc.take(), vec![facts("ada", "box", Some("env"))]);
    }
}

#[test]
fn shell_facts_decode_percent_escapes_and_utf8_byte_by_byte() {
    let mut osc = OscSniffer::default();
    for byte in b"\x1b]6973;flair;u=a%3Bb%3Dc;h=h%C3%B6st;v=my%20env\x07" {
        osc.advance(&[*byte]);
    }
    assert_eq!(osc.take(), vec![facts("a;b=c", "höst", Some("my env"))]);
}

#[test]
fn shell_facts_missing_empty_and_unknown_keys() {
    let mut osc = OscSniffer::default();
    osc.advance(b"\x1b]6973;flair;u=ada;h=box\x07");
    osc.advance(b"\x1b]6973;flair;u=ada;h=box;v=\x07");
    osc.advance(b"\x1b]6973;flair;x=1;h=box;future;v=env\x07");
    osc.advance(b"\x1b]6973;flair\x07");
    assert_eq!(
        osc.take(),
        vec![
            facts("ada", "box", None),
            facts("ada", "box", None),
            facts("", "box", Some("env")),
            facts("", "", None),
        ]
    );
}

#[test]
fn malformed_shell_facts_and_foreign_tags_are_ignored() {
    let mut osc = OscSniffer::default();
    for sequence in [
        "\x1b]6973;flair;u=a%00b;h=box\x07",
        "\x1b]6973;flair;u=ada;h=b%XXox\x07",
        "\x1b]6973;flair;u=ada;h=box;v=%4\x07",
        "\x1b]6973;flair;u=ada;h=box;v=%+1\x07",
        "\x1b]6973;flair;u=%FF;h=box\x07",
        "\x1b]6973;flair;u=a\0b;h=box\x07",
        "\x1b]6973;other;u=ada;h=box\x07",
        "\x1b]6973;flairx;u=ada\x07",
        "\x1b]6973\x07",
    ] {
        osc.advance(sequence.as_bytes());
        assert!(osc.take().is_empty(), "{sequence:?}");
    }
}

#[test]
fn oversized_shell_facts_are_dropped() {
    let mut osc = OscSniffer::default();
    let long = "a".repeat(9000);
    osc.advance(format!("\x1b]6973;flair;u={long};h=box\x07").as_bytes());
    assert!(osc.take().is_empty());
    osc.advance(b"\x1b]6973;flair;u=ada;h=box\x07");
    assert_eq!(osc.take(), vec![facts("ada", "box", None)]);
}
