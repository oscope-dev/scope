use crate::check::{ActionRunResult, DoctorActionRun};
use anyhow::Result;
use colored::Colorize;
use petgraph::algo::all_simple_paths;
use petgraph::dot::{Config, Dot};
use petgraph::prelude::*;
use scope_lib::prelude::{DoctorGroup, ModelRoot};
use std::collections::{BTreeMap, BTreeSet};
use tracing::{debug, error, info, warn};

#[derive(Debug, Default)]
struct PathRunResult {
    did_succeed: bool,
    succeeded_groups: Vec<String>,
    failed_group: Vec<String>,
    skipped_group: Vec<String>,
}

pub struct RunGroups<T>
where
    T: DoctorActionRun,
{
    pub(crate) group_actions: BTreeMap<String, Vec<T>>,
    pub(crate) all_paths: Vec<Vec<String>>,
}

impl<T> RunGroups<T>
where
    T: DoctorActionRun,
{
    pub async fn execute(&self) -> Result<i32> {
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut did_succeed = true;

        for path in &self.all_paths {
            let mut full_path = Vec::new();
            for target_group in path {
                if visited.contains(target_group) {
                    info!("{} has already been run", target_group);
                    continue;
                }
                if let Some(actions) = self.group_actions.get(target_group) {
                    full_path.push((target_group, actions));
                }
            }

            let result = self.run_path(full_path).await?;
            for successful_model in result.succeeded_groups {
                visited.insert(successful_model);
            }
            did_succeed = did_succeed && result.did_succeed;
        }

        if did_succeed {
            Ok(0)
        } else {
            Ok(1)
        }
    }

    async fn run_path(&self, path: Vec<(&String, &Vec<T>)>) -> Result<PathRunResult> {
        let mut skip_remaining = false;
        let mut run_result = PathRunResult {
            did_succeed: true,
            succeeded_groups: vec![],
            failed_group: vec![],
            skipped_group: vec![],
        };

        for (group_name, actions) in path {
            debug!(target: "user", "Running check {}", group_name);

            if skip_remaining {
                run_result.skipped_group.push(group_name.to_string());
            }
            let mut has_failure = false;

            for action in actions {
                if skip_remaining {
                    warn!(target: "user", "Check `{}` was skipped.", group_name.bold());
                    run_result.skipped_group.push(group_name.to_string());
                    continue;
                }

                let action_result = action.run_action().await?;

                match action_result {
                    ActionRunResult::CheckSucceeded => {
                        info!(target: "user", group = group_name, name = action.name(), "Check was successful");
                    }
                    ActionRunResult::CheckFailedFixSucceedVerifySucceed => {
                        info!(target: "user", group = group_name, name = action.name(), "Check initially failed, fix was successful");
                    }
                    ActionRunResult::CheckFailedFixFailed => {
                        error!(target: "user", group = group_name, name = action.name(), "Check failed, fix ran and {}", "failed".red().bold());
                    }
                    ActionRunResult::CheckFailedFixSucceedVerifyFailed => {
                        error!(target: "user", group = group_name, name = action.name(), "Check initially failed, fix ran, verification {}", "failed".red().bold());
                    }
                    ActionRunResult::CheckFailedNoRunFix => {
                        info!(target: "user", group = group_name, name = action.name(), "Check failed, fix was not run");
                    }
                    ActionRunResult::CheckFailedNoFixProvided => {
                        error!(target: "user", group = group_name, name = action.name(), "Check failed, no fix provided");
                    }
                    ActionRunResult::CheckFailedFixFailedStop => {
                        error!(target: "user", group = group_name, name = action.name(), "Check failed, fix ran and {} and aborted", "failed".red().bold());
                    }
                }

                if action_result.is_failure() {
                    if let Some(help_text) = &action.help_text() {
                        error!(target: "user", group = group_name, name = action.name(), "Action Help: {}", help_text);
                    }
                    if let Some(help_url) = &action.help_url() {
                        error!(target: "user", group = group_name, name = action.name(), "For more help, please visit {}", help_url);
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
                        run_result.failed_group.push(group_name.to_string());
                        if action.required() {
                            skip_remaining = true;
                        }
                        has_failure = true;
                    }
                }
            }

            if has_failure {
                run_result.failed_group.push(group_name.to_string());
                run_result.did_succeed = false;
            } else {
                run_result.succeeded_groups.push(group_name.to_string());
            }
        }

        Ok(run_result)
    }
}

pub fn compute_group_order(
    groups: &BTreeMap<String, ModelRoot<DoctorGroup>>,
    desired_groups: BTreeSet<String>,
) -> Vec<Vec<String>> {
    let mut graph = DiGraph::<&str, i32>::new();

    let start = graph.add_node("root");
    let end = graph.add_node("desired");
    let mut node_graph: BTreeMap<String, NodeIndex> = BTreeMap::new();
    for name in groups.keys() {
        node_graph.insert(name.to_string(), graph.add_node(name));
    }

    for (name, model) in groups {
        let this = node_graph.get(name).unwrap();
        let mut needs_start = true;
        for dep in &model.spec.requires {
            if let Some(other) = node_graph.get(dep) {
                graph.add_edge(*other, *this, 1);
                needs_start = false;
            } else {
                warn!(target: "user", "{} needs {} but no such dependency found, ignoring dependency", name, dep);
            }
        }

        if needs_start {
            graph.add_edge(start, *this, 1);
        }
    }

    for name in &desired_groups {
        if let Some(this) = node_graph.get(name) {
            graph.add_edge(*this, end, 1);
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
            let name = graph.node_weight(*node).unwrap().to_string();
            named_path.push(name)
        }
        all_paths.push(named_path);
    }
    all_paths
}

#[cfg(test)]
mod tests {
    use crate::check::tests::build_run_fail_fix_succeed_action;
    use crate::check::{ActionRunResult, MockDoctorActionRun};
    use crate::runner::{compute_group_order, RunGroups};
    use crate::tests::{group_noop, make_root_model_additional, root_noop};
    use anyhow::Result;
    use std::collections::{BTreeMap, BTreeSet};

    #[tokio::test]
    async fn with_no_dep_will_have_no_tasks() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        assert_eq!(0, compute_group_order(&groups, BTreeSet::new()).len());

        Ok(())
    }

    #[tokio::test]
    async fn with_one_path_will_give_path() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        assert_eq!(
            vec![vec!["step_1", "step_2"]],
            compute_group_order(&groups, BTreeSet::from(["step_2".to_string()]))
        );

        Ok(())
    }

    #[tokio::test]
    async fn with_two_paths_will_give_path() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            root_noop,
            group_noop,
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        let step_3 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_3"),
            root_noop,
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_3".to_string(), step_3);

        assert_eq!(
            vec![vec!["step_1", "step_3"], vec!["step_1", "step_2"]],
            compute_group_order(
                &groups,
                BTreeSet::from(["step_2".to_string(), "step_3".to_string()])
            )
        );

        Ok(())
    }

    fn make_action_run(result: ActionRunResult) -> Vec<MockDoctorActionRun> {
        let mut run = MockDoctorActionRun::new();
        run.expect_run_action()
            .returning(move || Ok(result.clone()));
        run.expect_help_text().return_const(None);
        run.expect_help_url().return_const(None);
        run.expect_name().returning(|| "foo".to_string());
        run.expect_required().return_const(true);
        vec![run]
    }

    fn will_not_run() -> Vec<MockDoctorActionRun> {
        let run = MockDoctorActionRun::new();
        vec![run]
    }

    #[tokio::test]
    async fn run_with_multiple_paths_only_run_group_once() -> Result<()> {
        let mut group_actions = BTreeMap::new();

        group_actions.insert(
            "step_1".to_string(),
            make_action_run(ActionRunResult::CheckSucceeded),
        );
        group_actions.insert(
            "step_2".to_string(),
            make_action_run(ActionRunResult::CheckSucceeded),
        );
        group_actions.insert(
            "step_3".to_string(),
            make_action_run(ActionRunResult::CheckSucceeded),
        );

        let run_groups = RunGroups {
            group_actions,
            all_paths: vec![
                vec!["step_1".to_string(), "step_3".to_string()],
                vec!["step_1".to_string(), "step_2".to_string()],
            ],
        };

        let exit_code = run_groups.execute().await?;
        assert_eq!(0, exit_code);

        Ok(())
    }

    #[tokio::test]
    async fn test_dep_fails_wont_run_others() -> Result<()> {
        let mut group_actions = BTreeMap::new();
        group_actions.insert(
            "step_1".to_string(),
            make_action_run(ActionRunResult::CheckFailedFixSucceedVerifyFailed),
        );
        group_actions.insert("step_2".to_string(), will_not_run());
        group_actions.insert("step_3".to_string(), will_not_run());

        let run_groups = RunGroups {
            group_actions,
            all_paths: vec![
                vec!["step_1".to_string(), "step_3".to_string()],
                vec!["step_1".to_string(), "step_2".to_string()],
            ],
        };

        let exit_code = run_groups.execute().await?;
        assert_eq!(1, exit_code);

        Ok(())
    }

    #[tokio::test]
    async fn test_branch_fails_but_other_branch_continues() -> Result<()> {
        let mut group_actions = BTreeMap::new();
        group_actions.insert(
            "step_1".to_string(),
            make_action_run(ActionRunResult::CheckSucceeded),
        );
        group_actions.insert(
            "step_2".to_string(),
            make_action_run(ActionRunResult::CheckFailedFixSucceedVerifyFailed),
        );
        group_actions.insert("step_3".to_string(), will_not_run());
        group_actions.insert(
            "step_4".to_string(),
            make_action_run(ActionRunResult::CheckSucceeded),
        );

        let run_groups = RunGroups {
            group_actions,
            all_paths: vec![
                vec![
                    "step_1".to_string(),
                    "step_2".to_string(),
                    "step_3".to_string(),
                ],
                vec!["step_1".to_string(), "step_4".to_string()],
            ],
        };

        let exit_code = run_groups.execute().await?;
        assert_eq!(1, exit_code);

        Ok(())
    }
}
