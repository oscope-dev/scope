use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::EnumString;

mod doctor_group;
mod known_error;
mod report_location;

#[derive(
    Serialize, Deserialize, Debug, strum::Display, Clone, PartialEq, EnumString, JsonSchema,
)]
/// Version of the Scope API
pub enum V1AlphaApiVersion {
    #[serde(rename = "scope.github.com/v1alpha")]
    #[strum(serialize = "scope.github.com/v1alpha")]
    /// Current latest version of the resources.
    ScopeV1Alpha,
}

pub mod prelude {
    pub use super::V1AlphaApiVersion;
    pub use super::doctor_group::*;
    pub use super::known_error::*;
    pub use super::report_location::*;
}
