use crate::commands::*;
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
    Init(DoctorInitArgs),
}

pub async fn doctor_root(args: &DoctorArgs) -> Result<()> {
    match &args.command {
        DoctorCommands::List(args) => doctor_list(args).await,
        DoctorCommands::Run(args) => doctor_run(args).await,
        DoctorCommands::Init(args) => doctor_init(args).await,
    }
}