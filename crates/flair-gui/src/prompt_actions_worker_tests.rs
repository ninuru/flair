use super::*;

#[test]
fn context_output_larger_than_a_pipe_buffer_is_drained_before_exit() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf '%32768s' x"]);
    let output = run(command).unwrap();
    assert_eq!(output.len(), 32768);
    assert!(output.ends_with('x'));
}

#[test]
fn excessive_context_output_reports_the_limit_instead_of_a_pipe_timeout() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf '%262144s' x"]);
    assert!(run(command).unwrap_err().contains("too large"));
}
