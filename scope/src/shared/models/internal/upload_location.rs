use crate::models::prelude::{ModelMetadata, V1AlphaReportLocation};
use crate::models::HelpMetadata;
use crate::prelude::{ReportDestinationSpec, ReportDestinationTemplates};
use derivative::Derivative;
use minijinja::Environment;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Clone)]
pub enum ReportUploadLocationDestination {
    RustyPaste {
        url: String,
    },
    GithubIssue {
        owner: String,
        repo: String,
        tags: Vec<String>,
    },
    Local {
        destination: String,
    },
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Clone)]
pub struct ReportTemplates {
    #[derivative(PartialEq = "ignore")]
    templates: BTreeMap<String, String>,
    doctor_template: String,
    title_template: String,
    analyze_template: String,
}

impl Default for ReportTemplates {
    fn default() -> Self {
        let mut templates = BTreeMap::new();
        templates.insert(
            "message".to_string(),
            ReportTemplates::default_message_template(),
        );
        ReportTemplates {
            templates,
            title_template: ReportTemplates::default_title_template(),
            doctor_template: ReportTemplates::default_doctor_template(),
            analyze_template: ReportTemplates::default_command_template(),
        }
    }
}

impl ReportTemplates {
    fn make_env<'a>(&self) -> anyhow::Result<Environment<'a>> {
        let mut env = Environment::new();
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);

        for (name, template) in &self.templates {
            let name = name.clone();
            let template = template.clone();
            env.add_template_owned(name, template)?;
        }

        env.add_template_owned("title".to_string(), self.title_template.clone())?;
        env.add_template_owned("analyze".to_string(), self.analyze_template.clone())?;
        env.add_template_owned("doctor".to_string(), self.doctor_template.clone())?;

        Ok(env)
    }

    pub fn add_template(&mut self, name: &str, template: &str) {
        self.templates
            .insert(name.to_string(), template.to_string());
    }

    pub fn render_title<S: Serialize>(&self, ctx: S) -> anyhow::Result<String> {
        let binding = self.make_env()?;
        let title_template = binding.get_template("title")?;
        Ok(title_template.render(ctx)?)
    }

    pub fn render_doctor<S: Serialize>(&self, ctx: S) -> anyhow::Result<String> {
        let mut env = self.make_env()?;
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);
        let title_template = env.get_template("doctor")?;
        Ok(title_template.render(ctx)?)
    }

    pub fn render_command<S: Serialize>(&self, ctx: S) -> anyhow::Result<String> {
        let env = self.make_env()?;
        let title_template = env.get_template("command")?;
        Ok(title_template.render(ctx)?)
    }

    fn default_message_template() -> String {
        "== Error report for {{ command }}.".to_string()
    }

    fn default_title_template() -> String {
        "Scope bug report: `{{ entrypoint }}`".to_string()
    }

    fn default_doctor_template() -> String {
        include_str!("../../grouped_body.jinja").to_string()
    }

    fn default_command_template() -> String {
        include_str!("../../unstructured_body.jinja").to_string()
    }
}

impl TryFrom<ReportDestinationTemplates> for ReportTemplates {
    type Error = anyhow::Error;

    fn try_from(inputs: ReportDestinationTemplates) -> Result<Self, Self::Error> {
        let mut templates = BTreeMap::new();
        templates.insert(
            "message".to_string(),
            ReportTemplates::default_message_template(),
        );
        for (name, template) in inputs.extra_definitions {
            templates.insert(name, template);
        }

        Ok(ReportTemplates {
            templates,
            title_template: inputs
                .title
                .unwrap_or_else(ReportTemplates::default_title_template),
            doctor_template: inputs
                .doctor
                .unwrap_or_else(ReportTemplates::default_doctor_template),
            analyze_template: inputs
                .analyze
                .unwrap_or_else(ReportTemplates::default_command_template),
        })
    }
}

#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Clone)]
pub struct ReportUploadLocation {
    pub metadata: ModelMetadata,
    pub full_name: String,
    pub destination: ReportUploadLocationDestination,
    pub templates: ReportTemplates,
    pub additional_data: BTreeMap<String, String>,
}

impl HelpMetadata for ReportUploadLocation {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn full_name(&self) -> String {
        self.full_name.to_string()
    }
}

impl TryFrom<V1AlphaReportLocation> for ReportUploadLocation {
    type Error = anyhow::Error;

    fn try_from(value: V1AlphaReportLocation) -> Result<Self, Self::Error> {
        let destination = match value.spec.destination {
            ReportDestinationSpec::RustyPaste(ref def) => {
                ReportUploadLocationDestination::RustyPaste {
                    url: def.url.to_string(),
                }
            }
            ReportDestinationSpec::GithubIssue(ref github_issue) => {
                ReportUploadLocationDestination::GithubIssue {
                    owner: github_issue.owner.to_string(),
                    repo: github_issue.repo.to_string(),
                    tags: github_issue.tags.clone(),
                }
            }
            ReportDestinationSpec::Local(ref loc) => ReportUploadLocationDestination::Local {
                destination: loc.directory.clone(),
            },
        };

        let report_templates = ReportTemplates::try_from(value.spec.templates.clone())?;
        Ok(ReportUploadLocation {
            full_name: value.full_name(),
            metadata: value.metadata,
            destination,
            templates: report_templates,
            additional_data: value.spec.additional_data,
        })
    }
}
