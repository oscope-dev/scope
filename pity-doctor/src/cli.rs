use crate::commands::*;
use anyhow::Result;
use clap::{Args, Subcommand};
use pity_lib::prelude::ParsedConfig;

#[derive(Debug, Args)]
pub struct DoctorArgs {
    #[clap(subcommand)]
    command: DoctorCommands,
}

#[derive(Debug, Subcommand)]
enum DoctorCommands {
    /// Run checks against your machine, generating support output.
    Run(DoctorRunArgs),
    /// List all doctor config, giving you the ability to know what is possible
    List(DoctorListArgs),
    /// Create an example config file
    Init(DoctorInitArgs),
}

pub async fn doctor_root(configs: Vec<ParsedConfig>, args: &DoctorArgs) -> Result<()> {
    let mut exec_config = Vec::new();
    for raw_config in configs {
        match raw_config {
            ParsedConfig::DoctorExec(exec) => exec_config.push(exec)
        }
    }
    match &args.command {
        DoctorCommands::List(args) => doctor_list(exec_config, args).await,
        DoctorCommands::Run(args) => doctor_run(exec_config, args).await,
        DoctorCommands::Init(args) => doctor_init(exec_config, args).await,
    }
}