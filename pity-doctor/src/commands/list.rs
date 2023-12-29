use crate::check::CheckRuntime;
use anyhow::Result;
use clap::Args;
use colored::*;
use pity_lib::prelude::{ExecCheck, ModelRoot};
use tracing::info;

#[derive(Debug, Args)]
pub struct DoctorListArgs {}

pub async fn doctor_list(configs: Vec<ModelRoot<ExecCheck>>, _args: &DoctorListArgs) -> Result<()> {
    info!("Loaded config {:?}", configs);
    info!(target: "user", "Available checks that will run");
    info!(target: "user", "{:^20}{:^40}", "Name".white().bold(), "Description".white().bold());
    for check in configs {
        info!(target: "user", "{:^20}{:^40}", check.name().white().bold(), check.description());
    }
    Ok(())
}
