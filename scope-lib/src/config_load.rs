use crate::models::{
    parse_config, DoctorExecCheckSpec, KnownErrorSpec, ModelRoot, ParsedConfig,
    ReportDefinitionSpec, ReportUploadLocationSpec,
};
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use colored::*;
use directories::{BaseDirs, UserDirs};
use itertools::Itertools;
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
    #[clap(long, env = "SCOPE_CONFIG_DIR", global(true))]
    extra_config: Vec<String>,

    /// When set, default config files will not be loaded and only specified config will be loaded.
    #[arg(
        long,
        env = "SCOPE_DISABLE_DEFAULT_CONFIG",
        default_value = "false",
        global(true)
    )]
    disable_default_config: bool,

    /// Override the working directory
    #[arg(long, short = 'C', global(true))]
    working_dir: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FoundConfig {
    pub working_dir: PathBuf,
    pub exec_check: BTreeMap<String, ModelRoot<DoctorExecCheckSpec>>,
    pub known_error: BTreeMap<String, ModelRoot<KnownErrorSpec>>,
    pub report_upload: BTreeMap<String, ModelRoot<ReportUploadLocationSpec>>,
    pub report_definition: Option<ModelRoot<ReportDefinitionSpec>>,
    pub config_path: Vec<PathBuf>,
    pub bin_path: String,
}

impl FoundConfig {
    pub fn new(working_dir: PathBuf, config_path: Vec<PathBuf>) -> Self {
        let bin_path = std::env::var("PATH").unwrap_or_default();
        let scope_path = config_path
            .iter()
            .map(|x| x.join("bin").display().to_string())
            .join(":");
        Self {
            working_dir,
            exec_check: BTreeMap::new(),
            known_error: BTreeMap::new(),
            report_upload: BTreeMap::new(),
            report_definition: None,
            config_path,
            bin_path: [bin_path, scope_path].join(":"),
        }
    }

    pub fn get_report_definition(&self) -> ReportDefinitionSpec {
        self.report_definition
            .as_ref()
            .map(|x| x.spec.clone())
            .clone()
            .unwrap_or_else(|| ReportDefinitionSpec {
                template: "== Error report for {{ command }}.".to_string(),
                additional_data: Default::default(),
            })
    }

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

        let config_path = self.find_scope_paths(&working_dir);
        let mut found_config = FoundConfig::new(working_dir, config_path);

        for file_path in self.find_config_files(&found_config.config_path)? {
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

    fn find_config_files(&self, config_dirs: &Vec<PathBuf>) -> Result<Vec<PathBuf>> {
        let mut config_files = Vec::new();
        for path in config_dirs {
            config_files.extend(expand_path(path)?);
        }

        Ok(config_files)
    }

    fn find_scope_paths(&self, working_dir: &Path) -> Vec<PathBuf> {
        let mut config_paths = Vec::new();

        if !self.disable_default_config {
            for scope_dir in build_config_path(working_dir) {
                debug!("Checking if {} exists", scope_dir.display().to_string());
                if scope_dir.exists() {
                    config_paths.push(scope_dir)
                }
            }
        }

        for extra_config in &self.extra_config {
            let scope_dir = Path::new(&extra_config);
            debug!("Checking if {} exists", scope_dir.display().to_string());
            if scope_dir.exists() {
                config_paths.push(scope_dir.to_path_buf())
            }
        }

        config_paths
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

pub fn build_config_path(working_dir: &Path) -> Vec<PathBuf> {
    let mut scope_path = Vec::new();

    let working_dir = fs::canonicalize(working_dir).expect("working dir to be a path");
    let search_dir = working_dir.to_path_buf();
    for search_dir in search_dir.ancestors() {
        let scope_dir: PathBuf = search_dir.join(".scope");
        scope_path.push(scope_dir)
    }

    if let Some(user_dirs) = UserDirs::new() {
        scope_path.push(user_dirs.home_dir().join(".scope"));
    }

    if let Some(base_dirs) = BaseDirs::new() {
        scope_path.push(base_dirs.config_dir().join(".scope"));
    }

    scope_path
}
