use crate::core::ModelMetadata;

use crate::v1alpha::V1AlphaApiVersion;
use crate::InternalScopeModel;
use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheckSpec {
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default)]
    pub commands: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorFixSpec {
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub help_text: Option<String>,
    #[serde(default)]
    pub help_url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorGroupActionSpec {
    pub name: Option<String>,
    pub description: Option<String>,
    pub check: DoctorCheckSpec,
    pub fix: Option<DoctorFixSpec>,
    #[serde(default = "doctor_group_action_required_default")]
    pub required: bool,
}

fn doctor_group_action_required_default() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorGroupSpec {
    #[serde(default)]
    pub needs: Vec<String>,
    pub description: Option<String>,
    pub actions: Vec<DoctorGroupActionSpec>,
}

#[derive(Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, JsonSchema)]
pub enum DoctorGroupKind {
    #[strum(serialize = "ScopeDoctorGroup")]
    ScopeDoctorGroup,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
pub struct V1AlphaDoctorGroup {
    pub api_version: V1AlphaApiVersion,
    pub kind: DoctorGroupKind,
    pub metadata: ModelMetadata,
    pub spec: DoctorGroupSpec,
}

impl crate::ScopeModel<DoctorGroupSpec> for V1AlphaDoctorGroup {
    fn api_version(&self) -> String {
        Self::int_api_version()
    }

    fn kind(&self) -> String {
        Self::int_kind()
    }

    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn spec(&self) -> &DoctorGroupSpec {
        &self.spec
    }
}

impl InternalScopeModel<DoctorGroupSpec, V1AlphaDoctorGroup> for V1AlphaDoctorGroup {
    fn int_api_version() -> String {
        V1AlphaApiVersion::ScopeV1Alpha.to_string()
    }

    fn int_kind() -> String {
        DoctorGroupKind::ScopeDoctorGroup.to_string()
    }

    #[cfg(test)]
    fn examples() -> Vec<String> {
        vec!["v1alpha/DoctorGroup.yaml".to_string()]
    }
}
