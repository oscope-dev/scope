use crate::shared::models::prelude::{ReportUploadLocation, ReportUploadLocationDestination};
use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ReportDestinationGithubIssueSpec {
    owner: String,
    repo: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
enum ReportDestinationSpec {
    RustyPaste { url: String },
    GithubIssue(ReportDestinationGithubIssueSpec),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ReportLocationSpec {
    #[serde(with = "serde_yaml::with::singleton_map")]
    destination: ReportDestinationSpec,
}
pub(super) fn parse(value: &Value) -> Result<ReportUploadLocation> {
    let parsed: ReportLocationSpec = serde_yaml::from_value(value.clone())?;
    let destination = match parsed.destination {
        ReportDestinationSpec::RustyPaste { url } => {
            ReportUploadLocationDestination::RustyPaste { url }
        }
        ReportDestinationSpec::GithubIssue(github_issue) => {
            ReportUploadLocationDestination::GithubIssue {
                owner: github_issue.owner,
                repo: github_issue.repo,
                tags: github_issue.tags,
            }
        }
    };
    Ok(ReportUploadLocation { destination })
}

#[cfg(test)]
mod tests {
    use crate::shared::models::parse_models_from_string;
    use crate::shared::models::prelude::{ReportUploadLocation, ReportUploadLocationDestination};
    use std::path::Path;

    #[test]
    fn test_parse_scope_report_loc() {
        let text = "
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportLocation
metadata:
  name: report
spec:
  destination:
      rustyPaste:
        url: https://foo.bar
---
apiVersion: scope.github.com/v1alpha
kind: ScopeReportLocation
metadata:
  name: github
spec:
  destination:
      githubIssue:
        owner: scope
        repo: party
 ";

        let path = Path::new("/foo/bar/file.yaml");
        let configs = parse_models_from_string(path, text).unwrap();
        assert_eq!(2, configs.len());

        assert_eq!(
            configs[0].get_report_upload_spec().unwrap(),
            ReportUploadLocation {
                destination: ReportUploadLocationDestination::RustyPaste {
                    url: "https://foo.bar".to_string()
                },
            }
        );

        assert_eq!(
            configs[1].get_report_upload_spec().unwrap(),
            ReportUploadLocation {
                destination: ReportUploadLocationDestination::GithubIssue {
                    owner: "scope".to_string(),
                    repo: "party".to_string(),
                    tags: Vec::new(),
                }
            }
        );
    }
}
