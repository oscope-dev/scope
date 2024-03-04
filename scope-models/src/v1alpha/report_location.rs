use crate::core::ModelMetadata;
use crate::v1alpha::V1AlphaApiVersion;
use derive_builder::Builder;

use crate::{HelpMetadata, InternalScopeModel, ScopeModel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportDestinationGithubIssueSpec {
    pub owner: String,
    pub repo: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub enum ReportDestinationSpec {
    RustyPaste { url: String },
    GithubIssue(ReportDestinationGithubIssueSpec),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportLocationSpec {
    #[serde(with = "serde_yaml::with::singleton_map")]
    #[schemars(with = "ReportDestinationSpec")]
    pub destination: ReportDestinationSpec,
}

#[derive(Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, JsonSchema)]
pub enum ReportLocationKind {
    #[strum(serialize = "ScopeReportLocation")]
    ScopeReportLocation,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct V1AlphaReportLocation {
    pub api_version: V1AlphaApiVersion,
    pub kind: ReportLocationKind,
    pub metadata: ModelMetadata,
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
