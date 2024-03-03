use crate::prelude::{HelpMetadata, ReportDefinition};
use dev_scope_model::prelude::{ModelMetadata, V1AlphaReportLocation};
use dev_scope_model::ScopeModel;

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
}
#[derive(Debug, PartialEq, Clone)]
pub struct ReportUploadLocation {
    pub metadata: ModelMetadata,
    pub full_name: String,
    pub destination: ReportUploadLocationDestination,
}

impl HelpMetadata for ReportUploadLocation {
    fn description(&self) -> String {
        format!("Upload resource {}", self.metadata.name)
    }

    fn name(&self) -> String {
        self.metadata.name.to_string()
    }

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
            dev_scope_model::prelude::ReportDestinationSpec::RustyPaste { ref url } => {
                ReportUploadLocationDestination::RustyPaste {
                    url: url.to_string(),
                }
            }
            dev_scope_model::prelude::ReportDestinationSpec::GithubIssue(ref github_issue) => {
                ReportUploadLocationDestination::GithubIssue {
                    owner: github_issue.owner.to_string(),
                    repo: github_issue.repo.to_string(),
                    tags: github_issue.tags.clone(),
                }
            }
        };
        Ok(ReportUploadLocation {
            full_name: value.full_name(),
            metadata: value.metadata,
            destination,
        })
    }
}
