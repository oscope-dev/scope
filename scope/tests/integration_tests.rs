use assert_cmd::Command;
use assert_fs::{prelude::*, TempDir};
use predicates::prelude::*;
use std::path::PathBuf;

fn setup_working_dir() -> TempDir {
    let file_path = PathBuf::from(format!("{}/../examples", env!("CARGO_MANIFEST_DIR")));

    let temp = TempDir::new().unwrap();
    temp.copy_from(file_path, &["*", "**/*"]).unwrap();

    temp
}

#[test]
fn test_list_reports_all_config() {
    let working_dir = setup_working_dir();
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_list_reports_all_config")
        .arg("list")
        .assert();

    result
        .success()
        .stdout(predicate::str::contains("Doctor Checks"))
        .stdout(predicate::str::contains("path-exists"))
        .stdout(predicate::str::contains("error-exists"))
        .stdout(predicate::str::contains(
            "Check if the word error is in the logs",
        ))
        .stdout(predicate::str::contains("setup"))
        .stdout(predicate::str::contains(".scope/known-error.yaml"))
        .stdout(
            predicate::str::is_match(r"bar\s+External sub-command, run `scope bar` for help")
                .unwrap(),
        );
    working_dir.close().unwrap();
}

#[test]
fn test_doctor_list() {
    let working_dir = setup_working_dir();
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_doctor_list")
        .arg("doctor")
        .arg("list")
        .assert();

    result
        .success()
        .stdout(predicate::str::contains("path-exists"))
        .stdout(predicate::str::contains(
            "Check your shell for basic functionality",
        ))
        .stdout(predicate::str::contains("setup"));
    working_dir.close().unwrap();
}

#[test]
fn test_sub_command_works() {
    let working_dir = setup_working_dir();

    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_sub_command_works")
        .arg("-d")
        .arg("bar")
        .assert();

    result.success().stdout(predicate::str::contains("in bar"));
    working_dir.close().unwrap();
}

#[test]
fn test_run_check_path_exists() {
    let working_dir = setup_working_dir();

    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_run_check_path_exists")
        .arg("doctor")
        .arg("run")
        .arg("--only=path-exists")
        .assert();

    result.success().stdout(predicate::str::contains(
        "Check failed. Fix ran successfully",
    ));

    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_run_check_path_exists_2")
        .arg("doctor")
        .arg("run")
        .arg("--only=path-exists-fix-in-scope-dir")
        .assert();

    result.success().stdout(predicate::str::contains(
        "Check failed. Fix ran successfully.",
    ));

    working_dir.close().unwrap();
}

#[test]
fn test_run_setup() {
    let working_dir = setup_working_dir();
    working_dir
        .child("foo/requirements.txt")
        .write_str("initial cache")
        .unwrap();

    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_run_setup_1")
        .arg("doctor")
        .arg("run")
        .arg("--only=setup")
        .arg(&format!(
            "--cache-dir={}/.cache",
            working_dir.to_str().unwrap()
        ))
        .assert();

    result
        .success()
        .stdout(predicate::str::contains(
            "Check failed. Fix ran successfully.",
        ))
        .stdout(predicate::str::contains("Failed to write updated cache to disk").not());

    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_run_setup_2")
        .arg("doctor")
        .arg("run")
        .arg("--only=setup")
        .arg(&format!(
            "--cache-dir={}/.cache",
            working_dir.to_str().unwrap()
        ))
        .assert();

    result.success().stdout(predicate::str::contains(
        "Check failed. Fix ran successfully.",
    ));

    working_dir
        .child("foo/requirements.txt")
        .write_str("cache buster")
        .unwrap();
    let mut cmd = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    let result = cmd
        .current_dir(working_dir.path())
        .env("SCOPE_RUN_ID", "test_run_setup_3")
        .arg("doctor")
        .arg("run")
        .arg("--only=setup")
        .arg(&format!(
            "--cache-dir={}/.cache",
            working_dir.to_str().unwrap()
        ))
        .assert();

    result.success().stdout(predicate::str::contains(
        "Check failed. Fix ran successfully.",
    ));
}
