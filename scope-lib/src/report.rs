use crate::capture::{CaptureOpts, OutputCapture};
use crate::config_load::FoundConfig;
use crate::models::ReportUploadLocation;
use crate::prelude::OutputDestination;
use anyhow::{anyhow, Result};
use minijinja::{context, Environment};
use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use std::fs::File;
use std::io::Write;
use tracing::{debug, info, warn};

pub struct ReportBuilder<'a> {
    message: String,
    command_results: String,
    config: &'a FoundConfig,
}

impl<'a> ReportBuilder<'a> {
    pub async fn new(capture: &OutputCapture, config: &'a FoundConfig) -> Result<Self> {
        let message = Self::make_default_message(&capture.command, config)?;

        let mut this = Self {
            message,
            command_results: String::new(),
            config,
        };

        this.add_capture(capture)?;

        for command in config.get_report_definition().additional_data.values() {
            let args: Vec<String> = command.split(' ').map(|x| x.to_string()).collect();
            let capture = OutputCapture::capture_output(CaptureOpts {
                working_dir: &config.working_dir,
                args: &args,
                output_dest: OutputDestination::Null,
                path: &config.bin_path,
            })
            .await?;
            this.add_capture(&capture)?;
        }

        Ok(this)
    }

    fn add_capture(&mut self, capture: &OutputCapture) -> Result<()> {
        self.command_results.push('\n');
        self.command_results
            .push_str(&capture.create_report_text()?);

        Ok(())
    }

    pub fn write_local_report(&self) -> Result<()> {
        let report = self.make_report_test();

        let base_report_loc = write_to_report_file("base", &report)?;
        info!(target: "always", "The basic report was created at {}", base_report_loc);

        Ok(())
    }

    fn make_default_message(command: &str, config: &FoundConfig) -> Result<String> {
        let mut env = Environment::new();
        let report_def = config.get_report_definition();
        env.add_template("tmpl", &report_def.template)?;
        let template = env.get_template("tmpl")?;
        let template = template.render(context! { command => command })?;

        Ok(template)
    }

    fn make_report_test(&self) -> String {
        format!(
            "{}\n\n## Captured Data\n\n{}",
            self.message, self.command_results
        )
    }

    pub async fn distribute_report(&self) -> Result<()> {
        let report = self.make_report_test();

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

    let file_path = format!("/tmp/scope/scope-{}-{}.md", prefix, id);
    let mut file = File::create(&file_path)?;
    file.write_all(text.as_bytes())?;

    Ok(file_path)
}
