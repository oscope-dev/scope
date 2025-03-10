use super::error::AnalyzeError;
use crate::models::HelpMetadata;
use crate::prelude::{
    generate_env_vars, CaptureError, CaptureOpts, DefaultExecutionProvider, DoctorFix,
    ExecutionProvider, OutputCapture, OutputDestination,
};
use crate::shared::prelude::FoundConfig;
use anyhow::Result;
use clap::{Args, Subcommand};
use std::collections::BTreeMap;
use std::env;
use std::io::Cursor;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader, Stdin};
use tracing::{debug, error, info, warn};

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

    /// Runs a command and detects errors in the output
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
    /// The command to run
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

pub async fn analyze_root(found_config: &FoundConfig, args: &AnalyzeArgs) -> Result<i32> {
    match &args.command {
        AnalyzeCommands::Logs(args) => analyze_logs(found_config, args).await,
        AnalyzeCommands::Command(args) => analyze_command(found_config, args).await,
    }
}

async fn analyze_logs(found_config: &FoundConfig, args: &AnalyzeLogsArgs) -> Result<i32> {
    let result = match args.location.as_str() {
        "-" => process_lines(found_config, read_from_stdin().await?).await?,
        file_path => process_lines(found_config, read_from_file(file_path).await?).await?,
    };

    report_result(&result);
    Ok(result.to_exit_code())
}

async fn analyze_command(found_config: &FoundConfig, args: &AnalyzeCommandArgs) -> Result<i32> {
    let exec_runner = DefaultExecutionProvider::default();

    let command = args.command.clone();
    let path = env::var("PATH").unwrap_or_default();

    let capture_opts: CaptureOpts = CaptureOpts {
        working_dir: &found_config.working_dir,
        env_vars: Default::default(),
        path: &path,
        args: &command,
        output_dest: OutputDestination::StandardOutWithPrefix("analyzing".to_string()),
    };

    let result = process_lines(
        found_config,
        read_from_command(&exec_runner, capture_opts).await?,
    )
    .await?;

    report_result(&result);
    Ok(result.to_exit_code())
}

fn report_result(status: &AnalyzeStatus) {
    match status {
        AnalyzeStatus::NoKnownErrorsFound => info!(target: "always", "No known errors found"),
        AnalyzeStatus::KnownErrorFoundNoFixFound => {
            info!(target: "always", "No automatic fix available")
        }
        AnalyzeStatus::KnownErrorFoundUserDenied => warn!(target: "always", "User denied fix"),
        AnalyzeStatus::KnownErrorFoundFixFailed => error!(target: "always", "Fix failed"),
        AnalyzeStatus::KnownErrorFoundFixSucceeded => info!(target: "always", "Fix succeeded"),
    }
}

async fn process_lines<T>(found_config: &FoundConfig, input: T) -> Result<AnalyzeStatus>
where
    T: AsyncRead,
    T: AsyncBufReadExt,
    T: Unpin,
{
    let mut result = AnalyzeStatus::NoKnownErrorsFound;
    let mut known_errors: BTreeMap<_, _> = found_config.known_error.clone();
    let mut line_number = 0;

    let mut lines = input.lines();

    while let Some(line) = lines.next_line().await? {
        let mut known_errors_to_remove = Vec::new();
        for (name, ke) in &known_errors {
            debug!("Checking known error {}", ke.name());
            if ke.regex.is_match(&line) {
                warn!(target: "always", "Known error '{}' found on line {}", ke.name(), line_number);
                info!(target: "always", "\t==> {}", ke.help_text);

                result = match &ke.fix {
                    Some(fix) => {
                        info!(target: "always", "found a fix!");

                        tracing_indicatif::suspend_tracing_indicatif(|| {
                            let exec_path = ke.metadata.exec_path();
                            prompt_and_run_fix(&found_config.working_dir, exec_path, fix)
                        })
                        .await?
                    }
                    None => AnalyzeStatus::KnownErrorFoundNoFixFound,
                };

                known_errors_to_remove.push(name.clone());
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

    Ok(result)
}

async fn prompt_and_run_fix(
    working_dir: &PathBuf,
    exec_path: String,
    fix: &DoctorFix,
) -> Result<AnalyzeStatus> {
    let fix_prompt = &fix.prompt.as_ref();
    let prompt_text = fix_prompt
        .map(|p| p.text.clone())
        .unwrap_or("Would you like to run it?".to_string());
    let extra_context = &fix_prompt.map(|p| p.extra_context.clone()).flatten();

    let prompt = {
        let base_prompt = inquire::Confirm::new(&prompt_text).with_default(false);
        match extra_context {
            Some(help_text) => base_prompt.with_help_message(help_text),
            None => base_prompt,
        }
    };

    if prompt.prompt().unwrap() {
        let outputs = run_fix(working_dir, &exec_path, fix).await?;
        // failure indicates an issue with us actually executing it,
        // not the success/failure of the command itself.
        let max_exit_code = outputs
            .iter()
            .map(|c| c.exit_code.unwrap_or(-1))
            .max()
            .unwrap();

        match max_exit_code {
            0 => Ok(AnalyzeStatus::KnownErrorFoundFixSucceeded),
            _ => {
                if let Some(help_text) = &fix.help_text {
                    error!(target: "user", "Fix Help: {}", help_text);
                }
                if let Some(help_url) = &fix.help_url {
                    error!(target: "user", "For more help, please visit {}", help_url);
                }

                Ok(AnalyzeStatus::KnownErrorFoundFixFailed)
            }
        }
    } else {
        Ok(AnalyzeStatus::KnownErrorFoundUserDenied)
    }
}

async fn run_fix(
    working_dir: &PathBuf,
    exec_path: &str,
    fix: &DoctorFix,
) -> Result<Vec<OutputCapture>> {
    let exec_runner = DefaultExecutionProvider::default();

    let commands = fix.command.as_ref().expect("Expected a command");

    let mut outputs = Vec::<OutputCapture>::new();
    for cmd in commands.expand() {
        let capture_opts = CaptureOpts {
            working_dir,
            args: &[cmd],
            output_dest: OutputDestination::StandardOutWithPrefix("fixing".to_string()),
            path: exec_path,
            env_vars: generate_env_vars(),
        };
        let output = exec_runner.run_command(capture_opts).await?;
        let exit_code = output.exit_code.expect("Expected an exit code");
        outputs.push(output);
        if exit_code != 0 {
            break;
        }
    }

    Ok(outputs)
}

async fn read_from_command(
    exec_runner: &DefaultExecutionProvider,
    capture_opts: CaptureOpts<'_>,
) -> Result<BufReader<Cursor<String>>, CaptureError> {
    let output = exec_runner.run_command(capture_opts).await?;

    let cursor = Cursor::new(output.generate_user_output());

    Ok(BufReader::new(cursor))
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

#[derive(Copy, Clone)]
enum AnalyzeStatus {
    NoKnownErrorsFound,
    KnownErrorFoundNoFixFound,
    KnownErrorFoundUserDenied,
    KnownErrorFoundFixFailed,
    KnownErrorFoundFixSucceeded,
}

impl AnalyzeStatus {
    fn to_exit_code(self) -> i32 {
        match self {
            // we need this to return a success code
            AnalyzeStatus::KnownErrorFoundFixSucceeded => 0,
            // all others can return their discriminant value
            status => status as i32,
        }
    }
}
