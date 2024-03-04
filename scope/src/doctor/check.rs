use super::file_cache::{FileCache, FileCacheStatus};
use anyhow::Result;
use std::cmp;
use std::cmp::max;

use crate::models::HelpMetadata;
use crate::shared::prelude::{
    CaptureError, CaptureOpts, DoctorGroup, DoctorGroupAction, DoctorGroupActionCommand,
    DoctorGroupCachePath, ExecutionProvider, OutputDestination,
};
use async_trait::async_trait;
use derive_builder::Builder;
use educe::Educe;
use mockall::automock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info, instrument};

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

#[derive(Debug, PartialEq, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum ActionRunResult {
    CheckSucceeded,
    CheckFailedFixSucceedVerifySucceed,
    CheckFailedFixFailed,
    CheckFailedFixSucceedVerifyFailed,
    CheckFailedNoRunFix,
    CheckFailedNoFixProvided,
    CheckFailedFixFailedStop,
}

impl ActionRunResult {
    pub(crate) fn is_failure(&self) -> bool {
        match self {
            ActionRunResult::CheckSucceeded => false,
            ActionRunResult::CheckFailedFixSucceedVerifySucceed => false,
            ActionRunResult::CheckFailedFixFailed => true,
            ActionRunResult::CheckFailedFixSucceedVerifyFailed => true,
            ActionRunResult::CheckFailedNoRunFix => true,
            ActionRunResult::CheckFailedNoFixProvided => true,
            ActionRunResult::CheckFailedFixFailedStop => true,
        }
    }
}

#[automock]
#[async_trait::async_trait]
pub trait DoctorActionRun: Send + Sync {
    async fn run_action(&self) -> Result<ActionRunResult>;
    fn required(&self) -> bool;
    fn name(&self) -> String;
    fn help_text(&self) -> Option<String>;
    fn help_url(&self) -> Option<String>;
}

#[derive(Educe, Builder)]
#[educe(Debug)]
#[builder(setter(into))]
pub struct DefaultDoctorActionRun {
    pub model: DoctorGroup,
    pub action: DoctorGroupAction,
    pub working_dir: PathBuf,
    pub file_cache: Arc<dyn FileCache>,
    pub run_fix: bool,
    #[educe(Debug(ignore))]
    pub exec_runner: Arc<dyn ExecutionProvider>,
    #[educe(Debug(ignore))]
    pub glob_walker: Arc<dyn GlobWalker>,
}

#[async_trait::async_trait]
impl DoctorActionRun for DefaultDoctorActionRun {
    #[instrument(skip_all, fields(model.name = self.model.name(), action.name = self.action.name, action.description = self.action.description ))]
    async fn run_action(&self) -> Result<ActionRunResult> {
        let check_status = self.evaluate_checks().await?;
        if check_status == CacheResults::FixNotRequired {
            return Ok(ActionRunResult::CheckSucceeded);
        }

        if !self.run_fix {
            return Ok(ActionRunResult::CheckFailedNoRunFix);
        }

        let fix_result = self.run_fixes().await?;
        match fix_result {
            i32::MIN..=-1 => {
                return Ok(ActionRunResult::CheckFailedNoFixProvided);
            }
            0 => {}
            1...100 => return Ok(ActionRunResult::CheckFailedFixFailed),
            _ => return Ok(ActionRunResult::CheckFailedFixFailedStop),
        }

        if let Some(check_status) = self.evaluate_command_checks().await? {
            if check_status != CacheResults::FixNotRequired {
                return Ok(ActionRunResult::CheckFailedFixSucceedVerifyFailed);
            }
        }

        self.update_caches().await;

        Ok(ActionRunResult::CheckFailedFixSucceedVerifySucceed)
    }

    fn required(&self) -> bool {
        self.action.required
    }

    fn name(&self) -> String {
        self.action.name.to_string()
    }

    fn help_text(&self) -> Option<String> {
        self.action.fix.help_text.clone()
    }

    fn help_url(&self) -> Option<String> {
        self.action.fix.help_url.clone()
    }
}

impl DefaultDoctorActionRun {
    async fn update_caches(&self) {
        if let Some(cache_path) = &self.action.check.files {
            let result = self
                .glob_walker
                .update_cache(
                    &cache_path.base_path,
                    &cache_path.paths,
                    &self.model.metadata.name(),
                    self.file_cache.clone(),
                )
                .await;

            if let Err(e) = result {
                info!("Unable to update cache, dropping update {:?}", e);
                info!(target: "user", "Unable to update file cache, next run will re-run this action.")
            }
        }
    }

    async fn run_fixes(&self) -> Result<i32, RuntimeError> {
        let mut highest_exit_code = -1;
        if let Some(action_command) = &self.action.fix.command {
            for command in &action_command.commands {
                let result = self.run_single_fix(command).await?;
                highest_exit_code = max(highest_exit_code, result);
                if highest_exit_code >= 100 {
                    return Ok(highest_exit_code);
                }
            }
        }

        Ok(highest_exit_code)
    }

    async fn run_single_fix(&self, command: &str) -> Result<i32, RuntimeError> {
        let args = vec![command.to_string()];
        let capture = self
            .exec_runner
            .run_command(CaptureOpts {
                working_dir: &self.working_dir,
                args: &args,
                output_dest: OutputDestination::StandardOut,
                path: &self.model.metadata.exec_path(),
                env_vars: Default::default(),
            })
            .await?;

        info!("fix ran {} and exited {:?}", command, capture.exit_code);

        Ok(capture.exit_code.unwrap_or(-1))
    }

    async fn evaluate_checks(&self) -> Result<CacheResults, RuntimeError> {
        if let Some(cache_path) = &self.action.check.files {
            let result = self.evaluate_path_check(cache_path).await?;
            if result.is_success() {
                return Ok(result);
            }
        }

        if let Some(res) = self.evaluate_command_checks().await? {
            return Ok(res);
        }

        Ok(CacheResults::FixRequired)
    }

    async fn evaluate_command_checks(&self) -> Result<Option<CacheResults>, RuntimeError> {
        if let Some(action_command) = &self.action.check.command {
            let result = self.run_check_command(action_command).await?;
            return Ok(Some(result));
        }

        Ok(None)
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
                &self.model.metadata.name(),
                self.file_cache.clone(),
            )
            .await?;

        if result {
            Ok(CacheResults::FixNotRequired)
        } else {
            Ok(CacheResults::FixRequired)
        }
    }

    async fn run_check_command(
        &self,
        action_command: &DoctorGroupActionCommand,
    ) -> Result<CacheResults, RuntimeError> {
        let mut result: Option<CacheResults> = None;
        for command in &action_command.commands {
            let args = vec![command.clone()];
            let path = format!(
                "{}:{}",
                self.model.metadata().containing_dir(),
                self.model.metadata().exec_path()
            );
            let output = self
                .exec_runner
                .run_command(CaptureOpts {
                    working_dir: &self.working_dir,
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
pub trait GlobWalker: Send + Sync {
    async fn have_globs_changed(
        &self,
        base_dir: &Path,
        paths: &[String],
        cache_name: &str,
        file_cache: Arc<dyn FileCache>,
    ) -> Result<bool, RuntimeError>;

    async fn update_cache(
        &self,
        base_dir: &Path,
        paths: &[String],
        cache_name: &str,
        file_cache: Arc<dyn FileCache>,
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
        file_cache: Arc<dyn FileCache>,
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
        file_cache: Arc<dyn FileCache>,
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
pub(crate) mod tests {
    use crate::doctor::check::{
        ActionRunResult, DefaultDoctorActionRun, DoctorActionRun, MockGlobWalker, RuntimeError,
    };
    use crate::doctor::file_cache::{FileCache, NoOpCache};
    use crate::doctor::tests::build_root_model;
    use crate::shared::prelude::*;
    use anyhow::{anyhow, Result};
    use std::path::PathBuf;
    use std::sync::Arc;

    pub fn build_run_fail_fix_succeed_action() -> DoctorGroupAction {
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
            .fix(
                DoctorGroupActionFixBuilder::default()
                    .command(Some(DoctorGroupActionCommand::from(vec!["fix"])))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    }

    pub fn build_file_fix_action() -> DoctorGroupAction {
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
            .fix(
                DoctorGroupActionFixBuilder::default()
                    .command(Some(DoctorGroupActionCommand::from(vec!["fix"])))
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap()
    }

    pub fn command_result(
        mock: &mut MockExecutionProvider,
        command: &'static str,
        expected_results: Vec<i32>,
    ) {
        let mut counter = 0;
        mock.expect_run_command()
            .times(expected_results.len())
            .withf(move |params| params.args[0].eq(command))
            .returning(move |_| {
                let resp_code = expected_results[counter];
                counter += 1;
                Ok(OutputCaptureBuilder::default()
                    .exit_code(Some(resp_code))
                    .build()
                    .unwrap())
            });
    }

    pub fn setup_test(
        actions: Vec<DoctorGroupAction>,
        exec_runner: MockExecutionProvider,
        glob_walker: MockGlobWalker,
    ) -> DefaultDoctorActionRun {
        let model = build_root_model(actions.clone());
        let path = PathBuf::from("/tmp/foo");
        let file_cache: Arc<dyn FileCache> = Arc::<NoOpCache>::default();

        DefaultDoctorActionRun {
            model,
            action: actions[0].clone(),
            working_dir: path,
            file_cache,
            run_fix: true,
            exec_runner: Arc::new(exec_runner),
            glob_walker: Arc::new(glob_walker),
        }
    }

    #[tokio::test]
    async fn test_only_exec_will_check_passes() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        command_result(&mut exec_runner, "check", vec![0]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::CheckSucceeded, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_only_exec_will_re_run() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        command_result(&mut exec_runner, "check", vec![1, 0]);
        command_result(&mut exec_runner, "fix", vec![0]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::CheckFailedFixSucceedVerifySucceed, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_fail_fix_succeed_check_fails() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        command_result(&mut exec_runner, "check", vec![1, 1]);
        command_result(&mut exec_runner, "fix", vec![0]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::CheckFailedFixSucceedVerifyFailed, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_fail_fix_fails() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();

        command_result(&mut exec_runner, "check", vec![1]);
        command_result(&mut exec_runner, "fix", vec![1]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::CheckFailedFixFailed, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_works() -> Result<()> {
        let action = build_file_fix_action();

        let mut glob_walker = MockGlobWalker::new();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "fix", vec![0]);

        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));
        glob_walker
            .expect_update_cache()
            .times(1)
            .returning(|_, _, _, _| Ok(()));

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::CheckFailedFixSucceedVerifySucceed, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_works_unable_to_update_cache() -> Result<()> {
        let action = build_file_fix_action();

        let mut glob_walker = MockGlobWalker::new();
        let mut exec_runner = MockExecutionProvider::new();

        command_result(&mut exec_runner, "fix", vec![0]);

        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));
        glob_walker
            .expect_update_cache()
            .times(1)
            .returning(|_, _, _, _| Err(RuntimeError::AnyError(anyhow!("bogus error"))));

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::CheckFailedFixSucceedVerifySucceed, result);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_fails() -> Result<()> {
        let action = build_file_fix_action();
        let mut exec_runner = MockExecutionProvider::new();
        let mut glob_walker = MockGlobWalker::new();

        command_result(&mut exec_runner, "fix", vec![1]);

        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action().await?;
        assert_eq!(ActionRunResult::CheckFailedFixFailed, result);

        Ok(())
    }
}
