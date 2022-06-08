use crate::check::CheckRuntime;
use anyhow::Result;
use clap::Args;
use colored::*;
use tracing::info;

#[derive(Debug, Args)]
pub struct DoctorListArgs {
    /// Override the configuration to be used.
    #[clap(long, env = "PITY_DOCTOR_CONFIG_FILE")]
    config: Option<String>,
}

pub async fn doctor_list(args: &DoctorListArgs) -> Result<()> {
    let config = crate::config::read_config(&args.config).await?;
    info!("Loaded config {:?}", config);
    info!(target: "user", "Avaliable checks that will run");
    info!(target: "user", "{:^20}{:^40}", "Name".white().bold(), "Description".white().bold());
    for check in config.checks {
        info!(target: "user", "{:^20}{:^40}", check.name().white().bold(), check.description());
    }
    Ok(())
}
