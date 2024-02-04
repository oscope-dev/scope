use crate::check::{ActionRunResult, DefaultGlobWalker, DefaultDoctorActionRun, GlobWalker, DoctorActionRun};
use crate::file_cache::FileCache;
use anyhow::Result;
use colored::Colorize;
use petgraph::algo::all_simple_paths;
use petgraph::dot::{Config, Dot};
use petgraph::visit::NodeRef;
use petgraph::{algo, prelude::*};
use scope_lib::prelude::{
    DefaultExecutionProvider, DoctorGroup, ExecutionProvider, ModelRoot, ScopeModel,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

#[derive(Debug, Default)]
struct PathRunResult {
    did_succeed: bool,
    succeeded_groups: Vec<String>,
    failed_group: Vec<String>,
    skipped_group: Vec<String>,
}

pub struct RunGroups {
    pub(crate) groups: BTreeMap<String, dyn DoctorActionRun>,
    pub(crate) desired_groups: BTreeSet<String>,
    pub(crate) file_cache: Arc<dyn FileCache>,
    pub(crate) working_dir: PathBuf,
    pub(crate) run_fixes: bool,
    pub(crate) exec_runner: Arc<dyn ExecutionProvider>,
    pub(crate) glob_walker: Arc<dyn GlobWalker>,
}

impl RunGroups {
    pub async fn execute(&self) -> Result<i32> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let group_paths = self.compute_group_order()?;

        let mut did_succeed = true;

        for path in group_paths {
            let mut full_path = Vec::new();
            for target_group in path {
                if visited.contains(&target_group) {
                    info!("{} has already been run", target_group);
                    continue;
                }
                if let Some(model) = self.groups.get(&target_group) {
                    full_path.push(model);
                }
            }

            let result = self.run_path(full_path).await?;
            for successful_model in result.succeeded_groups {
                visited.insert(successful_model);
            }
            did_succeed = did_succeed && result.did_succeed;
        }

        if let Err(e) = self.file_cache.persist().await {
            info!("Unable to store cache {:?}", e);
            warn!(target: "user", "Unable to update cache, re-runs may redo work");
        }

        if did_succeed {
            Ok(0)
        } else {
            Ok(1)
        }
    }

    async fn run_path(&self, path: Vec<&ModelRoot<DoctorGroup>>) -> Result<PathRunResult> {
        let mut skip_remaining = false;
        let mut run_result = PathRunResult {
            did_succeed: true,
            succeeded_groups: vec![],
            failed_group: vec![],
            skipped_group: vec![],
        };

        for model in path {
            debug!(target: "user", "Running check {}", model.name());

            if skip_remaining {
                run_result.skipped_group.push(model.name().to_string());
            }
            let mut has_failure = false;

            for action in &model.spec.actions {
                if skip_remaining {
                    warn!(target: "user", "Check `{}` was skipped.", model.name().bold());
                    run_result.skipped_group.push((&action.name).to_string());
                    continue;
                }

                let run = DefaultDoctorActionRun {
                    model: model.clone(),
                    action: action.clone(),
                    working_dir: self.working_dir.clone(),
                    file_cache: self.file_cache.clone(),
                    run_fix: self.run_fixes,
                    exec_runner: self.exec_runner.clone(),
                    glob_walker: self.glob_walker.clone(),
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
                        skip_remaining = true;
                        has_failure = true;
                    }
                    _ => {
                        run_result.failed_group.push((&action.name).to_string());
                        if action.required {
                            skip_remaining = true;
                        }
                        has_failure = true;
                    }
                }
            }

            if has_failure {
                run_result.failed_group.push(model.name().to_string());
                run_result.did_succeed = false;
            } else {
                run_result.succeeded_groups.push(model.name().to_string());
            }
        }

        Ok(run_result)
    }

    #[instrument(skip(self))]
    pub fn compute_group_order(&self) -> Result<Vec<Vec<String>>> {
        let mut graph = DiGraph::<&str, i32>::new();

        let start = graph.add_node("root");
        let end = graph.add_node("desired");
        let mut node_graph: BTreeMap<String, NodeIndex> = BTreeMap::new();
        for name in self.groups.keys() {
            node_graph.insert(name.to_string(), graph.add_node(&name));
        }

        for (name, model) in &self.groups {
            let this = node_graph.get(name).unwrap();
            let mut needs_start = true;
            for dep in &model.spec.requires {
                if let Some(other) = node_graph.get(dep) {
                    graph.add_edge(other.clone(), this.clone(), 1);
                    needs_start = false;
                } else {
                    warn!(target: "user", "{} needs {} but no such dependency found, ignoring dependency", name, dep);
                }
            }

            if needs_start {
                graph.add_edge(start, this.clone(), 1);
            }
        }

        for name in &self.desired_groups {
            if let Some(this) = node_graph.get(name) {
                graph.add_edge(this.clone(), end, 1);
            }
        }

        debug!(
            format = "graphviz",
            "{:?}",
            Dot::with_config(&graph, &[Config::NodeIndexLabel])
        );

        let mut all_paths = Vec::new();

        for path in all_simple_paths::<Vec<_>, _>(&graph, start, end, 0, None) {
            let mut named_path = Vec::new();
            for node in path.iter() {
                if node == &start || node == &end {
                    continue;
                }
                let name = graph.node_weight(node.clone()).unwrap().to_string();
                named_path.push(name)
            }
            all_paths.push(named_path);
        }
        Ok(all_paths)
    }
}

#[cfg(test)]
mod tests {
    use crate::check::tests::{build_run_fail_fix_succeed_action, command_result};
    use crate::check::{GlobWalker, MockGlobWalker};
    use crate::file_cache::{NoOpCache};
    use crate::runner::RunGroups;
    use crate::tests::{group_noop, make_root_model_additional, root_noop};
    use anyhow::Result;
    use scope_lib::prelude::{ExecutionProvider, MockExecutionProvider};
    use std::sync::Arc;

    fn make_group_runner(
        exec_runner: Arc<dyn ExecutionProvider>,
        glob_walker: Arc<dyn GlobWalker>,
    ) -> RunGroups {
        RunGroups {
            groups: Default::default(),
            desired_groups: Default::default(),
            file_cache: Arc::new(NoOpCache::default()),
            working_dir: Default::default(),
            run_fixes: true,
            exec_runner,
            glob_walker,
        }
    }

    #[tokio::test]
    async fn with_no_dep_will_have_no_tasks() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        let mut run_group = make_group_runner(Arc::new(exec_runner), Arc::new(glob_walker));

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        run_group.groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_2".to_string(), step_2);

        assert_eq!(0, run_group.compute_group_order().unwrap().len());

        Ok(())
    }

    #[tokio::test]
    async fn with_one_path_will_give_path() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        let mut run_group = make_group_runner(Arc::new(exec_runner), Arc::new(glob_walker));

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        run_group.groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_2".to_string(), step_2);
        run_group.desired_groups.insert("step_2".to_string());

        assert_eq!(
            vec![vec!["step_1", "step_2"]],
            run_group.compute_group_order()?
        );

        Ok(())
    }

    #[tokio::test]
    async fn with_two_paths_will_give_path() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        let mut run_group = make_group_runner(Arc::new(exec_runner), Arc::new(glob_walker));

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        run_group.groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_2".to_string(), step_2);
        run_group.desired_groups.insert("step_2".to_string());

        let step_3 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_3"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_3".to_string(), step_3);
        run_group.desired_groups.insert("step_3".to_string());

        assert_eq!(
            vec![vec!["step_1", "step_3"], vec!["step_1", "step_2"]],
            run_group.compute_group_order()?
        );

        Ok(())
    }

    #[tokio::test]
    async fn run_with_multiple_paths_only_run_group_once() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        command_result(&mut exec_runner, "check", vec![0, 0, 0]);

        let mut run_group = make_group_runner(Arc::new(exec_runner), Arc::new(glob_walker));

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        run_group.groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_2".to_string(), step_2);
        run_group.desired_groups.insert("step_2".to_string());

        let step_3 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_3"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_3".to_string(), step_3);
        run_group.desired_groups.insert("step_3".to_string());

        let exit_code = run_group.execute().await?;
        assert_eq!(0, exit_code);

        Ok(())
    }

    #[tokio::test]
    async fn run_fails_wont_run_failed_deps() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        command_result(&mut exec_runner, "check", vec![1, 1]);
        command_result(&mut exec_runner, "fix", vec![0]);

        let mut run_group = make_group_runner(Arc::new(exec_runner), Arc::new(glob_walker));

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        run_group.groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_2".to_string(), step_2);
        run_group.desired_groups.insert("step_2".to_string());

        let step_3 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_3"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        run_group.groups.insert("step_3".to_string(), step_3);
        run_group.desired_groups.insert("step_3".to_string());

        let exit_code = run_group.execute().await?;
        assert_eq!(1, exit_code);

        Ok(())
    }
}
