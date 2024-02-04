use crate::shared::models::prelude::ReportDefinition;
use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ReportDefinitionSpec {
    #[serde(default)]
    additional_data: BTreeMap<String, String>,
    template: String,
}
pub(super) fn parse(value: &Value) -> Result<ReportDefinition> {
    let parsed: ReportDefinitionSpec = serde_yaml::from_value(value.clone())?;

    Ok(ReportDefinition {
        template: parsed.template.trim().to_string(),
        additional_data: parsed.additional_data,
    })
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
