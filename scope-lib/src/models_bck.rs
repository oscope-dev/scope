

use crate::{HelpMetadata, FILE_PATH_ANNOTATION};
use anyhow::anyhow;
use derivative::Derivative;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use strum::EnumString;

impl TryFrom<&ModelRoot<Value>> for ParsedConfig {
    type Error = anyhow::Error;

    fn try_from(root: &ModelRoot<Value>) -> Result<Self, Self::Error> {
        let api_version: &str = &root.api_version.trim().to_ascii_lowercase();
        let kind: &str = &root.kind.trim().to_ascii_lowercase();

        let known_kinds = KnownKinds::try_from(kind)
            .unwrap_or_else(|_| KnownKinds::UnknownKind(kind.to_string()));
        let api_versions = KnownApiVersion::try_from(api_version)
            .unwrap_or_else(|_| KnownApiVersion::UnknownApiVersion(api_version.to_string()));
        let file_path = PathBuf::from(root.file_path());
        let containing_dir = file_path.parent().unwrap();

        let parsed = match (api_versions, known_kinds) {
            (KnownApiVersion::ScopeV1Alpha, KnownKinds::ScopeDoctorCheck) => {
                let exec_check = parser::parse_v1_doctor_check(containing_dir, &root.spec)?;
                ParsedConfig::DoctorCheck(root.with_spec(exec_check))
            }
            (KnownApiVersion::ScopeV1Alpha, KnownKinds::ScopeKnownError) => {
                let known_error = parser::parse_v1_known_error(&root.spec)?;
                ParsedConfig::KnownError(root.with_spec(known_error))
            }
            (KnownApiVersion::ScopeV1Alpha, KnownKinds::ScopeReportLocation) => {
                let report_upload = parser::parse_v1_report_location(&root.spec)?;
                ParsedConfig::ReportUpload(root.with_spec(report_upload))
            }
            (KnownApiVersion::ScopeV1Alpha, KnownKinds::ScopeReportDefinition) => {
                let report_upload = parser::parse_v1_report_definition(&root.spec)?;
                ParsedConfig::ReportDefinition(root.with_spec(report_upload))
            }
            (KnownApiVersion::ScopeV1Alpha, KnownKinds::ScopeDoctorSetup) => {
                let setup = parser::parse_v1_doctor_setup(containing_dir, &root.spec)?;
                ParsedConfig::DoctorSetup(root.with_spec(setup))
            }
            _ => return Err(anyhow!("Unable to parse {}/{}", api_version, kind)),
        };

        Ok(parsed)
    }
}

mod parser {
    use crate::models::{DoctorSetupSpecCache, ReportDefinitionSpec, ReportUploadLocation};
    use anyhow::Result;
    use regex::Regex;
    use serde::{Deserialize, Serialize};
    use serde_yaml::Value;
    use std::collections::{BTreeMap, VecDeque};
    use std::path::Path;
}
