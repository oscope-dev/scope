use std::path::Path;

use crate::models::prelude::{ModelMetadata, V1AlphaKnownError};
use crate::models::HelpMetadata;
use derivative::Derivative;
use regex::Regex;

use super::fix::DoctorFix;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Clone)]
pub struct KnownError {
    pub full_name: String,
    pub metadata: ModelMetadata,
    pub pattern: String,
    #[derivative(PartialEq = "ignore")]
    pub regex: Regex,
    pub help_text: String,
    pub fix: Option<DoctorFix>,
}

impl HelpMetadata for KnownError {
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

        let binding = value.metadata.containing_dir();
        let containing_dir = Path::new(&binding);
        let working_dir = value
            .metadata
            .annotations
            .working_dir
            .as_ref()
            .unwrap()
            .clone();

        let maybe_fix = match value.spec.fix {
            Some(ref fix) => Some(DoctorFix::from_spec(
                containing_dir,
                &working_dir,
                fix.clone(),
            )?),
            None => None,
        };

        Ok(KnownError {
            full_name: value.full_name(),
            metadata: value.metadata,
            pattern: value.spec.pattern,
            regex,
            help_text: value.spec.help,
            fix: maybe_fix,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{
        DoctorFixSpec, KnownErrorKind, KnownErrorSpec, ModelMetadataAnnotations, V1AlphaApiVersion,
        V1AlphaKnownError,
    };
    use crate::shared::models::parse_models_from_string;

    use std::collections::BTreeMap;
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
        let work_dir = Path::new("/foo/bar");
        let configs = parse_models_from_string(work_dir, path, text).unwrap();
        assert_eq!(1, configs.len());
        let model = configs[0].get_known_error_spec().unwrap();

        assert_eq!("error-exists", model.metadata.name);
        assert_eq!("ScopeKnownError/error-exists", model.full_name);
        assert_eq!("The command had an error, try reading the logs around there to find out what happened.", model.help_text);
        assert_eq!("error", model.pattern);
    }

    #[test]
    fn try_from_spec() {
        let model_metadata = ModelMetadata {
            name: "some test error".to_string(),
            description: "some description".to_string(),
            annotations: ModelMetadataAnnotations {
                file_path: Some("/foo/bar/file.yaml".to_string()),
                file_dir: Some("/foo/bar".to_string()),
                working_dir: Some("/some/work/dir".to_string()),
                bin_path: None,
                extra: BTreeMap::new(),
            },
            labels: BTreeMap::new(),
        };

        let input = V1AlphaKnownError {
            api_version: V1AlphaApiVersion::ScopeV1Alpha,
            kind: KnownErrorKind::ScopeKnownError,
            metadata: model_metadata.clone(),
            spec: KnownErrorSpec {
                help: "some help text".to_string(),
                pattern: "some regex pattern".to_string(),
                fix: Some(DoctorFixSpec {
                    commands: vec!["echo 'fix it!'".to_string()],
                    help_text: None,
                    help_url: None,
                    prompt: None,
                }),
            },
        };

        let actual = KnownError::try_from(input.clone()).unwrap();

        assert_eq!(
            KnownError {
                full_name: "ScopeKnownError/some test error".to_string(),
                metadata: input.metadata,
                pattern: input.spec.pattern.clone(),
                regex: Regex::new(&input.spec.pattern).unwrap(),
                help_text: input.spec.help,
                fix: Some(
                    DoctorFix::from_spec(
                        Path::new(&model_metadata.containing_dir()),
                        &model_metadata.annotations.working_dir.unwrap(),
                        input.spec.fix.unwrap()
                    )
                    .unwrap()
                ),
            },
            actual
        )
    }
}
