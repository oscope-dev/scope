use crate::shared::models::internal::extract_command_path;
use derive_builder::Builder;
use dev_scope_model::prelude::{ModelMetadata, V1AlphaDoctorGroup};
use dev_scope_model::HelpMetadata;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorGroupAction {
    pub name: String,
    pub description: String,
    pub fix: DoctorGroupActionFix,
    pub check: DoctorGroupActionCheck,
    pub required: bool,
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorGroupActionFix {
    #[builder(default)]
    pub command: Option<DoctorGroupActionCommand>,
    #[builder(default)]
    pub help_text: Option<String>,
    #[builder(default)]
    pub help_url: Option<String>,
}

impl DoctorGroupAction {
    pub fn make_from(
        name: &str,
        description: &str,
        fix_command: Option<Vec<&str>>,
        check_path: Option<(&str, Vec<&str>)>,
        check_command: Option<Vec<&str>>,
    ) -> Self {
        Self {
            required: true,
            name: name.to_string(),
            description: description.to_string(),
            fix: DoctorGroupActionFix {
                command: fix_command.map(DoctorGroupActionCommand::from),
                help_text: None,
                help_url: None,
            },
            check: DoctorGroupActionCheck {
                command: check_command.map(DoctorGroupActionCommand::from),
                files: check_path.map(|(base, paths)| DoctorGroupCachePath {
                    base_path: PathBuf::from(base),
                    paths: crate::shared::convert_to_string(paths),
                }),
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorGroupCachePath {
    pub paths: Vec<String>,
    pub base_path: PathBuf,
}

impl From<(&str, Vec<&str>)> for DoctorGroupCachePath {
    fn from(value: (&str, Vec<&str>)) -> Self {
        let pb = PathBuf::from(value.0);
        let paths = crate::shared::convert_to_string(value.1);

        Self {
            paths,
            base_path: pb,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorGroupActionCheck {
    pub command: Option<DoctorGroupActionCommand>,
    pub files: Option<DoctorGroupCachePath>,
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorGroupActionCommand {
    pub commands: Vec<String>,
}

impl From<Vec<&str>> for DoctorGroupActionCommand {
    fn from(value: Vec<&str>) -> Self {
        let commands = value.iter().map(|x| x.to_string()).collect();
        Self { commands }
    }
}

impl<T> From<(&Path, Vec<T>)> for DoctorGroupActionCommand
where
    String: for<'a> From<&'a T>,
{
    fn from((base_path, command_strings): (&Path, Vec<T>)) -> Self {
        let commands = command_strings
            .iter()
            .map(|s| {
                let exec: String = s.into();
                extract_command_path(base_path, &exec)
            })
            .collect();

        DoctorGroupActionCommand { commands }
    }
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorGroup {
    pub full_name: String,
    pub metadata: ModelMetadata,
    pub requires: Vec<String>,
    pub actions: Vec<DoctorGroupAction>,
}

impl HelpMetadata for DoctorGroup {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        self.full_name.to_string()
    }
}

impl TryFrom<V1AlphaDoctorGroup> for DoctorGroup {
    type Error = anyhow::Error;

    fn try_from(model: V1AlphaDoctorGroup) -> Result<Self, Self::Error> {
        let binding = model.containing_dir();
        let containing_dir = Path::new(&binding);
        let mut actions: Vec<_> = Default::default();
        for (count, spec_action) in model.spec.actions.iter().enumerate() {
            let spec_action = spec_action.clone();
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
            full_name: model.full_name(),
            metadata: model.metadata,
            actions,
            requires: model.spec.needs,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::models::parse_models_from_string;
    use crate::shared::models::prelude::{
        DoctorGroupAction, DoctorGroupActionCheck, DoctorGroupActionCommand, DoctorGroupActionFix,
    };
    use crate::shared::prelude::DoctorGroupCachePath;

    use std::path::Path;

    #[test]
    fn parse_group_1() {
        let test_file = format!("{}/examples/group-1.yaml", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(test_file).unwrap();
        let path = Path::new("/foo/bar/.scope/file.yaml");
        let configs = parse_models_from_string(path, &text).unwrap();
        assert_eq!(1, configs.len());

        let dg = configs[0].get_doctor_group().unwrap();
        assert_eq!("foo", dg.metadata.name);
        assert_eq!(
            "/foo/bar/.scope/file.yaml",
            dg.metadata.annotations.file_path.unwrap()
        );
        assert_eq!("/foo/bar/.scope", dg.metadata.annotations.file_dir.unwrap());
        assert_eq!("ScopeDoctorGroup/foo", dg.full_name);
        assert_eq!(vec!["bar"], dg.requires);

        assert_eq!(
            dg.actions[0],
            DoctorGroupAction {
                name: "1".to_string(),
                required: false,
                description: "foo1".to_string(),
                fix: DoctorGroupActionFix {
                    command: Some(DoctorGroupActionCommand::from(vec![
                        "/foo/bar/.scope/fix1.sh"
                    ])),
                    help_text: Some("There is a good way to fix this, maybe...".to_string()),
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
            }
        );
        assert_eq!(
            dg.actions[1],
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
        );
    }
}
