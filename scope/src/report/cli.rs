use crate::prelude::{
    DefaultExecutionProvider, DefaultTemplatedReportBuilder, TemplatedReportBuilder,
};
use crate::shared::prelude::{CaptureOpts, FoundConfig, OutputCapture, OutputDestination};
use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use tracing::{instrument, warn};

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Where the report will be generated, if not set a location will be determined at runtime.
    #[arg(long, short = 'o')]
    report_location: Option<String>,

    /// The command that should be run and reported on
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

#[instrument("scope report", skip_all)]
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

    let entrypoint = format!("{:?}", args.command.join(" "));
    let exec_runner = Arc::new(DefaultExecutionProvider::default());
    let report_definition = found_config.get_report_definition();

    for location in found_config.report_upload.values() {
        let mut builder = DefaultTemplatedReportBuilder::from_capture(
            &entrypoint,
            &capture,
            &report_definition,
            location,
        )?;
        builder
            .run_and_capture_additional_data(
                &report_definition.additional_data,
                found_config,
                exec_runner.clone(),
            )
            .await
            .ok();

        if let Err(e) = builder.distribute_report().await {
            warn!(target: "user", "Unable to upload report: {}", e);
        }
    }

    Ok(exit_code)
}
