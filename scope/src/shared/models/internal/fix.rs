use crate::{prelude::DoctorFixPromptSpec, shared::prelude::*};
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
    pub prompt: Option<DoctorFixPrompt>,
}

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorFixPrompt {
    #[builder(default)]
    pub text: String,
    #[builder(default)]
    pub extra_context: Option<String>,
}

impl From<DoctorFixPromptSpec> for DoctorFixPrompt {
    fn from(value: DoctorFixPromptSpec) -> Self {
        DoctorFixPrompt {
            text: value.text,
            extra_context: value.extra_context,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_from_spec() {
        let spec = DoctorFixPromptSpec {
            text: "do you want to do the thing?".to_string(),
            extra_context: None,
        };

        let actual = DoctorFixPrompt::from(spec);

        assert_eq!(
            DoctorFixPrompt {
                text: "do you want to do the thing?".to_string(),
                extra_context: None
            },
            actual
        )
    }
}
