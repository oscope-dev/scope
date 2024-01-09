use crate::models::prelude::*;
use crate::models::v1alpha::extract_command_path;
use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DoctorSetupSpecExec {
    Exec(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DoctorSetupSpecCache {
    Paths(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DoctorSetupSpec {
    #[serde(default = "default_order")]
    pub order: i32,
    #[serde(with = "serde_yaml::with::singleton_map")]
    pub cache: DoctorSetupSpecCache,
    #[serde(with = "serde_yaml::with::singleton_map")]
    pub setup: DoctorSetupSpecExec,
    pub description: String,
}

fn default_order() -> i32 {
    100
}

pub(super) fn parse(containing_dir: &Path, value: &Value) -> Result<DoctorSetup> {
    let parsed: DoctorSetupSpec = serde_yaml::from_value(value.clone())?;

    let cache = match parsed.cache {
        DoctorSetupSpecCache::Paths(paths) => DoctorSetupCache::Paths(DoctorSetupCachePath {
            paths,
            base_path: containing_dir.parent().unwrap().to_path_buf(),
        }),
    };

    let exec = match parsed.setup {
        DoctorSetupSpecExec::Exec(commands) => DoctorSetupExec::Exec(
            commands
                .iter()
                .map(|p| extract_command_path(containing_dir, p))
                .collect(),
        ),
    };

    Ok(DoctorSetup {
        order: parsed.order,
        cache,
        exec,
        description: parsed.description,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::{
        DoctorSetup, DoctorSetupCache, DoctorSetupCachePath, DoctorSetupExec,
    };
    use crate::models::v1alpha::doctor_setup::{
        DoctorSetupSpec, DoctorSetupSpecCache, DoctorSetupSpecExec,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn default_value() {
        let spec = DoctorSetupSpec {
            order: 100,
            cache: DoctorSetupSpecCache::Paths(vec!["foo".to_string()]),
            setup: DoctorSetupSpecExec::Exec(vec!["bar".to_string()]),
            description: "desc".to_string(),
        };

        let text = serde_yaml::to_string(&spec).unwrap();
        assert_eq!(
            "order: 100
cache:
  paths:
  - foo
setup:
  exec:
  - bar
description: desc\n",
            text
        );
    }

    #[test]
    fn test_parse_scope_setup() {
        let text = "
---
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorSetup
metadata:
  name: setup
spec:
  cache:
    paths:
     - flig/bar/**/*
  setup:
    exec:
      - bin/setup
  description: Check your shell for basic functionality
";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        assert_eq!(
            configs[0].get_doctor_setup_spec().unwrap(),
            DoctorSetup {
                order: 100,
                description: "Check your shell for basic functionality".to_string(),
                exec: DoctorSetupExec::Exec(vec!["/foo/bar/bin/setup".to_string()]),
                cache: DoctorSetupCache::Paths(DoctorSetupCachePath {
                    paths: vec!["flig/bar/**/*".to_string()],
                    base_path: PathBuf::from("/foo"),
                }),
            }
        );
    }
}
