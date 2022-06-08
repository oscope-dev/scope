use super::check::{CheckRuntime, RuntimeError, RuntimeResult};
use super::error::*;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env::current_exe;
use std::fs;
use std::path::PathBuf;

const DEFAULT_CONFIG_FILE: &str = "pity.doctor.yaml";

#[derive(Debug, Serialize, Deserialize)]
pub struct DoctorConfig {
    pub checks: Vec<CheckConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CheckConfig {
    #[serde(rename = "exec")]
    Exec(super::check::ExecCheck),
}

#[async_trait]
impl CheckRuntime for CheckConfig {
    async fn exec(&self) -> Result<RuntimeResult, RuntimeError> {
        match self {
            CheckConfig::Exec(check) => check.exec().await,
        }
    }

    fn description(&self) -> String {
        match self {
            CheckConfig::Exec(check) => check.description(),
        }
    }
    fn help_text(&self) -> String {
        match self {
            CheckConfig::Exec(check) => check.help_text(),
        }
    }
    fn name(&self) -> String {
        match self {
            CheckConfig::Exec(check) => check.name(),
        }
    }
}

pub async fn read_config(
    config_override: &Option<String>,
) -> Result<DoctorConfig, crate::error::ConfigError> {
    let config_file = match config_override {
        None => {
            let current_path = current_exe()?;
            let config_dir = current_path.parent();
            let config_dir = match config_dir {
                Some(path) => path,
                None => {
                    return Err(ConfigError::UnableToFindConfigFile {
                        path: current_path.to_str().unwrap().to_owned(),
                    });
                }
            };
            config_dir.join(DEFAULT_CONFIG_FILE)
        }
        Some(path) => PathBuf::from(path),
    };

    if !config_file.exists() {
        return Err(ConfigError::UnableToFindConfigFile {
            path: config_file.to_str().unwrap().to_owned(),
        });
    }

    let config_text = fs::read_to_string(config_file)?;
    let parsed_config: DoctorConfig = serde_yaml::from_str(&config_text)?;

    Ok(parsed_config)
}