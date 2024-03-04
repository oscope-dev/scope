use crate::core::ModelMetadata;

use crate::v1alpha::V1AlphaApiVersion;
use derive_builder::Builder;

use crate::{HelpMetadata, InternalScopeModel, ScopeModel};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ReportDefinitionSpec {
    #[serde(default)]
    pub additional_data: BTreeMap<String, String>,
    pub template: String,
}

#[derive(Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, JsonSchema)]
pub enum ReportDefinitionKind {
    #[strum(serialize = "ScopeReportDefinition")]
    ScopeReportDefinition,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct V1AlphaReportDefinition {
    pub api_version: V1AlphaApiVersion,
    pub kind: ReportDefinitionKind,
    pub metadata: ModelMetadata,
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
