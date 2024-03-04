use crate::models::core::ModelMetadata;
use crate::models::v1alpha::V1AlphaApiVersion;
use crate::models::{HelpMetadata, InternalScopeModel, ScopeModel};
use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct KnownErrorSpec {
    pub help: String,
    pub pattern: String,
}

#[derive(Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, JsonSchema)]
pub enum KnownErrorKind {
    #[strum(serialize = "ScopeKnownError")]
    ScopeKnownError,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct V1AlphaKnownError {
    pub api_version: V1AlphaApiVersion,
    pub kind: KnownErrorKind,
    pub metadata: ModelMetadata,
    pub spec: KnownErrorSpec,
}

impl HelpMetadata for V1AlphaKnownError {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        format!("{}/{}", self.kind(), self.name())
    }
}

impl ScopeModel<KnownErrorSpec> for V1AlphaKnownError {
    fn api_version(&self) -> String {
        Self::int_api_version()
    }

    fn kind(&self) -> String {
        Self::int_kind()
    }

    fn spec(&self) -> &KnownErrorSpec {
        &self.spec
    }
}

impl InternalScopeModel<KnownErrorSpec, V1AlphaKnownError> for V1AlphaKnownError {
    fn int_api_version() -> String {
        V1AlphaApiVersion::ScopeV1Alpha.to_string()
    }

    fn int_kind() -> String {
        KnownErrorKind::ScopeKnownError.to_string()
    }

    #[cfg(test)]
    fn examples() -> Vec<String> {
        vec!["v1alpha/KnownError.yaml".to_string()]
    }
}
