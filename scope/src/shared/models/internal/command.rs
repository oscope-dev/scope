use anyhow::Result;
use derive_builder::Builder;
use std::path::Path;

use super::{extract_command_path, substitute_templates};

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorCommands {
    pub commands: Vec<String>,
}

impl DoctorCommands {
    pub fn from_commands(
        containing_dir: &Path,
        working_dir: &str,
        commands: &Vec<String>,
    ) -> Result<DoctorCommands> {
        let mut templated_commands = Vec::new();
        for command in commands {
            templated_commands.push(substitute_templates(working_dir, command)?);
        }
        Ok(DoctorCommands::from((containing_dir, templated_commands)))
    }

    /// Performs shell expansion
    pub fn expand(&self) -> Vec<String> {
        self.commands
            .iter()
            .map(|cmd| Self::expand_command(cmd))
            .collect()
    }

    /// splits a commands and performs shell expansion its parts
    fn expand_command(command: &str) -> String {
        command
            .split(' ')
            //consider doing a full expansion that includes env vars?
            .map(|word| shellexpand::tilde(word))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl<T> From<(&Path, Vec<T>)> for DoctorCommands
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

        DoctorCommands { commands }
    }
}

#[cfg(test)]
impl From<Vec<&str>> for DoctorCommands {
    /// This is only used by some tests and should NOT be used in production code
    /// because it does not properly pre-pend the command with a base path.
    fn from(value: Vec<&str>) -> Self {
        let commands = value.iter().map(|x| x.to_string()).collect();
        Self { commands }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_vec_str() {
        let input = vec!["echo 'foo'", "false"];

        let actual = DoctorCommands::from(input.clone());

        assert_eq!(
            DoctorCommands {
                commands: vec![input[0].to_string(), input[1].to_string(),]
            },
            actual
        )
    }

    #[test]
    fn from_path_and_vec_string() {
        let base_path = Path::new("/foo/bar");
        let input = vec!["echo 'foo'", "baz/qux", "./qux"];

        let actual =
            DoctorCommands::from((base_path, input.iter().map(|cmd| cmd.to_string()).collect()));

        assert_eq!(
            DoctorCommands {
                commands: vec![
                    "echo 'foo'".to_string(),
                    "baz/qux".to_string(),
                    "/foo/bar/qux".to_string(),
                ]
            },
            actual
        )
    }

    #[test]
    fn from_commands_succeeds() {
        let containing_dir = Path::new("/foo/bar");
        let working_dir = "/some/working_dir";
        let commands = vec!["{{ working_dir }}/foo.sh", "./bar.sh"]
            .iter()
            .map(|cmd| cmd.to_string())
            .collect::<Vec<String>>();

        let actual = DoctorCommands::from_commands(containing_dir, working_dir, &commands)
            .expect("Expected Ok");

        let templated_commands = commands
            .iter()
            .map(|cmd| substitute_templates(&working_dir, &cmd).unwrap())
            .collect::<Vec<String>>();

        let expected = DoctorCommands::from((containing_dir, templated_commands));

        assert_eq!(expected, actual)
    }
}
