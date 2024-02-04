use crate::shared::HelpMetadata;
use derivative::Derivative;
use regex::Regex;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Clone)]
pub struct KnownError {
    pub description: String,
    pub pattern: String,
    #[derivative(PartialEq = "ignore")]
    pub regex: Regex,
    pub help_text: String,
}

impl HelpMetadata for KnownError {
    fn description(&self) -> &str {
        &self.description
    }
}
