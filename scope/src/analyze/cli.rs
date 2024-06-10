use super::error::AnalyzeError;
use crate::models::HelpMetadata;
use crate::prelude::{CaptureOpts, DefaultExecutionProvider, ExecutionProvider};
use crate::shared::prelude::FoundConfig;
use anyhow::Result;
use clap::{Args, Subcommand};
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader, Stdin, Stdout};
use tokio::process::ChildStdout;
use tracing::{debug, info, warn};

#[derive(Debug, Args)]
pub struct AnalyzeArgs {
    #[clap(subcommand)]
    command: AnalyzeCommands,
}

#[derive(Debug, Subcommand)]
enum AnalyzeCommands {
    /// Reads a log file and detects errors in it
    #[clap(alias("log"))]
    Logs(AnalyzeLogsArgs),

    /// Runs a command and detects errors in it's output
    #[clap()]
    Command(AnalyzeCommandArgs),
}

#[derive(Debug, Args)]
struct AnalyzeLogsArgs {
    /// Location that the logs should be searched, for stdin use '-'
    location: String,
}

#[derive(Debug, Args)]
struct AnalyzeCommandArgs {
    /// Command to run
    command: Vec<String>,
}

pub async fn analyze_root(found_config: &FoundConfig, args: &AnalyzeArgs) -> Result<i32> {
    match &args.command {
        AnalyzeCommands::Logs(args) => analyze_logs(found_config, args).await,
        AnalyzeCommands::Command(args) => analyze_command(found_config, args).await,
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

async fn analyze_command(found_config: &FoundConfig, args: &AnalyzeCommandArgs) -> Result<i32> {
    let exec_runner = DefaultExecutionProvider::default();

    let capture_opts: CaptureOpts = CaptureOpts {
        working_dir: &found_config.working_dir,
        env_vars: Default::default(),
        path: &env::var("PATH").unwrap_or_default(),
        args: &args.command,
        output_dest: crate::prelude::OutputDestination::StandardOut,
    };

    let has_known_error = process_lines(
        found_config,
        read_from_command(&capture_opts, &exec_runner).await?,
    )
    .await?;

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
        println!("Line: {:?}", line);
        let mut known_errors_to_remove = Vec::new();
        for (name, ke) in &known_errors {
            debug!("Checking known error {}", ke.name());
            if ke.regex.is_match(&line) {
                warn!(target: "always", "Known error '{}' found on line {}", ke.name(), line_number);
                info!(target: "always", "\t==> {}", ke.help_text);
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

async fn read_from_command(
    capture_opts: &CaptureOpts<'_>,
    exec_runner: &DefaultExecutionProvider,
) -> Result<BufReader<ChildStdout>, AnalyzeError> {
    let (stdout, _) = exec_runner.run_command_async(capture_opts).await;

    // TODO: combine stdout and stderr
    Ok(BufReader::new(stdout))
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
