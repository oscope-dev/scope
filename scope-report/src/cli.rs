use anyhow::Result;
use clap::Args;
use scope_lib::prelude::{FoundConfig, OutputCapture, OutputDestination, ReportBuilder};
use tracing::warn;

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Where the report will be generated, if not set a location will be determined at runtime.
    #[arg(long, short = 'o')]
    report_location: Option<String>,

    /// The command that should be run and reported on
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

pub async fn report_root(found_config: &FoundConfig, args: &ReportArgs) -> Result<i32> {
    let capture = OutputCapture::capture_output(
        &found_config.working_dir,
        &args.command,
        &OutputDestination::Logging,
    )
    .await?;
    let exit_code = capture.exit_code.unwrap_or(-1);
    let report_builder = ReportBuilder::new(capture, &found_config.report_upload);

    if found_config.report_upload.is_empty() {
        report_builder.write_local_report()?;
        return Ok(exit_code);
    }

    if let Err(e) = report_builder.distribute_report().await {
        warn!(target: "user", "Unable to upload report: {}", e);
    }

    Ok(exit_code)
}
