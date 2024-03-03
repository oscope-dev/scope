use std::collections::BTreeMap;
use dev_scope_model::prelude::{ModelMetadata, V1AlphaReportDefinition};

#[derive(Debug, PartialEq, Clone)]
pub struct ReportDefinition {
    pub metadata: ModelMetadata,
    pub additional_data: BTreeMap<String, String>,
    pub template: String,
}

impl TryFrom<V1AlphaReportDefinition> for ReportDefinition {
    type Error = anyhow::Error;

    fn try_from(value: V1AlphaReportDefinition) -> Result<Self, Self::Error> {
        Ok(ReportDefinition {
            metadata: value.metadata,
            template: value.spec.template.trim().to_string(),
            additional_data: value.spec.additional_data,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::shared::models::parse_models_from_string;
    use crate::shared::models::prelude::ReportDefinition;
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

        assert_eq!(
            configs[0].get_report_def_spec().unwrap(),
            ReportDefinition {
                template: "hello bob".to_string(),
                additional_data: [("env".to_string(), "env".to_string())].into()
            }
        );
    }
}