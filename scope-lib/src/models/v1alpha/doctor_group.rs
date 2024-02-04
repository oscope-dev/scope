use crate::models::prelude::*;
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
    #[serde(default)]
    pub help_text: Option<String>,
    #[serde(default)]
    pub help_url: Option<String>,
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
    #[serde(default)]
    pub needs: Vec<String>,
    pub description: Option<String>,
    pub actions: Vec<DoctorGroupActionSpec>,
}

pub(super) fn parse(containing_dir: &Path, value: &Value) -> Result<DoctorGroup> {
    let parsed: DoctorGroupSpec = serde_yaml::from_value(value.clone())?;

    let mut actions: Vec<_> = Default::default();
    for (count, spec_action) in parsed.actions.into_iter().enumerate() {
        let help_text = spec_action
            .fix
            .as_ref()
            .and_then(|x| x.help_text.as_ref().map(|st| st.trim().to_string()).clone());
        let help_url = spec_action.fix.as_ref().and_then(|x| x.help_url.clone());
        let fix_command = spec_action.fix.as_ref().map(|commands| {
            DoctorGroupActionCommand::from((containing_dir, commands.commands.clone()))
        });

        actions.push(DoctorGroupAction {
            name: spec_action.name.unwrap_or_else(|| format!("{}", count + 1)),
            required: spec_action.required,
            description: spec_action
                .description
                .unwrap_or_else(|| "default".to_string()),
            fix: DoctorGroupActionFix {
                command: fix_command,
                help_text,
                help_url,
            },
            check: DoctorGroupActionCheck {
                command: spec_action
                    .check
                    .commands
                    .map(|commands| DoctorGroupActionCommand::from((containing_dir, commands))),
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
        requires: parsed.needs,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::{
        DoctorGroup, DoctorGroupAction, DoctorGroupActionCheck, DoctorGroupActionCommand,
        DoctorGroupActionFix,
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
                requires: vec!["bar".to_string()],
                description: "Check your shell for basic functionality".to_string(),
                actions: vec![
                    DoctorGroupAction {
                        name: "1".to_string(),
                        required: false,
                        description: "foo1".to_string(),
                        fix: DoctorGroupActionFix {
                            command: Some(DoctorGroupActionCommand::from(vec![
                                "/foo/bar/.scope/fix1.sh"
                            ])),
                            help_text: Some(
                                "There is a good way to fix this, maybe...".to_string()
                            ),
                            help_url: Some("https://go.example.com/fixit".to_string()),
                        },
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
                        fix: DoctorGroupActionFix {
                            command: None,
                            help_text: None,
                            help_url: None,
                        },
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
