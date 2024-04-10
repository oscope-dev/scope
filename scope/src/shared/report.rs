use super::capture::{CaptureOpts, OutputCapture};
use super::config_load::FoundConfig;
use super::models::prelude::ReportUploadLocationDestination;
use super::prelude::OutputDestination;
use anyhow::{anyhow, Result};
use jsonwebtoken::EncodingKey;
use minijinja::{context, Environment};
use octocrab::models::{AppId, InstallationToken};
use octocrab::params::apps::CreateInstallationAccessToken;
use octocrab::Octocrab;
use secrecy::SecretString;
use std::fs::File;
use std::io::Write;
use tracing::{debug, info, warn};
use url::Url;

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
                env_vars: Default::default(),
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
            if let Err(e) = &dest.destination.upload(&report).await {
                warn!(target: "user", "Unable to upload to {}: {}", dest.metadata.name(), e);
            }
        }

        Ok(())
    }
}

impl ReportUploadLocationDestination {
    async fn upload(&self, report: &str) -> Result<()> {
        match self {
            ReportUploadLocationDestination::RustyPaste { url } => {
                ReportUploadLocationDestination::upload_to_rusty_paste(url, report).await
            }
            ReportUploadLocationDestination::GithubIssue { owner, repo, tags } => {
                ReportUploadLocationDestination::upload_to_github_issue(
                    owner,
                    repo,
                    tags.clone(),
                    report,
                )
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
        let client = get_octocrab(repo).await?;

        let title = match report.find('\n') {
            Some(value) => report[0..value].to_string(),
            None => "Scope bug report".to_string(),
        };

        let res = client
            .issues(owner, repo)
            .create(title)
            .body(report)
            .labels(tags)
            .send()
            .await;

        match res {
            Ok(issue) => {
                debug!("Created issue was {:?}", issue);
                info!(target: "always", "Report was uploaded to {}.", issue.html_url)
            }
            Err(e) => {
                warn!(target: "always", "Unable to upload report to GitHub because {}", e)
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

async fn get_octocrab(repo: &str) -> Result<Octocrab> {
    match (
        std::env::var("SCOPE_GH_APP_ID"),
        std::env::var("SCOPE_GH_APP_KEY"),
        std::env::var("SCOPE_GH_TOKEN"),
    ) {
        (Ok(app_id), Ok(app_key), _) => {
            // Influenced by https://github.com/XAMPPRocky/octocrab/blob/main/examples/github_app_authentication_manual.rs
            let app_id = app_id.parse::<u64>()?;
            let app_key = EncodingKey::from_rsa_pem(app_key.as_bytes())?;

            let client = Octocrab::builder().app(AppId(app_id), app_key).build()?;

            let installations = client
                .apps()
                .installations()
                .send()
                .await
                .unwrap()
                .take_items();

            let mut create_access_token = CreateInstallationAccessToken::default();
            create_access_token.repositories = vec![repo.to_string()];

            let access_token_url =
                Url::parse(installations[0].access_tokens_url.as_ref().unwrap()).unwrap();

            let access: InstallationToken = client
                .post(access_token_url.path(), Some(&create_access_token))
                .await
                .unwrap();

            Ok(Octocrab::builder().personal_token(access.token).build()?)
        }
        (_, _, Ok(token)) => {
            let token = SecretString::new(token);
            Ok(Octocrab::builder().personal_token(token).build()?)
        }
        (_, _, _) => Err(anyhow!("No GitHub auth configured")),
    }
}

pub fn write_to_report_file(prefix: &str, text: &str) -> Result<String> {
    let id = nanoid::nanoid!(10, &nanoid::alphabet::SAFE);

    let file_path = format!("/tmp/scope/scope-{}-{}.md", prefix, id);
    let mut file = File::create(&file_path)?;
    file.write_all(text.as_bytes())?;

    Ok(file_path)
}
