use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Clone)]
pub struct ReportDefinitionSpec {
    pub additional_data: BTreeMap<String, String>,
    pub template: String,
}
