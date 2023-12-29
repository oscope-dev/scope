use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ModelMetadata {
    pub name: String,
    #[serde(default)]
    pub annotations: BTreeMap<String, String>,
    #[serde(default)]
    pub labels: BTreeMap<String, String>,
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
}

#[derive(Debug, PartialEq, Clone)]
pub struct ExecCheck {
    pub target: PathBuf,
    pub description: String,
    pub help_text: String,
}

#[derive(Debug, PartialEq)]
pub enum ParsedConfig {
    DoctorExec(ModelRoot<ExecCheck>),
}

pub fn parse_config(base_path: &Path, config: &str) -> Result<Vec<ParsedConfig>> {
    let parsed: Value = serde_yaml::from_str(config)?;
    let values = match parsed {
        Value::Sequence(arr) => {
            let mut result = Vec::new();
            for item in arr {
                result.push(parse_value(base_path, item)?);
            }
            result
        }
        Value::Mapping(_) => vec![parse_value(base_path, parsed)?],
        _ => return Err(anyhow!("Input file wasn't an array or an object")),
    };
    Ok(values)
}

fn parse_value(base_path: &Path, value: Value) -> Result<ParsedConfig> {
    let root: ModelRoot<Value> = serde_yaml::from_value(value)?;
    let api_version: &str = &root.api_version;
    let kind: &str = &root.kind;

    let parsed = match (api_version, kind) {
        ("pity.github.com/v1alpha", "ExecCheck") => {
            let exec_check = parser::parse_v1_exec_check(base_path, &root.spec)?;
            ParsedConfig::DoctorExec(root.with_spec(exec_check))
        }
        (version, kind) => return Err(anyhow!("Unable to parse {}/{}", version, kind)),
    };

    Ok(parsed)
}

mod parser {
    use serde::{Deserialize, Serialize};
    use serde_yaml::Value;
    use std::path::Path;

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    struct ExecCheckV1Alpha {
        target: String,
        description: String,
        help: String,
    }

    pub(super) fn parse_v1_exec_check(
        base_path: &Path,
        value: &Value,
    ) -> anyhow::Result<super::ExecCheck> {
        let parsed: ExecCheckV1Alpha = serde_yaml::from_value(value.clone())?;
        Ok(super::ExecCheck {
            help_text: parsed.help,
            target: base_path.join(parsed.target),
            description: parsed.description,
        })
    }
}

#[test]
fn test_parse() {
    let text = "apiVersion: pity.github.com/v1alpha
kind: ExecCheck
metadata:
  name: path-exists
spec:
  target: scripts/does-path-env-exist.sh
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.";

    let path = Path::new("/foo/bar");
    let configs = parse_config(path, text).unwrap();
    assert_eq!(1, configs.len());
    let doctor_exec = &configs[0];
    let ParsedConfig::DoctorExec(model) = doctor_exec;
    assert_eq!(
        ExecCheck {
            description: "Check your shell for basic functionality".to_string(),
            help_text: "You're shell does not have a path env. Reload your shell.".to_string(),
            target: path.join("scripts/does-path-env-exist.sh"),
        },
        model.spec
    );
}
