use assert_cmd::assert::Assert;
use assert_cmd::Command;
use assert_fs::{prelude::*, TempDir};
use predicates::prelude::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn setup_working_dir() -> TempDir {
    let file_path = PathBuf::from(format!("{}/../examples", env!("CARGO_MANIFEST_DIR")));

    let temp = TempDir::new().unwrap();
    temp.copy_from(file_path, &["*", "**/*"]).unwrap();

    temp
}

struct ScopeTestHelper<'a> {
    work_dir: TempDir,
    name: &'a str,
    counter: AtomicUsize,
}

impl<'a> ScopeTestHelper<'a> {
    fn new(name: &'a str) -> Self {
        Self {
            work_dir: setup_working_dir(),
            name,
            counter: AtomicUsize::new(0),
        }
    }

    fn run_command(&self, args: &[&str]) -> Assert {
        let mut cmd = Command::cargo_bin("scope").unwrap();
        cmd.current_dir(self.work_dir.path())
            .env(
                "SCOPE_RUN_ID",
                format!(
                    "{}-{}",
                    self.name,
                    self.counter.fetch_add(1, Ordering::Relaxed)
                ),
            )
            .env("SCOPE_OUTPUT_PROGRESS", "plain")
            .env("NO_COLOR", "1")
            .args(args)
            .assert()
    }

    /// Execute `doctor run` (with optional args) with a cache-dir that's relative to the working dir.
    fn doctor_run(&self, args: Option<&[&str]>) -> Assert {
        let cache_args = format!("--cache-dir={}/.cache", self.work_dir.to_str().unwrap());

        let mut run_command = vec!["doctor", "run", &cache_args];

        if let Some(extra) = args {
            for entry in extra {
                run_command.push(entry)
            }
        }

        self.run_command(&run_command)
    }

    fn clean_work_dir(mut self) {
        self.work_dir.close().unwrap();
    }
}

#[test]
fn test_list_reports_all_config() {
    let test_helper = ScopeTestHelper::new("test_list_reports_all_config");
    let result = test_helper.run_command(&["list"]);

    result
        .success()
        .stdout(predicate::str::contains("ScopeDoctorCheck/path-exists"))
        .stdout(predicate::str::contains("ScopeKnownError/error-exists"))
        .stdout(predicate::str::contains("ScopeKnownError/disk-full"))
        .stdout(predicate::str::contains("ScopeDoctorGroup/group-1"))
        .stdout(predicate::str::contains("ScopeReportDefinition/template"))
        .stdout(predicate::str::contains("ScopeReportLocation/github"))
        .stdout(predicate::str::contains(
            "Check if the word error is in the logs",
        ))
        .stdout(predicate::str::contains("setup"))
        .stdout(predicate::str::contains(".scope/known-error.yaml"))
        .stdout(predicate::str::contains("Resource 'ScopeDoctorGroup/setup' didn't match the schema for ScopeDoctorGroup. Additional properties are not allowed ('extra' was unexpected)"))
        .stdout(
            predicate::str::is_match(r"bar\s+External sub-command, run `scope bar` for help")
                .unwrap(),
        );
    test_helper.clean_work_dir();
}

#[test]
fn test_doctor_list() {
    let test_helper = ScopeTestHelper::new("test_doctor_list");
    let result = test_helper.run_command(&["doctor", "list"]);

    result
        .success()
        .stdout(predicate::str::contains("ScopeDoctorGroup/group-1"))
        .stdout(predicate::str::contains(
            "Check your shell for basic functionality",
        ));
    test_helper.clean_work_dir();
}

#[test]
fn test_sub_command_works() {
    let test_helper = ScopeTestHelper::new("test_sub_command_works");
    let result = test_helper.run_command(&["-vv", "bar"]);

    result.success().stdout(predicate::str::contains("in bar"));
    test_helper.clean_work_dir();
}

#[test]
fn test_run_check_path_exists() {
    let test_helper = ScopeTestHelper::new("test_run_check_path_exists");
    let result = test_helper.doctor_run(Some(&["--only=path-exists"]));

    result.success().stdout(predicate::str::contains(
        "Check initially failed, fix was successful, group: \"path-exists\", name: \"1\"",
    ));

    let result = test_helper.doctor_run(Some(&["--only=path-exists-fix-in-scope-dir"]));

    result.success().stdout(predicate::str::contains(
        "Check initially failed, fix was successful, group: \"path-exists-fix-in-scope-dir\", name: \"1\"",
    ));

    test_helper.clean_work_dir();
}

#[test]
fn test_run_setup() {
    let test_helper = ScopeTestHelper::new("test_run_setup");

    test_helper
        .work_dir
        .child("foo/requirements.txt")
        .write_str("initial cache")
        .unwrap();

    let result = test_helper.doctor_run(Some(&["--only=setup"]));

    result
        .success()
        .stdout(predicate::str::contains(
            "Check initially failed, fix was successful, group: \"setup\", name: \"1\"",
        ))
        .stdout(predicate::str::contains("Failed to write updated cache to disk").not());

    let result = test_helper.doctor_run(Some(&["--only=setup"]));

    result.success().stdout(predicate::str::contains(
        "Check was successful, group: \"setup\", name: \"1\"",
    ));

    test_helper
        .work_dir
        .child("foo/requirements.txt")
        .write_str("cache buster")
        .unwrap();

    let result = test_helper.doctor_run(Some(&["--only=setup"]));

    result.success().stdout(predicate::str::contains(
        "Check initially failed, fix was successful, group: \"setup\", name: \"1\"",
    ));
}

#[test]
fn test_run_group_1() {
    let test_helper = ScopeTestHelper::new("test_run_group_1");
    test_helper
        .work_dir
        .child("foo/requirements.txt")
        .write_str("initial cache")
        .unwrap();

    let result = test_helper.doctor_run(Some(&["--only=group-1"]));

    result
        .failure()
        .stdout(predicate::str::contains(
            "Check initially failed, fix was successful, group: \"group-1\", name: \"fail then pass\"",
        ))
        .stdout(predicate::str::contains("Fix ran successfully, group: \"group-1\", name: \"sleep\""))
        .stdout(predicate::str::contains(
            "Check failed, no fix provided, group: \"group-1\", name: \"paths\"",
        ))
        .stdout(predicate::str::contains("Failed to write updated cache to disk").not());
}

#[test]
fn test_run_templated() {
    let test_helper = ScopeTestHelper::new("test_run_templated");
    let result = test_helper.doctor_run(Some(&["--only=templated"]));

    result.success().stdout(predicate::str::contains(
        "Check was successful, group: \"templated\", name: \"hushlogin\"",
    ));
}

#[test]
fn test_no_tasks_found() {
    let test_helper = ScopeTestHelper::new("test_no_tasks_found");
    let result = test_helper.doctor_run(Some(&["--only=bogus-group"]));

    result.success().stdout(predicate::str::contains(
        "Could not find any tasks to execute",
    ));
}
