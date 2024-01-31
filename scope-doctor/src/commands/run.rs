use crate::check::{ActionRunResult, DefaultGlobWalker, DoctorActionRun};
use crate::file_cache::{FileBasedCache, FileCache, NoOpCache};
use anyhow::Result;
use clap::Parser;
use colored::*;
use scope_lib::prelude::{
    DefaultExecutionProvider, DoctorGroup, FoundConfig, ModelRoot, ScopeModel,
};
use std::collections::BTreeMap;

use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

#[derive(Debug, Parser)]
pub struct DoctorRunArgs {
    /// When set, only the checks listed will run
    #[arg(short, long)]
    pub only: Option<Vec<String>>,
    /// When set, if a fix is specified it will also run.
    #[arg(long, short, default_value = "true")]
    fix: Option<bool>,
    /// Location to store cache between runs
    #[arg(long, env = "SCOPE_DOCTOR_CACHE_DIR")]
    pub cache_dir: Option<String>,
    /// When set cache will be disabled, forcing all file based checks to run.
    #[arg(long, short, default_value = "false")]
    pub no_cache: bool,
}

fn get_cache(args: &DoctorRunArgs) -> Arc<dyn FileCache> {
    if args.no_cache {
        Arc::<NoOpCache>::default()
    } else {
        let cache_dir = args
            .cache_dir
            .clone()
            .unwrap_or_else(|| "/tmp/scope".to_string());
        let cache_path = PathBuf::from(cache_dir).join("cache-file.json");
        match FileBasedCache::new(&cache_path) {
            Ok(cache) => Arc::new(cache),
            Err(e) => {
                warn!("Unable to create cache {:?}", e);
                Arc::<NoOpCache>::default()
            }
        }
    }
}

pub async fn doctor_run(found_config: &FoundConfig, args: &DoctorRunArgs) -> Result<i32> {
    let mut check_map: BTreeMap<String, ModelRoot<DoctorGroup>> = Default::default();
    for check in found_config.doctor_group.values() {
        let should_group_run = match &args.only {
            None => true,
            Some(names) => names.contains(&check.name().to_string()),
        };

        if should_group_run {
            if let Some(old) = check_map.insert(check.full_name(), check.clone()) {
                warn!(target: "user", "Check {} has multiple definitions, only the last will be processed.", old.name().bold());
            }
        }
    }

    let cache: Arc<dyn FileCache> = get_cache(args);

    let checks_to_run: Vec<_> = check_map.values().collect();

    let mut should_pass = true;
    let mut skip_remaining = false;
    let exec_runner = Arc::new(DefaultExecutionProvider::default());
    let glob_walker = Arc::new(DefaultGlobWalker::default());

    for model in checks_to_run {
        debug!(target: "user", "Running check {}", model.name());

        if skip_remaining {
            warn!(target: "user", "Check `{}` was skipped.", model.name().bold());
            continue;
        }

        for action in &model.spec.actions {
            let run = DoctorActionRun {
                model: model.clone(),
                action: action.clone(),
                working_dir: found_config.working_dir.clone(),
                file_cache: cache.clone(),
                run_fix: args.fix.unwrap_or(true),
                exec_runner: exec_runner.clone(),
                glob_walker: glob_walker.clone(),
            };

            let action_result = run.run_action().await?;

            match action_result {
                ActionRunResult::CheckSucceeded => {
                    info!(target: "user", group = model.name(), name = action.name, "Check was successful");
                }
                ActionRunResult::CheckFailedFixSucceedVerifySucceed => {
                    info!(target: "user", group = model.name(), name = action.name, "Check initially failed, fix was successful");
                }
                ActionRunResult::CheckFailedFixFailed => {
                    error!(target: "user", group = model.name(), name = action.name, "Check failed, fix ran and {}", "failed".red().bold());
                }
                ActionRunResult::CheckFailedFixSucceedVerifyFailed => {
                    error!(target: "user", group = model.name(), name = action.name, "Check initially failed, fix ran, verification {}", "failed".red().bold());
                }
                ActionRunResult::CheckFailedNoRunFix => {
                    info!(target: "user", group = model.name(), name = action.name, "Check failed, fix was not run");
                }
                ActionRunResult::CheckFailedNoFixProvided => {
                    error!(target: "user", group = model.name(), name = action.name, "Check failed, no fix provided");
                }
                ActionRunResult::CheckFailedFixFailedStop => {
                    error!(target: "user", group = model.name(), name = action.name, "Check failed, fix ran and {} and aborted", "failed".red().bold());
                }
            }

            if action_result.is_failure() {
                if let Some(help_text) = &action.fix.help_text {
                    error!(target: "user", group = model.name(), name = action.name, "Action Help: {}", help_text);
                }
                if let Some(help_url) = &action.fix.help_url {
                    error!(target: "user", group = model.name(), name = action.name, "For more help, please visit {}", help_url);
                }
            }

            match action_result {
                ActionRunResult::CheckSucceeded
                | ActionRunResult::CheckFailedFixSucceedVerifySucceed => {}
                ActionRunResult::CheckFailedFixFailedStop => {
                    should_pass = false;
                    skip_remaining = true;
                }
                _ => {
                    if action.required {
                        skip_remaining = true;
                    }
                    should_pass = false;
                }
            }
        }
    }

    if let Err(e) = cache.persist().await {
        info!("Unable to store cache {:?}", e);
        warn!(target: "user", "Unable to update cache, re-runs may redo work");
    }

    if should_pass {
        Ok(0)
    } else {
        Ok(1)
    }
}
