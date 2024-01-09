use crate::models::ModelRoot;
use serde_yaml::Value;

mod doctor_exec;
mod doctor_setup;
mod known_error;
mod report_definition;
mod upload_location;

use self::doctor_exec::DoctorExecCheckSpec;
use self::doctor_setup::DoctorSetupSpec;
use self::known_error::KnownErrorSpec;
use self::report_definition::ReportDefinitionSpec;
use self::upload_location::ReportUploadLocationSpec;

pub mod prelude {
    pub use super::ParsedConfig;
    pub use super::{
        doctor_exec::*, doctor_setup::*, known_error::*, report_definition::*, upload_location::*,
    };
}

#[derive(Debug, PartialEq)]
pub enum ParsedConfig {
    DoctorCheck(ModelRoot<DoctorExecCheckSpec>),
    KnownError(ModelRoot<KnownErrorSpec>),
    ReportUpload(ModelRoot<ReportUploadLocationSpec>),
    ReportDefinition(ModelRoot<ReportDefinitionSpec>),
    DoctorSetup(ModelRoot<DoctorSetupSpec>),
}

#[cfg(test)]
impl ParsedConfig {
    pub fn get_report_upload_spec(&self) -> Option<ReportUploadLocationSpec> {
        match self {
            ParsedConfig::ReportUpload(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    pub fn get_report_def_spec(&self) -> Option<ReportDefinitionSpec> {
        match self {
            ParsedConfig::ReportDefinition(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    pub fn get_known_error_spec(&self) -> Option<KnownErrorSpec> {
        match self {
            ParsedConfig::KnownError(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    pub fn get_doctor_check_spec(&self) -> Option<DoctorExecCheckSpec> {
        match self {
            ParsedConfig::DoctorCheck(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    pub fn get_doctor_setup_spec(&self) -> Option<DoctorSetupSpec> {
        match self {
            ParsedConfig::DoctorSetup(root) => Some(root.spec.clone()),
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
