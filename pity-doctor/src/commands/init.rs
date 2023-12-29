use anyhow::Result;
use clap::Args;
use pity_lib::prelude::{ExecCheck, ModelRoot};

#[derive(Debug, Args)]
pub struct DoctorInitArgs {
    /// Location to write the default init directory.
    output: String,
}

pub async fn doctor_init(_configs: Vec<ModelRoot<ExecCheck>>, _args: &DoctorInitArgs) -> Result<()> {
    Ok(())
}