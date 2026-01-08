use predicates::prelude::predicate;

#[allow(dead_code)]
mod common;
use common::*;

#[test]
fn test_run_command_with_known_error_stdout() {
    let helper = ScopeTestHelper::new("test_run_command_with_known_error_stdout", "known-errors");

    let results = helper.analyze_command("bin/error-stdout.sh");

    results
        .failure()
        .stdout(predicate::str::contains("analyzing:  error"))
        .stdout(predicate::str::contains(
            "Known error 'error-exists' found on line 2",
        ));
}

#[test]
fn test_run_command_with_known_error_stderr() {
    let helper = ScopeTestHelper::new("test_run_command_with_known_error_stderr", "known-errors");

    let results = helper.analyze_command("bin/error-stderr.sh");

    results
        .failure()
        .stderr(predicate::str::contains("analyzing:  error"))
        .stdout(predicate::str::contains(
            "Known error 'error-exists' found on line 2",
        ));
}
