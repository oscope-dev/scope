use anyhow::Result;
use clap::{Parser, Subcommand};
use human_panic::setup_panic;
use tracing::{error};
use pity_doctor::prelude::*;
use pity_report::prelude::{report_root, ReportArgs};
use pity_lib::prelude::{LoggingOpts};

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

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run checks that will "checkup" your machine.
    Doctor(DoctorArgs),
    /// Generate a bug report based from a command that was ran
    Report(ReportArgs)
}

#[tokio::main]
async fn main() {
    setup_panic!();
    dotenv::dotenv().ok();
    let opts = Cli::parse();

    let _guard = opts.logging.configure_logging("root");
    let error_code = match handle_commands(&opts.command).await {
        Ok(_) => 0,
        Err(e) => {
            error!("Critical Error. {}", e);
            1
        }
    };

    std::process::exit(error_code);
}

async fn handle_commands(command: &Command) -> Result<()> {
    match command {
        Command::Doctor(args) => doctor_root(args).await,
        Command::Report(args) => report_root(args).await,
    }
}
