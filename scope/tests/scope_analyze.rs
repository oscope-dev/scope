use predicates::prelude::predicate;

#[allow(dead_code)]
mod common;
use common::*;

#[test]
fn test_run_command_with_known_error_stdout() {
    let helper = ScopeTestHelper::new("test_run_command_with_known_error_stdout", "known-errors");

    let results = helper.analyze_command("./error-stdout.sh");

    results.failure().stdout(predicate::str::contains(
        "Known error 'error-exists' found on line 2",
    ));
}

#[test]
fn test_run_command_with_known_error_stderr() {
    let helper = ScopeTestHelper::new("test_run_command_with_known_error_stderr", "known-errors");

    let results = helper.analyze_command("./error-stderr.sh");

    results.failure().stdout(predicate::str::contains(
        "Known error 'error-exists' found on line 2",
    ));
}
