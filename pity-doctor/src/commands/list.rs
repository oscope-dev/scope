use crate::check::CheckRuntime;
use anyhow::Result;
use clap::Args;
use colored::*;
use pity_lib::prelude::{ExecCheck, FoundConfig, ModelRoot};
use tracing::info;

#[derive(Debug, Args)]
pub struct DoctorListArgs {}

pub async fn doctor_list(found_config: &FoundConfig, _args: &DoctorListArgs) -> Result<()> {
    info!(target: "user", "Available checks that will run");
    info!(target: "user", "{:^20}{:^40}", "Name".white().bold(), "Description".white().bold());
    for check in found_config.exec_check.values() {
        info!(target: "user", "{:^20}{:^40}", check.name().white().bold(), check.description());
    }
    Ok(())
}
