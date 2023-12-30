use crate::UserListing;
use anyhow::{anyhow, Result};
use derivative::Derivative;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FILE_PATH_ANNOTATION: &str = "pity.github.com/file-path";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelMetadata {
    pub name: String,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoot<V> {
    pub api_version: String,
    pub kind: String,
    pub metadata: ModelMetadata,
    pub spec: V,
}

impl<V> ModelRoot<V> {
    pub fn with_spec<T>(&self, spec: T) -> ModelRoot<T> {
        ModelRoot {
            api_version: self.api_version.clone(),
            kind: self.kind.clone(),
            metadata: self.metadata.clone(),
            spec,
        }
    }
}

impl UserListing for ModelRoot<DoctorExecCheckSpec> {
    fn name(&self) -> &str {
        &self.metadata.name
    }
    fn description(&self) -> &str {
        &self.spec.description
    }

    fn location(&self) -> String {
        self.metadata
            .annotations
            .get(FILE_PATH_ANNOTATION)
            .cloned()
            .unwrap_or_default()
    }
}

impl UserListing for ModelRoot<KnownErrorSpec> {
    fn name(&self) -> &str {
        &self.metadata.name
    }
    fn description(&self) -> &str {
        &self.spec.description
    }

    fn location(&self) -> String {
        self.metadata
            .annotations
            .get(FILE_PATH_ANNOTATION)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorExecCheckSpec {
    pub target: PathBuf,
    pub description: String,
    pub help_text: String,
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Clone)]
pub struct KnownErrorSpec {
    pub description: String,
    pub pattern: String,
    #[derivative(PartialEq = "ignore")]
    pub regex: Regex,
    pub help_text: String,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ReportUploadLocation {
    RustyPaste { url: String },
}
#[derive(Debug, PartialEq, Clone)]
pub struct ReportUploadSpec {
    pub additional_data: BTreeMap<String, String>,
    pub destination: ReportUploadLocation,
}

#[derive(Debug, PartialEq)]
pub enum ParsedConfig {
    DoctorCheck(ModelRoot<DoctorExecCheckSpec>),
    KnownError(ModelRoot<KnownErrorSpec>),
    ReportUpload(ModelRoot<ReportUploadSpec>),
}

pub fn parse_config(file_path: &Path, config: &str) -> Result<Vec<ParsedConfig>> {
    let parsed: Value = serde_yaml::from_str(config)?;
    let values = match parsed {
        Value::Sequence(arr) => {
            let mut result = Vec::new();
            for item in arr {
                result.push(parse_value(file_path, item)?);
            }
            result
        }
        Value::Mapping(_) => vec![parse_value(file_path, parsed)?],
        _ => return Err(anyhow!("Input file wasn't an array or an object")),
    };
    Ok(values)
}

fn parse_value(file_path: &Path, value: Value) -> Result<ParsedConfig> {
    let mut root: ModelRoot<Value> = serde_yaml::from_value(value)?;
    let api_version: &str = &root.api_version.trim().to_ascii_lowercase();
    let kind: &str = &root.kind.trim().to_ascii_lowercase();

    root.metadata.annotations.insert(
        FILE_PATH_ANNOTATION.to_string(),
        file_path.display().to_string(),
    );

    let containing_dir = file_path.parent().unwrap();
    let parsed = match (api_version, kind) {
        ("pity.github.com/v1alpha", "pitydoctorcheck") => {
            let exec_check = parser::parse_v1_doctor_check(containing_dir, &root.spec)?;
            ParsedConfig::DoctorCheck(root.with_spec(exec_check))
        }
        ("pity.github.com/v1alpha", "pityknownerror") => {
            let known_error = parser::parse_v1_known_error(&root.spec)?;
            ParsedConfig::KnownError(root.with_spec(known_error))
        }
        ("pity.github.com/v1alpha", "pityreport") => {
            let report_upload = parser::parse_v1_report(&root.spec)?;
            ParsedConfig::ReportUpload(root.with_spec(report_upload))
        }
        (version, kind) => return Err(anyhow!("Unable to parse {}/{}", version, kind)),
    };

    Ok(parsed)
}

mod parser {
    use crate::models::ReportUploadLocation;
    use anyhow::Result;
    use regex::Regex;
    use serde::{Deserialize, Serialize};
    use serde_yaml::Value;
    use std::collections::BTreeMap;
    use std::path::Path;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    enum DoctorCheckTypeV1Alpha {
        Exec { target: String },
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct DoctorCheckV1Alpha {
        #[serde(flatten)]
        run_type: DoctorCheckTypeV1Alpha,
        description: String,
        help: String,
    }

    pub(super) fn parse_v1_doctor_check(
        base_path: &Path,
        value: &Value,
    ) -> Result<super::DoctorExecCheckSpec> {
        let parsed: DoctorCheckV1Alpha = serde_yaml::from_value(value.clone())?;
        let target = match parsed.run_type {
            DoctorCheckTypeV1Alpha::Exec { target } => base_path.join(target),
        };
        Ok(super::DoctorExecCheckSpec {
            help_text: parsed.help,
            target,
            description: parsed.description,
        })
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct KnownErrorV1Alpha {
        description: String,
        help: String,
        pattern: String,
    }
    pub(super) fn parse_v1_known_error(value: &Value) -> Result<super::KnownErrorSpec> {
        let parsed: KnownErrorV1Alpha = serde_yaml::from_value(value.clone())?;
        let regex = Regex::new(&parsed.pattern)?;
        Ok(super::KnownErrorSpec {
            pattern: parsed.pattern,
            regex,
            help_text: parsed.help,
            description: parsed.description,
        })
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    enum ReportDestinationV1Alpha {
        RustyPaste { url: String },
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct ReportV1Alpha {
        additional_data: BTreeMap<String, String>,
        #[serde(with = "serde_yaml::with::singleton_map")]
        destination: ReportDestinationV1Alpha,
    }
    pub(super) fn parse_v1_report(value: &Value) -> Result<super::ReportUploadSpec> {
        let parsed: ReportV1Alpha = serde_yaml::from_value(value.clone())?;
        let destination = match parsed.destination {
            ReportDestinationV1Alpha::RustyPaste { url } => {
                ReportUploadLocation::RustyPaste { url }
            }
        };
        Ok(super::ReportUploadSpec {
            additional_data: parsed.additional_data,
            destination,
        })
    }
}

#[test]
fn test_parse_pity_doctor_check_exec() {
    let text = "apiVersion: pity.github.com/v1alpha
kind: PityDoctorCheck
metadata:
  name: path-exists
spec:
  exec:
    target: scripts/does-path-env-exist.sh
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.";

    let path = Path::new("/foo/bar/file.yaml");
    let configs = parse_config(path, text).unwrap();
    assert_eq!(1, configs.len());
    if let ParsedConfig::DoctorCheck(model) = &configs[0] {
        assert_eq!(
            DoctorExecCheckSpec {
                description: "Check your shell for basic functionality".to_string(),
                help_text: "You're shell does not have a path env. Reload your shell.".to_string(),
                target: PathBuf::from("/foo/bar/scripts/does-path-env-exist.sh"),
            },
            model.spec
        );
    } else {
        unreachable!();
    }
}

#[test]
fn test_parse_pity_known_error() {
    let text = "apiVersion: pity.github.com/v1alpha
kind: PityKnownError
metadata:
  name: error-exists
spec:
  description: Check if the word error is in the logs
  pattern: error
  help: The command had an error, try reading the logs around there to find out what happened.";

    let path = Path::new("/foo/bar/file.yaml");
    let configs = parse_config(path, text).unwrap();
    assert_eq!(1, configs.len());
    if let ParsedConfig::KnownError(model) = &configs[0] {
        assert_eq!(
            KnownErrorSpec {
                description: "Check if the word error is in the logs".to_string(),
                help_text: "The command had an error, try reading the logs around there to find out what happened.".to_string(),
                pattern: "error".to_string(),
                regex: Regex::new("error").unwrap()
            },
            model.spec
        );
    } else {
        unreachable!()
    }
}

#[test]
fn test_parse_pity_report() {
    let text = "apiVersion: pity.github.com/v1alpha
kind: PityReport
metadata:
  name: report
spec:
  additionalData:
    env: env
  destination:
      rustyPaste:
        url: https://foo.bar";

    let path = Path::new("/foo/bar/file.yaml");
    let configs = parse_config(path, text).unwrap();
    assert_eq!(1, configs.len());
    if let ParsedConfig::ReportUpload(model) = &configs[0] {
        assert_eq!(
            ReportUploadSpec {
                destination: ReportUploadLocation::RustyPaste {
                    url: "https://foo.bar".to_string()
                },
                additional_data: [("env".to_string(), "env".to_string())].into()
            },
            model.spec
        );
    } else {
        unreachable!()
    }
}
