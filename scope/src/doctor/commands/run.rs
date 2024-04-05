use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};

use crate::doctor::check::{DefaultDoctorActionRun, DefaultGlobWalker};
use crate::doctor::file_cache::{FileBasedCache, FileCache, NoOpCache};
use crate::doctor::runner::{compute_group_order, RunGroups};
use crate::shared::prelude::{DefaultExecutionProvider, FoundConfig};

#[derive(Debug, Parser, Default)]
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
    let transform = transform_inputs(found_config, args);

    let all_paths = compute_group_order(&found_config.doctor_group, transform.desired_groups);
    if all_paths.is_empty() {
        warn!(target: "user", "Could not find any tasks to execute");
    }

    let run_groups = RunGroups {
        group_actions: transform.groups,
        all_paths,
    };

    let exit_code = run_groups.execute().await?;

    if let Err(e) = transform.file_cache.persist().await {
        info!("Unable to store cache {:?}", e);
        warn!(target: "user", "Unable to update cache, re-runs may redo work");
    }

    Ok(exit_code)
}

struct RunTransform {
    groups: BTreeMap<String, Vec<DefaultDoctorActionRun>>,
    desired_groups: BTreeSet<String>,
    file_cache: Arc<dyn FileCache>,
}

fn transform_inputs(found_config: &FoundConfig, args: &DoctorRunArgs) -> RunTransform {
    let mut groups = BTreeMap::new();
    let mut desired_groups = BTreeSet::new();

    let file_cache: Arc<dyn FileCache> = get_cache(args);
    let exec_runner = Arc::new(DefaultExecutionProvider::default());
    let glob_walker = Arc::new(DefaultGlobWalker::default());

    for check in found_config.doctor_group.values() {
        let should_group_run = match &args.only {
            None => check.run_by_default,
            Some(names) => names.contains(&check.metadata.name().to_string()),
        };

        let mut action_runs = Vec::new();

        for action in &check.actions {
            let run = DefaultDoctorActionRun {
                model: check.clone(),
                action: action.clone(),
                working_dir: found_config.working_dir.clone(),
                file_cache: file_cache.clone(),
                run_fix: args.fix.unwrap_or(true),
                exec_runner: exec_runner.clone(),
                glob_walker: glob_walker.clone(),
            };

            action_runs.push(run);
        }

        groups.insert(check.metadata.name().to_string(), action_runs);

        if should_group_run {
            desired_groups.insert(check.metadata.name().to_string());
        }
    }

    RunTransform {
        groups,
        desired_groups,
        file_cache,
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::doctor::commands::run::transform_inputs;
    use crate::doctor::commands::DoctorRunArgs;
    use crate::doctor::tests::{group_noop, make_root_model_additional, meta_noop};
    use crate::prelude::FoundConfig;

    #[test]
    fn test_will_include_by_default() {
        let mut fc = FoundConfig::empty(PathBuf::from("/tmp"));
        fc.doctor_group.insert(
            "included".to_string(),
            make_root_model_additional(vec![], |meta| meta.name("included"), group_noop),
        );
        let args = DoctorRunArgs {
            only: None,
            no_cache: true,
            ..Default::default()
        };

        let transform = transform_inputs(&fc, &args);
        assert_eq!(
            BTreeSet::from(["included".to_string()]),
            transform.desired_groups
        );
    }

    #[test]
    fn test_include_will_skip() {
        let mut fc = FoundConfig::empty(PathBuf::from("/tmp"));
        fc.doctor_group.insert(
            "not-included".to_string(),
            make_root_model_additional(vec![], meta_noop, |g| g.run_by_default(false)),
        );
        let args = DoctorRunArgs {
            only: None,
            no_cache: true,
            ..Default::default()
        };

        let transform = transform_inputs(&fc, &args);
        assert!(transform.desired_groups.is_empty());
    }
}
