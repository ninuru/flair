use std::collections::BTreeSet;

use super::*;

#[test]
fn all_lists_eleven_distinct_chips_in_band_order() {
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
        ]
    );
    let distinct: BTreeSet<_> = Chip::ALL.iter().map(|chip| format!("{chip:?}")).collect();
    assert_eq!(distinct.len(), 11);
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
