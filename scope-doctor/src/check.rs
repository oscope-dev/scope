use crate::file_cache::{FileCache, FileCacheStatus};
use anyhow::Result;
use std::cmp;
use std::cmp::max;

use async_trait::async_trait;
use colored::Colorize;
use educe::Educe;
use mockall::automock;
use scope_lib::prelude::{
    CaptureError, CaptureOpts, DoctorGroup, DoctorGroupAction, DoctorGroupActionCommand,
    DoctorGroupCachePath, ExecutionProvider, ModelRoot, OutputDestination, ScopeModel,
};
use std::path::Path;
use thiserror::Error;
use tracing::{error, info, instrument, warn};

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Unable to process file. {error:?}")]
    IoError {
        #[from]
        error: std::io::Error,
    },
    #[error("Unable to parse UTF-8 output. {error:?}")]
    FromUtf8Error {
        #[from]
        error: std::string::FromUtf8Error,
    },
    #[error(transparent)]
    CaptureError(#[from] CaptureError),
    #[error(transparent)]
    AnyError(#[from] anyhow::Error),
    #[error(transparent)]
    PatternError(#[from] glob::PatternError),
}

#[derive(Debug, Clone, PartialEq, Ord, Eq, PartialOrd)]
pub enum CacheResults {
    FixNotRequired = 1,
    FixRequired = 2,
    StopExecution = 3,
}

impl CacheResults {
    fn is_success(&self) -> bool {
        self == &CacheResults::FixNotRequired
    }
}

#[derive(Debug, PartialEq)]
pub enum CorrectionResults {
    NoFixSpecified,
    Success,
    FailContinue,
    FailAndStop,
}

#[derive(Debug, PartialEq)]
pub enum ActionRunResult {
    Succeeded,
    Failed,
    Stop,
}

#[derive(Educe)]
#[educe(Debug)]
pub struct DoctorActionRun<'a> {
    pub model: &'a ModelRoot<DoctorGroup>,
    pub action: &'a DoctorGroupAction,
    pub working_dir: &'a Path,
    #[allow(clippy::borrowed_box)]
    pub file_cache: &'a Box<dyn FileCache>,
    pub run_fix: bool,
    #[educe(Debug(ignore))]
    pub exec_runner: &'a dyn ExecutionProvider,
    #[educe(Debug(ignore))]
    pub glob_walker: &'a dyn GlobWalker,
}

impl<'a> DoctorActionRun<'a> {
    #[instrument(skip_all, fields(model.name = self.model.name(), action.name = self.action.name, action.description = self.action.description ))]
    pub async fn run_action(&self) -> Result<ActionRunResult> {
        let check_status = self.evaluate_checks().await?;
        let should_continue = match check_status {
            CacheResults::FixNotRequired => {
                info!(target: "user", name = self.model.name(), "Check was successful");
                ActionRunResult::Succeeded
            }
            CacheResults::FixRequired => {
                if !self.run_fix {
                    info!(target: "user", group = self.model.name(), name = self.action.name,  "Check failed. {}: Run with --fix to auto-fix", "Suggestion".bold());
                    ActionRunResult::Succeeded
                } else {
                    let fix_results = self.run_fixes().await?;
                    match fix_results {
                        CorrectionResults::Success => {
                            info!(target: "user",group = self.model.name(), name = self.action.name, "Check failed. {} ran successfully", "Fix".bold());
                            ActionRunResult::Succeeded
                        }
                        CorrectionResults::NoFixSpecified => {
                            info!(target: "user", group = self.model.name(), name = self.action.name, "Check failed. No fix was specified");
                            ActionRunResult::Stop
                        }
                        CorrectionResults::FailContinue => {
                            error!(target: "user", group = self.model.name(), name = self.action.name, "Check failed. The fix ran and {}", "Failed".red().bold());
                            ActionRunResult::Failed
                        }
                        CorrectionResults::FailAndStop => {
                            error!(target: "user", group = self.model.name(), name = self.action.name, "Check failed. The fix ran and {}, and was required", "Failed".red().bold());
                            ActionRunResult::Stop
                        }
                    }
                }
            }
            CacheResults::StopExecution => {
                error!(target: "user", "Check `{}#{}` has failed and wants to stop execution. All other checks will be skipped.", self.model.name().bold(), self.action.description.bold());
                ActionRunResult::Stop
            }
        };

        Ok(should_continue)
    }

    pub async fn run_fixes(&self) -> Result<CorrectionResults, RuntimeError> {
        if self.action.fix.is_none() {
            return Ok(CorrectionResults::NoFixSpecified);
        }

        let mut highest_exit_code = 0;
        if let Some(action_command) = &self.action.fix {
            if action_command.commands.is_empty() {
                return Ok(CorrectionResults::NoFixSpecified);
            }
            for command in &action_command.commands {
                let result = self.run_single_fix(command).await?;
                highest_exit_code = max(highest_exit_code, result);
                if highest_exit_code >= 100 {
                    return Ok(CorrectionResults::FailAndStop);
                }
            }
        }

        if highest_exit_code == 0 {
            if let Some(paths) = &self.action.check.files {
                if let Err(e) = self
                    .glob_walker
                    .update_cache(
                        &paths.base_path,
                        &paths.paths,
                        self.model.name(),
                        self.file_cache,
                    )
                    .await
                {
                    warn!("Unable to update cache: {:?}", e);
                    info!(target: "user", "Unable to properly update cache, next run will re-evaluate action");
                }
            }
        }

        if highest_exit_code != 0 {
            return if self.action.required {
                Ok(CorrectionResults::FailAndStop)
            } else {
                Ok(CorrectionResults::FailContinue)
            };
        }

        if let Some(action_command) = &self.action.check.command {
            let check_status = self.evaluate_command_check(action_command).await?;
            info!("re-running check returned {:?}", check_status);
            match check_status {
                CacheResults::StopExecution => Ok(CorrectionResults::FailAndStop),
                CacheResults::FixNotRequired => Ok(CorrectionResults::Success),
                CacheResults::FixRequired => {
                    if self.action.required {
                        Ok(CorrectionResults::FailAndStop)
                    } else {
                        Ok(CorrectionResults::FailContinue)
                    }
                }
            }
        } else {
            Ok(CorrectionResults::Success)
        }
    }

    async fn run_single_fix(&self, command: &str) -> Result<i32, RuntimeError> {
        let args = vec![command.to_string()];
        let capture = self
            .exec_runner
            .run_command(CaptureOpts {
                working_dir: self.working_dir,
                args: &args,
                output_dest: OutputDestination::StandardOut,
                path: &self.model.exec_path(),
                env_vars: Default::default(),
            })
            .await?;

        info!("fix ran {} and exited {:?}", command, capture.exit_code);

        Ok(capture.exit_code.unwrap_or(-1))
    }

    pub async fn evaluate_checks(&self) -> Result<CacheResults, RuntimeError> {
        if let Some(cache_path) = &self.action.check.files {
            let result = self.evaluate_path_check(cache_path).await?;
            if !result.is_success() {
                return Ok(result);
            }
        }

        if let Some(action_command) = &self.action.check.command {
            let result = self.evaluate_command_check(action_command).await?;
            if !result.is_success() {
                return Ok(result);
            }
        }

        Ok(CacheResults::FixRequired)
    }

    async fn evaluate_path_check(
        &self,
        paths: &DoctorGroupCachePath,
    ) -> Result<CacheResults, RuntimeError> {
        let result = self
            .glob_walker
            .have_globs_changed(
                &paths.base_path,
                &paths.paths,
                self.model.name(),
                self.file_cache,
            )
            .await?;

        if result {
            Ok(CacheResults::FixNotRequired)
        } else {
            Ok(CacheResults::FixRequired)
        }
    }

    async fn evaluate_command_check(
        &self,
        action_command: &DoctorGroupActionCommand,
    ) -> Result<CacheResults, RuntimeError> {
        let mut result: Option<CacheResults> = None;
        for command in &action_command.commands {
            let args = vec![command.clone()];
            let path = format!("{}:{}", self.model.containing_dir(), self.model.exec_path());
            let output = self
                .exec_runner
                .run_command(CaptureOpts {
                    working_dir: self.working_dir,
                    args: &args,
                    output_dest: OutputDestination::Logging,
                    path: &path,
                    env_vars: Default::default(),
                })
                .await?;

            info!(
                "check ran command {} and result was {:?}",
                command, output.exit_code
            );

            let command_result = match output.exit_code {
                Some(0) => CacheResults::FixNotRequired,
                Some(100..=i32::MAX) => CacheResults::StopExecution,
                _ => CacheResults::FixRequired,
            };

            let next = match &result {
                None => command_result,
                Some(prev) => cmp::max(prev.clone(), command_result.clone()),
            };

            result.replace(next);
            if result == Some(CacheResults::StopExecution) {
                break;
            }
        }

        Ok(result.unwrap_or(CacheResults::FixRequired))
    }
}

#[automock]
#[async_trait]
#[allow(clippy::borrowed_box)]
pub trait GlobWalker {
    async fn have_globs_changed(
        &self,
        base_dir: &Path,
        paths: &[String],
        cache_name: &str,
        file_cache: &Box<dyn FileCache>,
    ) -> Result<bool, RuntimeError>;

    async fn update_cache(
        &self,
        base_dir: &Path,
        paths: &[String],
        cache_name: &str,
        file_cache: &Box<dyn FileCache>,
    ) -> Result<(), RuntimeError>;
}

#[derive(Debug, Default)]
pub struct DefaultGlobWalker {}

#[async_trait]
impl GlobWalker for DefaultGlobWalker {
    async fn have_globs_changed(
        &self,
        base_dir: &Path,
        paths: &[String],
        cache_name: &str,
        file_cache: &Box<dyn FileCache>,
    ) -> Result<bool, RuntimeError> {
        use glob::glob;

        for glob_str in paths {
            let glob_path = format!("{}/{}", base_dir.display(), glob_str);
            for path in glob(&glob_path)?.filter_map(Result::ok) {
                let file_result = file_cache.check_file(cache_name.to_string(), &path).await?;
                let check_result = file_result == FileCacheStatus::FileMatches;
                if !check_result {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    async fn update_cache(
        &self,
        base_dir: &Path,
        paths: &[String],
        cache_name: &str,
        file_cache: &Box<dyn FileCache>,
    ) -> Result<(), RuntimeError> {
        use glob::glob;

        for glob_str in paths {
            let glob_path = format!("{}/{}", base_dir.display(), glob_str);
            for path in glob(&glob_path)?.filter_map(Result::ok) {
                file_cache
                    .update_cache_entry(cache_name.to_string(), &path)
                    .await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::check::{ActionRunResult, DoctorActionRun, MockGlobWalker, RuntimeError};
    use crate::file_cache::{FileCache, NoOpCache};
    use anyhow::{anyhow, Result};
    use scope_lib::prelude::{
        DoctorGroup, DoctorGroupAction, DoctorGroupActionBuilder, DoctorGroupActionCheckBuilder,
        DoctorGroupActionCommand, DoctorGroupBuilder, DoctorGroupCachePath, MockExecutionProvider,
        ModelMetadataBuilder, ModelRoot, ModelRootBuilder, OutputCaptureBuilder,
    };
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn build_run_fail_fix_succeed_action() -> DoctorGroupAction {
        DoctorGroupActionBuilder::default()
            .description("a test action")
            .name("action")
            .required(true)
            .check(
                DoctorGroupActionCheckBuilder::default()
                    .files(None)
                    .command(Some(DoctorGroupActionCommand::from(vec!["check"])))
                    .build()
                    .unwrap(),
            )
            .fix(Some(DoctorGroupActionCommand::from(vec!["fix"])))
            .build()
            .unwrap()
    }

    fn build_file_fix_action() -> DoctorGroupAction {
        DoctorGroupActionBuilder::default()
            .description("a test action")
            .name("action")
            .required(true)
            .check(
                DoctorGroupActionCheckBuilder::default()
                    .command(None)
                    .files(Some(DoctorGroupCachePath::from(("/foo", vec!["**/*"]))))
                    .build()
                    .unwrap(),
            )
            .fix(Some(DoctorGroupActionCommand::from(vec!["fix"])))
            .build()
            .unwrap()
    }

    fn make_model(actions: Vec<DoctorGroupAction>) -> ModelRoot<DoctorGroup> {
        let group = DoctorGroupBuilder::default()
            .description("a description")
            .actions(actions)
            .build()
            .unwrap();

        ModelRootBuilder::default()
            .api_version("fake")
            .kind("fake-kind")
            .metadata(
                ModelMetadataBuilder::default()
                    .name("fake-model")
                    .annotations(BTreeMap::default())
                    .labels(BTreeMap::default())
                    .build()
                    .unwrap(),
            )
            .spec(group)
            .build()
            .unwrap()
    }

    fn command_result(
        mock: &mut MockExecutionProvider,
        command: &'static str,
        expected_results: Vec<i32>,
    ) {
        let mut exectation = mock
            .expect_run_command()
            .withf(|params| params.args == vec![command.to_string()])
            .times(expected_results.len());
        for code in expected_results {
            exectation = exectation.returning(move |_| {
                Ok(OutputCaptureBuilder::default()
                    .exit_code(Some(code))
                    .build()
                    .unwrap())
            });
        }
    }

    #[tokio::test]
    async fn test_only_exec_will_re_run() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "check", vec![1, 0]);
        command_result(&mut exec_runner, "fix", vec![0]);

        let glob_walker = MockGlobWalker::new();

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Succeeded, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_fail_fix_succeed_check_fails() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "check", vec![1, 1]);
        command_result(&mut exec_runner, "fix", vec![0]);

        let glob_walker = MockGlobWalker::new();

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Stop, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_fail_fix_fails() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "check", vec![1]);
        command_result(&mut exec_runner, "fix", vec![1]);

        let glob_walker = MockGlobWalker::new();

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Stop, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_not_required_continue() -> Result<()> {
        let mut action = build_run_fail_fix_succeed_action();
        action.required = false;
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "check", vec![1]);
        command_result(&mut exec_runner, "fix", vec![1]);

        let glob_walker = MockGlobWalker::new();

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Failed, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_works() -> Result<()> {
        let action = build_file_fix_action();
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "fix", vec![0]);

        let mut glob_walker = MockGlobWalker::new();
        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));
        glob_walker
            .expect_update_cache()
            .times(1)
            .returning(|_, _, _, _| Ok(()));

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Succeeded, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_works_unable_to_update_cache() -> Result<()> {
        let action = build_file_fix_action();
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "fix", vec![0]);

        let mut glob_walker = MockGlobWalker::new();
        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));
        glob_walker
            .expect_update_cache()
            .times(1)
            .returning(|_, _, _, _| Err(RuntimeError::AnyError(anyhow!("bogus error"))));

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Succeeded, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_fails() -> Result<()> {
        let action = build_file_fix_action();
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "fix", vec![1]);

        let mut glob_walker = MockGlobWalker::new();
        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Stop, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_not_required_succeed() -> Result<()> {
        let mut action = build_file_fix_action();
        action.required = false;
        let model = make_model(vec![action.clone()]);
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Box<dyn FileCache> = Box::<NoOpCache>::default();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "fix", vec![1]);

        let mut glob_walker = MockGlobWalker::new();
        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));

        let run = DoctorActionRun {
            model: &model,
            action: &action,
            working_dir: &path,
            file_cache: &file_cache,
            run_fix: true,
            exec_runner: &exec_runner,
            glob_walker: &glob_walker,
        };

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::Failed, result);

        Ok(())
    }
}
