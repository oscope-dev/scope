use super::check::{ActionRunResult, DoctorActionRun};
use crate::shared::prelude::DoctorGroup;
use anyhow::Result;
use colored::Colorize;
use petgraph::dot::{Config, Dot};
use petgraph::prelude::*;
use petgraph::visit::{DfsPostOrder, Walker};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use tracing::{debug, error, info, info_span, warn, Span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

#[derive(Debug, Default)]
pub struct PathRunResult {
    pub did_succeed: bool,
    pub succeeded_groups: BTreeSet<String>,
    pub failed_group: BTreeSet<String>,
    pub skipped_group: BTreeSet<String>,
}

impl Display for PathRunResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut output = Vec::new();
        output.push(format!(
            "{} groups {}",
            self.succeeded_groups.len(),
            "succeeded".bold()
        ));
        if !self.failed_group.is_empty() {
            output.push(format!(
                "{} groups {}",
                self.failed_group.len(),
                "failed".bold().red()
            ));
        }
        if !self.skipped_group.is_empty() {
            output.push(format!(
                "{} groups {}",
                self.skipped_group.len(),
                "skipped".bold().yellow()
            ));
        }

        write!(f, "{}", output.join(", "))
    }
}

pub struct RunGroups<T>
where
    T: DoctorActionRun,
{
    pub(crate) group_actions: BTreeMap<String, Vec<T>>,
    pub(crate) all_paths: Vec<String>,
}

impl<T> RunGroups<T>
where
    T: DoctorActionRun,
{
    pub async fn execute(&self) -> Result<PathRunResult> {
        let header_span = info_span!("doctor run", "indicatif.pb_show" = true);
        header_span.pb_set_length(self.all_paths.len() as u64);
        header_span.pb_set_message("scope doctor run");

        let _span = header_span.enter();

        let mut full_path = Vec::new();
        for path in &self.all_paths {
            if let Some(actions) = self.group_actions.get(path) {
                full_path.push((path, actions));
            }
        }

        self.run_path(full_path).await
    }

    async fn run_path(&self, path: Vec<(&String, &Vec<T>)>) -> Result<PathRunResult> {
        let mut skip_remaining = false;
        let mut run_result = PathRunResult {
            did_succeed: true,
            succeeded_groups: BTreeSet::new(),
            failed_group: BTreeSet::new(),
            skipped_group: BTreeSet::new(),
        };

        for (group_name, actions) in path {
            Span::current().pb_inc(1);
            debug!(target: "user", "Running check {}", group_name);

            let group_span = info_span!("group", "indicatif.pb_show" = true);
            group_span.pb_set_length(actions.len() as u64);
            group_span.pb_set_message(&format!("group {}", group_name));
            let _span = group_span.enter();

            if skip_remaining {
                run_result.skipped_group.insert(group_name.to_string());
            }
            let mut has_failure = false;

            for action in actions {
                group_span.pb_inc(1);
                if skip_remaining {
                    info!(target: "user", "Check `{}/{}` was skipped.", group_name.bold(), action.name());
                    run_result.skipped_group.insert(group_name.to_string());
                    continue;
                }

                let action_result = action.run_action().await?;

                match action_result {
                    ActionRunResult::CheckSucceeded => {
                        info!(target: "progress", group = group_name, name = action.name(), "Check was successful");
                    }
                    ActionRunResult::NoCheckFixSucceeded => {
                        info!(target: "progress", group = group_name, name = action.name(), "Fix ran successfully");
                    }
                    ActionRunResult::CheckFailedFixSucceedVerifySucceed => {
                        info!(target: "progress", group = group_name, name = action.name(), "Check initially failed, fix was successful");
                    }
                    ActionRunResult::CheckFailedFixFailed => {
                        error!(target: "user", group = group_name, name = action.name(), "Check failed, fix ran and {}", "failed".red().bold());
                    }
                    ActionRunResult::CheckFailedFixSucceedVerifyFailed => {
                        error!(target: "user", group = group_name, name = action.name(), "Check initially failed, fix ran, verification {}", "failed".red().bold());
                    }
                    ActionRunResult::CheckFailedNoRunFix => {
                        info!(target: "progress", group = group_name, name = action.name(), "Check failed, fix was not run");
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
                    | ActionRunResult::NoCheckFixSucceeded
                    | ActionRunResult::CheckFailedFixSucceedVerifySucceed => {}
                    ActionRunResult::CheckFailedFixFailedStop => {
                        skip_remaining = true;
                        has_failure = true;
                    }
                    _ => {
                        if action.required() {
                            skip_remaining = true;
                        }
                        has_failure = true;
                    }
                }
            }

            if has_failure {
                run_result.failed_group.insert(group_name.to_string());
                run_result.did_succeed = false;
            } else {
                run_result.succeeded_groups.insert(group_name.to_string());
            }
        }

        Ok(run_result)
    }
}

pub fn compute_group_order(
    groups: &BTreeMap<String, DoctorGroup>,
    desired_groups: BTreeSet<String>,
) -> Vec<String> {
    let mut graph = DiGraph::<&str, i32>::new();
    let mut node_graph: BTreeMap<String, NodeIndex> = BTreeMap::new();

    for name in groups.keys() {
        node_graph.insert(name.to_string(), graph.add_node(name));
    }

    for (name, model) in groups {
        let this = node_graph.get(name).unwrap();
        for dep in &model.requires {
            if let Some(other) = node_graph.get(dep) {
                graph.add_edge(*other, *this, 1);
            } else {
                warn!(target: "user", "{} needs {} but no such dependency found, ignoring dependency", name, dep);
            }
        }
    }

    let start = graph.add_node("start");

    for name in &desired_groups {
        if let Some(this) = node_graph.get(name) {
            graph.add_edge(*this, start, 1);
        }
    }

    debug!(
        format = "graphviz",
        "{:?}",
        Dot::with_config(&graph, &[Config::NodeIndexLabel])
    );

    graph.reverse();

    let mut order = Vec::new();
    for node in DfsPostOrder::new(&graph, start).iter(&graph) {
        if node == start {
            continue;
        }
        let name = graph.node_weight(node).unwrap().to_string();
        order.push(name)
    }

    order
}

#[cfg(test)]
mod tests {
    use crate::doctor::check::tests::build_run_fail_fix_succeed_action;
    use crate::doctor::check::{ActionRunResult, MockDoctorActionRun};
    use crate::doctor::runner::{compute_group_order, RunGroups};
    use crate::doctor::tests::{group_noop, make_root_model_additional};
    use anyhow::Result;
    use std::collections::{BTreeMap, BTreeSet};

    #[tokio::test]
    async fn test_compute_group_order_with_no_dep_will_have_no_tasks() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            group_noop,
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        assert_eq!(0, compute_group_order(&groups, BTreeSet::new()).len());

        Ok(())
    }

    #[tokio::test]
    async fn test_compute_group_order_with_one_dep_will_include_dep() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            group_noop,
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        assert_eq!(
            vec!["step_1", "step_2"],
            compute_group_order(&groups, BTreeSet::from(["step_2".to_string()]))
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_compute_group_order_with_reversed_definition_order() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            |group| group.requires(vec!["step_2".to_string()]),
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            |group| group.requires(vec!["step_3".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        let step_3 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_3"),
            group_noop,
        );
        groups.insert("step_3".to_string(), step_3);

        assert_eq!(
            vec!["step_3", "step_2", "step_1"],
            compute_group_order(&groups, BTreeSet::from(["step_1".to_string()]))
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_compute_group_order_with_multiple_dependencies() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            group_noop,
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        let step_3 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_3"),
            |group| group.requires(vec!["step_1".to_string(), "step_2".to_string()]),
        );
        groups.insert("step_3".to_string(), step_3);

        assert_eq!(
            vec!["step_1", "step_2", "step_3"],
            compute_group_order(&groups, BTreeSet::from(["step_3".to_string()]))
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_compute_group_order_with_single_shared_dependency() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();

        let mut groups = BTreeMap::new();

        let step_1 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_1"),
            group_noop,
        );
        groups.insert("step_1".to_string(), step_1);

        let step_2 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_2"),
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_2".to_string(), step_2);

        let step_3 = make_root_model_additional(
            vec![action.clone()],
            |meta| meta.name("step_3"),
            |group| group.requires(vec!["step_1".to_string()]),
        );
        groups.insert("step_3".to_string(), step_3);

        assert_eq!(
            vec!["step_1", "step_3"],
            compute_group_order(&groups, BTreeSet::from(["step_3".to_string()]))
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
    async fn test_execute_run_with_multiple_paths_only_run_group_once() -> Result<()> {
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
                "step_1".to_string(),
                "step_2".to_string(),
                "step_3".to_string(),
            ],
        };

        let exit_code = run_groups.execute().await?;
        assert_eq!(true, exit_code.did_succeed);

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_dep_fails_wont_run_others() -> Result<()> {
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
                "step_1".to_string(),
                "step_2".to_string(),
                "step_3".to_string(),
            ],
        };

        let exit_code = run_groups.execute().await?;
        assert_eq!(false, exit_code.did_succeed);

        Ok(())
    }

    #[tokio::test]
    async fn test_execute_branch_fails_but_other_branch_continues() -> Result<()> {
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
                "step_1".to_string(),
                "step_2".to_string(),
                "step_3".to_string(),
                "step_4".to_string(),
            ],
        };

        let exit_code = run_groups.execute().await?;
        assert_eq!(false, exit_code.did_succeed);

        Ok(())
    }
}
