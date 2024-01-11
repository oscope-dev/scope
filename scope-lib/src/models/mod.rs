use crate::models::internal::ParsedConfig;
use crate::{FILE_DIR_ANNOTATION, FILE_PATH_ANNOTATION};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;

use strum::EnumString;

mod internal;
mod v1alpha;

pub mod prelude {
    pub use super::internal::prelude::*;
    pub use super::ScopeModel;
    pub use super::{ModelMetadata, ModelRoot};
}

#[derive(Debug, PartialEq, EnumString)]
#[strum(ascii_case_insensitive)]
pub enum KnownApiVersion {
    #[strum(serialize = "scope.github.com/v1alpha")]
    ScopeV1Alpha,
    #[strum(default)]
    UnknownApiVersion(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct ModelMetadata {
    pub name: String,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
}

impl ModelMetadata {
    fn file_path(&self) -> String {
        self.annotations
            .get(FILE_PATH_ANNOTATION)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }

    fn containing_dir(&self) -> String {
        self.annotations
            .get(FILE_DIR_ANNOTATION)
            .cloned()
            .unwrap_or_else(|| "unknown".to_string())
    }
}

pub trait ScopeModel {
    fn name(&self) -> &str;
    fn kind(&self) -> &str;
    fn full_name(&self) -> String {
        format!("{}/{}", self.kind(), self.name())
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoot<V> {
    pub api_version: String,
    pub kind: String,
    pub metadata: ModelMetadata,
    pub spec: V,
}

impl<V> ModelRoot<V> {
    pub fn with_spec<T>(&self, spec: T) -> ModelRoot<T> {
        ModelRoot {
            api_version: self.api_version.clone(),
            kind: self.kind.clone(),
            metadata: self.metadata.clone(),
            spec,
        }
    }

    pub fn file_path(&self) -> String {
        self.metadata.file_path()
    }
    pub fn containing_dir(&self) -> String {
        self.metadata.containing_dir()
    }
}

impl<V> ScopeModel for ModelRoot<V> {
    fn name(&self) -> &str {
        &self.metadata.name
    }

    fn kind(&self) -> &str {
        &self.kind
    }
}

impl TryFrom<&ModelRoot<Value>> for ParsedConfig {
    type Error = anyhow::Error;

    fn try_from(root: &ModelRoot<Value>) -> Result<Self, Self::Error> {
        let api_version: &str = &root.api_version.trim().to_ascii_lowercase();
        let api_versions = KnownApiVersion::try_from(api_version)
            .unwrap_or_else(|_| KnownApiVersion::UnknownApiVersion(api_version.to_string()));

        match api_versions {
            KnownApiVersion::ScopeV1Alpha => Ok(v1alpha::parse_v1_alpha1(root)?),
            KnownApiVersion::UnknownApiVersion(_) => {
                Err(anyhow!("Unable to parse {}", api_version))
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn parse_models_from_string(
    file_path: &std::path::Path,
    input: &str,
) -> anyhow::Result<Vec<ParsedConfig>> {
    use crate::config_load::parse_model;
    use serde_yaml::Deserializer;

    let mut models = Vec::new();
    for doc in Deserializer::from_str(input) {
        if let Some(parsed_model) = parse_model(doc, file_path) {
            models.push(parsed_model.try_into()?)
        }
    }

    Ok(models)
}
