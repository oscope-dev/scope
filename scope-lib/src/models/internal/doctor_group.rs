use crate::HelpMetadata;
use std::path::PathBuf;

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorGroupAction {
    pub description: String,
    pub fix: Option<DoctorGroupActionCommand>,
    pub check: DoctorGroupActionCheck,
}

impl DoctorGroupAction {
    pub fn make_from(
        description: &str,
        fix_command: Option<Vec<&str>>,
        check_path: Option<(&str, Vec<&str>)>,
        check_command: Option<Vec<&str>>,
    ) -> Self {
        Self {
            description: description.to_string(),
            fix: fix_command.map(|commands| DoctorGroupActionCommand {
                commands: commands.iter().map(|x| x.to_string()).collect(),
            }),
            check: DoctorGroupActionCheck {
                command: check_command.map(|commands| DoctorGroupActionCommand {
                    commands: commands.iter().map(|x| x.to_string()).collect(),
                }),
                files: check_path.map(|(base, paths)| DoctorGroupCachePath {
                    base_path: PathBuf::from(base),
                    paths: paths.iter().map(|x| x.to_string()).collect(),
                }),
            },
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorGroupCachePath {
    pub paths: Vec<String>,
    pub base_path: PathBuf,
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorGroupActionCheck {
    pub command: Option<DoctorGroupActionCommand>,
    pub files: Option<DoctorGroupCachePath>,
}

impl From<(&str, Vec<&str>)> for DoctorGroupCachePath {
    fn from(value: (&str, Vec<&str>)) -> Self {
        let pb = PathBuf::from(value.0);
        let paths = value.1.iter().map(|x| x.to_string()).collect();

        Self {
            paths,
            base_path: pb,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorGroupActionCommand {
    pub commands: Vec<String>,
}

impl From<Vec<&str>> for DoctorGroupActionCommand {
    fn from(value: Vec<&str>) -> Self {
        let commands = value.iter().map(|x| x.to_string()).collect();
        Self { commands }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorGroup {
    pub description: String,
    pub actions: Vec<DoctorGroupAction>,
}

impl HelpMetadata for DoctorGroup {
    fn description(&self) -> &str {
        &self.description
    }
}
