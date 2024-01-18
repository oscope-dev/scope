use anyhow::Result;
use clap::CommandFactory;
use clap::{Parser, Subcommand};
use colored::Colorize;
use human_panic::setup_panic;
use lazy_static::lazy_static;
use regex::Regex;
use scope_doctor::prelude::*;
use scope_lib::prelude::{
    CaptureOpts, ConfigOptions, FoundConfig, LoggingOpts, ModelRoot, OutputCapture,
    OutputDestination, ScopeModel,
};
use scope_lib::{HelpMetadata, CONFIG_FILE_PATH_ENV, RUN_ID_ENV_VAR};
use scope_report::prelude::{report_root, ReportArgs};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use tracing::{debug, error, info};

/// (Oscilli)scope
///
/// Scope is a tool to enable teams to manage local machine
/// checks. An example would be a team that supports other
/// engineers may want to verify that the engineer asking
/// for support's machine is setup correctly.
#[derive(Parser)]
#[clap(author, version=env!("SCOPE_VERSION"), about)]
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
    List,
    #[command(external_subcommand)]
    #[allow(clippy::enum_variant_names)]
    ExternalSubCommand(Vec<String>),
}

#[tokio::main]
async fn main() {
    setup_panic!();
    dotenv::dotenv().ok();
    let opts = Cli::parse();

    let _guard = opts
        .logging
        .configure_logging(&opts.config.get_run_id(), "root");
    let error_code = run_subcommand(opts).await;

    std::process::exit(error_code);
}

async fn run_subcommand(opts: Cli) -> i32 {
    let loaded_config = match opts.config.load_config().await {
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
        Command::List => show_config(found_config).map(|_| 0),
        Command::ExternalSubCommand(args) => exec_sub_command(found_config, args).await,
    }
}

async fn exec_sub_command(found_config: &FoundConfig, args: &[String]) -> Result<i32> {
    let mut args = args.to_owned();
    let command = match args.first() {
        None => return Err(anyhow::anyhow!("Sub command not provided")),
        Some(cmd) => {
            format!("scope-{}", cmd)
        }
    };
    let _ = std::mem::replace(&mut args[0], command);

    debug!("Executing {:?}", args);

    let config_file_path = found_config.write_raw_config_to_disk()?;
    let capture = OutputCapture::capture_output(CaptureOpts {
        working_dir: &found_config.working_dir,
        args: &args,
        output_dest: OutputDestination::StandardOut,
        path: &found_config.bin_path,
        env_vars: BTreeMap::from([
            (
                CONFIG_FILE_PATH_ENV.to_string(),
                config_file_path.display().to_string(),
            ),
            (RUN_ID_ENV_VAR.to_string(), found_config.run_id.clone()),
        ]),
    })
    .await?;

    capture
        .exit_code
        .ok_or_else(|| anyhow::anyhow!("Unable to exec {}", args.join(" ")))
}

lazy_static! {
    static ref SCOPE_SUBCOMMAND_REGEX: Regex = Regex::new("^scope-.*").unwrap();
}

fn show_config(found_config: &FoundConfig) -> Result<()> {
    let mut extra_line = false;
    if !found_config.doctor_exec.is_empty() {
        info!(target: "user", "Doctor Checks");
        print_details(
            &found_config.working_dir,
            found_config.doctor_exec.values().collect(),
        );
        extra_line = true;
    }

    if !found_config.doctor_setup.is_empty() {
        info!(target: "user", "Doctor Setup");
        print_details(
            &found_config.working_dir,
            found_config.doctor_setup.values().collect(),
        );
        extra_line = true;
    }

    if !found_config.known_error.is_empty() {
        if extra_line {
            info!(target: "user", "");
        }

        info!(target: "user", "Known Errors");
        print_details(
            &found_config.working_dir,
            found_config.known_error.values().collect(),
        );
        extra_line = true;
    }

    if extra_line {
        info!(target: "user", "");
    }
    info!(target: "user", "Commands");
    print_commands(found_config);
    Ok(())
}

fn print_commands(found_config: &FoundConfig) {
    if let Ok(commands) = which::which_re_in(
        SCOPE_SUBCOMMAND_REGEX.clone(),
        Some(OsString::from(&found_config.bin_path)),
    ) {
        let mut command_map = BTreeMap::new();
        for command in commands {
            let command_name = command.file_name().unwrap().to_str().unwrap().to_string();
            let command_name = command_name.replace("scope-", "");
            command_map.entry(command_name.clone()).or_insert_with(|| {
                format!(
                    "External sub-command, run `scope {}` for help",
                    command_name
                )
            });
        }
        for command in Cli::command().get_subcommands() {
            command_map
                .entry(command.get_name().to_string())
                .or_insert_with(|| command.get_about().unwrap_or_default().to_string());
        }

        let mut command_names: Vec<_> = command_map.keys().collect();
        command_names.sort();

        info!(target: "user", "{:^20}{:^60}", "Name".white().bold(), "Description".white().bold());
        info!(target: "user", "{:^80}", str::repeat("-", 80));
        for command_name in command_names {
            let description = command_map.get(command_name.as_str()).unwrap();
            info!(target: "user", "{:^20} {:^60}", command_name.white().bold(), description);
        }
    }
}

fn print_details<T>(working_dir: &Path, config: Vec<&ModelRoot<T>>)
where
    T: HelpMetadata,
{
    info!(target: "user", "{:^20}{:^60}{:^40}", "Name".white().bold(), "Description".white().bold(), "Path".white().bold());
    info!(target: "user", "{:^120}", str::repeat("-", 120));
    for check in config {
        let mut loc = check.file_path();
        let diff_path = pathdiff::diff_paths(&loc, working_dir);
        if let Some(diff) = diff_path {
            loc = diff.display().to_string();
        } else if loc.len() > 35 {
            loc = format!("...{}", loc.split_off(loc.len() - 35));
        }
        info!(target: "user", "{:^20} {:^60} {:^40}", check.name().white().bold(), check.spec.description(), loc);
    }
}
