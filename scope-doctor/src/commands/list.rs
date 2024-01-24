use anyhow::Result;
use clap::Args;
use colored::*;
use scope_lib::prelude::{FoundConfig, HelpMetadata, ScopeModel};
use tracing::info;

#[derive(Debug, Args)]
pub struct DoctorListArgs {}

pub async fn doctor_list(found_config: &FoundConfig, _args: &DoctorListArgs) -> Result<()> {
    info!(target: "user", "Available checks that will run");
    info!(target: "user", "{:<20}{:<40}", "Name".white().bold(), "Description".white().bold());
    for check in found_config.doctor_exec.values() {
        info!(target: "user", "{:<20}{:<40}", check.name().white().bold(), check.spec.description());
    }
    for check in found_config.doctor_setup.values() {
        info!(target: "user", "{:<20}{:<40}", check.name().white().bold(), check.spec.description());
    }
    Ok(())
}
