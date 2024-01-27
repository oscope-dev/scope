use crate::models::prelude::*;
use crate::models::v1alpha::extract_command_path;
use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheckSpec {
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default)]
    pub commands: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DoctorFixSpec {
    #[serde(default)]
    pub commands: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DoctorGroupSpec {
    pub description: Option<String>,
    pub actions: Vec<DoctorGroupActionSpec>,
}

pub(super) fn parse(containing_dir: &Path, value: &Value) -> Result<DoctorGroup> {
    let parsed: DoctorGroupSpec = serde_yaml::from_value(value.clone())?;

    let mut actions: Vec<_> = Default::default();
    let mut count = 0;
    for spec_action in parsed.actions {
        count += 1;
        actions.push(DoctorGroupAction {
            name: spec_action.name.unwrap_or_else(|| format!("{}", count)),
            required: spec_action.required,
            description: spec_action
                .description
                .unwrap_or_else(|| "default".to_string()),
            fix: spec_action.fix.map(|commands| DoctorGroupActionCommand {
                commands: commands
                    .commands
                    .iter()
                    .map(|s| extract_command_path(containing_dir, s))
                    .collect(),
            }),
            check: DoctorGroupActionCheck {
                command: spec_action
                    .check
                    .commands
                    .map(|commands| DoctorGroupActionCommand {
                        commands: commands
                            .iter()
                            .map(|s| extract_command_path(containing_dir, s))
                            .collect(),
                    }),
                files: spec_action.check.paths.map(|paths| DoctorGroupCachePath {
                    paths,
                    base_path: containing_dir.parent().unwrap().to_path_buf(),
                }),
            },
        })
    }

    Ok(DoctorGroup {
        description: parsed.description.unwrap_or_else(|| "default".to_string()),
        actions,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::{
        DoctorGroup, DoctorGroupAction, DoctorGroupActionCheck, DoctorGroupActionCommand,
    };
    use crate::prelude::DoctorGroupCachePath;
    use std::path::Path;

    #[test]
    fn parse_group_1() {
        let text = include_str!("examples/group-1.yaml");
        let path = Path::new("/foo/bar/.scope/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        assert_eq!(
            configs[0].get_doctor_group().unwrap(),
            DoctorGroup {
                description: "Check your shell for basic functionality".to_string(),
                actions: vec![
                    DoctorGroupAction {
                        name: "1".to_string(),
                        required: false,
                        description: "foo1".to_string(),
                        fix: Some(DoctorGroupActionCommand::from(vec![
                            "/foo/bar/.scope/fix1.sh"
                        ])),
                        check: DoctorGroupActionCheck {
                            command: Some(DoctorGroupActionCommand::from(vec![
                                "/foo/bar/.scope/foo1.sh"
                            ])),
                            files: Some(DoctorGroupCachePath::from((
                                "/foo/bar",
                                vec!["flig/bar/**/*"]
                            )))
                        }
                    },
                    DoctorGroupAction {
                        name: "2".to_string(),
                        required: true,
                        description: "foo2".to_string(),
                        fix: None,
                        check: DoctorGroupActionCheck {
                            command: Some(DoctorGroupActionCommand::from(vec!["sleep infinity"])),
                            files: Some(DoctorGroupCachePath::from(("/foo/bar", vec!["*/*.txt"])))
                        }
                    }
                ]
            }
        );
    }
}
