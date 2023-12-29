use clap::Parser;
use human_panic::setup_panic;
use tracing::info;
use pity_lib::prelude::{LoggingOpts, OutputCapture, OutputDestination, write_to_report_file};

/// A wrapper CLI that can be used to capture output from a program, check if there are known errors
/// and let the user know.
///
/// `pity-intercept` will execute `/usr/bin/env -S [utility] [args...]` capture the output from
/// STDOUT and STDERR. After the program exits, the exit code will be checked, and if it's non-zero
/// the output will be parsed for known errors.
#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: LoggingOpts,

    /// Add additional "successful" exit codes. A sub-command that exists 0 will always be considered
    /// a success.
    #[arg(short, long)]
    successful_exit: Vec<u8>,

    /// Command to execute withing pity-intercept.
    #[arg(required = true)]
    utility: String,

    /// Arguments to be passed to the utility
    args: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_panic!();
    dotenv::dotenv().ok();
    let opts = Cli::parse();

    let _guard = opts
        .logging
        .with_new_default(tracing::level_filters::LevelFilter::WARN)
        .configure_logging("intercept");
    let mut command = vec![opts.utility];
    command.extend(opts.args);

    let capture = OutputCapture::capture_output(&command, &OutputDestination::StandardOut).await?;

    let file_path = write_to_report_file("intercept", &capture.create_report_text()?)?;
    info!(target:"user", "Report created at {}", file_path);

    Ok(())
}
