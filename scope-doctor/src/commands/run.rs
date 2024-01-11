use crate::check::{CacheResults, CheckRuntime, DoctorTypes};
use crate::file_cache::{CacheStorage, FileBasedCache, FileCache, NoOpCache};
use anyhow::Result;
use clap::Parser;
use colored::*;
use scope_lib::prelude::{FoundConfig, ScopeModel};
use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::PathBuf;
use tracing::{debug, info, warn};

#[derive(Debug, Parser)]
pub struct DoctorRunArgs {
    /// When set, only the checks listed will run
    #[arg(short, long)]
    pub only: Option<Vec<String>>,
    /// When set, if a fix is specified it will also run.
    #[arg(long, short, default_value = "true")]
    fix: bool,
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
                warn!(target: "user", "Check {} has multiple definitions, only the last will be processed.", old.name().bold());
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

    for model in checks_to_run {
        debug!(target: "user", "Running check {}", model.name());

        let exec_result = model.check_cache(found_config, cache.deref()).await?;
        match exec_result {
            CacheResults::FixRequired => {
                handle_check_failure(args.fix, found_config, model, cache.deref()).await?;
                should_pass = false;
            }
            CacheResults::NoWorkNeeded => {
                info!(target: "user", "Check {} was successful", model.name().bold());
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
) -> Result<()> {
    if check.has_correction() {
        warn!(target: "user", "Check {} failed. {}: {}", check.name().bold(), "Suggestion".bold(), check.help_text());
        return Ok(());
    };

    if !is_fix {
        info!(target: "user", "Check {} failed. {}: Run with --fix to auto-fix", check.name().bold(), "Suggestion".bold());
        return Ok(());
    }

    check.run_correction(found_config, cache).await?;

    Ok(())
}
