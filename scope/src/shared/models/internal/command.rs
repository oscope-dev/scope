use anyhow::Result;
use derive_builder::Builder;
use std::path::Path;

use super::{extract_command_path, substitute_templates};

#[derive(Debug, PartialEq, Clone, Builder)]
pub struct DoctorCommand {
    text: String,
}

impl DoctorCommand {
    pub fn try_new(containing_dir: &Path, working_dir: &str, command: &str) -> Result<Self> {
        let rendered_cmd = substitute_templates(working_dir, command)?;
        let qualified_cmd = extract_command_path(containing_dir, &rendered_cmd);
        Ok(DoctorCommand::from_str(&qualified_cmd))
    }

    /// Performs no template rendering, path qualification, or shell expansion.
    /// This is useful for when you want to use a command as-is, without any modifications
    ///
    // We don't want to implement FromStr for DoctorCommand because we're not parsing a string into an instance.
    // Calling this with cmd.parse() doesn't make sense here.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(cmd: &str) -> Self {
        DoctorCommand {
            text: cmd.to_string(),
        }
    }

    //TODO: I would prefer this to happen in the constructor
    /// splits a commands and performs shell expansion its parts
    pub fn expand(&self) -> String {
        Self::expand_command(&self.text)
    }

    // keeping this to make it easier to do in a constructor later
    fn expand_command(command: &str) -> String {
        command
            .split(' ')
            //consider doing a full expansion that includes env vars?
            .map(|word| shellexpand::tilde(word))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorCommands {
    pub commands: Vec<DoctorCommand>,
}

impl DoctorCommands {
    pub fn from_commands(
        containing_dir: &Path,
        working_dir: &str,
        // commands: &Vec<String>,
        commands: &[String],
    ) -> Result<DoctorCommands> {
        commands
            .iter()
            .map(|command| DoctorCommand::try_new(containing_dir, working_dir, command))
            .collect::<Result<Vec<_>>>()
            .map(|commands| DoctorCommands { commands })
    }

    /// Performs shell expansion
    pub fn expand(&self) -> Vec<String> {
        self.commands.iter().map(|cmd| cmd.expand()).collect()
    }
}

#[cfg(test)]
impl From<Vec<&str>> for DoctorCommands {
    /// This is only used by some tests and should NOT be used in production code
    /// because it does not properly pre-pend the command with a base path.
    fn from(value: Vec<&str>) -> Self {
        let commands = value
            .iter()
            .map(|cmd| DoctorCommand::from_str(cmd))
            .collect();
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
                commands: vec![
                    DoctorCommand::from_str(input[0]),
                    DoctorCommand::from_str(input[1]),
                ]
            },
            actual
        )
    }

    #[test]
    fn from_commands_inserts_execution_path() {
        let base_path = Path::new("/foo/bar");
        let input = vec!["echo 'foo'", "baz/qux", "./qux"];

        let actual = DoctorCommands::from_commands(
            base_path,
            "/some/working_dir",
            &input
                .iter()
                .map(|cmd| cmd.to_string())
                .collect::<Vec<String>>(),
        )
        .expect("Expected Ok");

        assert_eq!(
            DoctorCommands {
                commands: vec![
                    DoctorCommand::from_str("echo 'foo'"),
                    DoctorCommand::from_str("baz/qux"),
                    DoctorCommand::from_str("/foo/bar/qux"),
                ]
            },
            actual
        )
    }

    #[test]
    fn from_commands_inserts_working_dir() {
        let containing_dir = Path::new("/foo/bar");
        let working_dir = "/some/working_dir";
        let commands = vec!["{{ working_dir }}/foo.sh", "./bar.sh"]
            .iter()
            .map(|cmd| cmd.to_string())
            .collect::<Vec<String>>();

        let actual = DoctorCommands::from_commands(containing_dir, working_dir, &commands)
            .expect("Expected Ok");

        let expected = DoctorCommands {
            commands: vec![
                DoctorCommand::from_str("/some/working_dir/foo.sh"),
                DoctorCommand::from_str("/foo/bar/bar.sh"),
            ],
        };

        assert_eq!(expected, actual)
    }

    mod try_new_tests {
        use super::*;

        #[test]
        fn try_new_with_simple_command() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "echo hello";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            let expected = DoctorCommand::from_str("echo hello");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_working_dir_template() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "{{ working_dir }}/script.sh";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            let expected = DoctorCommand::from_str("/some/working_dir/script.sh");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_relative_path() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "./script.sh";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            let expected = DoctorCommand::from_str("/foo/bar/script.sh");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_relative_path_and_args() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "./script.sh --verbose";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            let expected = DoctorCommand::from_str("/foo/bar/script.sh --verbose");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_template_and_relative_path() {
            let containing_dir = Path::new("/project/root");
            let working_dir = "/build/dir";
            let command = "{{ working_dir }}/check.sh && ./validate.sh";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            // extract_command_path only processes the first command/token, 
            // not all relative paths in the entire command string
            // This is likely a BUG in the extract_command_path() implementation
            let expected = DoctorCommand::from_str("/build/dir/check.sh && ./validate.sh");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_absolute_path() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "/usr/bin/env python3 test.py";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            let expected = DoctorCommand::from_str("/usr/bin/env python3 test.py");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_unknown_template() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "{{ unknown_template }}/script.sh";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            // Unknown templates are erased (current behavior)
            // I can argue that this should error instead, but for now we document its behavior
            let expected = DoctorCommand::from_str("/script.sh");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_nested_relative_path() {
            let containing_dir = Path::new("/project/root");
            let working_dir = "/build/dir";
            let command = "./scripts/build.sh";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            let expected = DoctorCommand::from_str("/project/root/scripts/build.sh");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_empty_command() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            // Empty string split produces a single empty string, not an empty iterator
            // This case should probably return an error instead of an empty command
            // but for now we document its behavior
            let expected = DoctorCommand::from_str("");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_complex_command() {
            let containing_dir = Path::new("/project");
            let working_dir = "/work";
            let command = "cd {{ working_dir }} && ./build.sh && echo 'done'";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            // extract_command_path only processes the first command/token,
            // not all relative paths in the entire command string
            // This is likely a BUG in the extract_command_path() implementation
            let expected = DoctorCommand::from_str("cd /work && ./build.sh && echo 'done'");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_multiple_templates() {
            let containing_dir = Path::new("/project");
            let working_dir = "/build";
            let command = "{{ working_dir }}/setup.sh && {{ working_dir }}/build.sh";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            // Multiple templates should all be substituted
            let expected = DoctorCommand::from_str("/build/setup.sh && /build/build.sh");
            assert_eq!(expected, actual);
        }

        #[test]
        fn try_new_with_whitespace_only_command() {
            let containing_dir = Path::new("/foo/bar");
            let working_dir = "/some/working_dir";
            let command = "   ";

            let actual = DoctorCommand::try_new(containing_dir, working_dir, command)
                .expect("Expected Ok");

            // Whitespace should be preserved
            let expected = DoctorCommand::from_str("   ");
            assert_eq!(expected, actual);
        }
    }
}
