use dev_scope_model::prelude::{ModelMetadata, V1AlphaReportLocation};

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
    pub destination: ReportUploadLocationDestination,
}

impl TryFrom<V1AlphaReportLocation> for ReportUploadLocation {
    type Error = anyhow::Error;

    fn try_from(value: V1AlphaReportLocation) -> Result<Self, Self::Error> {
        let destination = match value.spec.destination {
            dev_scope_model::prelude::ReportDestinationSpec::RustyPaste { url } => {
                ReportUploadLocationDestination::RustyPaste { url }
            }
            dev_scope_model::prelude::ReportDestinationSpec::GithubIssue(github_issue) => {
                ReportUploadLocationDestination::GithubIssue {
                    owner: github_issue.owner,
                    repo: github_issue.repo,
                    tags: github_issue.tags,
                }
            }
        };
        Ok(ReportUploadLocation { metadata: value.metadata, destination })
    }
}
