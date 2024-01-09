use crate::check::{CacheResults, CheckRuntime};
use anyhow::Result;
use clap::Parser;
use colored::*;
use scope_lib::prelude::{
    DoctorExec, FoundConfig, ModelRoot,
};
use std::collections::BTreeMap;
use tracing::{debug, error, info, warn};

#[derive(Debug, Parser)]
pub struct DoctorRunArgs {
    /// When set, only the checks listed will run
    #[arg(short, long)]
    only: Option<Vec<String>>,
    /// When set, if a fix is specified it will also run.
    #[arg(long, short, default_value = "true")]
    fix: bool,
}

pub async fn doctor_run(found_config: &FoundConfig, args: &DoctorRunArgs) -> Result<i32> {
    let mut check_map: BTreeMap<String, ModelRoot<DoctorExec>> = Default::default();
    let mut check_order: Vec<String> = Default::default();
    for check in found_config.doctor_exec.values() {
        let name = check.name();
        if let Some(old) = check_map.insert(name.to_string(), check.clone()) {
            warn!(target: "user", "Check {} has multiple definitions, only the last will be processed.", old.name().bold());
        } else {
            check_order.push(name.to_string());
        }
    }

    let checks_names_to_run = match &args.only {
        Some(only_run) => only_run.clone(),
        None => check_order,
    };

    let mut should_pass = true;

    for check_name in checks_names_to_run {
        debug!(target: "user", "Running check {}", check_name);
        let check = match check_map.get(&check_name) {
            None => {
                error!(target: "user", "Check {} was not found, skipping!.", check_name.bold());
                continue;
            }
            Some(check) => check,
        };

        let exec_result = check.check_cache(found_config).await?;
        match exec_result {
            CacheResults::FixRequired => {
                handle_check_failure(args.fix, found_config, check).await?;
                should_pass = false;
            }
            CacheResults::NoWorkNeeded => {
                info!(target: "user", "Check {} was successful", check_name.bold());
            }
        }
    }

    if should_pass {
        Ok(0)
    } else {
        Ok(1)
    }
}

async fn handle_check_failure(
    is_fix: bool,
    found_config: &FoundConfig,
    check: &ModelRoot<DoctorExec>,
) -> Result<()> {
    if check.spec.fix_exec.is_none() {
        warn!(target: "user", "Check {} failed. {}: {}", check.name().bold(), "Suggestion".bold(), check.help_text());
        return Ok(());
    };

    if !is_fix {
        info!(target: "user", "Check {} failed. {}: Run with --fix to auto-fix", check.name().bold(), "Suggestion".bold());
        return Ok(());
    }

    check.run_correction(found_config).await?;

    Ok(())
}
