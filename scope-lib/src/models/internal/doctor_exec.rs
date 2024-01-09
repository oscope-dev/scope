use crate::HelpMetadata;

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorExecCheckSpec {
    pub check_exec: String,
    pub fix_exec: Option<String>,
    pub description: String,
    pub help_text: String,
}

impl HelpMetadata for DoctorExecCheckSpec {
    fn description(&self) -> &str {
        &self.description
    }
}
