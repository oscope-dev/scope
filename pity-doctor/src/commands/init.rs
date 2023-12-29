use anyhow::Result;
use clap::Args;
use pity_lib::prelude::FoundConfig;

#[derive(Debug, Args)]
pub struct DoctorInitArgs {
    /// Location to write the default init directory.
    output: String,
}

pub async fn doctor_init(_found_config: &FoundConfig, _args: &DoctorInitArgs) -> Result<()> {
    Ok(())
}
