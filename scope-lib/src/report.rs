use crate::capture::{OutputCapture, OutputDestination};
use crate::models::{ModelRoot, ReportUploadLocation, ReportUploadSpec};
use anyhow::{anyhow, Result};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use tracing::{debug, info, warn};

pub struct ReportBuilder {
    command_capture: OutputCapture,
    destinations: BTreeMap<String, ModelRoot<ReportUploadSpec>>,
}

impl ReportBuilder {
    pub fn new(
        capture: OutputCapture,
        destinations: &BTreeMap<String, ModelRoot<ReportUploadSpec>>,
    ) -> Self {
        Self {
            command_capture: capture,
            destinations: destinations.clone(),
        }
    }

    pub fn write_local_report(&self) -> Result<()> {
        let base_report = self.command_capture.create_report_text(None)?;
        let base_report_loc = write_to_report_file("base", &base_report)?;
        info!(target: "always", "The basic report was created at {}", base_report_loc);

        Ok(())
    }

    pub async fn distribute_report(&self) -> Result<()> {
        let base_report = self.command_capture.create_report_text(None)?;

        for (name, dest) in &self.destinations {
            let mut dest_report = base_report.clone();
            for (name, command) in &dest.spec.additional_data {
                let capture = OutputCapture::capture_output(
                    &self.command_capture.working_dir,
                    &[command.to_string()],
                    &OutputDestination::Null,
                )
                .await?;
                dest_report.push('\n');
                dest_report.push_str(&capture.create_report_text(Some(&format!("== {}", name)))?);
            }

            if let Err(e) = &dest.spec.destination.upload(&dest_report).await {
                warn!(target: "user", "Unable to upload to {}: {}", name, e);
            }
        }

        Ok(())
    }
}

impl ReportUploadLocation {
    async fn upload(&self, report: &str) -> Result<()> {
        match self {
            ReportUploadLocation::RustyPaste { url } => {
                ReportUploadLocation::upload_to_rusty_paste(url, report).await
            }
            ReportUploadLocation::GithubIssue { owner, repo, tags } => {
                ReportUploadLocation::upload_to_github_issue(owner, repo, tags.clone(), report)
                    .await
            }
        }
    }

    async fn upload_to_github_issue(
        owner: &str,
        repo: &str,
        tags: Vec<String>,
        report: &str,
    ) -> Result<()> {
        let gh_auth = match std::env::var("GH_TOKEN") {
            Ok(v) => v,
            Err(_) => {
                return Err(anyhow!(
                    "GH_TOKEN env var was not set with token to access GitHub"
                ))
            }
        };

        let body = json::object! {
            title: "Scope bug report",
            body: report,
            labels: tags
        };

        let client = reqwest::Client::new();
        let res = client
            .post(format!(
                "https://api.github.com/repos/{}/{}/issues",
                owner, repo
            ))
            .header(ACCEPT, "application/vnd.github+json")
            .header(AUTHORIZATION, format!("Bearer {}", gh_auth))
            .header(USER_AGENT, "scope")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .body(body.dump())
            .send()
            .await;

        match res {
            Ok(res) => {
                debug!("API Response was {:?}", res);
                let status = res.status();
                match res.text().await {
                    Err(e) => {
                        warn!(target: "user", "Unable to read Github response: {:?}", e)
                    }
                    Ok(body) => {
                        let body = body.trim();
                        if status.is_success() {
                            match json::parse(body) {
                                Ok(json_body) => {
                                    info!(target: "always", "Report was uploaded to {}.", json_body["html_url"])
                                }
                                Err(e) => {
                                    warn!(server = "github", "GitHub response {}", body);
                                    warn!(server = "github", "GitHub parse error {:?}", e);
                                    warn!(target: "always", server="github", "GitHub responded with weird response, please check the logs.");
                                }
                            }
                        } else {
                            info!(target: "always", server="github", "Report upload failed for {}.", body)
                        }
                    }
                }
            }
            Err(e) => {
                warn!(target: "always", "Unable to upload report to server because {}", e)
            }
        }

        Ok(())
    }

    async fn upload_to_rusty_paste(url: &str, report: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let some_file = reqwest::multipart::Part::stream(report.to_string())
            .file_name("file")
            .mime_str("text/plain")?;

        let form = reqwest::multipart::Form::new().part("file", some_file);

        let res = client.post(url).multipart(form).send().await;

        match res {
            Ok(res) => {
                debug!(server = "RustyPaste", "API Response was {:?}", res);
                let status = res.status();
                match res.text().await {
                    Err(e) => {
                        warn!(target: "user",server="RustyPaste",  "Unable to fetch body from Server: {:?}", e)
                    }
                    Ok(body) => {
                        let body = body.trim();
                        if !status.is_success() {
                            info!(target: "always", server="RustyPaste", "Report was uploaded to {}.", body)
                        } else {
                            info!(target: "always", server="RustyPaste", "Report upload failed for {}.", body)
                        }
                    }
                }
            }
            Err(e) => {
                warn!(target: "always", server="RustyPaste", "Unable to upload report to server because {}", e)
            }
        }
        Ok(())
    }
}

pub fn write_to_report_file(prefix: &str, text: &str) -> Result<String> {
    let id = nanoid::nanoid!(10, &nanoid::alphabet::SAFE);

    let file_path = format!("/tmp/scope/scope-{}-{}.txt", prefix, id);
    let mut file = File::create(&file_path)?;
    file.write_all(text.as_bytes())?;

    Ok(file_path)
}
