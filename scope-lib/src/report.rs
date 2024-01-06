use crate::capture::OutputCapture;
use crate::config_load::FoundConfig;
use crate::models::{ReportDefinitionSpec, ReportUploadLocation};
use anyhow::{anyhow, Result};
use minijinja::{context, Environment};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use std::fs::File;
use std::io::Write;
use tracing::{debug, info, warn};

fn default_report_spec() -> ReportDefinitionSpec {
    ReportDefinitionSpec {
        template: "Error report for {{ command }}.".to_string(),
        additional_data: Default::default(),
    }
}

pub struct ReportBuilder<'a> {
    message: String,
    base_report: String,
    command: String,
    config: &'a FoundConfig,
}

impl<'a> ReportBuilder<'a> {
    pub fn new(capture: OutputCapture, config: &'a FoundConfig) -> Result<Self> {
        Ok(Self {
            message: format!("= Unable to run `{}`", capture.command),
            base_report: capture.create_report_text(None)?,
            command: capture.command,
            config,
        })
    }

    pub fn with_message(&mut self, message: String) {
        self.message = message;
    }

    pub fn write_local_report(&self) -> Result<()> {
        let base_report_loc = write_to_report_file("base", &self.base_report)?;
        info!(target: "always", "The basic report was created at {}", base_report_loc);

        Ok(())
    }

    pub fn ask_user_for_message(&mut self) -> Result<()> {
        let report_spec = self
            .config
            .report_definition
            .as_ref()
            .cloned()
            .map(|x| x.spec.clone())
            .unwrap_or_else(default_report_spec);

        let mut env = Environment::new();
        env.add_template("tmpl", &report_spec.template)?;
        let template = env.get_template("tmpl")?;
        let template = template.render(context! { command => self.command })?;

        self.message = template;
        Ok(())
    }

    pub async fn distribute_report(&self) -> Result<()> {
        let report = format!(
            "{}\n\n== Captured Data\n\n{}",
            self.message, self.base_report
        );

        for dest in self.config.report_upload.values() {
            if let Err(e) = &dest.spec.destination.upload(&report).await {
                warn!(target: "user", "Unable to upload to {}: {}", dest.name(), e);
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

        let title = match report.find('\n') {
            Some(value) => report[0..value].to_string(),
            None => "Scope bug report".to_string(),
        };

        let body = json::object! {
            title: title,
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
