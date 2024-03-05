use crate::models::core::ModelMetadata;

use crate::models::v1alpha::V1AlphaApiVersion;
use derive_builder::Builder;

use crate::models::{HelpMetadata, InternalScopeModel, ScopeModel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Definition of the Report Definition
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportDefinitionSpec {
    #[serde(default)]
    /// defines additional data that needs to be pulled from the system when reporting a bug.
    /// `additionalData` is a map of `string:string`, the value is a command that should be run.
    /// When a report is built, the commands will be run and automatically included in the report.
    pub additional_data: BTreeMap<String, String>,

    /// a Jinja2 style template, to be included. The text should be in Markdown format. Scope
    /// injects `command` as the command that was run.
    pub template: String,
}

#[derive(Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, JsonSchema)]
pub enum ReportDefinitionKind {
    #[strum(serialize = "ScopeReportDefinition")]
    ScopeReportDefinition,
}

/// A `ScopeReportDefinition` tells scope how to collect details about the system when there
/// is an issue they need to report.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct V1AlphaReportDefinition {
    /// API version of the resource
    pub api_version: V1AlphaApiVersion,
    /// The type of resource.
    pub kind: ReportDefinitionKind,
    /// Standard set of options including name, description for the resource.
    /// Together `kind` and `metadata.name` are required to be unique. If there are duplicate, the
    /// resources "closest" to the execution dir will take precedence.
    pub metadata: ModelMetadata,
    /// Options for the resource.
    pub spec: ReportDefinitionSpec,
}

impl HelpMetadata for V1AlphaReportDefinition {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        format!("{}/{}", self.kind(), self.name())
    }
}

impl ScopeModel<ReportDefinitionSpec> for V1AlphaReportDefinition {
    fn api_version(&self) -> String {
        Self::int_api_version()
    }

    fn kind(&self) -> String {
        Self::int_kind()
    }

    fn spec(&self) -> &ReportDefinitionSpec {
        &self.spec
    }
}

impl InternalScopeModel<ReportDefinitionSpec, V1AlphaReportDefinition> for V1AlphaReportDefinition {
    fn int_api_version() -> String {
        V1AlphaApiVersion::ScopeV1Alpha.to_string()
    }

    fn int_kind() -> String {
        ReportDefinitionKind::ScopeReportDefinition.to_string()
    }
    #[cfg(test)]
    fn examples() -> Vec<String> {
        vec!["v1alpha/ReportDefinition.yaml".to_string()]
    }
}
