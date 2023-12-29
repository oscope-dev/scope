use std::fs;
use anyhow::Result;
use clap::{Parser, Subcommand};
use human_panic::setup_panic;
use tracing::{error};
use pity_doctor::prelude::*;
use pity_report::prelude::{report_root, ReportArgs};
use pity_lib::prelude::{LoggingOpts, parse_config, ParsedConfig};

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
    let configs = find_configs().await?;
    match command {
        Command::Doctor(args) => doctor_root(configs, args).await,
        Command::Report(args) => report_root(args).await,
    }
}

async fn find_configs() -> Result<Vec<ParsedConfig>> {
    let mut search_dir = std::env::current_dir()?;

    let mut configs = Vec::new();

    loop {
        let pity_dir = search_dir.join(".pity");
        if pity_dir.exists() {
            for dir_entry in fs::read_dir(&pity_dir)? {
                if let Ok(entry) = dir_entry {
                    let file_contents = fs::read_to_string(entry.path())?;
                    configs.extend(parse_config(pity_dir.as_path(), &file_contents)?);
                }
            }
        }

        let parent_dir = search_dir.parent();
        if let Some(dir) = parent_dir {
            if dir == search_dir.as_path() {
                break;
            } else {
                search_dir = dir.to_path_buf();
            }
        } else {
            break;
        }
    }

    Ok(configs)
}
