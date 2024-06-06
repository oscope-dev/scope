use std::collections::BTreeMap;
use crate::models::core::ModelMetadata;
use crate::models::v1alpha::V1AlphaApiVersion;
use crate::models::{HelpMetadata, InternalScopeModel, ScopeModel};
use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How to load the report to GitHub Issue
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportDestinationGithubIssueSpec {
    /// `owner` of the repository for the issue
    pub owner: String,

    /// `repo` the name of the repo for the issue
    pub repo: String,

    #[serde(default)]
    /// A list of tags to be added to the issue
    pub tags: Vec<String>,
}

/// How to upload a report to RustyPaste
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportDestinationRustyPasteSpec {
    /// URL of RustyPaste
    pub url: String,
}

/// Create a report that is only local
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportDestinationLocalSpec {
    /// Directory to put the report into
    pub directory: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub enum ReportDestinationSpec {
    RustyPaste(ReportDestinationRustyPasteSpec),
    GithubIssue(ReportDestinationGithubIssueSpec),
    Local(ReportDestinationLocalSpec),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportDestinationTemplates {
    /// Title to use when creating the issue. This is a Jinja2 style template. `entrypoint` is
    /// provided as a variable, which is the scope command run.
    pub title: Option<String>,

    /// Template to use when generating a bug report with `scope doctor`
    /// A Jinja2 style template, to be included. The text should be in Markdown format. Scope
    /// injects `command` as the command that was run.
    pub doctor: Option<String>,

    /// Template to use when generating a bug without with analyze or intercept
    /// A Jinja2 style template, to be included. The text should be in Markdown format. Scope
    /// injects `command` as the command that was run.
    pub command: Option<String>,

    #[serde(default, flatten)]
    /// Additional templates, when provided they will be available to `doctor` or `command` templates.
    /// Each template is a Jinja2 style template, to be included. The text should be in Markdown
    /// format. Scope injects `command` as the command that was run.
    pub extra_definitions: BTreeMap<String, String>
}

/// Define where to upload the report to
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportLocationSpec {
    #[serde(with = "serde_yaml::with::singleton_map")]
    #[schemars(with = "ReportDestinationSpec")]
    /// Destination the report should be uploaded to
    pub destination: ReportDestinationSpec,

    #[serde(default)]
    /// Templates to use when uploading a report
    pub templates: ReportDestinationTemplates,

    #[serde(default)]
    /// defines additional data that needs to be pulled from the system when reporting a bug.
    /// `additionalData` is a map of `string:string`, the value is a command that should be run.
    /// When a report is built, the commands will be run and automatically included in the report.
    pub additional_data: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, JsonSchema)]
pub enum ReportLocationKind {
    #[strum(serialize = "ScopeReportLocation")]
    ScopeReportLocation,
}

/// A `ScopeReportLocation` tells where to upload a report to.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct V1AlphaReportLocation {
    /// API version of the resource
    pub api_version: V1AlphaApiVersion,
    /// The type of resource.
    pub kind: ReportLocationKind,
    /// Standard set of options including name, description for the resource.
    /// Together `kind` and `metadata.name` are required to be unique. If there are duplicate, the
    /// resources "closest" to the execution dir will take precedence.
    pub metadata: ModelMetadata,
    /// Options for the resource.
    pub spec: ReportLocationSpec,
}

impl HelpMetadata for V1AlphaReportLocation {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        format!("{}/{}", self.kind(), self.name())
    }
}

impl ScopeModel<ReportLocationSpec> for V1AlphaReportLocation {
    fn api_version(&self) -> String {
        V1AlphaReportLocation::int_api_version()
    }

    fn kind(&self) -> String {
        V1AlphaReportLocation::int_kind()
    }

    fn spec(&self) -> &ReportLocationSpec {
        &self.spec
    }
}

impl InternalScopeModel<ReportLocationSpec, V1AlphaReportLocation> for V1AlphaReportLocation {
    fn int_api_version() -> String {
        V1AlphaApiVersion::ScopeV1Alpha.to_string()
    }

    fn int_kind() -> String {
        ReportLocationKind::ScopeReportLocation.to_string()
    }
    #[cfg(test)]
    fn examples() -> Vec<String> {
        vec![
            "v1alpha/ReportLocation.github.yaml".to_string(),
            "v1alpha/ReportLocation.rustyPaste.yaml".to_string(),
        ]
    }
}
