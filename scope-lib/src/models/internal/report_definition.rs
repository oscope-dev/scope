use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Clone)]
pub struct ReportDefinition {
    pub additional_data: BTreeMap<String, String>,
    pub template: String,
}
