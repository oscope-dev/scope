use crate::models::v1alpha::extract_command_path;
use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::models::prelude::{
    DoctorGroup, DoctorGroupAction, DoctorGroupActionCheck, DoctorGroupActionCommand,
};
use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DoctorCheckType {
    target: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DoctorCheckSpec {
    #[serde(default = "default_order")]
    order: i32,
    #[serde(with = "serde_yaml::with::singleton_map")]
    check: DoctorCheckType,
    #[serde(with = "serde_yaml::with::singleton_map", default)]
    fix: Option<DoctorCheckType>,
    description: String,
    help: String,
}

fn default_order() -> i32 {
    100
}

pub(super) fn parse(base_path: &Path, value: &Value) -> Result<DoctorGroup> {
    let parsed: DoctorCheckSpec = serde_yaml::from_value(value.clone())?;

    let check_path = extract_command_path(base_path, &parsed.check.target);
    let fix_exec = parsed.fix.map(|path| DoctorGroupActionCommand {
        commands: vec![extract_command_path(base_path, &path.target)],
    });

    Ok(DoctorGroup {
        actions: vec![DoctorGroupAction {
            name: "1".to_string(),
            required: true,
            description: parsed.description.clone(),
            fix: fix_exec,
            check: DoctorGroupActionCheck {
                command: Some(DoctorGroupActionCommand {
                    commands: vec![check_path],
                }),
                files: None,
            },
        }],
        description: parsed.description,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::{DoctorGroup, DoctorGroupAction};
    use std::path::Path;

    #[test]
    fn test_parse_scope_doctor_check_exec() {
        let text = "---
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorCheck
metadata:
  name: path-exists
spec:
  check:
    target: ./scripts/does-path-env-exist.sh
  fix:
    target: 'true'
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.
---
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorCheck
metadata:
  name: path-exists
spec:
  check:
    target: /scripts/does-path-env-exist.sh
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.
---
apiVersion: scope.github.com/v1alpha
kind: ScopeDoctorCheck
metadata:
  name: path-exists
spec:
  check:
    target: does-path-env-exist.sh
  description: Check your shell for basic functionality
  help: You're shell does not have a path env. Reload your shell.
";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(3, configs.len());
        assert_eq!(
            configs[0].get_doctor_group().unwrap(),
            DoctorGroup {
                description: "Check your shell for basic functionality".to_string(),
                actions: vec![DoctorGroupAction::make_from(
                    "1",
                    "Check your shell for basic functionality",
                    Some(vec!["true"]),
                    None,
                    Some(vec!["/foo/bar/scripts/does-path-env-exist.sh"])
                )]
            }
        );
        assert_eq!(
            configs[1].get_doctor_group().unwrap(),
            DoctorGroup {
                description: "Check your shell for basic functionality".to_string(),
                actions: vec![DoctorGroupAction::make_from(
                    "1",
                    "Check your shell for basic functionality",
                    None,
                    None,
                    Some(vec!["/scripts/does-path-env-exist.sh"])
                )]
            }
        );
        assert_eq!(
            configs[2].get_doctor_group().unwrap(),
            DoctorGroup {
                description: "Check your shell for basic functionality".to_string(),
                actions: vec![DoctorGroupAction::make_from(
                    "1",
                    "Check your shell for basic functionality",
                    None,
                    None,
                    Some(vec!["does-path-env-exist.sh"])
                )]
            }
        );
    }
}
