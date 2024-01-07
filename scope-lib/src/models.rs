use crate::{HelpMetadata, FILE_PATH_ANNOTATION};
use anyhow::anyhow;
use derivative::Derivative;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use strum::EnumString;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ModelMetadata {
    pub name: String,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

impl ModelMetadata {
    fn file_path(&self) -> String {
        self.annotations
            .get(FILE_PATH_ANNOTATION)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoot<V> {
    pub api_version: String,
    pub kind: String,
    pub metadata: ModelMetadata,
    pub spec: V,
}

#[derive(Debug, PartialEq, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum KnownKinds {
    ScopeReportDefinition,
    ScopeReportLocation,
    ScopeKnownError,
    ScopeDoctorCheck,
    #[strum(default)]
    UnknownKind(String),
}

#[derive(Debug, PartialEq, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum KnownApiVersion {
    #[strum(serialize = "scope.github.com/v1alpha")]
    ScopeV1Alpha,
    #[strum(default)]
    UnknownApiVersion(String),
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

    pub fn file_path(&self) -> String {
        self.metadata.file_path()
    }

    pub fn name(&self) -> &str {
        &self.metadata.name
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }
}

impl HelpMetadata for DoctorExecCheckSpec {
    fn description(&self) -> &str {
        &self.description
    }
}

impl HelpMetadata for KnownErrorSpec {
    fn description(&self) -> &str {
        &self.description
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
pub struct ReportUploadLocationSpec {
    pub destination: ReportUploadLocation,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ReportDefinitionSpec {
    pub additional_data: BTreeMap<String, String>,
    pub template: String,
}

#[derive(Debug, PartialEq)]
pub enum ParsedConfig {
    DoctorCheck(ModelRoot<DoctorExecCheckSpec>),
    KnownError(ModelRoot<KnownErrorSpec>),
    ReportUpload(ModelRoot<ReportUploadLocationSpec>),
    ReportDefinition(ModelRoot<ReportDefinitionSpec>),
}

#[cfg(test)]
impl ParsedConfig {
    fn get_report_upload_spec(&self) -> Option<ReportUploadLocationSpec> {
        match self {
            ParsedConfig::ReportUpload(root) => Some(root.spec.clone()),
            _ => None,
        }
    }

    fn get_report_def_spec(&self) -> Option<ReportDefinitionSpec> {
        match self {
            ParsedConfig::ReportDefinition(root) => Some(root.spec.clone()),
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

impl TryFrom<ModelRoot<Value>> for ParsedConfig {
    type Error = anyhow::Error;

    fn try_from(value: ModelRoot<Value>) -> std::result::Result<Self, Self::Error> {
        ParsedConfig::try_from(&value)
    }
}

impl TryFrom<&ModelRoot<Value>> for ParsedConfig {
    type Error = anyhow::Error;

    fn try_from(root: &ModelRoot<Value>) -> std::result::Result<Self, Self::Error> {
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
            _ => return Err(anyhow!("Unable to parse {}/{}", api_version, kind)),
        };

        Ok(parsed)
    }
}

mod parser {
    use crate::models::{ReportDefinitionSpec, ReportUploadLocation};
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
    struct ReportLocationV1Alpha {
        #[serde(with = "serde_yaml::with::singleton_map")]
        destination: ReportDestinationV1Alpha,
    }
    pub(super) fn parse_v1_report_location(
        value: &Value,
    ) -> Result<super::ReportUploadLocationSpec> {
        let parsed: ReportLocationV1Alpha = serde_yaml::from_value(value.clone())?;
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
        Ok(super::ReportUploadLocationSpec { destination })
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

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct ReportDefinitionV1Alpha {
        #[serde(default)]
        additional_data: BTreeMap<String, String>,
        template: String,
    }
    pub(super) fn parse_v1_report_definition(value: &Value) -> Result<ReportDefinitionSpec> {
        let parsed: ReportDefinitionV1Alpha = serde_yaml::from_value(value.clone())?;

        Ok(ReportDefinitionSpec {
            template: parsed.template.trim().to_string(),
            additional_data: parsed.additional_data,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config_load::parse_model;
    use crate::models::ParsedConfig;
    use anyhow::Result;
    use serde_yaml::Deserializer;
    use std::path::Path;

    fn parse_models_from_string(file_path: &Path, input: &str) -> Result<Vec<ParsedConfig>> {
        let mut models = Vec::new();
        for doc in Deserializer::from_str(input) {
            if let Some(parsed_model) = parse_model(doc, file_path) {
                models.push(parsed_model.try_into()?)
            }
        }

        Ok(models)
    }

    #[test]
    fn test_parse_scope_doctor_check_exec() {
        let text = "---
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorCheck
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
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorCheck
metadata:
  name: path-exists
spec:
  check:
    target: /scripts/does-path-env-exist.sh
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.
";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
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
    fn test_parse_scope_known_error() {
        let text = "apiVersion: scope.github.com/v1alpha
kind: ScopeKnownError
metadata:
  name: error-exists
spec:
  description: Check if the word error is in the logs
  pattern: error
  help: The command had an error, try reading the logs around there to find out what happened.";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        assert_eq!(configs[0].get_known_error_spec().unwrap(), KnownErrorSpec {
            description: "Check if the word error is in the logs".to_string(),
            help_text: "The command had an error, try reading the logs around there to find out what happened.".to_string(),
            pattern: "error".to_string(),
            regex: Regex::new("error").unwrap()
        });
    }

    #[test]
    fn test_parse_scope_report_loc() {
        let text = "
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportLocation
metadata:
  name: report
spec:
  destination:
      rustyPaste:
        url: https://foo.bar
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportLocation
metadata:
  name: github
spec:
  destination:
      githubIssue:
        owner: scope
        repo: party
 ";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(2, configs.len());

        assert_eq!(
            configs[0].get_report_upload_spec().unwrap(),
            ReportUploadLocationSpec {
                destination: ReportUploadLocation::RustyPaste {
                    url: "https://foo.bar".to_string()
                },
            }
        );

        assert_eq!(
            configs[1].get_report_upload_spec().unwrap(),
            ReportUploadLocationSpec {
                destination: ReportUploadLocation::GithubIssue {
                    owner: "scope".to_string(),
                    repo: "party".to_string(),
                    tags: Vec::new(),
                }
            }
        );
    }

    #[test]
    fn test_parse_scope_report_def() {
        let text = "
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportDefinition
metadata:
  name: report
spec:
  additionalData:
    env: env
  template: |
    hello bob
 ";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());

        assert_eq!(
            configs[0].get_report_def_spec().unwrap(),
            ReportDefinitionSpec {
                template: "hello bob".to_string(),
                additional_data: [("env".to_string(), "env".to_string())].into()
            }
        );
    }
}
