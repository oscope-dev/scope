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
    pub check_exec: String,
    pub fix_exec: Option<String>,
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
    RustyPaste {
        url: String,
    },
    GithubIssue {
        owner: String,
        repo: String,
        tags: Vec<String>,
    },
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

#[cfg(test)]
impl ParsedConfig {
    fn get_report_upload_spec(&self) -> Option<ReportUploadSpec> {
        match self {
            ParsedConfig::ReportUpload(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    fn get_known_error_spec(&self) -> Option<KnownErrorSpec> {
        match self {
            ParsedConfig::KnownError(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    fn get_doctor_check_spec(&self) -> Option<DoctorExecCheckSpec> {
        match self {
            ParsedConfig::DoctorCheck(root) => Some(root.spec.clone()),
            _ => None,
        }
    }
}

pub fn parse_config(file_path: &Path, config: &str) -> Result<Vec<ParsedConfig>> {
    let mut parsed_configs = Vec::new();
    for doc in serde_yaml::Deserializer::from_str(config) {
        let value = Value::deserialize(doc)?;
        parsed_configs.push(parse_value(file_path, value)?)
    }
    Ok(parsed_configs)
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
    use std::collections::{BTreeMap, VecDeque};
    use std::path::Path;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct DoctorCheckTypeV1Alpha {
        target: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct DoctorCheckV1Alpha {
        #[serde(with = "serde_yaml::with::singleton_map")]
        check: DoctorCheckTypeV1Alpha,
        #[serde(with = "serde_yaml::with::singleton_map", default)]
        fix: Option<DoctorCheckTypeV1Alpha>,
        description: String,
        help: String,
    }

    pub(super) fn parse_v1_doctor_check(
        base_path: &Path,
        value: &Value,
    ) -> Result<super::DoctorExecCheckSpec> {
        let parsed: DoctorCheckV1Alpha = serde_yaml::from_value(value.clone())?;

        let check_path = extract_command_path(base_path, &parsed.check.target);
        let fix_exec = parsed
            .fix
            .map(|path| extract_command_path(base_path, &path.target));

        Ok(super::DoctorExecCheckSpec {
            help_text: parsed.help,
            check_exec: check_path,
            fix_exec,
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
    struct ReportDestinationGithubIssueV1Alpha {
        owner: String,
        repo: String,
        #[serde(default)]
        tags: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    enum ReportDestinationV1Alpha {
        RustyPaste { url: String },
        GithubIssue(ReportDestinationGithubIssueV1Alpha),
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct ReportV1Alpha {
        #[serde(default)]
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
            ReportDestinationV1Alpha::GithubIssue(github_issue) => {
                ReportUploadLocation::GithubIssue {
                    owner: github_issue.owner,
                    repo: github_issue.repo,
                    tags: github_issue.tags,
                }
            }
        };
        Ok(super::ReportUploadSpec {
            additional_data: parsed.additional_data,
            destination,
        })
    }

    fn extract_command_path(parent_dir: &Path, command: &str) -> String {
        let mut parts: VecDeque<_> = command.split(" ").collect();
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
}

#[test]
fn test_parse_pity_doctor_check_exec() {
    let text = "---
apiVersion: pity.github.com/v1alpha
kind: PityDoctorCheck
metadata:
  name: path-exists
spec:
  check:
    target: scripts/does-path-env-exist.sh
  fix:
    target: /bin/true
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.
---
apiVersion: pity.github.com/v1alpha
kind: PityDoctorCheck
metadata:
  name: path-exists
spec:
  check:
    target: /scripts/does-path-env-exist.sh
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.
";

    let path = Path::new("/foo/bar/file.yaml");
    let configs = parse_config(path, text).unwrap();
    assert_eq!(2, configs.len());
    assert_eq!(
        configs[0].get_doctor_check_spec().unwrap(),
        DoctorExecCheckSpec {
            description: "Check your shell for basic functionality".to_string(),
            help_text: "You're shell does not have a path env. Reload your shell.".to_string(),
            check_exec: "/foo/bar/scripts/does-path-env-exist.sh".to_string(),
            fix_exec: Some("/bin/true".to_string())
        }
    );
    assert_eq!(
        configs[1].get_doctor_check_spec().unwrap(),
        DoctorExecCheckSpec {
            description: "Check your shell for basic functionality".to_string(),
            help_text: "You're shell does not have a path env. Reload your shell.".to_string(),
            check_exec: "/scripts/does-path-env-exist.sh".to_string(),
            fix_exec: None,
        }
    );
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
    assert_eq!(configs[0].get_known_error_spec().unwrap(), KnownErrorSpec {
                description: "Check if the word error is in the logs".to_string(),
                help_text: "The command had an error, try reading the logs around there to find out what happened.".to_string(),
                pattern: "error".to_string(),
                regex: Regex::new("error").unwrap()
            });
}

#[test]
fn test_parse_pity_report() {
    let text = "
---
apiVersion: pity.github.com/v1alpha
kind: PityReport
metadata:
  name: report
spec:
  additionalData:
    env: env
  destination:
      rustyPaste:
        url: https://foo.bar
---
apiVersion: pity.github.com/v1alpha
kind: PityReport
metadata:
  name: github
spec:
  additionalData:
    env: env
  destination:
      githubIssue:
        owner: pity
        repo: party
 ";

    let path = Path::new("/foo/bar/file.yaml");
    let configs = parse_config(path, text).unwrap();
    assert_eq!(2, configs.len());

    assert_eq!(
        configs[0].get_report_upload_spec().unwrap(),
        ReportUploadSpec {
            destination: ReportUploadLocation::RustyPaste {
                url: "https://foo.bar".to_string()
            },
            additional_data: [("env".to_string(), "env".to_string())].into()
        }
    );

    assert_eq!(
        configs[1].get_report_upload_spec().unwrap(),
        ReportUploadSpec {
            destination: ReportUploadLocation::GithubIssue {
                owner: "pity".to_string(),
                repo: "party".to_string(),
                tags: Vec::new(),
            },
            additional_data: [("env".to_string(), "env".to_string())].into()
        }
    );
}
