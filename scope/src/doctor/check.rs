use super::file_cache::{FileCache, FileCacheStatus};
use anyhow::Result;
use std::cmp;
use std::cmp::max;
use std::collections::BTreeMap;

use crate::models::HelpMetadata;
use crate::prelude::ReportBuilder;
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
pub enum CacheStatus {
    FixNotRequired = 1,
    FixRequired = 2,
    StopExecution = 3,
    CacheNotDefined = 4,
}

impl CacheStatus {
    fn is_success(&self) -> bool {
        self == &CacheStatus::FixNotRequired || self == &CacheStatus::CacheNotDefined
    }
}

#[derive(Debug, Clone)]
pub struct CacheResults {
    pub status: CacheStatus,
    pub output: Option<String>,
}

impl From<CacheStatus> for CacheResults {
    fn from(value: CacheStatus) -> Self {
        Self {
            status: value,
            output: None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum ActionRunStatus {
    CheckSucceeded,
    CheckFailedFixSucceedVerifySucceed,
    CheckFailedFixFailed,
    CheckFailedFixSucceedVerifyFailed,
    CheckFailedNoRunFix,
    CheckFailedNoFixProvided,
    CheckFailedFixFailedStop,
    NoCheckFixSucceeded,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ActionRunResult {
    pub status: ActionRunStatus,
    pub error_output: Option<String>,
}

impl ActionRunResult {
    pub fn new(status: ActionRunStatus, error_output: Option<String>) -> Self {
        Self {
            status,
            error_output,
        }
    }
}

impl From<ActionRunStatus> for ActionRunResult {
    fn from(value: ActionRunStatus) -> Self {
        Self {
            status: value,
            error_output: None,
        }
    }
}

impl ActionRunStatus {
    pub(crate) fn is_failure(&self) -> bool {
        match self {
            ActionRunStatus::CheckSucceeded => false,
            ActionRunStatus::CheckFailedFixSucceedVerifySucceed => false,
            ActionRunStatus::CheckFailedFixFailed => true,
            ActionRunStatus::CheckFailedFixSucceedVerifyFailed => true,
            ActionRunStatus::CheckFailedNoRunFix => true,
            ActionRunStatus::CheckFailedNoFixProvided => true,
            ActionRunStatus::CheckFailedFixFailedStop => true,
            ActionRunStatus::NoCheckFixSucceeded => false,
        }
    }
}

#[automock]
#[async_trait::async_trait]
pub trait DoctorActionRun: Send + Sync {
    async fn run_action(&self, report: &mut ReportBuilder) -> Result<ActionRunResult>;
    fn required(&self) -> bool;
    fn name(&self) -> String;
    fn description(&self) -> String;
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
    async fn run_action(&self, report: &mut ReportBuilder) -> Result<ActionRunResult> {
        let check_results = self.evaluate_checks(report).await?;
        let check_status = check_results.status;
        if check_status == CacheStatus::FixNotRequired {
            return Ok(ActionRunStatus::CheckSucceeded.into());
        }

        if !self.run_fix {
            return Ok(ActionRunStatus::CheckFailedNoRunFix.into());
        }

        let fix_result = self.run_fixes(report).await?;

        match fix_result {
            i32::MIN..=-1 => {
                return Ok(ActionRunStatus::CheckFailedNoFixProvided.into());
            }
            0 => {}
            1...100 => {
                return Ok(ActionRunResult::new(
                    ActionRunStatus::CheckFailedFixFailed,
                    check_results.output,
                ));
            }
            _ => {
                return Ok(ActionRunResult::new(
                    ActionRunStatus::CheckFailedFixFailedStop,
                    check_results.output,
                ));
            }
        }

        if check_status == CacheStatus::CacheNotDefined {
            self.update_caches().await;
            return Ok(ActionRunStatus::NoCheckFixSucceeded.into());
        }

        if let Some(check_results) = self.evaluate_command_checks(report).await? {
            if check_results.status != CacheStatus::FixNotRequired {
                return Ok(ActionRunResult::new(
                    ActionRunStatus::CheckFailedFixSucceedVerifyFailed,
                    check_results.output,
                ));
            }
        }

        self.update_caches().await;

        Ok(ActionRunStatus::CheckFailedFixSucceedVerifySucceed.into())
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

    fn description(&self) -> String {
        self.action.description.to_string()
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

    async fn run_fixes(&self, report: &mut ReportBuilder) -> Result<i32, RuntimeError> {
        let mut highest_exit_code = -1;
        if let Some(action_command) = &self.action.fix.command {
            for command in &action_command.commands {
                let result = self.run_single_fix(command, report).await?;
                highest_exit_code = max(highest_exit_code, result);
                if highest_exit_code >= 100 {
                    return Ok(highest_exit_code);
                }
            }
        }

        Ok(highest_exit_code)
    }

    async fn run_single_fix(
        &self,
        command: &str,
        report: &mut ReportBuilder,
    ) -> Result<i32, RuntimeError> {
        let args = vec![command.to_string()];
        let capture = self
            .exec_runner
            .run_command(CaptureOpts {
                working_dir: &self.working_dir,
                args: &args,
                output_dest: OutputDestination::StandardOutWithPrefix(format!(
                    "{}/{}",
                    self.model.metadata.name(),
                    self.action.name
                )),
                path: &self.model.metadata.exec_path(),
                env_vars: self.generate_env_vars(),
            })
            .await?;

        report.add_capture(&capture)?;

        info!("fix ran {} and exited {:?}", command, capture.exit_code);

        Ok(capture.exit_code.unwrap_or(-1))
    }

    fn generate_env_vars(&self) -> BTreeMap<String, String> {
        let mut env_vars = BTreeMap::new();
        env_vars.insert(
            "SCOPE_BIN_DIR".to_string(),
            std::env::current_exe()
                .unwrap()
                .parent()
                .expect("executable should be in a directory")
                .to_str()
                .expect("bin directory should be a valid string")
                .to_string(),
        );
        env_vars
    }

    async fn evaluate_checks(
        &self,
        report: &mut ReportBuilder,
    ) -> Result<CacheResults, RuntimeError> {
        let mut path_check = None;
        let mut command_check = None;
        if let Some(cache_path) = &self.action.check.files {
            let result = self.evaluate_path_check(cache_path).await?;
            if !result.is_success() {
                return Ok(result.into());
            }

            path_check = Some(result);
        }

        if let Some(results) = self.evaluate_command_checks(report).await? {
            if !results.status.is_success() {
                return Ok(results);
            }
            command_check = Some(results);
        }

        match (path_check, command_check) {
            (None, None) => Ok(CacheStatus::CacheNotDefined.into()),
            (Some(p), None) if p.is_success() => Ok(CacheStatus::FixNotRequired.into()),
            (None, Some(c)) if c.status.is_success() => Ok(CacheStatus::FixNotRequired.into()),
            (Some(p), Some(c)) if p.is_success() && c.status.is_success() => {
                Ok(CacheStatus::FixNotRequired.into())
            }
            _ => Ok(CacheStatus::FixRequired.into()),
        }
    }

    async fn evaluate_command_checks(
        &self,
        report: &mut ReportBuilder,
    ) -> Result<Option<CacheResults>, RuntimeError> {
        if let Some(action_command) = &self.action.check.command {
            let result = self.run_check_command(action_command, report).await?;
            return Ok(Some(result));
        }

        Ok(None)
    }

    async fn evaluate_path_check(
        &self,
        paths: &DoctorGroupCachePath,
    ) -> Result<CacheStatus, RuntimeError> {
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
            Ok(CacheStatus::FixNotRequired)
        } else {
            Ok(CacheStatus::FixRequired)
        }
    }

    async fn run_check_command(
        &self,
        action_command: &DoctorGroupActionCommand,
        report: &mut ReportBuilder,
    ) -> Result<CacheResults, RuntimeError> {
        info!("Evaluating {:?}", action_command);
        let mut result: Option<CacheStatus> = None;
        let mut command_output = String::new();
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
                    env_vars: self.generate_env_vars(),
                })
                .await?;

            report.add_capture(&output)?;

            command_output.push_str(&output.generate_user_output());
            command_output.push('\n');

            info!(
                "check ran command {} and result was {:?}",
                command, output.exit_code
            );

            let command_result = match output.exit_code {
                Some(0) => CacheStatus::FixNotRequired,
                Some(100..=i32::MAX) => CacheStatus::StopExecution,
                _ => CacheStatus::FixRequired,
            };

            let next = match &result {
                None => command_result,
                Some(prev) => cmp::max(prev.clone(), command_result.clone()),
            };

            result.replace(next);
            if result == Some(CacheStatus::StopExecution) {
                break;
            }
        }

        let trimmed_output = command_output.trim();
        let trimmed_output = if trimmed_output.is_empty() {
            None
        } else {
            Some(trimmed_output.to_string())
        };

        let status = result.unwrap_or(CacheStatus::FixRequired);
        Ok(CacheResults {
            status,
            output: trimmed_output,
        })
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

#[automock]
trait FileSystem: Send + Sync {
    fn find_files(&self, glob_pattern: &str) -> Result<Vec<PathBuf>>;
}

#[derive(Debug, Default)]
struct DefaultFileSystem {}

/// Abstract away filesystem access for use in testing.
/// This trait should be a thin wrapper around actions to the filesystem, ideally just action
/// and error handling. Adding more logic will make testing impossible without setting up a
/// filesystem.
impl FileSystem for DefaultFileSystem {
    /// Search for a glob pattern. This function expects the path to be absolute already,
    /// so that it's not dependent on the working directory.
    fn find_files(&self, glob_pattern: &str) -> Result<Vec<PathBuf>> {
        Ok(glob::glob(glob_pattern)?.filter_map(Result::ok).collect())
    }
}

#[derive(Educe)]
#[educe(Debug)]
pub struct DefaultGlobWalker {
    #[educe(Debug(ignore))]
    file_system: Box<dyn FileSystem>,
}

impl Default for DefaultGlobWalker {
    fn default() -> Self {
        Self {
            file_system: Box::<DefaultFileSystem>::default(),
        }
    }
}

fn make_absolute(base_dir: &Path, glob: &String) -> String {
    if glob.starts_with('/') {
        glob.to_string()
    } else {
        format!("{}/{}", base_dir.display(), glob)
    }
}

#[async_trait]
impl GlobWalker for DefaultGlobWalker {
    async fn have_globs_changed(
        &self,
        base_dir: &Path,
        paths: &[String],
        cache_name: &str,
        file_cache: Arc<dyn FileCache>,
    ) -> Result<bool, RuntimeError> {
        for glob_str in paths {
            let glob_path = make_absolute(base_dir, glob_str);
            for path in self.file_system.find_files(&glob_path)? {
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
        for glob_str in paths {
            let glob_path = make_absolute(base_dir, glob_str);
            for path in self.file_system.find_files(&glob_path)? {
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
        ActionRunStatus, DefaultDoctorActionRun, DefaultGlobWalker, DoctorActionRun, GlobWalker,
        MockFileSystem, MockGlobWalker, RuntimeError,
    };
    use crate::doctor::file_cache::{FileCache, MockFileCache, NoOpCache};
    use crate::doctor::tests::build_root_model;
    use crate::shared::prelude::*;
    use anyhow::{anyhow, Result};
    use predicates::prelude::predicate;
    use std::path::{Path, PathBuf};
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
            .withf(move |params| {
                params.args[0].eq(command) && params.env_vars.contains_key("SCOPE_BIN_DIR")
            })
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
        let mut report = ReportBuilder::new_blank("Test".to_string());

        command_result(&mut exec_runner, "check", vec![0]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action(&mut report).await?;
        assert_eq!(ActionRunStatus::CheckSucceeded, result.status);

        Ok(())
    }

    #[tokio::test]
    async fn test_only_exec_will_re_run() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();
        let mut report = ReportBuilder::new_blank("Test".to_string());

        command_result(&mut exec_runner, "check", vec![1, 0]);
        command_result(&mut exec_runner, "fix", vec![0]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action(&mut report).await?;
        assert_eq!(
            ActionRunStatus::CheckFailedFixSucceedVerifySucceed,
            result.status
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_fail_fix_succeed_check_fails() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();
        let mut report = ReportBuilder::new_blank("Test".to_string());

        command_result(&mut exec_runner, "check", vec![1, 1]);
        command_result(&mut exec_runner, "fix", vec![0]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action(&mut report).await?;
        assert_eq!(
            ActionRunStatus::CheckFailedFixSucceedVerifyFailed,
            result.status
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_fail_fix_fails() -> Result<()> {
        let action = build_run_fail_fix_succeed_action();
        let mut exec_runner = MockExecutionProvider::new();
        let glob_walker = MockGlobWalker::new();
        let mut report = ReportBuilder::new_blank("Test".to_string());

        command_result(&mut exec_runner, "check", vec![1]);
        command_result(&mut exec_runner, "fix", vec![1]);

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action(&mut report).await?;
        assert_eq!(ActionRunStatus::CheckFailedFixFailed, result.status);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_works() -> Result<()> {
        let action = build_file_fix_action();
        let mut glob_walker = MockGlobWalker::new();
        let mut exec_runner = MockExecutionProvider::new();
        let mut report = ReportBuilder::new_blank("Test".to_string());

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

        let result = run.run_action(&mut report).await?;
        assert_eq!(
            ActionRunStatus::CheckFailedFixSucceedVerifySucceed,
            result.status
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_works_unable_to_update_cache() -> Result<()> {
        let action = build_file_fix_action();
        let mut glob_walker = MockGlobWalker::new();
        let mut exec_runner = MockExecutionProvider::new();
        let mut report = ReportBuilder::new_blank("Test".to_string());

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

        let result = run.run_action(&mut report).await?;
        assert_eq!(
            ActionRunStatus::CheckFailedFixSucceedVerifySucceed,
            result.status
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_file_cache_invalid_fix_fails() -> Result<()> {
        let action = build_file_fix_action();
        let mut exec_runner = MockExecutionProvider::new();
        let mut glob_walker = MockGlobWalker::new();
        let mut report = ReportBuilder::new_blank("Test".to_string());

        command_result(&mut exec_runner, "fix", vec![1]);

        glob_walker
            .expect_have_globs_changed()
            .times(1)
            .returning(|_, _, _, _| Ok(false));
        glob_walker.expect_update_cache().never();

        let run = setup_test(vec![action], exec_runner, glob_walker);

        let result = run.run_action(&mut report).await?;
        assert_eq!(ActionRunStatus::CheckFailedFixFailed, result.status);

        Ok(())
    }

    #[tokio::test]
    async fn test_glob_walker_update_path_will_add_base_dir_to_path() {
        let mut file_system = MockFileSystem::new();
        let mut file_cache = MockFileCache::new();

        file_cache
            .expect_update_cache_entry()
            .once()
            .with(
                predicate::eq("file_cache".to_string()),
                predicate::eq(Path::new("/foo/bar")),
            )
            .returning(|_, _| Ok(()));

        file_system
            .expect_find_files()
            .once()
            .with(predicate::eq("/foo/root/*.txt"))
            .returning(|_| Ok(vec![PathBuf::from("/foo/bar")]));

        let walker = DefaultGlobWalker {
            file_system: Box::new(file_system),
        };

        let res = walker
            .update_cache(
                Path::new("/foo/root"),
                &["*.txt".to_string()],
                "file_cache",
                Arc::new(file_cache),
            )
            .await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_glob_walker_update_path_honor_abs_paths() {
        let mut file_system = MockFileSystem::new();
        let mut file_cache = MockFileCache::new();

        file_cache
            .expect_update_cache_entry()
            .once()
            .with(
                predicate::eq("file_cache".to_string()),
                predicate::eq(Path::new("/foo/bar")),
            )
            .returning(|_, _| Ok(()));

        file_system
            .expect_find_files()
            .once()
            .with(predicate::eq("/a/abs/path/*.txt"))
            .returning(|_| Ok(vec![PathBuf::from("/foo/bar")]));

        let walker = DefaultGlobWalker {
            file_system: Box::new(file_system),
        };

        let res = walker
            .update_cache(
                Path::new("/foo/root"),
                &["/a/abs/path/*.txt".to_string()],
                "file_cache",
                Arc::new(file_cache),
            )
            .await;
        assert!(res.is_ok());
    }
}
