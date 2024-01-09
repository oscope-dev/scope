use crate::commands::DoctorRunArgs;
use async_trait::async_trait;
use colored::Colorize;
use scope_lib::prelude::ScopeModel;
use scope_lib::prelude::{
    CaptureError, CaptureOpts, DoctorExec, DoctorSetup, DoctorSetupExec, FoundConfig, ModelRoot,
    OutputCapture, OutputDestination,
};
use std::collections::BTreeSet;
use std::ops::Deref;
use thiserror::Error;
use tracing::{info, warn};

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
    #[error("Fix was not specified")]
    FixNotDefined,
    #[error(transparent)]
    CaptureError(#[from] CaptureError),
}

pub enum DoctorTypes {
    Exec(ModelRoot<DoctorExec>),
    Setup(ModelRoot<DoctorSetup>),
}

impl Deref for DoctorTypes {
    type Target = dyn CheckRuntime;

    fn deref(&self) -> &Self::Target {
        match self {
            DoctorTypes::Exec(e) => e,
            DoctorTypes::Setup(s) => s,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum CacheResults {
    NoWorkNeeded,
    FixRequired,
}

#[derive(Debug, PartialEq)]
pub enum CorrectionResults {
    Success,
    Failure,
}

#[async_trait]
pub trait CheckRuntime: ScopeModel {
    fn order(&self) -> i32;

    fn should_run_check(&self, runtime_args: &DoctorRunArgs) -> bool {
        let check_names: BTreeSet<_> = match &runtime_args.only {
            None => return true,
            Some(check_names) => check_names.iter().map(|x| x.to_lowercase()).collect(),
        };

        let names = BTreeSet::from([self.name().to_lowercase(), self.full_name().to_lowercase()]);
        !check_names.is_disjoint(&names)
    }

    async fn check_cache(&self, found_config: &FoundConfig) -> Result<CacheResults, RuntimeError>;

    async fn run_correction(
        &self,
        found_config: &FoundConfig,
    ) -> Result<CorrectionResults, RuntimeError>;

    fn has_correction(&self) -> bool;

    fn help_text(&self) -> String;
}

#[async_trait]
impl CheckRuntime for ModelRoot<DoctorExec> {
    fn order(&self) -> i32 {
        self.spec.order
    }

    #[tracing::instrument(skip_all, fields(check_name = %self.full_name()))]
    async fn check_cache(&self, found_config: &FoundConfig) -> Result<CacheResults, RuntimeError> {
        let args = vec![self.spec.check_exec.clone()];
        let output = OutputCapture::capture_output(CaptureOpts {
            working_dir: &found_config.working_dir,
            args: &args,
            output_dest: OutputDestination::Logging,
            path: &found_config.bin_path,
            env_vars: Default::default(),
        })
        .await?;

        let cache_results = match output.exit_code == Some(0) {
            true => CacheResults::NoWorkNeeded,
            false => CacheResults::FixRequired,
        };

        Ok(cache_results)
    }

    #[tracing::instrument(skip_all, fields(check_name = %self.full_name()))]
    async fn run_correction(
        &self,
        found_config: &FoundConfig,
    ) -> Result<CorrectionResults, RuntimeError> {
        let check_path = match &self.spec.fix_exec {
            None => return Err(RuntimeError::FixNotDefined),
            Some(path) => path.to_string(),
        };

        let args = vec![check_path];
        let capture = OutputCapture::capture_output(CaptureOpts {
            working_dir: &found_config.working_dir,
            args: &args,
            output_dest: OutputDestination::StandardOut,
            path: &found_config.bin_path,
            env_vars: Default::default(),
        })
        .await?;

        if capture.exit_code == Some(0) {
            info!(target: "user", "Check {} failed. {} ran successfully", self.name().bold(), "Fix".bold());
            Ok(CorrectionResults::Success)
        } else {
            warn!(target: "user", "Check {} failed. The fix ran and {}.", self.name().bold(), "Failed".red().bold());
            Ok(CorrectionResults::Failure)
        }
    }

    fn has_correction(&self) -> bool {
        self.spec.fix_exec.is_some()
    }

    fn help_text(&self) -> String {
        self.spec.help_text.to_owned()
    }
}

#[async_trait]
impl CheckRuntime for ModelRoot<DoctorSetup> {
    fn order(&self) -> i32 {
        self.spec.order
    }

    #[tracing::instrument(skip_all, fields(check_name = %self.full_name()))]
    async fn check_cache(&self, _found_config: &FoundConfig) -> Result<CacheResults, RuntimeError> {
        Ok(CacheResults::FixRequired)
    }

    #[tracing::instrument(skip_all, fields(check_name = %self.full_name()))]
    async fn run_correction(
        &self,
        found_config: &FoundConfig,
    ) -> Result<CorrectionResults, RuntimeError> {
        let DoctorSetupExec::Exec(commands) = &self.spec.exec;

        for command in commands {
            let args = vec![command.clone()];
            let capture = OutputCapture::capture_output(CaptureOpts {
                working_dir: &found_config.working_dir,
                args: &args,
                output_dest: OutputDestination::StandardOut,
                path: &found_config.bin_path,
                env_vars: Default::default(),
            })
            .await?;

            if capture.exit_code != Some(0) {
                return Ok(CorrectionResults::Failure);
            }
        }
        Ok(CorrectionResults::Success)
    }

    fn has_correction(&self) -> bool {
        true
    }

    fn help_text(&self) -> String {
        match &self.spec.exec {
            DoctorSetupExec::Exec(commands) => {
                format!("Run {} to setup", commands.join(", "))
            }
        }
    }
}
