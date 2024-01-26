use crate::models::prelude::DoctorGroup;
use crate::models::ModelRoot;
use serde_yaml::Value;

mod doctor_group;
mod known_error;
mod report_definition;
mod upload_location;

use self::known_error::KnownError;
use self::report_definition::ReportDefinition;
use self::upload_location::ReportUploadLocation;

pub mod prelude {
    pub use super::ParsedConfig;
    pub use super::{doctor_group::*, known_error::*, report_definition::*, upload_location::*};
}

#[derive(Debug, PartialEq)]
pub enum ParsedConfig {
    KnownError(ModelRoot<KnownError>),
    ReportUpload(ModelRoot<ReportUploadLocation>),
    ReportDefinition(ModelRoot<ReportDefinition>),
    DoctorGroup(ModelRoot<DoctorGroup>),
}

#[cfg(test)]
impl ParsedConfig {
    pub fn get_report_upload_spec(&self) -> Option<ReportUploadLocation> {
        match self {
            ParsedConfig::ReportUpload(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    pub fn get_report_def_spec(&self) -> Option<ReportDefinition> {
        match self {
            ParsedConfig::ReportDefinition(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    pub fn get_known_error_spec(&self) -> Option<KnownError> {
        match self {
            ParsedConfig::KnownError(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    pub fn get_doctor_group(&self) -> Option<DoctorGroup> {
        match self {
            ParsedConfig::DoctorGroup(root) => Some(root.spec.clone()),
            _ => None,
        }
    }
}

impl TryFrom<ModelRoot<Value>> for ParsedConfig {
    type Error = anyhow::Error;

    fn try_from(value: ModelRoot<Value>) -> Result<Self, Self::Error> {
        ParsedConfig::try_from(&value)
    }
}
