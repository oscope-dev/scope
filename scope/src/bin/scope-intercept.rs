use clap::Parser;
use dev_scope::prelude::*;
use human_panic::setup_panic;
use std::env;
use tracing::{debug, enabled, error, info, warn, Level};

/// A wrapper CLI that can be used to capture output from a program, check if there are known errors
/// and let the user know.
///
/// `scope-intercept` will execute `/usr/bin/env -S [utility] [args...]` capture the output from
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
    successful_exit: Vec<i32>,

    #[clap(flatten)]
    config_options: ConfigOptions,

    /// Command to execute withing scope-intercept.
    #[arg(required = true)]
    utility: String,

    /// Arguments to be passed to the utility
    args: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_panic!();
    dotenvy::dotenv().ok();
    let exe_path = std::env::current_exe().unwrap();
    let env_path = exe_path.parent().unwrap().join("../etc/scope.env");
    dotenvy::from_path(env_path).ok();
    let opts = Cli::parse();

    let (_guard, file_location) = opts
        .logging
        .with_new_default(tracing::level_filters::LevelFilter::WARN)
        .configure_logging(&opts.config_options.get_run_id(), "intercept")
        .await;

    let exit_code = run_command(opts).await.unwrap_or_else(|e| {
        error!(target: "user", "Fatal error {:?}", e);
        1
    });

    if exit_code != 0 || enabled!(Level::DEBUG) {
        info!(target: "user", "More detailed logs at {}", file_location);
    }

    std::process::exit(exit_code);
}

async fn run_command(opts: Cli) -> anyhow::Result<i32> {
    let mut command = vec![opts.utility];
    command.extend(opts.args);
    let current_dir = std::env::current_dir()?;
    let path = env::var("PATH").unwrap_or_default();

    let capture = OutputCapture::capture_output(CaptureOpts {
        working_dir: &current_dir,
        args: &command,
        output_dest: OutputDestination::StandardOut,
        path: &path,
        env_vars: Default::default(),
    })
    .await?;

    let mut accepted_exit_codes = vec![0];
    accepted_exit_codes.extend(opts.successful_exit);

    let exit_code = capture.exit_code.unwrap_or(-1);
    if accepted_exit_codes.contains(&exit_code) {
        return Ok(exit_code);
    }

    error!(target: "user", "Command failed, checking for a known error");
    let found_config = opts.config_options.load_config().await.unwrap_or_else(|e| {
        error!(target: "user", "Unable to load configs from disk: {:?}", e);
        FoundConfig::empty(env::current_dir().unwrap())
    });

    let command_output = capture.generate_output();

    for known_error in found_config.known_error.values() {
        debug!("Checking known error {}", known_error.name());
        if known_error.regex.is_match(&command_output) {
            info!(target: "always", "Known error '{}' found", known_error.name());
            info!(target: "always", "\t==> {}", known_error.help_text);
        }
    }

    if found_config.report_upload.is_empty() {
        return Ok(exit_code);
    }

    let ans = inquire::Confirm::new("Do you want to upload a bug report?")
        .with_default(true)
        .with_help_message(
            "This will allow you to share the error with other engineers for support.",
        )
        .prompt();

    let report_builder = ReportBuilder::new(&capture, &found_config).await?;
    if let Ok(true) = ans {
        if let Err(e) = report_builder.distribute_report().await {
            warn!(target: "user", "Unable to upload report: {}", e);
        }
    } else {
        report_builder.write_local_report().ok();
    }
    Ok(exit_code)
}
