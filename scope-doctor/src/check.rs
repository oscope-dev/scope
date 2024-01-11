use crate::commands::DoctorRunArgs;
use crate::file_cache::{FileCache, FileCacheStatus};
use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;
use glob::glob;
use scope_lib::prelude::{
    CaptureError, CaptureOpts, DoctorExec, DoctorSetup, DoctorSetupExec, FoundConfig, ModelRoot,
    OutputCapture, OutputDestination,
};
use scope_lib::prelude::{DoctorSetupCache, ScopeModel};
use std::collections::BTreeSet;
use std::future::Future;
use std::ops::Deref;
use std::path::PathBuf;
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
    #[error(transparent)]
    AnyError(#[from] anyhow::Error),
    #[error(transparent)]
    PatternError(#[from] glob::PatternError),
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

    async fn check_cache<'a>(
        &self,
        found_config: &FoundConfig,
        file_cache: &'a dyn FileCache,
    ) -> Result<CacheResults, RuntimeError>;

    async fn run_correction<'a>(
        &self,
        found_config: &FoundConfig,
        file_cache: &'a dyn FileCache,
    ) -> Result<CorrectionResults, RuntimeError>;

    fn has_correction(&self) -> bool;

    fn help_text(&self) -> String;
}

#[async_trait]
impl CheckRuntime for ModelRoot<DoctorExec> {
    fn order(&self) -> i32 {
        self.spec.order
    }

    async fn check_cache<'a>(
        &self,
        found_config: &FoundConfig,
        _file_cache: &'a dyn FileCache,
    ) -> Result<CacheResults, RuntimeError> {
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

    async fn run_correction<'a>(
        &self,
        found_config: &FoundConfig,
        _file_cache: &'a dyn FileCache,
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

    async fn check_cache<'a>(
        &self,
        _found_config: &FoundConfig,
        file_cache: &'a dyn FileCache,
    ) -> Result<CacheResults, RuntimeError> {
        let check_full_name = self.full_name();
        let result = process_glob(&self, |path| {
            let check_full_name = check_full_name.clone();
            async move {
                let file_result = file_cache.check_file(check_full_name, &path).await?;
                Ok(file_result == FileCacheStatus::FileMatches)
            }
        })
        .await?;

        if result {
            Ok(CacheResults::NoWorkNeeded)
        } else {
            Ok(CacheResults::FixRequired)
        }
    }

    async fn run_correction<'a>(
        &self,
        found_config: &FoundConfig,
        file_cache: &'a dyn FileCache,
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

        let check_full_name = self.full_name();
        if let Err(e) = process_glob(&self, |path| {
            let check_full_name = check_full_name.clone();
            async move {
                file_cache
                    .update_cache_entry(check_full_name, &path)
                    .await?;
                Ok(true)
            }
        })
        .await
        {
            info!("Error when updating cache {:?}", e);
            warn!(target: "user", "Unable to update file cache, next run will see changes.");
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

async fn process_glob<'b, F, Ret: 'b>(
    model: &ModelRoot<DoctorSetup>,
    fun: F,
) -> Result<bool, RuntimeError>
where
    F: Fn(PathBuf) -> Ret,
    Ret: Future<Output = Result<bool, RuntimeError>>,
{
    let cache = match &model.spec.cache {
        DoctorSetupCache::Paths(p) => p,
    };

    let base_path_str = cache.base_path.display().to_string();
    for glob_str in &cache.paths {
        let glob_path = format!("{}/{}", base_path_str, glob_str);
        for path in glob(&glob_path)?.filter_map(Result::ok) {
            let check_result = fun(path).await?;
            if !check_result {
                return Ok(false);
            }
        }
    }

    Ok(true)
}
