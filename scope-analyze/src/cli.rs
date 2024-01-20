use crate::error::AnalyzeError;
use anyhow::Result;
use clap::{Args, Subcommand};
use scope_lib::prelude::{FoundConfig, ScopeModel};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader, Stdin};
use tracing::{debug, info};

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    #[clap(subcommand)]
    command: AnalyzeCommands,
}

#[derive(Debug, Subcommand)]
enum AnalyzeCommands {
    /// Run checks against your machine, generating support output.
    #[clap(alias("log"))]
    Logs(AnalyzeLogsArgs),
}

#[derive(Debug, Args)]
struct AnalyzeLogsArgs {
    /// Location that the logs should be searched, for stdin use '-'
    location: String,
}

pub async fn analyze_root(found_config: &FoundConfig, args: &AnalyzeArgs) -> Result<i32> {
    match &args.command {
        AnalyzeCommands::Logs(args) => analyze_logs(found_config, args).await,
    }
}

async fn analyze_logs(found_config: &FoundConfig, args: &AnalyzeLogsArgs) -> Result<i32> {
    let has_known_error = match args.location.as_str() {
        "-" => process_lines(found_config, read_from_stdin().await?).await?,
        file_path => process_lines(found_config, read_from_file(file_path).await?).await?,
    };

    if has_known_error {
        Ok(1)
    } else {
        Ok(0)
    }
}

async fn process_lines<T>(found_config: &FoundConfig, input: T) -> Result<bool>
where
    T: AsyncRead,
    T: AsyncBufReadExt,
    T: Unpin,
{
    let mut has_known_error = false;
    let mut known_errors: BTreeMap<_, _> = found_config.known_error.clone();
    let mut line_number = 0;

    let mut lines = input.lines();

    while let Some(line) = lines.next_line().await? {
        let mut known_errors_to_remove = Vec::new();
        for (name, ke) in &known_errors {
            debug!("Checking known error {}", ke.name());
            if ke.spec.regex.is_match(&line) {
                info!(target: "always", "Known error '{}' found on line {}", ke.name(), line_number);
                info!(target: "always", "\t==> {}", ke.spec.help_text);
                known_errors_to_remove.push(name.clone());
                has_known_error = true;
            }
        }

        for name in known_errors_to_remove {
            known_errors.remove(&name);
        }

        line_number += 1;

        if known_errors.is_empty() {
            info!(target: "always", "All known errors detected, ignoring rest of output.");
            break;
        }
    }

    Ok(has_known_error)
}

async fn read_from_stdin() -> Result<BufReader<Stdin>, AnalyzeError> {
    Ok(BufReader::new(tokio::io::stdin()))
}

async fn read_from_file(file_name: &str) -> Result<BufReader<File>, AnalyzeError> {
    let file_path = PathBuf::from(file_name);
    if !file_path.exists() {
        return Err(AnalyzeError::FileNotFound {
            file_name: file_name.to_string(),
        });
    }
    Ok(BufReader::new(File::open(file_path).await?))
}
