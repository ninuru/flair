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
