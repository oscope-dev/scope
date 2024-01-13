use crate::models::prelude::KnownError;
use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct KnownErrorSpec {
    description: String,
    help: String,
    pattern: String,
}
pub(super) fn parse(value: &Value) -> Result<KnownError> {
    let parsed: KnownErrorSpec = serde_yaml::from_value(value.clone())?;
    let regex = Regex::new(&parsed.pattern)?;
    Ok(KnownError {
        pattern: parsed.pattern,
        regex,
        help_text: parsed.help,
        description: parsed.description,
    })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::KnownError;
    use regex::Regex;
    use std::path::Path;

    #[test]
    fn test_parse_scope_known_error() {
        let text = "apiVersion: scope.github.com/v1alpha
kind: ScopeKnownError
metadata:
  name: error-exists
spec:
  description: Check if the word error is in the logs
  pattern: error
  help: The command had an error, try reading the logs around there to find out what happened.";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        assert_eq!(configs[0].get_known_error_spec().unwrap(), KnownError {
            description: "Check if the word error is in the logs".to_string(),
            help_text: "The command had an error, try reading the logs around there to find out what happened.".to_string(),
            pattern: "error".to_string(),
            regex: Regex::new("error").unwrap()
        });
    }
}
