use anyhow::Result;
use std::path::Path;

use crate::{
    prelude::{DoctorFixPromptSpec, DoctorFixSpec},
    shared::prelude::*,
};
use derive_builder::Builder;

#[derive(Debug, PartialEq, Clone, Builder)]
#[builder(setter(into))]
pub struct DoctorFix {
    #[builder(default)]
    pub command: Option<DoctorCommands>,
    #[builder(default)]
    pub help_text: Option<String>,
    #[builder(default)]
    pub help_url: Option<String>,
    #[builder(default)]
    pub prompt: Option<DoctorFixPrompt>,
}

impl DoctorFix {
    pub fn empty() -> Self {
        DoctorFix {
            command: None,
            help_text: None,
            help_url: None,
            prompt: None,
        }
    }

    pub fn from_spec(containing_dir: &Path, working_dir: &str, fix: DoctorFixSpec) -> Result<Self> {
        let commands = DoctorCommands::from_commands(containing_dir, working_dir, &fix.commands)?;
        let help_text = fix
            .help_text
            .as_ref()
            .map(|st| st.trim().to_string())
            .clone();
        let help_url = fix.help_url.clone();
        let prompt = fix.prompt.map(DoctorFixPrompt::from);

        Ok(DoctorFix {
            command: Some(commands),
            help_text,
            help_url,
            prompt,
        })
    }
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

    #[test]
    fn empty_returns_a_fix_full_of_none() {
        // I can argue that we should use Option<DoctorFix> instead,
        // but for now, this is where we're at.
        assert_eq!(
            DoctorFix {
                command: None,
                help_text: None,
                help_url: None,
                prompt: None,
            },
            DoctorFix::empty()
        )
    }

    #[test]
    fn from_spec_translates_to_fix() {
        let spec = DoctorFixSpec {
            commands: [
                "some/command",
                "./other_command",
                "{{ working_dir }}/.foo.sh",
            ]
            .iter()
            .map(|cmd| cmd.to_string())
            .collect(),
            help_text: Some("text".to_string()),
            help_url: Some("https.example.com".to_string()),
            prompt: Some(DoctorFixPromptSpec {
                text: "do you want to do the thing?".to_string(),
                extra_context: Some("additional context".to_string()),
            }),
        };

        let expected = DoctorFix {
            command: Some(
                DoctorCommands::from_commands(
                    Path::new("/some/dir"),
                    "/some/work/dir",
                    &spec.commands,
                )
                .unwrap(),
            ),
            help_text: spec.help_text.clone(),
            help_url: spec.help_url.clone(),
            prompt: Some(DoctorFixPrompt::from(spec.prompt.clone().unwrap())),
        };

        let actual = DoctorFix::from_spec(Path::new("/some/dir"), "/some/work/dir", spec).unwrap();

        assert_eq!(expected, actual)
    }
}
