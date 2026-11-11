use std::collections::BTreeSet;

use super::*;

#[test]
fn all_lists_twelve_distinct_chips_in_band_order() {
    assert_eq!(
        Chip::ALL,
        [
            Chip::User,
            Chip::Host,
            Chip::Cwd,
            Chip::GitBranch,
            Chip::GitAheadBehind,
            Chip::GitChanges,
            Chip::Venv,
            Chip::Docker,
            Chip::Kube,
            Chip::Exit,
            Chip::Time,
            Chip::Date,
        ]
    );
    let distinct: BTreeSet<_> = Chip::ALL.iter().map(|chip| format!("{chip:?}")).collect();
    assert_eq!(distinct.len(), 12);
}

#[test]
fn prompt_info_defaults_to_nothing_known() {
    assert_eq!(
        PromptInfo::default(),
        PromptInfo {
            cwd: None,
            git: None,
            last: None,
            shell: None,
            docker: None,
            kube: None,
        }
    );
}

#[test]
fn labels_reject_control_and_invisible_format_characters() {
    for bad in [
        "a\x1bb",
        "a\nb",
        "a\u{85}b",
        "a\u{202E}b",
        "a\u{202A}b",
        "a\u{2066}b",
        "a\u{2069}b",
        "a\u{200B}b",
        "a\u{200F}b",
        "a\u{FEFF}b",
        "a\u{2028}b",
        "a\u{061C}b",
    ] {
        assert!(!plain(bad), "{bad:?}");
        assert_eq!(label(bad), None, "{bad:?}");
    }
    assert!(plain("main/fix-ö 界"));
    assert_eq!(label("feature/ö"), Some("feature/ö".into()));
}

#[test]
fn labels_are_capped_at_64_characters_on_a_char_boundary() {
    let long = "ö".repeat(70);
    let capped = label(&long).unwrap();
    assert_eq!(capped.chars().count(), LABEL_MAX);
    assert_eq!(LABEL_MAX, 64);
    assert_eq!(label(&"a".repeat(64)).unwrap().len(), 64);
}
