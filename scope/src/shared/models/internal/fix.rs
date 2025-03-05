use crate::shared::prelude::*;
use derive_builder::Builder;

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorFix {
    #[builder(default)]
    pub command: Option<DoctorCommand>,
    #[builder(default)]
    pub help_text: Option<String>,
    #[builder(default)]
    pub help_url: Option<String>,
    #[builder(default)]
    pub prompt: Option<DoctorGroupActionFixPrompt>,
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorGroupActionFixPrompt {
    #[builder(default)]
    pub text: String,
    #[builder(default)]
    pub extra_context: Option<String>,
}

#[cfg(test)]
mod tests {}
