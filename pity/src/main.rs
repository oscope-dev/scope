use anyhow::Result;
use clap::{Parser, Subcommand};
use directories::{BaseDirs, UserDirs};
use human_panic::setup_panic;
use pity_doctor::prelude::*;
use pity_lib::prelude::{parse_config, LoggingOpts, ParsedConfig};
use pity_report::prelude::{report_root, ReportArgs};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, warn};

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
    Report(ReportArgs),
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
    match handle_commands(&opts.command).await {
        Ok(_) => 0,
        Err(e) => {
            error!(target: "user", "Critical Error. {}", e);
            1
        }
    }
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
        configs.extend(parse_dir(pity_dir)?);

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

    if let Some(user_dirs) = UserDirs::new() {
        configs.extend(parse_dir(user_dirs.home_dir().join(".pity"))?);
    }

    if let Some(base_dirs) = BaseDirs::new() {
        configs.extend(parse_dir(base_dirs.config_dir().join(".pity"))?);
    }

    debug!(target: "user", "Found config {:?}", configs);

    Ok(configs)
}

fn parse_dir(pity_dir: PathBuf) -> Result<Vec<ParsedConfig>> {
    let mut configs = Vec::new();

    debug!(target: "user", "Searching dir {:?}", pity_dir);
    if pity_dir.exists() {
        for dir_entry in fs::read_dir(&pity_dir)? {
            if let Ok(entry) = dir_entry {
                if !entry.path().is_file() {
                    continue;
                }
                let file_path = entry.path();
                let extension = file_path.extension();
                if extension == Some(OsStr::new("yaml")) || extension == Some(OsStr::new("yml")) {
                    debug!(target: "user", "Found file {:?}", file_path);
                    let file_contents = fs::read_to_string(entry.path())?;
                    match parse_config(pity_dir.as_path(), &file_contents) {
                        Ok(parsed) => configs.extend(parsed),
                        Err(e) => {
                            warn!(target: "user", "Unable to parse {:?}: {}", entry.path(), e);
                        }
                    }
                }
            }
        }
    }

    Ok(configs)
}
