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
    pub destination: ReportUploadLocationDestination,
}
