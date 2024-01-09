#[derive(Debug, PartialEq, Clone)]
pub enum ReportUploadLocation {
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
pub struct ReportUploadLocationSpec {
    pub destination: ReportUploadLocation,
}
