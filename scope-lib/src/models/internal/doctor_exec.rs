use crate::HelpMetadata;

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorExec {
    pub order: i32,
    pub check_exec: String,
    pub fix_exec: Option<String>,
    pub description: String,
    pub help_text: String,
}

impl HelpMetadata for DoctorExec {
    fn description(&self) -> &str {
        &self.description
    }
}
