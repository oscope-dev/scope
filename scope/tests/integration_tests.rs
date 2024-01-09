use assert_cmd::Command;
use assert_fs::{prelude::*, TempDir};
use predicates::prelude::*;
use std::path::PathBuf;

fn get_example_file(name: &str) -> PathBuf {
    let file_path = PathBuf::from(format!(
        "{}/../examples/{}",
        env!("CARGO_MANIFEST_DIR"),
        name
    ))
    .canonicalize()
    .unwrap();
    println!("Example file {}", file_path.display());
    file_path
}

fn setup_working_dir() -> TempDir {
    let temp = TempDir::new().unwrap();
    let scope_dir = temp.child(".scope");
    scope_dir
        .child("known-error.yaml")
        .write_file(&get_example_file("known-error.yaml"))
        .unwrap();
    scope_dir
        .child("doctor-check.yaml")
        .write_file(&get_example_file("doctor-check.yaml"))
        .unwrap();
    scope_dir
        .child("doctor-setup.yaml")
        .write_file(&get_example_file("doctor-setup.yaml"))
        .unwrap();
    scope_dir
        .child("bin/scope-bar")
        .write_file(&get_example_file("bin/scope-bar"))
        .unwrap();
    scope_dir
        .child("scripts/does-path-env-exist.sh")
        .write_file(&get_example_file("scripts/does-path-env-exist.sh"))
        .unwrap();

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
        .stdout(predicate::str::contains("Doctor Setup"))
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
            "Check your shell for basic functionalityc",
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
