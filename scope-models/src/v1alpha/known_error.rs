use crate::core::ModelMetadata;
use crate::v1alpha::V1AlphaApiVersion;
use crate::InternalScopeModel;
use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnownErrorSpec {
    pub description: String,
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
pub struct V1AlphaKnownError {
    pub api_version: V1AlphaApiVersion,
    pub kind: KnownErrorKind,
    pub metadata: ModelMetadata,
    pub spec: KnownErrorSpec,
}

impl crate::ScopeModel<KnownErrorSpec> for V1AlphaKnownError {
    fn api_version(&self) -> String {
        Self::int_api_version()
    }

    fn kind(&self) -> String {
        Self::int_kind()
    }

    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
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
