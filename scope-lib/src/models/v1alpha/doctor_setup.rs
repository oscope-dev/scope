use crate::models::prelude::*;
use crate::models::v1alpha::extract_command_path;
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DoctorSetupSpecExecV1Alpha {
    Exec(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DoctorSetupSpecCacheV1Alpha {
    Paths(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DoctorSetupSpecV1Alpha {
    #[serde(with = "serde_yaml::with::singleton_map")]
    pub cache: DoctorSetupSpecCacheV1Alpha,
    #[serde(with = "serde_yaml::with::singleton_map")]
    pub setup: DoctorSetupSpecExecV1Alpha,
    pub description: String,
    pub help_text: String,
}

pub(super) fn parse(containing_dir: &Path, value: &Value) -> Result<DoctorSetupSpec> {
    let parsed: DoctorSetupSpecV1Alpha = serde_yaml::from_value(value.clone())?;

    let cache = match parsed.cache {
        DoctorSetupSpecCacheV1Alpha::Paths(paths) => {
            DoctorSetupSpecCache::Paths(DoctorSetupSpecCachePath {
                paths,
                base_path: containing_dir.parent().unwrap().to_path_buf(),
            })
        }
    };

    let exec = match parsed.setup {
        DoctorSetupSpecExecV1Alpha::Exec(commands) => DoctorSetupSpecExec::Exec(
            commands
                .iter()
                .map(|p| extract_command_path(containing_dir, &p))
                .collect(),
        ),
    };

    Ok(DoctorSetupSpec {
        cache,
        exec,
        help_text: parsed.help_text,
        description: parsed.description,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::{
        DoctorSetupSpec, DoctorSetupSpecCache, DoctorSetupSpecCachePath, DoctorSetupSpecExec,
    };
    use std::path::Path;

    #[test]
    fn test_parse_scope_setup() {
        let text = "
---
apiVersion: scope.github.com/v1alpha
kind: ScopeSetup
metadata:
  name: setup
spec:
  order: 100
  cache:
    paths:
     - foo/bar/**/*
  setup:
    exec:
      - bin/setup
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.
";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        assert_eq!(
            configs[0].get_doctor_setup_spec().unwrap(),
            DoctorSetupSpec {
                description: "Check your shell for basic functionality".to_string(),
                help_text: "You're shell does not have a path env. Reload your shell.".to_string(),
                exec: DoctorSetupSpecExec::Exec(vec!["/foo/bin/setup".to_string()]),
                cache: DoctorSetupSpecCache::Paths(DoctorSetupSpecCachePath {
                    paths: vec![],
                    base_path: Default::default(),
                }),
            }
        );
    }
}
