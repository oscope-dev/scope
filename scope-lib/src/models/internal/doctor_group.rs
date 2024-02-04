use crate::{models::v1alpha::extract_command_path, HelpMetadata};
use derive_builder::Builder;
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
                    paths: crate::convert_to_string(paths),
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
        let paths = crate::convert_to_string(value.1);

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
    pub requires: Vec<String>,
    pub description: String,
    pub actions: Vec<DoctorGroupAction>,
}

impl HelpMetadata for DoctorGroup {
    fn description(&self) -> &str {
        &self.description
    }
}
