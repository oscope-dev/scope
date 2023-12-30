use std::collections::{BTreeMap};
use std::fs::File;
use std::io::Write;
use anyhow::Result;
use tracing::{debug, info, warn};
use crate::capture::{OutputCapture, OutputDestination};
use crate::models::{ModelRoot, ReportUploadLocation, ReportUploadSpec};

pub struct ReportBuilder {
    command_capture: OutputCapture,
    destinations: BTreeMap<String, ModelRoot<ReportUploadSpec>>
}

impl ReportBuilder {
    pub fn new(capture: OutputCapture, destinations: &BTreeMap<String, ModelRoot<ReportUploadSpec>>) -> Self {
        Self {
            command_capture: capture,
            destinations: destinations.clone(),
        }
    }

    pub fn write_local_report(&self) -> Result<()>{
        let base_report = self.command_capture.create_report_text(None)?;
        let base_report_loc = write_to_report_file("base", &base_report)?;
        info!(target: "always", "The basic report was created at {}", base_report_loc);

        Ok(())
    }

    pub async fn distribute_report(&self) -> Result<()> {
        let base_report = self.command_capture.create_report_text(None)?;

        let client = reqwest::Client::new();

        for (_, dest) in &self.destinations {
            let mut dest_report = base_report.clone();
            for (name, command) in &dest.spec.additional_data {
                let capture = OutputCapture::capture_output(&[command.to_string()], &OutputDestination::Null).await?;
                dest_report.push('\n');
                dest_report.push_str(&capture.create_report_text(Some(&format!("== {}", name)))?);
            }

            match &dest.spec.destination {
                ReportUploadLocation::RustyPaste { url } => {
                    let some_file = reqwest::multipart::Part::stream(dest_report)
                        .file_name("file")
                        .mime_str("text/plain")?;

                    let form = reqwest::multipart::Form::new().part("file", some_file);

                    let res = client.post(url)
                        .multipart(form)
                        .send()
                        .await;

                    match res {
                        Ok(res) => {
                            debug!("API Response was {:?}", res);
                            let status = res.status();
                            match res.text().await {
                                Err(e) => warn!(target: "user", "Unable to fetch body from Server: {:?}", e),
                                Ok(body) => {
                                    let body = body.trim();
                                    if !status.is_success() {
                                        info!(target: "always", "Report was uploaded to {}.", body)
                                    } else {
                                        info!(target: "always", "Report upload failed for {}.", body)
                                    }

                                }
                            }
                        },
                        Err(e) => warn!(target: "always", "Unable to upload report to server because {}", e)
                    }
                }
            }
        }

        Ok(())
    }
}

pub fn write_to_report_file(prefix: &str, text: &str) -> Result<String> {
    let id = nanoid::nanoid!(10, &nanoid::alphabet::SAFE);

    let file_path = format!("/tmp/pity/pity-{}-{}.txt", prefix, id);
    let mut file = File::create(&file_path)?;
    file.write_all(text.as_bytes())?;

    Ok(file_path)
}
