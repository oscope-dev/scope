use crate::HelpMetadata;
use derivative::Derivative;
use regex::Regex;

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Clone)]
pub struct KnownErrorSpec {
    pub description: String,
    pub pattern: String,
    #[derivative(PartialEq = "ignore")]
    pub regex: Regex,
    pub help_text: String,
}

impl HelpMetadata for KnownErrorSpec {
    fn description(&self) -> &str {
        &self.description
    }
}
