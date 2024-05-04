use super::capture::{CaptureError, CaptureOpts, OutputCapture};
use super::config_load::FoundConfig;
use super::models::prelude::ReportUploadLocationDestination;
use super::prelude::OutputDestination;
use crate::prelude::ReportUploadLocation;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use derive_builder::Builder;
use jsonwebtoken::EncodingKey;
use minijinja::{context, Environment};
use normpath::PathExt;
use octocrab::models::{AppId, InstallationToken};
use octocrab::params::apps::CreateInstallationAccessToken;
use octocrab::Octocrab;
use secrecy::SecretString;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use tracing::{debug, info, warn};
use url::Url;

#[derive(Clone, Debug)]
pub struct ReportBuilder {
    title: String,
    message: Option<String>,
    command_results: String,
}

impl<'a> ReportBuilder {
    pub async fn new_from_error(
        title: String,
        capture: &OutputCapture,
        config: &'a FoundConfig,
    ) -> Result<Self> {
        let message = Self::make_default_message(&capture.command, config)?;

        let mut this = Self {
            title,
            message: Some(message),
            command_results: String::new(),
        };

        this.add_capture(capture)?;

        this.add_additional_data(config).await?;

        Ok(this)
    }

    pub fn new_blank(title: String) -> Self {
        Self {
            title,
            message: None,
            command_results: String::new(),
        }
    }

    pub fn add_capture(&mut self, capture: &OutputCapture) -> Result<()> {
        self.command_results.push('\n');
        self.command_results
            .push_str(&capture.create_report_text()?);

        Ok(())
    }

    pub fn add_capture_error(&mut self, error: &CaptureError, command: &String) -> Result<()> {
        self.command_results.push('\n');
        self.command_results
            .push_str(&error.create_report_text(command)?);

        Ok(())
    }

    pub async fn add_additional_data(&mut self, config: &'a FoundConfig) -> Result<()> {
        for command in config.get_report_definition().additional_data.values() {
            let args: Vec<String> = command.split(' ').map(|x| x.to_string()).collect();
            let result = OutputCapture::capture_output(CaptureOpts {
                working_dir: &config.working_dir,
                args: &args,
                output_dest: OutputDestination::Null,
                path: &config.bin_path,
                env_vars: Default::default(),
            })
            .await;

            match result {
                Ok(capture) => self.add_capture(&capture)?,
                Err(error) => self.add_capture_error(&error, command)?,
            }
        }

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
        let top = self
            .message
            .clone()
            .map_or("".to_string(), |m| format!("{}\n\n", m));

        format!("{}## Captured Data\n{}", top, self.command_results)
    }

    pub async fn distribute_report(&self, config: &'a FoundConfig) -> Result<()> {
        let report = self.make_report_test();

        for dest in config.report_upload.values() {
            if let Err(e) = &dest.destination.upload(&self.title, &report).await {
                warn!(target: "user", "Unable to upload to {}: {}", dest.metadata.name(), e);
            }
        }

        Ok(())
    }
}

impl ReportUploadLocationDestination {
    async fn upload(&self, title: &str, report: &str) -> Result<()> {
        match self {
            ReportUploadLocationDestination::RustyPaste { url } => {
                ReportUploadLocationDestination::upload_to_rusty_paste(url, report).await
            }
            ReportUploadLocationDestination::GithubIssue { owner, repo, tags } => {
                ReportUploadLocationDestination::upload_to_github_issue(
                    owner,
                    repo,
                    tags.clone(),
                    title,
                    report,
                )
                .await
            }
            ReportUploadLocationDestination::Local { destination } => {
                let id = nanoid::nanoid!(10, &nanoid::alphabet::SAFE);
                fs::create_dir_all(destination)?;
                let file_path = format!("{}/scope-{}.md", destination, id);
                let mut file = File::create(&file_path)?;
                file.write_all(report.as_bytes())?;

                // make this path nicer
                let file_path = PathBuf::from(&file_path)
                    .normalize()
                    .map(|x| x.into_path_buf().display().to_string())
                    .unwrap_or(file_path);
                info!(target: "always", "Report was created at {}", file_path);

                Ok(())
            }
        }
    }

    async fn upload_to_github_issue(
        owner: &str,
        repo: &str,
        tags: Vec<String>,
        title: &str,
        report: &str,
    ) -> Result<()> {
        let client = get_octocrab(repo).await?;

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

#[derive(Debug, Clone, Default, Builder)]
#[builder(setter(into))]
pub struct ActionTaskReport {
    #[builder(default)]
    pub command: String,
    #[builder(default)]
    pub output: Option<String>,
    #[builder(default)]
    pub exit_code: Option<i32>,
    #[builder(default)]
    pub start_time: DateTime<Utc>,
    #[builder(default)]
    pub end_time: DateTime<Utc>,
}

impl From<&OutputCapture> for ActionTaskReport {
    fn from(value: &OutputCapture) -> Self {
        ActionTaskReport {
            exit_code: value.exit_code,
            output: Some(value.generate_user_output()),
            command: value.command.clone(),
            start_time: value.start_time,
            end_time: value.end_time,
        }
    }
}

impl ActionTaskReport {
    pub fn get_output(&self) -> String {
        match &self.output {
            Some(body) => body.to_string(),
            None => "No Output".to_string(),
        }
    }

    fn write_output<T>(&self, f: &mut T) -> Result<()>
    where
        T: std::fmt::Write,
    {
        writeln!(f)?;
        writeln!(f, "---")?;
        writeln!(f, "Command: `{}`\n", self.command)?;

        writeln!(f, "Output:\n")?;
        writeln!(f, "```text",)?;
        writeln!(f, "{}", self.get_output().trim())?;
        writeln!(f, "```\n",)?;

        writeln!(f, "|Name|Value|")?;
        writeln!(f, "|:---|:---|")?;
        writeln!(f, "| Exit code| `{}` |", self.exit_code.unwrap_or(-1))?;
        writeln!(f, "| Started at| `{}` |", self.start_time)?;
        writeln!(f, "| Finished at| `{}` |", self.end_time)?;

        Ok(())
    }
}

#[derive(Debug, Clone, Default, Builder)]
#[builder(setter(into))]
pub struct ActionReport {
    #[builder(default)]
    pub action_name: String,
    #[builder(default)]
    pub check: Vec<ActionTaskReport>,
    #[builder(default)]
    pub fix: Vec<ActionTaskReport>,
    #[builder(default)]
    pub validate: Vec<ActionTaskReport>,
}

#[derive(Debug, Clone)]
pub struct GroupReport {
    group_name: String,
    action_result: Vec<ActionReport>,
    additional_details: BTreeMap<String, String>,
}

impl GroupReport {
    pub fn add_action(&mut self, action_report: &ActionReport) {
        self.action_result.push(action_report.clone());
    }

    pub fn add_additional_details(&mut self, key: &str, value: &str) {
        self.additional_details
            .insert(key.to_string(), value.to_string());
    }

    pub fn new(group_name: &str) -> Self {
        Self {
            group_name: group_name.to_string(),
            action_result: Vec::new(),
            additional_details: BTreeMap::new(),
        }
    }
}

#[async_trait]
pub trait TemplatedReportBuilder {
    fn create_group(&mut self, group_result: &GroupReport) -> Result<()>;

    fn add_additional_data(&mut self, commands: BTreeMap<String, String>) -> Result<()>;

    async fn distribute_report(&self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct DefaultTemplatedReportBuilder {
    title: String,
    output: String,
    destination: ReportUploadLocation,
}

impl DefaultTemplatedReportBuilder {
    pub fn new(title: &str, dest: &ReportUploadLocation) -> Self {
        Self {
            title: title.to_string(),
            output: format!("# {}\n", title),
            destination: dest.clone(),
        }
    }
}

#[async_trait]
impl TemplatedReportBuilder for DefaultTemplatedReportBuilder {
    fn create_group(&mut self, group_result: &GroupReport) -> Result<()> {
        use std::fmt::Write;

        writeln!(self.output)?;
        writeln!(self.output, "## Group `{}`\n", group_result.group_name)?;
        for action in &group_result.action_result {
            writeln!(
                self.output,
                "### Action `{}/{}`",
                group_result.group_name, action.action_name
            )?;

            for check in &action.check {
                check.write_output(&mut self.output)?;
            }
        }

        if !group_result.additional_details.is_empty() {
            self.add_additional_data(group_result.additional_details.clone())?
        }

        Ok(())
    }

    fn add_additional_data(&mut self, additional_data: BTreeMap<String, String>) -> Result<()> {
        use std::fmt::Write;

        writeln!(self.output, "\n**Additional Capture Data**\n")?;
        writeln!(self.output, "| Name | Value |")?;
        writeln!(self.output, "|:---|:---|")?;

        for (name, result) in additional_data {
            writeln!(self.output, "|{}|<pre>{}</pre>|", name, result)?;
        }

        Ok(())
    }

    async fn distribute_report(&self) -> Result<()> {
        if let Err(e) = &self
            .destination
            .destination
            .upload(&self.title, &self.output)
            .await
        {
            warn!(target: "user", "Unable to upload to {}: {}", self.destination.metadata.name(), e);
        }

        Ok(())
    }
}
