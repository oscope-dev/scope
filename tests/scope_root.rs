#[allow(dead_code)]
mod common;

use assert_fs::fixture::{FileWriteStr, PathChild};
use common::*;
use predicates::prelude::predicate;

#[test]
fn test_will_find_child_configs() {
    let helper = ScopeTestHelper::new("test_will_find_child_configs", "nested-config");

    let results = helper.run_command(&["list"]);
    results
        .success()
        .stdout(predicate::str::contains("ScopeKnownError/disk-full"))
        .stdout(predicate::str::contains("The disk is full of files"))
        .stdout(predicate::str::contains(".scope/shared/disk-full.yaml"));

    helper.clean_work_dir();
}

#[test]
fn test_will_list_sub_command() {
    let test_helper = ScopeTestHelper::new("test_will_list_sub_command", "command-paths");
    let result = test_helper.run_command(&["list"]);

    result
        .success()
        .stdout(predicate::str::contains("external"))
        .stdout(predicate::str::contains(
            "External sub-command, run `scope external` for help",
        ));

    test_helper.clean_work_dir();
}

#[test]
fn test_external_sub_command_works() {
    let test_helper = ScopeTestHelper::new("test_sub_command_works", "command-paths");
    let result = test_helper.run_command(&["external"]);

    result
        .success()
        .stdout(predicate::str::contains("in external"));
    test_helper.clean_work_dir();
}

#[test]
fn test_extra_field_will_show_warn() {
    let test_helper = ScopeTestHelper::new("test_extra_field_will_show_warn", "empty");
    let example_file = "apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorGroup
metadata:
  name: fail-then-fix
  description: Run dep install
spec:
  extra: string
  actions: []
";
    test_helper
        .work_dir
        .child(".scope/bad-format.yml")
        .write_str(example_file)
        .unwrap();

    let result = test_helper.run_command(&["list"]);

    result.success().stdout(predicate::str::contains(
        "Resource 'ScopeDoctorGroup/fail-then-fix' didn't match the schema for ScopeDoctorGroup",
    ));
    test_helper.clean_work_dir();
}
