use crate::check::{ActionRunResult, DefaultGlobWalker, DefaultDoctorActionRun};
use crate::file_cache::{FileBasedCache, FileCache, NoOpCache};
use anyhow::Result;
use clap::Parser;
use colored::*;
use scope_lib::prelude::{
    DefaultExecutionProvider, DoctorGroup, FoundConfig, ModelRoot, ScopeModel,
};
use std::collections::{BTreeMap, BTreeSet};

use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use crate::runner::RunGroups;

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
    let mut desired_groups = BTreeSet::new();

    for check in found_config.doctor_group.values() {
        let should_group_run = match &args.only {
            None => true,
            Some(names) => names.contains(&check.name().to_string()),
        };

        if should_group_run {
            desired_groups.insert(check.name().to_string());
        }
    }

    let cache: Arc<dyn FileCache> = get_cache(args);
    let exec_runner = Arc::new(DefaultExecutionProvider::default());
    let glob_walker = Arc::new(DefaultGlobWalker::default());

    let run_groups = RunGroups {
        groups: found_config.doctor_group.clone(),
        desired_groups,
        file_cache: cache,
        working_dir: found_config.working_dir.clone(),
        run_fixes: args.fix.unwrap_or(true),
        exec_runner,
        glob_walker,
    };

    Ok(run_groups.execute().await?)
}
