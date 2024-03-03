use crate::shared::HelpMetadata;
use derivative::Derivative;
use dev_scope_model::prelude::{ModelMetadata, V1AlphaKnownError};
use dev_scope_model::ScopeModel;
use regex::Regex;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Clone)]
pub struct KnownError {
    pub full_name: String,
    pub metadata: ModelMetadata,
    pub description: String,
    pub pattern: String,
    #[derivative(PartialEq = "ignore")]
    pub regex: Regex,
    pub help_text: String,
}

impl HelpMetadata for KnownError {
    fn description(&self) -> String {
        self.description.to_string()
    }
    fn name(&self) -> String {
        self.metadata.name.to_string()
    }

    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        self.full_name.to_string()
    }
}

impl TryFrom<V1AlphaKnownError> for KnownError {
    type Error = anyhow::Error;

    fn try_from(value: V1AlphaKnownError) -> Result<Self, Self::Error> {
        let regex = Regex::new(&value.spec.pattern)?;
        Ok(KnownError {
            full_name: value.full_name(),
            metadata: value.metadata,
            pattern: value.spec.pattern,
            regex,
            help_text: value.spec.help,
            description: value.spec.description,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::models::parse_models_from_string;
    use crate::shared::models::prelude::KnownError;
    use dev_scope_model::prelude::ModelMetadata;
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
        let model = configs[0].get_known_error_spec().unwrap();

        assert_eq!("error-exists", model.metadata.name);
        assert_eq!("ScopeKnownError/error-exists", model.full_name);
        assert_eq!("Check if the word error is in the logs", model.description);
        assert_eq!("The command had an error, try reading the logs around there to find out what happened.", model.help_text);
        assert_eq!("error", model.pattern);
    }
}
