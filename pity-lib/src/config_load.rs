use crate::models::{
    parse_config, DoctorExecCheckSpec, KnownErrorSpec, ModelRoot, ParsedConfig, ReportUploadSpec,
    FILE_PATH_ANNOTATION,
};
use anyhow::Result;
use clap::{ArgGroup, Parser};
use directories::{BaseDirs, UserDirs};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

#[derive(Parser, Debug)]
#[clap(group = ArgGroup::new("config"))]
pub struct ConfigOptions {
    /// Add a paths to search for configuration. By default, `pity` will search up
    /// for `.pity` directories and attempt to load `.yml` and `.yaml` files for config.
    /// If the config directory is somewhere else, specifying this option will _add_
    /// the paths/files to the loaded config.
    #[clap(long, env = "PITY_CONFIG_DIR")]
    extra_config: Vec<String>,

    /// When set, default config files will not be loaded and only specified config will be loaded.
    #[arg(long, env = "PITY_DISABLE_DEFAULT_CONFIG", default_value = "false")]
    disable_default_config: bool,
}

#[derive(Default, Debug, Clone)]
pub struct FoundConfig {
    pub exec_check: BTreeMap<String, ModelRoot<DoctorExecCheckSpec>>,
    pub known_error: BTreeMap<String, ModelRoot<KnownErrorSpec>>,
    pub report_upload: BTreeMap<String, ModelRoot<ReportUploadSpec>>,
}

impl FoundConfig {
    fn add_model(&mut self, parsed_config: ParsedConfig) {
        match parsed_config {
            ParsedConfig::DoctorCheck(exec) => {
                let name = exec.metadata.name.clone();
                if let Some(old) = self.exec_check.insert(name, exec) {
                    let path = old
                        .metadata
                        .annotations
                        .get(FILE_PATH_ANNOTATION)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());
                    warn!(target: "user", "A DoctorCheck with duplicate name found, dropping check {} in {}", old.metadata.name, path);
                }
            }
            ParsedConfig::KnownError(known_error) => {
                let name = known_error.metadata.name.clone();
                if let Some(old) = self.known_error.insert(name, known_error) {
                    let path = old
                        .metadata
                        .annotations
                        .get(FILE_PATH_ANNOTATION)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());
                    warn!(target: "user", "A KnownError with duplicate name found, dropping KnownError {} in {}", old.metadata.name, path);
                }
            }
            ParsedConfig::ReportUpload(report_upload) => {
                let name = report_upload.metadata.name.clone();
                if let Some(old) = self.report_upload.insert(name, report_upload) {
                    let path = old
                        .metadata
                        .annotations
                        .get(FILE_PATH_ANNOTATION)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());
                    warn!(target: "user", "A ReportUpload with duplicate name found, dropping ReportUpload {} in {}", old.metadata.name, path);
                }
            }
        }
    }
}

impl ConfigOptions {
    pub fn load_config(&self) -> Result<FoundConfig> {
        let mut found_config = FoundConfig::default();
        for file_path in self.find_config_files()? {
            let file_contents = fs::read_to_string(&file_path)?;
            let parsed_file_contents = match parse_config(file_path.as_path(), &file_contents) {
                Ok(configs) => configs,
                Err(e) => {
                    warn!(target: "user", "Unable to parse {} because {:?}", file_path.display().to_string(), e);
                    continue;
                }
            };
            for config in parsed_file_contents {
                found_config.add_model(config);
            }
        }

        debug!("Loaded config {:?}", found_config);

        Ok(found_config)
    }

    fn find_config_files(&self) -> Result<Vec<PathBuf>> {
        let mut config_paths = Vec::new();

        if !self.disable_default_config {
            let search_dir = std::env::current_dir()?;
            for search_dir in search_dir.ancestors() {
                let pity_dir: PathBuf = search_dir.join(".pity");
                debug!("Checking if {} exists", pity_dir.display().to_string());
                if pity_dir.exists() {
                    config_paths.push(pity_dir)
                }
            }

            if let Some(user_dirs) = UserDirs::new() {
                config_paths.push(user_dirs.home_dir().join(".pity"));
            }

            if let Some(base_dirs) = BaseDirs::new() {
                config_paths.push(base_dirs.config_dir().join(".pity"));
            }
        }

        for extra_config in &self.extra_config {
            let pity_dir = Path::new(&extra_config);
            debug!("Checking if {} exists", pity_dir.display().to_string());
            if pity_dir.exists() {
                config_paths.push(pity_dir.to_path_buf())
            }
        }

        let mut config_files = Vec::new();
        for path in config_paths {
            config_files.extend(expand_path(&path)?);
        }

        Ok(config_files)
    }
}

fn expand_path(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    if path.is_dir() {
        let mut files = Vec::new();
        for dir_entry in fs::read_dir(path)?.flatten() {
            if !dir_entry.path().is_file() {
                continue;
            }

            let file_path = dir_entry.path();
            let extension = file_path.extension();
            if extension == Some(OsStr::new("yaml")) || extension == Some(OsStr::new("yml")) {
                debug!(target: "user", "Found file {:?}", file_path);
                files.push(file_path);
            }
        }

        return Ok(files);
    }

    warn!("Unknown file type {}", path.display().to_string());
    Ok(Vec::new())
}
