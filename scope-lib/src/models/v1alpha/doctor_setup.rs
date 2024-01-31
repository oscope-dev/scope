use crate::models::prelude::*;
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

pub(super) fn parse(containing_dir: &Path, value: &Value) -> Result<DoctorGroup> {
    let parsed: DoctorSetupSpec = serde_yaml::from_value(value.clone())?;

    let cache = match parsed.cache {
        DoctorSetupSpecCache::Paths(paths) => DoctorGroupCachePath {
            paths,
            base_path: containing_dir.parent().unwrap().to_path_buf(),
        },
    };

    let exec = match parsed.setup {
        DoctorSetupSpecExec::Exec(commands) => {
            DoctorGroupActionCommand::from((containing_dir, commands))
        }
    };

    Ok(DoctorGroup {
        actions: vec![DoctorGroupAction {
            name: "1".to_string(),
            required: true,
            description: parsed.description.clone(),
            fix: DoctorGroupActionFix {
                command: Some(exec),
                help_text: None,
                help_url: None,
            },
            check: DoctorGroupActionCheck {
                command: None,
                files: Some(cache),
            },
        }],
        description: parsed.description,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::{
        DoctorGroup, DoctorGroupAction, DoctorGroupActionCheck, DoctorGroupActionCommand,
        DoctorGroupActionFix, DoctorGroupCachePath,
    };
    use std::path::Path;

    #[test]
    fn test_parse_scope_setup() {
        let text = include_str!("examples/setup-1.yaml");
        let path = Path::new("/foo/bar/.scope/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        assert_eq!(
            configs[0].get_doctor_group().unwrap(),
            DoctorGroup {
                description: "Check your shell for basic functionality".to_string(),
                actions: vec![DoctorGroupAction {
                    name: "1".to_string(),
                    required: true,
                    description: "Check your shell for basic functionality".to_string(),
                    fix: DoctorGroupActionFix {
                        command: Some(DoctorGroupActionCommand::from(vec![
                            "/foo/bar/.scope/bin/setup"
                        ])),
                        help_text: None,
                        help_url: None,
                    },
                    check: DoctorGroupActionCheck {
                        command: None,
                        files: Some(DoctorGroupCachePath::from((
                            "/foo/bar",
                            vec!["flig/bar/**/*"]
                        )))
                    }
                }]
            }
        );
    }

    #[test]
    fn test_parse_command_without_rel_path() {
        let text = include_str!("examples/setup-2.yaml");
        let path = Path::new("/foo/bar/.scope/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        assert_eq!(
            configs[0].get_doctor_group().unwrap(),
            DoctorGroup {
                description: "Check your shell for basic functionality".to_string(),
                actions: vec![DoctorGroupAction {
                    name: "1".to_string(),
                    required: true,
                    description: "Check your shell for basic functionality".to_string(),
                    fix: DoctorGroupActionFix {
                        command: Some(DoctorGroupActionCommand::from(vec!["sleep infinity"])),
                        help_text: None,
                        help_url: None,
                    },
                    check: DoctorGroupActionCheck {
                        command: None,
                        files: Some(DoctorGroupCachePath::from((
                            "/foo/bar",
                            vec!["flig/bar/**/*"]
                        )))
                    }
                }]
            }
        );
    }
}
