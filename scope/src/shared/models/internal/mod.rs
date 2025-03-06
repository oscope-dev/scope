use crate::models::prelude::{
    ModelRoot, V1AlphaDoctorGroup, V1AlphaKnownError, V1AlphaReportLocation,
};
use crate::models::InternalScopeModel;
use crate::shared::prelude::*;
use anyhow::anyhow;
use anyhow::Result;
use minijinja::{context, Environment};
use path_clean::PathClean;
use serde_yaml::Value;
use std::collections::VecDeque;
use std::path::Path;

mod command;
mod doctor_group;
mod fix;
mod known_error;
mod upload_location;

use self::known_error::KnownError;
use self::upload_location::ReportUploadLocation;

pub mod prelude {
    pub use super::ParsedConfig;
    pub use super::{command::*, doctor_group::*, fix::*, known_error::*, upload_location::*};
}

#[derive(Debug, PartialEq)]
pub enum ParsedConfig {
    KnownError(KnownError),
    ReportUpload(ReportUploadLocation),
    DoctorGroup(DoctorGroup),
}

#[cfg(test)]
impl ParsedConfig {
    pub fn get_report_upload_spec(&self) -> Option<ReportUploadLocation> {
        match self {
            ParsedConfig::ReportUpload(root) => Some(root.clone()),
            _ => None,
        }
    }

    pub fn get_known_error_spec(&self) -> Option<KnownError> {
        match self {
            ParsedConfig::KnownError(root) => Some(root.clone()),
            _ => None,
        }
    }

    pub fn get_doctor_group(&self) -> Option<DoctorGroup> {
        match self {
            ParsedConfig::DoctorGroup(root) => Some(root.clone()),
            _ => None,
        }
    }
}

impl TryFrom<ModelRoot<Value>> for ParsedConfig {
    type Error = anyhow::Error;

    fn try_from(value: ModelRoot<Value>) -> Result<Self, Self::Error> {
        if let Ok(Some(known)) = V1AlphaDoctorGroup::known_type(&value) {
            return Ok(ParsedConfig::DoctorGroup(DoctorGroup::try_from(known)?));
        }
        if let Ok(Some(known)) = V1AlphaKnownError::known_type(&value) {
            return Ok(ParsedConfig::KnownError(KnownError::try_from(known)?));
        }
        if let Ok(Some(known)) = V1AlphaReportLocation::known_type(&value) {
            return Ok(ParsedConfig::ReportUpload(ReportUploadLocation::try_from(
                known,
            )?));
        }
        Err(anyhow!("Error was know a known type"))
    }
}

pub(crate) fn extract_command_path(parent_dir: &Path, exec: &str) -> String {
    let mut parts: VecDeque<_> = exec.split(' ').map(|x| x.to_string()).collect();
    let mut command = parts.pop_front().unwrap();

    if command.starts_with('.') {
        let full_command = parent_dir.join(command).clean().display().to_string();
        command = full_command;
    }

    parts.push_front(command);

    parts
        .iter()
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn substitute_templates(work_dir: &str, input_str: &str) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("input_str", input_str)?;
    let template = env.get_template("input_str")?;
    let result = template.render(context! { working_dir => work_dir })?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command_path() {
        let base_path = Path::new("/foo/bar");
        assert_eq!(
            "/foo/bar/scripts/foo.sh",
            extract_command_path(base_path, "./scripts/foo.sh")
        );
        assert_eq!(
            "/scripts/foo.sh",
            extract_command_path(base_path, "/scripts/foo.sh")
        );
        assert_eq!("foo", extract_command_path(base_path, "foo"));
        assert_eq!("foo bar", extract_command_path(base_path, "foo bar"));
    }

    mod substitute_templates_spec {
        use super::*;

        #[test]
        fn working_dir_is_subbed() {
            let working_dir = "/some/path";
            let command = "{{ working_dir }}/foo.sh";

            let actual = substitute_templates(&working_dir, &command).unwrap();

            assert_eq!("/some/path/foo.sh".to_string(), actual)
        }

        #[test]
        fn text_without_templates_is_passed_through() {
            let working_dir = "/some/path";
            let command = "./foo.sh";

            let actual = substitute_templates(&working_dir, &command).unwrap();

            assert_eq!("./foo.sh".to_string(), actual)
        }

        #[test]
        fn other_templates_are_erased() {
            // I don't believe this is intentional behavior,
            // but it is the current behavior.
            let working_dir = "/some/path";
            let command = "{{ not_a_thing }}/foo.sh";

            let actual = substitute_templates(&working_dir, &command).unwrap();

            assert_eq!("/foo.sh".to_string(), actual)
        }
    }
}
