use std::path::Path;

use derive_builder::Builder;

use super::extract_command_path;

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorCommand {
    pub commands: Vec<String>,
}

impl<T> From<(&Path, Vec<T>)> for DoctorCommand
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

        DoctorCommand { commands }
    }
}

#[cfg(test)]
impl From<Vec<&str>> for DoctorCommand {
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

        let actual = DoctorCommand::from(input.clone());

        assert_eq!(
            DoctorCommand {
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
            DoctorCommand::from((base_path, input.iter().map(|cmd| cmd.to_string()).collect()));

        assert_eq!(
            DoctorCommand {
                commands: vec![
                    "echo 'foo'".to_string(),
                    "baz/qux".to_string(),
                    "/foo/bar/qux".to_string(),
                ]
            },
            actual
        )
    }
}
