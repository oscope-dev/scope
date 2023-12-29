use anyhow::Result;
use clap::{Args};
use tracing::{info};
use pity_lib::prelude::{OutputCapture, OutputDestination, write_to_report_file};

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Where the report will be generated, if not set a location will be determined at runtime.
    #[arg(long, short = 'o')]
    report_location: Option<String>,

    /// The command that should be run and reported on
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

pub async fn report_root(args: &ReportArgs) -> Result<()> {
    let capture = OutputCapture::capture_output(&args.command, &OutputDestination::Logging).await?;
    let file_path = write_to_report_file("report", &capture.create_report_text()?)?;

    info!(target:"user", "Report created at {}", file_path);

    Ok(())
}

