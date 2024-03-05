use crate::models::core::ModelMetadata;

use crate::models::v1alpha::V1AlphaApiVersion;
use crate::models::{HelpMetadata, InternalScopeModel, ScopeModel};
use derive_builder::Builder;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What needs to be checked before the action will run. All `paths` will be checked first, then
/// `commands`. If a `path` has changed, the `command` will not run.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct DoctorCheckSpec {
    #[serde(default)]
    /// A list of globs to check for changes. When the glob matches a new file, or the contents
    /// of the file change, the check will require a fix.
    pub paths: Option<Vec<String>>,


    #[serde(default)]
    /// A list of commands to execute to check the environment.
    pub commands: Option<Vec<String>>,
}

/// Definition for fixing the environment.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct DoctorFixSpec {
    #[serde(default)]
    /// List of commands to run to fix the env.
    pub commands: Vec<String>,

    #[serde(default)]
    /// Text to display when no command is provided / fails to fix the env.
    pub help_text: Option<String>,

    #[serde(default)]
    /// Link to documentation to fix the issue.
    pub help_url: Option<String>,
}

/// An action is a single step used to check in a group. This is most commonly used to build a
/// series of tasks for a system, like `ruby`, `python`, and databases.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct DoctorGroupActionSpec {
    /// Name of the "action". When not provided, it will be the index of the action within the group.
    /// This is used when reporting status to the users.
    pub name: Option<String>,

    /// A description of this specific action, used for information to the users.
    pub description: Option<String>,

    /// The `check` run before `fix` (if provided). A check is used to determine if the fix needs
    /// to be executed, or fail the action if no fix is provided. If a fix is specified, the check
    /// will re-execute to ensure that the fix applied correctly.
    pub check: DoctorCheckSpec,

    /// A fix defines how to fix the issue that a `check` is validating. When provided, will only
    /// run when the `check` "fails".
    pub fix: Option<DoctorFixSpec>,

    #[serde(default = "doctor_group_action_required_default")]
    /// If false, the action is allowed to fail and let other actions in the group execute. Defaults
    /// to `true`.
    pub required: bool,
}

fn doctor_group_action_required_default() -> bool {
    true
}

/// Often used to describe how to fix a "system", like `ruby`, `python`, or databases. Able to
/// depend on other "system".
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct DoctorGroupSpec {
    #[serde(default)]
    /// A list of `ScopeDoctorGroup` that are required for this group to execute. If not all finish
    /// successfully, this group will not execute.
    pub needs: Vec<String>,

    /// A series of steps to check and fix for the group.
    pub actions: Vec<DoctorGroupActionSpec>,
}

#[derive(Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, JsonSchema)]
pub enum DoctorGroupKind {
    #[strum(serialize = "ScopeDoctorGroup")]
    ScopeDoctorGroup,
}

/// Resource used to define a `ScopeDoctorGroup`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Builder, JsonSchema)]
#[builder(setter(into))]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct V1AlphaDoctorGroup {
    /// API version of the resource
    pub api_version: V1AlphaApiVersion,
    /// The type of resource.
    pub kind: DoctorGroupKind,
    /// Standard set of options including name, description for the resource.
    /// Together `kind` and `metadata.name` are required to be unique. If there are duplicate, the
    /// resources "closest" to the execution dir will take precedence.
    pub metadata: ModelMetadata,
    /// Options for the resource.
    pub spec: DoctorGroupSpec,
}

impl HelpMetadata for V1AlphaDoctorGroup {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        format!("{}/{}", self.kind(), self.name())
    }
}

impl ScopeModel<DoctorGroupSpec> for V1AlphaDoctorGroup {
    fn api_version(&self) -> String {
        Self::int_api_version()
    }

    fn kind(&self) -> String {
        Self::int_kind()
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
