use crate::models::prelude::{ModelMetadata, V1AlphaReportDefinition};
use crate::models::HelpMetadata;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Clone)]
pub struct ReportDefinition {
    pub full_name: String,
    pub metadata: ModelMetadata,
    pub additional_data: BTreeMap<String, String>,
    pub template: String,
}

impl HelpMetadata for ReportDefinition {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        self.full_name.to_string()
    }
}

impl TryFrom<V1AlphaReportDefinition> for ReportDefinition {
    type Error = anyhow::Error;

    fn try_from(value: V1AlphaReportDefinition) -> Result<Self, Self::Error> {
        Ok(ReportDefinition {
            full_name: value.full_name(),
            metadata: value.metadata,
            template: value.spec.template.trim().to_string(),
            additional_data: value.spec.additional_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::models::parse_models_from_string;
    use std::collections::BTreeMap;

    use std::path::Path;

    #[test]
    fn test_parse_scope_report_def() {
        let text = "
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportDefinition
metadata:
  name: report
spec:
  additionalData:
    env: env
  template: |
    hello bob
 ";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(1, configs.len());
        let model = configs[0].get_report_def_spec().unwrap();

        assert_eq!("ScopeReportDefinition/report", model.full_name);
        assert_eq!("report", model.metadata.name());
        assert_eq!(
            "/foo/bar/file.yaml",
            model.metadata.annotations.file_path.unwrap()
        );
        assert_eq!("hello bob", model.template);

        let additional_data: BTreeMap<String, String> =
            [("env".to_string(), "env".to_string())].into();
        assert_eq!(additional_data, model.additional_data);
    }
}
