use crate::check::CheckRuntime;
use anyhow::Result;
use clap::Parser;
use colored::*;
use scope_lib::prelude::{
    DoctorExecCheckSpec, FoundConfig, ModelRoot, OutputCapture, OutputDestination,
};
use std::collections::BTreeMap;
use std::path::Path;
use tracing::{debug, error, info, warn};

#[derive(Debug, Parser)]
pub struct DoctorRunArgs {
    /// When set, only the checks listed will run
    #[arg(short, long)]
    only: Option<Vec<String>>,
    /// When set, if a fix is specified it will also run.
    #[arg(long, short, default_value = "false")]
    fix: bool,
}

pub async fn doctor_run(found_config: &FoundConfig, args: &DoctorRunArgs) -> Result<i32> {
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

        let exec_result = check.exec(&found_config.working_dir).await?;
        info!(check = %check_name, output= "stdout", successful=exec_result.success, "{}", exec_result.stdout);
        info!(check = %check_name, output= "stderr", successful=exec_result.success, "{}", exec_result.stderr);
        if exec_result.success {
            info!(target: "user", "Check {} was successful", check_name.bold());
        } else {
            handle_check_failure(args.fix, &found_config.working_dir, check).await?;
            should_pass = false;
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
    working_dir: &Path,
    check: &ModelRoot<DoctorExecCheckSpec>,
) -> Result<()> {
    let check_path = match &check.spec.fix_exec {
        None => {
            warn!(target: "user", "Check {} failed. {}: {}", check.name().bold(), "Suggestion".bold(), check.help_text());
            return Ok(());
        }
        Some(path) => path.to_string(),
    };

    if !is_fix {
        info!(target: "user", "Check {} failed. {}: Run with --fix to auto-fix", check.name().bold(), "Suggestion".bold());
        return Ok(());
    }

    let args = vec![check_path];
    let capture =
        OutputCapture::capture_output(working_dir, &args, &OutputDestination::StandardOut).await?;

    if capture.exit_code == Some(0) {
        info!(target: "user", "Check {} failed. {} ran successfully", check.name().bold(), "Fix".bold());
        Ok(())
    } else {
        warn!(target: "user", "Check {} failed. The fix ran and {}.", check.name().bold(), "Failed".red().bold());
        Ok(())
    }
}
