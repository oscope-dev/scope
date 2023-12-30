use crate::check::CheckRuntime;
use anyhow::Result;
use clap::Parser;
use colored::*;
use pity_lib::prelude::{DoctorExecCheckSpec, FoundConfig, ModelRoot};
use std::collections::BTreeMap;
use tracing::{error, info, warn};

#[derive(Debug, Parser)]
pub struct DoctorRunArgs {
    /// When set, only the checks listed will run
    #[arg(short, long)]
    only: Option<Vec<String>>,
}

pub async fn doctor_run(found_config: &FoundConfig, args: &DoctorRunArgs) -> Result<()> {
    let mut check_map: BTreeMap<String, ModelRoot<DoctorExecCheckSpec>> = Default::default();
    let mut check_order: Vec<String> = Default::default();
    for check in found_config.exec_check.values() {
        let name = check.name();
        if let Some(old) = check_map.insert(name.clone(), check.clone()) {
            warn!(target: "user", "Check {} has multiple definitions, only the last will be processed.", old.name().bold());
        } else {
            check_order.push(name);
        }
    }

    let checks_names_to_run = match &args.only {
        Some(only_run) => only_run.clone(),
        None => check_order,
    };

    for check_name in checks_names_to_run {
        let check = match check_map.get(&check_name) {
            None => {
                error!(target: "user", "Check {} was not found, skipping!.", check_name.bold());
                continue;
            }
            Some(check) => check,
        };

        let exec_result = check.exec().await?;
        info!(check = %check_name, output= "stdout", successful=exec_result.success, "{}", exec_result.stdout);
        info!(check = %check_name, output= "stderr", successful=exec_result.success, "{}", exec_result.stderr);
        if exec_result.success {
            info!(target: "user", "Check {} was successful", check_name.bold());
        } else {
            warn!(target: "user", "Check {} failed. {}: {}", check_name.bold(), "Suggestion".bold(), check.help_text());
        }
    }

    Ok(())
}
