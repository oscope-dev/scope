//! Integration tests for the library API.
//!
//! These tests verify that the library can be used programmatically
//! without CLI dependencies.

use dx_scope::{
    AnalyzeInput, AnalyzeOptions, AutoApprove, ConfigLoadOptions, DenyAll, DoctorRunOptions,
    InquireInteraction, NoOpProgress, ProgressReporter, UserInteraction,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn test_analyze_options_creation() {
    let options = AnalyzeOptions::default();
    assert!(options.known_errors.is_empty());
}

#[test]
fn test_analyze_options_with_known_errors() {
    let options = AnalyzeOptions::new(BTreeMap::new(), PathBuf::from("/tmp"));
    assert!(options.known_errors.is_empty());
    assert_eq!(options.working_dir, PathBuf::from("/tmp"));
}

#[test]
fn test_analyze_input_variants() {
    let _file_input = AnalyzeInput::from_file("/path/to/file");
    let _lines_input = AnalyzeInput::from_lines(vec!["line1".to_string()]);
    let _stdin_input = AnalyzeInput::Stdin;
}

#[test]
fn test_doctor_run_options_creation() {
    let options = DoctorRunOptions::default();
    assert!(options.only_groups.is_none());
    assert!(!options.run_fix);
}

#[test]
fn test_doctor_run_options_with_fixes() {
    let options = DoctorRunOptions::with_fixes();
    assert!(options.run_fix);
}

#[test]
fn test_doctor_run_options_ci_mode() {
    let options = DoctorRunOptions::ci_mode();
    assert!(!options.run_fix);
}

#[test]
fn test_doctor_run_options_for_groups() {
    let groups = vec!["build".to_string(), "test".to_string()];
    let options = DoctorRunOptions::for_groups(groups.clone());
    assert_eq!(options.only_groups, Some(groups));
}

#[test]
fn test_config_load_options_creation() {
    let options = ConfigLoadOptions::default();
    assert!(options.extra_config.is_empty());
    assert!(!options.disable_default_config);
}

#[test]
fn test_config_load_options_explicit_only() {
    let paths = vec![PathBuf::from("/config")];
    let options = ConfigLoadOptions::explicit_only(paths.clone());
    assert_eq!(options.extra_config, paths);
    assert!(options.disable_default_config);
}

#[test]
fn test_user_interaction_auto_approve() {
    let interaction = AutoApprove;
    assert!(interaction.confirm("Test?", None));
    assert!(interaction.confirm("Test?", Some("Help text")));
}

#[test]
fn test_user_interaction_deny_all() {
    let interaction = DenyAll;
    assert!(!interaction.confirm("Test?", None));
    assert!(!interaction.confirm("Test?", Some("Help text")));
}

#[test]
fn test_no_op_progress() {
    let progress = NoOpProgress;
    progress.start_group("test", 5);
    progress.advance_action("action", "desc");
    progress.finish_group();
    // Should not panic
}

#[test]
fn test_inquire_interaction_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<InquireInteraction>();
    assert_send_sync::<AutoApprove>();
    assert_send_sync::<DenyAll>();
    assert_send_sync::<NoOpProgress>();
}
