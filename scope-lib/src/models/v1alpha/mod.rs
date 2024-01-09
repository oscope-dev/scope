use crate::models::internal::ParsedConfig;
use crate::models::ModelRoot;
use anyhow::{anyhow, Result};
use serde_yaml::Value;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use strum::EnumString;

mod doctor_exec;
mod doctor_setup;
mod known_error;
mod report_definition;
mod report_location;

#[derive(Debug, PartialEq, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum KnownKinds {
    ScopeReportDefinition,
    ScopeReportLocation,
    ScopeKnownError,
    ScopeDoctorCheck,
    ScopeDoctorSetup,
    #[strum(default)]
    UnknownKind(String),
}

pub fn parse_v1_alpha1(root: &ModelRoot<Value>) -> Result<ParsedConfig> {
    let kind: &str = &root.kind.trim().to_ascii_lowercase();

    let known_kinds =
        KnownKinds::try_from(kind).unwrap_or_else(|_| KnownKinds::UnknownKind(kind.to_string()));
    let file_path = PathBuf::from(root.file_path());
    let containing_dir = file_path.parent().unwrap();

    let parsed = match known_kinds {
        KnownKinds::ScopeDoctorCheck => {
            let exec_check = doctor_exec::parse(containing_dir, &root.spec)?;
            ParsedConfig::DoctorCheck(root.with_spec(exec_check))
        }
        KnownKinds::ScopeKnownError => {
            let known_error = known_error::parse(&root.spec)?;
            ParsedConfig::KnownError(root.with_spec(known_error))
        }
        KnownKinds::ScopeReportLocation => {
            let report_upload = report_location::parse(&root.spec)?;
            ParsedConfig::ReportUpload(root.with_spec(report_upload))
        }
        KnownKinds::ScopeReportDefinition => {
            let report_upload = report_definition::parse(&root.spec)?;
            ParsedConfig::ReportDefinition(root.with_spec(report_upload))
        }
        KnownKinds::ScopeDoctorSetup => {
            let setup = doctor_setup::parse(containing_dir, &root.spec)?;
            ParsedConfig::DoctorSetup(root.with_spec(setup))
        }
        _ => return Err(anyhow!("Unable to parse v1alpha/{}", kind)),
    };

    Ok(parsed)
}

fn extract_command_path(parent_dir: &Path, command: &str) -> String {
    let mut parts: VecDeque<_> = command.split(' ').collect();
    let command = parts.pop_front().unwrap();

    if Path::new(command).is_absolute() {
        command.to_string()
    } else {
        let full_command = parent_dir.join(command).display().to_string();
        parts.push_front(&full_command);
        let parts: Vec<_> = parts.iter().cloned().collect();
        parts.join(" ")
    }
}

#[test]
fn test_extract_command_path() {
    let base_path = Path::new("/foo/bar");
    assert_eq!(
        "/foo/bar/scripts/foo.sh",
        extract_command_path(base_path, "scripts/foo.sh")
    );
    assert_eq!(
        "/scripts/foo.sh",
        extract_command_path(base_path, "/scripts/foo.sh")
    );
}
