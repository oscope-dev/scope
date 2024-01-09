use crate::models::v1alpha::extract_command_path;
use crate::prelude::DoctorExec;
use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use std::path::Path;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DoctorCheckType {
    target: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DoctorCheckSpec {
    #[serde(with = "serde_yaml::with::singleton_map")]
    check: DoctorCheckType,
    #[serde(with = "serde_yaml::with::singleton_map", default)]
    fix: Option<DoctorCheckType>,
    description: String,
    help: String,
}

pub(super) fn parse(base_path: &Path, value: &Value) -> Result<DoctorExec> {
    let parsed: DoctorCheckSpec = serde_yaml::from_value(value.clone())?;

    let check_path = extract_command_path(base_path, &parsed.check.target);
    let fix_exec = parsed
        .fix
        .map(|path| extract_command_path(base_path, &path.target));

    Ok(DoctorExec {
        help_text: parsed.help,
        check_exec: check_path,
        fix_exec,
        description: parsed.description,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::prelude::DoctorExec;
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
    target: scripts/does-path-env-exist.sh
  fix:
    target: /bin/true
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
";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(2, configs.len());
        assert_eq!(
            configs[0].get_doctor_check_spec().unwrap(),
            DoctorExec {
                description: "Check your shell for basic functionality".to_string(),
                help_text: "You're shell does not have a path env. Reload your shell.".to_string(),
                check_exec: "/foo/bar/scripts/does-path-env-exist.sh".to_string(),
                fix_exec: Some("/bin/true".to_string())
            }
        );
        assert_eq!(
            configs[1].get_doctor_check_spec().unwrap(),
            DoctorExec {
                description: "Check your shell for basic functionality".to_string(),
                help_text: "You're shell does not have a path env. Reload your shell.".to_string(),
                check_exec: "/scripts/does-path-env-exist.sh".to_string(),
                fix_exec: None,
            }
        );
    }
}
