use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct DoctorInitArgs {
    /// Location to write the default init directory.
    output: String,
}

pub async fn doctor_init(args: &DoctorInitArgs) -> Result<()> {
    Ok(())
}