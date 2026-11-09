use super::*;
#[test]
fn scenario_rejects_zero_size_and_unknown_fields() {
    assert!(Scenario::parse("cols = 0\nsteps = []").is_err());
    assert!(Scenario::parse("typo = true\nsteps = []").is_err());
}
#[test]
fn scenario_parses_typed_steps() {
    let scenario = Scenario::parse(
        r#"program = "/bin/sh"
args = ["-c", "printf ready; read x"]
[[steps]]
action = "wait"
text = "ready"
[[steps]]
action = "key"
key = "Enter"
ctrl = true
"#,
    )
    .unwrap();
    assert_eq!(scenario.steps.len(), 2);
    assert!(matches!(&scenario.steps[1], Step::Key { ctrl: true, .. }));
}

#[test]
fn scenario_size_is_the_grid_size_and_rejects_overflow() {
    assert_eq!(
        client_size(100, 30).unwrap(),
        GridSize {
            cols: 100,
            rows: 30
        }
    );
    assert!(client_size(1, 65535).is_ok());
    assert!(Scenario::parse("cols = 1000\nrows = 1001\nsteps = []").is_err());
    assert!(Scenario::parse("[[steps]]\naction = 'resize'\ncols = 1000\nrows = 1001").is_err());
}
