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
use tracing::{debug, info, warn};

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

    let cache: Box<dyn FileCache> = if args.no_cache {
        Box::<NoOpCache>::default()
    } else {
        let cache_dir = args
            .cache_dir
            .clone()
            .unwrap_or_else(|| "/tmp/scope".to_string());
        let cache_path = PathBuf::from(cache_dir).join("cache-file.json");
        Box::new(FileBasedCache::new(&cache_path)?)
    };

    let checks_to_run: Vec<_> = check_map.values().collect();

    let mut should_pass = true;
    let mut skip_remaining = false;

    for model in checks_to_run {
        debug!(target: "user", "Running check {}", model.name());

        if skip_remaining {
            warn!(target: "user", "Check `{}` was skipped.", model.name().bold());
            continue;
        }

        for action in &model.spec.actions {
            let run = DoctorActionRun {
                model,
                action,
                working_dir: &found_config.working_dir,
                file_cache: &cache,
                run_fix: args.fix.unwrap_or(true),
                exec_runner: &DefaultExecutionProvider::default(),
                glob_walker: &DefaultGlobWalker::default(),
            };

            match run.run_action().await? {
                ActionRunResult::Stop => {
                    skip_remaining = true;
                    should_pass = false;
                    break;
                }
                ActionRunResult::Failed => {
                    should_pass = false;
                }
                _ => {}
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
