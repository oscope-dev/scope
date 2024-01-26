use crate::file_cache::{CacheStorage, FileCacheStatus};
use anyhow::Result;

use colored::Colorize;
#[cfg(not(test))]
use glob::glob;
use scope_lib::prelude::{
    CaptureError, CaptureOpts, DoctorGroup, DoctorGroupAction, DoctorGroupActionCommand,
    DoctorGroupCachePath, ModelRoot, OutputCapture, OutputDestination, ScopeModel,
};
use std::future::Future;
use std::path::{Path, PathBuf};
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

#[derive(Debug, PartialEq)]
pub enum CacheResults {
    FixNotRequired,
    FixRequired,
    StopExecution,
}

impl CacheResults {
    fn is_success(&self) -> bool {
        self == &CacheResults::FixNotRequired
    }
}

impl From<&OutputCapture> for CacheResults {
    fn from(value: &OutputCapture) -> Self {
        match value.exit_code {
            Some(0) => CacheResults::FixNotRequired,
            Some(100..=i32::MAX) => CacheResults::StopExecution,
            _ => CacheResults::FixRequired,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum CorrectionResults {
    NoFixSpecified,
    Success,
    Failure,
    FailAndStop,
}

impl From<&OutputCapture> for CorrectionResults {
    fn from(value: &OutputCapture) -> Self {
        match value.exit_code {
            Some(0) => CorrectionResults::Success,
            Some(100..=i32::MAX) => CorrectionResults::FailAndStop,
            _ => CorrectionResults::Failure,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ActionRunResult {
    Succeeded,
    Failed,
    Stop,
}

#[derive(Debug)]
pub struct DoctorActionRun<'a> {
    pub model: &'a ModelRoot<DoctorGroup>,
    pub action: &'a DoctorGroupAction,
    pub working_dir: &'a Path,
    pub file_cache: &'a CacheStorage,
    pub run_fix: bool,
}

impl<'a> DoctorActionRun<'a> {
    #[instrument(skip_all, fields(check.name = self.model.name(), action.description = self.action.description ))]
    pub async fn run_action(&self) -> Result<ActionRunResult> {
        let check_status = self.evaluate_checks().await?;
        let should_continue = match check_status {
            CacheResults::FixNotRequired => {
                info!(target: "user", name = self.model.name(), "Check was successful.");
                ActionRunResult::Succeeded
            }
            CacheResults::FixRequired => {
                if !self.run_fix {
                    info!(target: "user", name = self.model.name(), "Check failed. {}: Run with --fix to auto-fix", "Suggestion".bold());
                    ActionRunResult::Succeeded
                } else {
                    let fix_results = self.run_fixes().await?;
                    match fix_results {
                        CorrectionResults::Success => {
                            info!(target: "user",name = self.model.name(), "Check failed. {} ran successfully.", "Fix".bold());
                            ActionRunResult::Succeeded
                        }
                        CorrectionResults::NoFixSpecified => {
                            info!(target: "user", name = self.model.name(), "Check failed. No fix was specified.");
                            ActionRunResult::Stop
                        }
                        CorrectionResults::Failure => {
                            error!(target: "user", name = self.model.name(), "Check failed. The fix ran and {}.", "Failed".red().bold());
                            ActionRunResult::Failed
                        }
                        CorrectionResults::FailAndStop => {
                            error!(target: "user", name = self.model.name(), "Check failed. The fix ran and {}. The fix exited with a 'stop' code, skipping remaining checks.", "Failed".red().bold());
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
        let mut output = None;
        if let Some(action_command) = &self.action.fix {
            for command in &action_command.commands {
                let result = self.run_single_fix(command).await?;
                match (result, &output) {
                    (CorrectionResults::FailAndStop, _) => {
                        return Ok(CorrectionResults::FailAndStop);
                    }
                    (CorrectionResults::Failure, _) => {
                        output.replace(CorrectionResults::Failure);
                    }
                    (CorrectionResults::Success, None) => {
                        output.replace(CorrectionResults::Success);
                    }
                    _ => {}
                }
            }
        }

        match output {
            None => Ok(CorrectionResults::NoFixSpecified),
            Some(v) => Ok(v),
        }
    }

    async fn run_single_fix(&self, command: &str) -> Result<CorrectionResults, RuntimeError> {
        let args = vec![command.to_string()];
        let capture = OutputCapture::capture_output(CaptureOpts {
            working_dir: self.working_dir,
            args: &args,
            output_dest: OutputDestination::StandardOut,
            path: &self.model.exec_path(),
            env_vars: Default::default(),
        })
        .await?;

        Ok(CorrectionResults::from(&capture))
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
        let result = process_glob(&paths.base_path, &paths.paths, |path| {
            let check_full_name = self.model.file_path().clone();
            async move {
                let file_result = self.file_cache.check_file(check_full_name, &path).await?;
                Ok(file_result == FileCacheStatus::FileMatches)
            }
        })
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
        for command in &action_command.commands {
            let args = vec![command.clone()];
            let path = format!("{}:{}", self.model.containing_dir(), self.model.exec_path());
            let output = OutputCapture::capture_output(CaptureOpts {
                working_dir: self.working_dir,
                args: &args,
                output_dest: OutputDestination::Logging,
                path: &path,
                env_vars: Default::default(),
            })
            .await?;

            let cache_results = CacheResults::from(&output);
            if !cache_results.is_success() {
                return Ok(cache_results);
            }
        }

        Ok(CacheResults::FixRequired)
    }
}

#[cfg(not(test))]
async fn process_glob<'b, F, Ret: 'b>(
    base_dir: &Path,
    paths: &Vec<String>,
    fun: F,
) -> Result<bool, RuntimeError>
where
    F: Fn(PathBuf) -> Ret,
    Ret: Future<Output = Result<bool, RuntimeError>>,
{
    for glob_str in paths {
        let glob_path = format!("{}/{}", base_dir.display(), glob_str);
        for path in glob(&glob_path)?.filter_map(Result::ok) {
            let check_result = fun(path).await?;
            if !check_result {
                return Ok(false);
            }
        }
    }

    Ok(true)
}

#[cfg(test)]
async fn process_glob<'b, F, Ret: 'b>(
    _base_dir: &Path,
    paths: &Vec<String>,
    fun: F,
) -> Result<bool, RuntimeError>
where
    F: Fn(PathBuf) -> Ret,
    Ret: Future<Output = Result<bool, RuntimeError>>,
{
    for glob_str in paths {
        let check_result = fun(PathBuf::from(&glob_str)).await?;
        if !check_result {
            return Ok(false);
        }
    }

    Ok(true)
}
