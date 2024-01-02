use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use human_panic::setup_panic;
use pity_doctor::prelude::*;
use pity_lib::prelude::{ConfigOptions, FoundConfig, LoggingOpts};
use pity_lib::UserListing;
use pity_report::prelude::{report_root, ReportArgs};
use std::path::Path;
use tracing::{error, info};

/// Pity the Fool
///
/// Pity is a tool to enable teams to manage local machine
/// checks. An example would be a team that supports other
/// engineers may want to verify that the engineer asking
/// for support's machine is setup correctly.
#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: LoggingOpts,

    #[clap(flatten)]
    config: ConfigOptions,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run checks that will "checkup" your machine.
    Doctor(DoctorArgs),
    /// Generate a bug report based from a command that was ran
    Report(ReportArgs),
    /// List the found config files, and resources detected
    Config,
}

#[tokio::main]
async fn main() {
    setup_panic!();
    dotenv::dotenv().ok();
    let opts = Cli::parse();

    let _guard = opts.logging.configure_logging("root");
    let error_code = run_subcommand(opts).await;

    std::process::exit(error_code);
}

async fn run_subcommand(opts: Cli) -> i32 {
    let loaded_config = match opts.config.load_config() {
        Err(e) => {
            error!(target: "user", "Failed to load configuration: {}", e);
            return 2;
        }
        Ok(c) => c,
    };

    handle_commands(&loaded_config, &opts.command)
        .await
        .unwrap_or_else(|e| {
            error!(target: "user", "Critical Error. {}", e);
            1
        })
}

async fn handle_commands(found_config: &FoundConfig, command: &Command) -> Result<i32> {
    match command {
        Command::Doctor(args) => doctor_root(found_config, args).await,
        Command::Report(args) => report_root(found_config, args).await,
        Command::Config => show_config(found_config).map(|_| 0),
    }
}

fn show_config(found_config: &FoundConfig) -> Result<()> {
    if !found_config.exec_check.is_empty() {
        info!(target: "user", "Doctor Checks");
        print_details(
            &found_config.working_dir,
            found_config.exec_check.values().collect(),
        );
    }

    if !found_config.known_error.is_empty() {
        info!(target: "user", "Known Errors");
        print_details(
            &found_config.working_dir,
            found_config.known_error.values().collect(),
        );
    }
    Ok(())
}

fn print_details<T>(working_dir: &Path, config: Vec<&T>)
where
    T: UserListing,
{
    info!(target: "user", "{:^20}{:^60}{:^40}", "Name".white().bold(), "Description".white().bold(), "Path".white().bold());
    for check in config {
        let mut loc = check.location();
        let diff_path = pathdiff::diff_paths(&loc, &working_dir);
        if let Some(diff) = diff_path {
            loc = diff.display().to_string();
        } else if loc.len() > 35 {
            loc = format!("...{}", loc.split_off(loc.len() - 35));
        }
        info!(target: "user", "{:^20} {:^60} {:^40}", check.name().white().bold(), check.description(), loc);
    }
}
