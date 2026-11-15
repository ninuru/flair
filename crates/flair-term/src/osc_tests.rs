use super::*;
use flair_core::agent::{AgentKind, AgentRecord, AgentReport, AgentState};

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

#[test]
fn shell_facts_with_control_or_invisible_format_characters_are_dropped() {
    let mut osc = OscSniffer::default();
    for sequence in [
        "\x1b]6973;flair;u=a%1Bb;h=box\x07",
        "\x1b]6973;flair;u=ada;h=b%0Aox\x07",
        "\x1b]6973;flair;u=ada;h=box;v=%C2%85\x07",
        "\x1b]6973;flair;u=%E2%80%AEada;h=box\x07",
        "\x1b]6973;flair;u=ada;h=%E2%81%A6box\x07",
        "\x1b]6973;flair;u=ada;h=box;v=e%E2%80%8Bnv\x07",
        "\x1b]6973;flair;u=%EF%BB%BFada;h=box\x07",
    ] {
        osc.advance(sequence.as_bytes());
        assert!(osc.take().is_empty(), "{sequence:?}");
    }
}

#[test]
fn shell_facts_are_capped_at_64_characters() {
    let mut osc = OscSniffer::default();
    let long = "%C3%B6".repeat(100);
    osc.advance(format!("\x1b]6973;flair;u={long};h={long};v={long}\x07").as_bytes());
    let capped = "ö".repeat(64);
    assert_eq!(
        osc.take(),
        vec![facts(&capped, &capped, Some(capped.as_str()))]
    );
}

#[test]
fn a_cwd_with_control_or_invisible_format_characters_is_dropped() {
    let mut osc = OscSniffer::default();
    for sequence in [
        "\x1b]7;file:///tmp/a%1Bb\x07",
        "\x1b]7;file:///tmp/a%0Ab\x07",
        "\x1b]7;file:///tmp/%E2%80%AEevil\x07",
    ] {
        osc.advance(sequence.as_bytes());
        assert!(osc.take().is_empty(), "{sequence:?}");
    }
}

#[test]
fn the_local_hostname_is_read_once_and_may_be_percent_encoded() {
    let mut osc = OscSniffer {
        hostname: Some("my host".into()),
        ..OscSniffer::default()
    };
    osc.advance(b"\x1b]7;file://my%20host/tmp/here\x07");
    osc.advance(b"\x1b]7;file://other/tmp/there\x07");
    assert_eq!(osc.take(), vec![TermEvent::Cwd(PathBuf::from("/tmp/here"))]);
    let mut fresh = OscSniffer::default();
    fresh.advance(b"\x1b]7;file://foreign.invalid/tmp\x07");
    assert!(fresh.hostname.is_some());
}

fn agent(text: &str) -> Vec<TermEvent> {
    let mut osc = OscSniffer::default();
    osc.advance(format!("\x1b]7501;{text}\x07").as_bytes());
    osc.take()
}

fn set(state: AgentState, kind: Option<AgentKind>) -> AgentRecord {
    AgentRecord {
        state,
        kind,
        app: None,
        title: None,
        msg: None,
    }
}

fn report(id: &str, record: AgentRecord) -> Vec<TermEvent> {
    vec![TermEvent::Agent(AgentReport::Set {
        id: id.to_owned(),
        record,
    })]
}

#[test]
fn agent_query_gets_the_capability_reply() {
    for terminator in ["\x07", "\x1b\\"] {
        let mut osc = OscSniffer::default();
        osc.advance(format!("\x1b]7501;?{terminator}").as_bytes());
        assert_eq!(
            osc.take(),
            vec![TermEvent::PtyWrite(
                b"\x1b]7501;?:states=idle,working,blocked,done,error:kinds=permission,question,auth\x1b\\"
                    .to_vec()
            )]
        );
    }
}

#[test]
fn agent_report_decodes_every_key() {
    assert_eq!(
        agent(
            "state=blocked:kind=permission:app=claude:id=a/b:title=Rml4IGxvZ2luIGJ1Zw==:msg=aGk=:progress=40"
        ),
        report(
            "a/b",
            AgentRecord {
                state: AgentState::Blocked,
                kind: Some(AgentKind::Permission),
                app: Some("claude".to_owned()),
                title: Some("Fix login bug".to_owned()),
                msg: Some("hi".to_owned()),
            }
        )
    );
}

#[test]
fn agent_report_trims_keeps_last_and_skips_unknown_or_malformed_pairs() {
    assert_eq!(
        agent(" state = working : state=done : color=red : junk : =x : title=Rml4IGxvZ2luIGJ1Zw"),
        report(
            "",
            AgentRecord {
                title: Some("Fix login bug".to_owned()),
                ..set(AgentState::Done, None)
            }
        )
    );
}

#[test]
fn agent_report_without_a_known_state_is_ignored() {
    assert!(agent("app=claude").is_empty());
    assert!(agent("state=sleeping").is_empty());
    assert!(agent("").is_empty());
}

#[test]
fn agent_report_with_bad_text_is_discarded_whole() {
    assert!(agent("state=done:title=!!!").is_empty());
    assert!(agent("state=done:title=AQ==").is_empty());
    assert!(agent("state=done:msg=/w==").is_empty());
    let long = "a".repeat(4100);
    assert!(agent(&format!("state=done:junk={long}")).is_empty());
}

#[test]
fn agent_text_over_its_limit_and_bad_app_are_skipped() {
    let title = "QUFB".repeat(65);
    let app = "x".repeat(33);
    assert_eq!(
        agent(&format!("state=done:title={title}:app={app}")),
        report("", set(AgentState::Done, None))
    );
    assert_eq!(
        agent("state=done:app=a\u{1}b"),
        report("", set(AgentState::Done, None))
    );
}

#[test]
fn agent_kind_needs_blocked() {
    assert_eq!(
        agent("state=working:kind=permission"),
        report("", set(AgentState::Working, None))
    );
    assert_eq!(
        agent("state=blocked:kind=auth"),
        report("", set(AgentState::Blocked, Some(AgentKind::Auth)))
    );
    assert_eq!(
        agent("state=blocked:kind=bogus"),
        report("", set(AgentState::Blocked, None))
    );
}

#[test]
fn agent_clear_with_and_without_id() {
    assert_eq!(
        agent("state=clear:id=a/b"),
        vec![TermEvent::Agent(AgentReport::Clear(Some("a/b".to_owned())))]
    );
    assert_eq!(
        agent("state=clear"),
        vec![TermEvent::Agent(AgentReport::Clear(None))]
    );
}

#[test]
fn agent_report_with_an_invalid_id_is_discarded() {
    let segment = "s".repeat(33);
    let deep = ["a"; 9].join("/");
    let long = vec!["s".repeat(32); 4].join("/");
    for id in [
        "a b",
        "a=b",
        "a//b",
        "/a",
        "a/",
        "é",
        segment.as_str(),
        deep.as_str(),
        long.as_str(),
    ] {
        assert!(
            agent(&format!("state=working:id={id}")).is_empty(),
            "{id:?}"
        );
    }
    let ok = ["a"; 8].join("/");
    assert_eq!(
        agent(&format!("state=working:id={ok}")),
        report(&ok, set(AgentState::Working, None))
    );
}

#[test]
fn reset_clears_all_agents() {
    let mut osc = OscSniffer::default();
    osc.advance(b"\x1bc");
    assert_eq!(osc.take(), vec![TermEvent::Agent(AgentReport::Clear(None))]);
}

#[test]
fn agent_text_with_invisible_or_bidi_format_characters_is_rejected() {
    for text in [
        "\u{202E}gnp.exe",
        "a\u{200B}b",
        "a\u{2028}b",
        "\u{2066}a",
        "\u{FEFF}a",
    ] {
        let encoded = BASE64.encode(text);
        assert!(
            agent(&format!("state=done:title={encoded}")).is_empty(),
            "{text:?}"
        );
        assert!(
            agent(&format!("state=done:msg={encoded}")).is_empty(),
            "{text:?}"
        );
        assert_eq!(
            agent(&format!("state=done:app={text}")),
            report("", set(AgentState::Done, None)),
            "{text:?}"
        );
    }
}
