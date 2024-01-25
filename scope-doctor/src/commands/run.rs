use crate::check::{CacheResults, CheckRuntime, CorrectionResults, DoctorTypes};
use crate::file_cache::{CacheStorage, FileBasedCache, FileCache, NoOpCache};
use anyhow::Result;
use clap::Parser;
use colored::*;
use scope_lib::prelude::{FoundConfig, ScopeModel};
use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::PathBuf;
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

pub async fn doctor_run(found_config: &FoundConfig, args: &DoctorRunArgs) -> Result<i32> {
    let mut check_map: BTreeMap<String, DoctorTypes> = Default::default();
    for check in found_config.doctor_exec.values() {
        if check.should_run_check(args) {
            if let Some(old) = check_map.insert(check.full_name(), DoctorTypes::Exec(check.clone()))
            {
                warn!(target: "user", "Check {} has multiple definitions, only the last will be processed.", old.name().bold());
            }
        }
    }

    for check in found_config.doctor_setup.values() {
        if check.should_run_check(args) {
            if let Some(old) =
                check_map.insert(check.full_name(), DoctorTypes::Setup(check.clone()))
            {
                warn!(target: "user", "Check `{}` has multiple definitions, only the last will be processed.", old.name().bold());
            }
        }
    }

    let cache = if args.no_cache {
        CacheStorage::NoCache(NoOpCache::default())
    } else {
        let cache_dir = args
            .cache_dir
            .clone()
            .unwrap_or_else(|| "/tmp/scope".to_string());
        let cache_path = PathBuf::from(cache_dir).join("cache-file.json");
        CacheStorage::File(FileBasedCache::new(&cache_path)?)
    };

    let mut checks_to_run: Vec<_> = check_map.values().collect();
    checks_to_run.sort_by_key(|l| l.order());

    let mut should_pass = true;
    let mut skip_remaining = false;

    for model in checks_to_run {
        debug!(target: "user", "Running check {}", model.name());

        if skip_remaining {
            warn!(target: "user", "Check `{}` was skipped.", model.name().bold());
            continue;
        }

        let exec_result = model.check_cache(found_config, cache.deref()).await?;
        match exec_result {
            CacheResults::FixRequired => {
                skip_remaining = handle_check_failure(
                    args.fix.unwrap_or(true),
                    found_config,
                    model,
                    cache.deref(),
                )
                .await?;
                should_pass = false;
            }
            CacheResults::CheckSucceeded => {
                info!(target: "user", "Check `{}` was successful.", model.name().bold());
            }
            CacheResults::FilesNotChanged => {
                info!(target: "user", "Check `{}` cache valid.", model.name().bold());
            }
            CacheResults::StopExecution => {
                error!(target: "user", "Check `{}` has failed and wants to stop execution. All other checks will be skipped.", model.name().bold());
                skip_remaining = true;
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

async fn handle_check_failure(
    is_fix: bool,
    found_config: &FoundConfig,
    check: &DoctorTypes,
    cache: &dyn FileCache,
) -> Result<bool> {
    if !check.has_correction() {
        error!(target: "user", "Check `{}` failed. {}: {}", check.name().bold(), "Suggestion".bold(), check.help_text());
        return Ok(true);
    };

    if !is_fix {
        info!(target: "user", "Check `{}` failed. {}: Run with --fix to auto-fix", check.name().bold(), "Suggestion".bold());
        return Ok(true);
    }

    let correction_result = check.run_correction(found_config, cache).await?;
    let continue_executing = match correction_result {
        CorrectionResults::Success => {
            info!(target: "user", "Check `{}` failed. {} ran successfully.", check.name().bold(), "Fix".bold());
            true
        }
        CorrectionResults::Failure => {
            error!(target: "user", "Check `{}` failed. The fix ran and {}.", check.name().bold(), "Failed".red().bold());
            true
        }
        CorrectionResults::FailAndStop => {
            error!(target: "user", "Check `{}` failed. The fix ran and {}. The fix exited with a 'stop' code, skipping remaining checks.", check.name().bold(), "Failed".red().bold());
            false
        }
    };

    Ok(continue_executing)
}
