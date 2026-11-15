use super::*;

#[test]
fn rank_orders_blocked_above_error_working_done_and_idle() {
    assert_eq!(AgentState::Blocked.rank(), 4);
    assert_eq!(AgentState::Error.rank(), 3);
    assert_eq!(AgentState::Working.rank(), 2);
    assert_eq!(AgentState::Done.rank(), 1);
    assert_eq!(AgentState::Idle.rank(), 0);
}

#[test]
fn label_names_the_state_and_the_blocked_kind() {
    assert_eq!(
        AgentState::Blocked.label(Some(AgentKind::Permission)),
        "Needs approval"
    );
    assert_eq!(
        AgentState::Blocked.label(Some(AgentKind::Auth)),
        "Needs login"
    );
    assert_eq!(
        AgentState::Blocked.label(Some(AgentKind::Question)),
        "Needs input"
    );
    assert_eq!(AgentState::Blocked.label(None), "Needs input");
    assert_eq!(AgentState::Working.label(None), "Working");
    assert_eq!(AgentState::Done.label(Some(AgentKind::Auth)), "Done");
    assert_eq!(AgentState::Error.label(None), "Failed");
    assert_eq!(AgentState::Idle.label(None), "Idle");
}
