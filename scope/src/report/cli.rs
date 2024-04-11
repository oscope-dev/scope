use crate::shared::prelude::{
    CaptureOpts, FoundConfig, OutputCapture, OutputDestination, ReportBuilder,
};
use anyhow::Result;
use clap::Args;
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
    let capture = OutputCapture::capture_output(CaptureOpts {
        working_dir: &found_config.working_dir,
        args: &args.command,
        output_dest: OutputDestination::Logging,
        path: &found_config.bin_path,
        env_vars: Default::default(),
    })
    .await?;
    let exit_code = capture.exit_code.unwrap_or(-1);

    let title = format!("Scope bug report: `{:?}`", args.command);
    let report_builder = ReportBuilder::new_from_error(title, &capture, found_config).await?;

    if found_config.report_upload.is_empty() {
        report_builder.write_local_report()?;
        return Ok(exit_code);
    }

    if let Err(e) = report_builder.distribute_report(found_config).await {
        warn!(target: "user", "Unable to upload report: {}", e);
    }

    Ok(exit_code)
}
