use crate::models::prelude::{ReportUploadLocation, ReportUploadLocationSpec};
use anyhow::Result;

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ReportDestinationGithubIssueV1Alpha {
    owner: String,
    repo: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
enum ReportDestinationV1Alpha {
    RustyPaste { url: String },
    GithubIssue(ReportDestinationGithubIssueV1Alpha),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ReportLocationV1Alpha {
    #[serde(with = "serde_yaml::with::singleton_map")]
    destination: ReportDestinationV1Alpha,
}
pub(super) fn parse(value: &Value) -> Result<ReportUploadLocationSpec> {
    let parsed: ReportLocationV1Alpha = serde_yaml::from_value(value.clone())?;
    let destination = match parsed.destination {
        ReportDestinationV1Alpha::RustyPaste { url } => ReportUploadLocation::RustyPaste { url },
        ReportDestinationV1Alpha::GithubIssue(github_issue) => ReportUploadLocation::GithubIssue {
            owner: github_issue.owner,
            repo: github_issue.repo,
            tags: github_issue.tags,
        },
    };
    Ok(ReportUploadLocationSpec { destination })
}

#[cfg(test)]
mod tests {
    use crate::models::parse_models_from_string;
    use crate::models::prelude::{ReportUploadLocation, ReportUploadLocationSpec};
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
            ReportUploadLocationSpec {
                destination: ReportUploadLocation::RustyPaste {
                    url: "https://foo.bar".to_string()
                },
            }
        );

        assert_eq!(
            configs[1].get_report_upload_spec().unwrap(),
            ReportUploadLocationSpec {
                destination: ReportUploadLocation::GithubIssue {
                    owner: "scope".to_string(),
                    repo: "party".to_string(),
                    tags: Vec::new(),
                }
            }
        );
    }
}
