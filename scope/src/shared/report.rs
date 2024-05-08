use super::capture::OutputCapture;
use super::config_load::FoundConfig;
use super::models::prelude::ReportUploadLocationDestination;
use crate::prelude::{ExecutionProvider, ReportDefinition, ReportUploadLocation};
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
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use tracing::{debug, info, warn};
use url::Url;

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

    async fn run_and_capture_additional_data(
        &mut self,
        commands: &BTreeMap<String, String>,
        found_config: &FoundConfig,
        exec_provider: Arc<dyn ExecutionProvider>,
    ) -> Result<()>;

    fn append(&mut self, body: &str) -> Result<()>;

    async fn distribute_report(&self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct DefaultTemplatedReportBuilder {
    // This looks a lot like our context object
    entrypoint: String,
    message: Option<String>,
    groups: Vec<GroupReport>,
    additional_data: BTreeMap<String, String>,

    // Not like the context object
    destination: ReportUploadLocation,
}

impl DefaultTemplatedReportBuilder {
    pub fn new(entrypoint: &str, dest: &ReportUploadLocation) -> Self {
        Self {
            entrypoint: entrypoint.to_string(),
            message: None,
            groups: vec![],
            additional_data: BTreeMap::new(),
            destination: dest.clone(),
        }
    }

    // TODO: refactor this to not accept an Output Capture. This needs to be a GroupReport
    pub fn from_capture(
        entrypoint: &str,
        capture: &OutputCapture,
        report_template: &ReportDefinition,
        dest: &ReportUploadLocation,
    ) -> Result<Self> {
        let mut this = Self::new(entrypoint, dest);

        let mut message = render_template(&capture.command, &report_template.template)?;
        message += "\n\n";
        message += &capture.create_report_text()?;
        this.message = Some(message);

        Ok(this)
    }

    fn get_title_template(_dest: &ReportUploadLocation) -> &str {
        // TODO: override from dest
        "Scope bug report: `{{ entrypoint }}`"
    }

    fn render_title(&self) -> Result<String> {
        let mut env = Environment::new();
        let _ = env.add_template("title", Self::get_title_template(&self.destination))?;

        let template = env.get_template("title")?;
        let rendered = template.render(context! { entrypoint => self.entrypoint })?;

        Ok(rendered)
    }

    fn get_body_template(_dest: &ReportUploadLocation) -> &'static str {
        // TODO: override from dest
        include_str!("body_template.jinja")
    }

    fn render_body(&self) -> Result<String> {
        let mut env = Environment::new();
        let _ = env.add_template("body", Self::get_body_template(&self.destination))?;

        let message = self.message.clone().unwrap_or("".to_string());

        let template = env.get_template("body")?;
        let ctx = context! {
            message => message,
            entrypoint => self.entrypoint,
            // convert to vec[vec[]] because minijinja does not suppport items on Dict
            // https://github.com/mitsuhiko/minijinja/issues/32
            additionalData => Vec::from_iter(self.additional_data.iter()),
            groups => self.get_groups_context(),
         };
        println!("{:?}", ctx);
        let rendered = template.render(ctx)?;

        Ok(rendered)
    }

    fn get_groups_context(&self) -> minijinja::value::Value {
        let mut output = vec![];

        for group in &self.groups {
            output.push(ReportGroupItemContext::from(&group));
        }

        minijinja::value::Value::from_serialize(&output)
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct ReportCommandResultContext {
    command: String,

    #[serde(rename = "exitCode")]
    exit_code: i32,

    #[serde(rename = "startTime")]
    start_time: String,

    #[serde(rename = "endTime")]
    end_time: String,
}

impl ReportCommandResultContext {
    fn from(report: &ActionTaskReport) -> Self {
        Self {
            command: report.command.to_string(),
            exit_code: report.exit_code.unwrap_or(-1),
            start_time: report.start_time.to_string(),
            end_time: report.end_time.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct ReportGroupItemContext {
    name: String,

    #[serde(default)]
    actions: Vec<ReportActionItemContext>,
}

impl ReportGroupItemContext {
    fn from(report: &GroupReport) -> Self {
        Self {
            name: report.group_name.to_string(),
            actions: (&report.action_result).into_iter().map(|action| ReportActionItemContext::from(&action)).collect::<Vec<_>>()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct ReportActionItemContext {
    name: String,

    #[serde(default)]
    check: Vec<ReportCommandResultContext>,

    #[serde(default)]
    fix: Vec<ReportCommandResultContext>,

    #[serde(default)]
    verify: Vec<ReportCommandResultContext>,
}

impl ReportActionItemContext {
    fn from(report: &ActionReport) -> Self {
        Self {
            name: report.action_name.to_string(),
            check: (&report.check)
                .into_iter()
                .map(|check| ReportCommandResultContext::from(&check) )
                .collect::<Vec<_>>(),
            fix: (&report.fix)
                .into_iter()
                .map(|check| ReportCommandResultContext::from(&check) )
                .collect::<Vec<_>>(),
            verify: (&report.validate)
                .into_iter()
                .map(|check| ReportCommandResultContext::from(&check) )
                .collect::<Vec<_>>(),
         }
    }
}

fn render_template(command: &str, template: &str) -> Result<String> {
    let mut env = Environment::new();
    env.add_template("tmpl", template)?;
    let template = env.get_template("tmpl")?;
    let template = template.render(context! { command => command })?;

    Ok(template)
}

#[async_trait]
impl TemplatedReportBuilder for DefaultTemplatedReportBuilder {
    fn create_group(&mut self, group_result: &GroupReport) -> Result<()> {
        self.groups.push(group_result.clone());

        println!("{:?}", &self.groups);

        Ok(())
    }

    fn add_additional_data(&mut self, additional_data: BTreeMap<String, String>) -> Result<()> {
        for (key, value) in additional_data.into_iter() {
            self.additional_data.insert(key, value);
        }

        Ok(())
    }

    async fn run_and_capture_additional_data(
        &mut self,
        commands: &BTreeMap<String, String>,
        found_config: &FoundConfig,
        exec_provider: Arc<dyn ExecutionProvider>,
    ) -> Result<()> {
        let mut additional_report_data = BTreeMap::new();
        for (name, command) in commands {
            let output = exec_provider
                .run_for_output(&found_config.bin_path, &found_config.working_dir, command)
                .await;
            additional_report_data.insert(name.to_string(), output);
        }
        if !additional_report_data.is_empty() {
            self.add_additional_data(additional_report_data)?;
        }

        Ok(())
    }

    fn append(&mut self, body: &str) -> Result<()> {
        match &self.message {
            None => {
                self.message = Some(body.to_string());
            }
            Some(message) => {
                self.message = Some(format!("{}\n{}", message, body));
            }
        }

        Ok(())
    }

    async fn distribute_report(&self) -> Result<()> {
        // TODO: this functionality could be grouped into a ReportRenderer trait?
        let title = self.render_title()?;
        let body = self.render_body()?;

        if let Err(e) = &self
            .destination
            .destination
            .upload(&title, &body)
            .await
        {
            warn!(target: "user", "Unable to upload to {}: {}", self.destination.metadata.name(), e);
        }

        Ok(())
    }
}
