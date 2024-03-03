use crate::shared::models::internal::ParsedConfig;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::path::Path;

use derive_builder::Builder;
use dev_scope_model::prelude::ModelRoot;
use strum::EnumString;

mod internal;

pub mod prelude {
    pub use super::internal::prelude::*;
}

#[cfg(test)]
pub(crate) fn parse_models_from_string(
    file_path: &std::path::Path,
    input: &str,
) -> anyhow::Result<Vec<ParsedConfig>> {
    use serde_yaml::Deserializer;

    let mut models = Vec::new();
    for doc in Deserializer::from_str(input) {
        if let Some(parsed_model) = crate::shared::config_load::parse_model(doc, file_path) {
            models.push(parsed_model.try_into()?)
        }
    }

    Ok(models)
}
