use super::commands::*;
use crate::shared::prelude::FoundConfig;
use anyhow::Result;
use clap::{Args, Subcommand};

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
    #[command(hide(true))]
    Init(DoctorInitArgs),
}

pub async fn doctor_root(found_config: &FoundConfig, args: &DoctorArgs) -> Result<i32> {
    match &args.command {
        DoctorCommands::List(args) => doctor_list(found_config, args).await.map(|_| 0),
        DoctorCommands::Run(args) => doctor_run(found_config, args).await,
        DoctorCommands::Init(args) => doctor_init(found_config, args).await.map(|_| 0),
    }
}
