use crate::models::{
    parse_config, DoctorExecCheckSpec, KnownErrorSpec, ModelRoot, ParsedConfig,
    ReportDefinitionSpec, ReportUploadLocationSpec,
};
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use colored::*;
use directories::{BaseDirs, UserDirs};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, warn};

#[derive(Parser, Debug)]
#[clap(group = ArgGroup::new("config"))]
pub struct ConfigOptions {
    /// Add a paths to search for configuration. By default, `scope` will search up
    /// for `.scope` directories and attempt to load `.yml` and `.yaml` files for config.
    /// If the config directory is somewhere else, specifying this option will _add_
    /// the paths/files to the loaded config.
    #[clap(long, env = "scope_CONFIG_DIR")]
    extra_config: Vec<String>,

    /// When set, default config files will not be loaded and only specified config will be loaded.
    #[arg(long, env = "scope_DISABLE_DEFAULT_CONFIG", default_value = "false")]
    disable_default_config: bool,

    /// Override the working directory
    #[arg(long, short = 'C')]
    working_dir: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FoundConfig {
    pub working_dir: PathBuf,
    pub exec_check: BTreeMap<String, ModelRoot<DoctorExecCheckSpec>>,
    pub known_error: BTreeMap<String, ModelRoot<KnownErrorSpec>>,
    pub report_upload: BTreeMap<String, ModelRoot<ReportUploadLocationSpec>>,
    pub report_definition: Option<ModelRoot<ReportDefinitionSpec>>,
}

impl FoundConfig {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            exec_check: BTreeMap::new(),
            known_error: BTreeMap::new(),
            report_upload: BTreeMap::new(),
            report_definition: None,
        }
    }
}

impl FoundConfig {
    fn add_model(&mut self, parsed_config: ParsedConfig) {
        match parsed_config {
            ParsedConfig::DoctorCheck(exec) => {
                insert_if_absent(&mut self.exec_check, exec);
            }
            ParsedConfig::KnownError(known_error) => {
                insert_if_absent(&mut self.known_error, known_error);
            }
            ParsedConfig::ReportUpload(report_upload) => {
                insert_if_absent(&mut self.report_upload, report_upload);
            }
            ParsedConfig::ReportDefinition(report_definition) => {
                if self.report_definition.is_none() {
                    self.report_definition.replace(report_definition);
                } else {
                    warn!(target: "user", "A ReportDefinition with duplicate name found, dropping ReportUpload {} in {}", report_definition.name(), report_definition.file_path());
                }
            }
        }
    }
}

fn insert_if_absent<T>(map: &mut BTreeMap<String, ModelRoot<T>>, entry: ModelRoot<T>) {
    let name = entry.name();
    if map.contains_key(name) {
        warn!(target: "user", "A {} with duplicate name found, dropping {} in {}", entry.kind().to_string().bold(), entry.name().bold(), entry.file_path());
    } else {
        map.insert(name.to_string(), entry);
    }
}

impl ConfigOptions {
    pub fn load_config(&self) -> Result<FoundConfig> {
        let current_dir = std::env::current_dir();
        let working_dir = match (current_dir, &self.working_dir) {
            (Ok(cwd), None) => cwd,
            (_, Some(dir)) => PathBuf::from(&dir),
            _ => {
                error!(target: "user", "Unable to get a working dir");
                return Err(anyhow!("Unable to get a working dir"));
            }
        };

        let mut found_config = FoundConfig::new(working_dir);

        for file_path in self.find_config_files(&found_config.working_dir)? {
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

    fn find_config_files(&self, working_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut config_paths = Vec::new();

        if !self.disable_default_config {
            let search_dir = working_dir.to_path_buf();
            for search_dir in search_dir.ancestors() {
                let scope_dir: PathBuf = search_dir.join(".scope");
                debug!("Checking if {} exists", scope_dir.display().to_string());
                if scope_dir.exists() {
                    config_paths.push(scope_dir)
                }
            }

            if let Some(user_dirs) = UserDirs::new() {
                config_paths.push(user_dirs.home_dir().join(".scope"));
            }

            if let Some(base_dirs) = BaseDirs::new() {
                config_paths.push(base_dirs.config_dir().join(".scope"));
            }
        }

        for extra_config in &self.extra_config {
            let scope_dir = Path::new(&extra_config);
            debug!("Checking if {} exists", scope_dir.display().to_string());
            if scope_dir.exists() {
                config_paths.push(scope_dir.to_path_buf())
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
