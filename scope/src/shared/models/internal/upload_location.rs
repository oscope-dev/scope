use crate::models::prelude::{ModelMetadata, V1AlphaReportLocation};
use crate::models::HelpMetadata;

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
            crate::models::prelude::ReportDestinationSpec::RustyPaste (ref def ) => {
                ReportUploadLocationDestination::RustyPaste {
                    url: def.url.to_string(),
                }
            }
            crate::models::prelude::ReportDestinationSpec::GithubIssue(ref github_issue) => {
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
