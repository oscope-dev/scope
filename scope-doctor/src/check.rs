use async_trait::async_trait;
use colored::Colorize;
use scope_lib::prelude::{
    CaptureError, CaptureOpts, DoctorExec, DoctorSetup, DoctorSetupExec, FoundConfig, ModelRoot,
    OutputCapture, OutputDestination,
};
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
pub trait CheckRuntime {
    async fn check_cache(&self, found_config: &FoundConfig) -> Result<CacheResults, RuntimeError>;
    async fn run_correction(
        &self,
        found_config: &FoundConfig,
    ) -> Result<CorrectionResults, RuntimeError>;
    fn help_text(&self) -> String;
}

#[async_trait]
impl CheckRuntime for ModelRoot<DoctorExec> {
    async fn check_cache(&self, found_config: &FoundConfig) -> Result<CacheResults, RuntimeError> {
        let args = vec![self.spec.check_exec.clone()];
        let output = OutputCapture::capture_output(CaptureOpts {
            working_dir: &found_config.working_dir,
            args: &args,
            output_dest: OutputDestination::Null,
            path: &found_config.bin_path,
            env_vars: Default::default(),
        })
        .await?;

        let cache_results = match output.exit_code == Some(0) {
            true => CacheResults::NoWorkNeeded,
            false => CacheResults::FixRequired,
        };

        info!(
            check = self.name(),
            output = "stdout",
            successful = cache_results == CacheResults::NoWorkNeeded,
            "{}",
            output.get_stdout()
        );
        info!(
            check = self.name(),
            output = "stderr",
            successful = cache_results == CacheResults::NoWorkNeeded,
            "{}",
            output.get_stderr()
        );

        Ok(cache_results)
    }

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

    fn help_text(&self) -> String {
        self.spec.help_text.to_owned()
    }
}

#[async_trait]
impl CheckRuntime for ModelRoot<DoctorSetup> {
    async fn check_cache(&self, found_config: &FoundConfig) -> Result<CacheResults, RuntimeError> {
        Ok(CacheResults::FixRequired)
    }

    async fn run_correction(
        &self,
        found_config: &FoundConfig,
    ) -> Result<CorrectionResults, RuntimeError> {
        Ok(CorrectionResults::Failure)
    }

    fn help_text(&self) -> String {
        match &self.spec.exec {
            DoctorSetupExec::Exec(commands) => {
                format!("Run {} to setup", commands.join(", "))
            }
        }
    }
}
