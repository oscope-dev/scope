use crate::models::prelude::{ModelMetadata, ModelRoot};
use crate::models::HelpMetadata;
use crate::shared::models::prelude::{
    DoctorGroup, KnownError, ParsedConfig, ReportDefinition, ReportUploadLocation,
};
use crate::shared::RUN_ID_ENV_VAR;
use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use colored::*;
use directories::{BaseDirs, UserDirs};
use itertools::Itertools;
use serde::Deserialize;
use serde_yaml::{Deserializer, Value};

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Write;
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

    /// When outputting logs, or other files, the run-id is the unique value that will define where these go.
    /// In the case that the run-id is re-used, the old values will be overwritten.
    #[arg(long, global(true), env = RUN_ID_ENV_VAR)]
    run_id: Option<String>,
}

impl ConfigOptions {
    pub fn generate_run_id() -> String {
        let id = nanoid::nanoid!(4, &nanoid::alphabet::SAFE);
        let now = chrono::Local::now();
        let current_time = now.format("%Y%m%d");
        format!("{}-{}", current_time, id)
    }
    pub fn get_run_id(&self) -> String {
        self.run_id.clone().unwrap_or_else(Self::generate_run_id)
    }

    pub async fn load_config(&self) -> Result<FoundConfig> {
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
        let found_config = FoundConfig::new(self, working_dir, config_path).await;

        debug!("Loaded config {:?}", found_config);

        Ok(found_config)
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

#[derive(Debug, Clone)]
pub struct FoundConfig {
    pub working_dir: PathBuf,
    pub raw_config: Vec<ModelRoot<Value>>,
    pub doctor_group: BTreeMap<String, DoctorGroup>,
    pub known_error: BTreeMap<String, KnownError>,
    pub report_upload: BTreeMap<String, ReportUploadLocation>,
    pub report_definition: Option<ReportDefinition>,
    pub config_path: Vec<PathBuf>,
    pub bin_path: String,
    pub run_id: String,
}

impl FoundConfig {
    pub fn empty(working_dir: PathBuf) -> Self {
        let bin_path = std::env::var("PATH").unwrap_or_default();

        Self {
            working_dir,
            raw_config: Vec::new(),
            doctor_group: BTreeMap::new(),
            known_error: BTreeMap::new(),
            report_upload: BTreeMap::new(),
            report_definition: None,
            config_path: Vec::new(),
            run_id: ConfigOptions::generate_run_id(),
            bin_path,
        }
    }
    pub async fn new(
        config_options: &ConfigOptions,
        working_dir: PathBuf,
        config_path: Vec<PathBuf>,
    ) -> Self {
        let default_path = std::env::var("PATH").unwrap_or_default();
        let scope_path = config_path
            .iter()
            .map(|x| x.join("bin").display().to_string())
            .join(":");

        let mut raw_config = load_all_config(&config_path).await;
        raw_config.sort_by_key(|x| x.full_name());

        let mut this = Self {
            working_dir,
            raw_config: raw_config.clone(),
            doctor_group: BTreeMap::new(),
            known_error: BTreeMap::new(),
            report_upload: BTreeMap::new(),
            report_definition: None,
            config_path,
            bin_path: [scope_path, default_path].join(":"),
            run_id: config_options.get_run_id(),
        };

        for raw_config in raw_config {
            if let Ok(value) = raw_config.try_into() {
                this.add_model(value);
            }
        }

        this
    }

    pub fn write_raw_config_to_disk(&self) -> Result<PathBuf> {
        let json = serde_json::to_string(&self.raw_config)?;
        let json_bytes = json.as_bytes();
        let file_path = PathBuf::from_iter(vec![
            "/tmp",
            "scope",
            &format!("config-{}.json", self.run_id),
        ]);

        debug!("Merged config destination is to {}", file_path.display());

        let mut file = File::create(&file_path)?;
        file.write_all(json_bytes)?;

        Ok(file_path)
    }

    pub fn get_report_definition(&self) -> ReportDefinition {
        self.report_definition
            .clone()
            .unwrap_or_else(|| ReportDefinition {
                full_name: "ReportDefinition/generated".to_string(),
                metadata: ModelMetadata::new("generated"),
                template: "== Error report for {{ command }}.".to_string(),
                additional_data: Default::default(),
            })
    }

    fn add_model(&mut self, parsed_config: ParsedConfig) {
        match parsed_config {
            ParsedConfig::DoctorGroup(exec) => {
                insert_if_absent(&mut self.doctor_group, exec);
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
                    warn!(target: "user", "A ReportDefinition with duplicate name found, dropping ReportUpload {} in {}", report_definition.name(), report_definition.metadata().file_path());
                }
            }
        }
    }
}

fn insert_if_absent<T: HelpMetadata>(map: &mut BTreeMap<String, T>, entry: T) {
    let name = entry.name().to_string();
    if map.contains_key(&name) {
        warn!(target: "user", "Duplicate {} found, dropping {} in {}", entry.full_name().to_string().bold(), entry.name().bold(), entry.metadata().file_path());
    } else {
        map.insert(name.to_string(), entry);
    }
}

async fn load_all_config(paths: &Vec<PathBuf>) -> Vec<ModelRoot<Value>> {
    let mut loaded_values = Vec::new();

    for file_path in expand_to_files(paths) {
        let file_contents = match fs::read_to_string(&file_path) {
            Err(e) => {
                warn!(target: "user", "Unable to read file {} because {}", file_path.display().to_string(), e);
                continue;
            }
            Ok(content) => content,
        };
        for doc in Deserializer::from_str(&file_contents) {
            if let Some(parsed_model) = parse_model(doc, &file_path) {
                loaded_values.push(parsed_model)
            }
        }
    }

    loaded_values
}

pub(crate) fn parse_model(doc: Deserializer, file_path: &Path) -> Option<ModelRoot<Value>> {
    let value = match Value::deserialize(doc) {
        Ok(value) => value,
        Err(e) => {
            warn!(target: "user", "Unable to load document from {} because {}", file_path.display(), e);
            return None;
        }
    };

    match serde_yaml::from_value::<ModelRoot<Value>>(value) {
        Ok(mut value) => {
            value.metadata.annotations.file_path = Some(file_path.display().to_string());

            value.metadata.annotations.file_dir =
                Some(file_path.parent().unwrap().display().to_string());

            value.metadata.annotations.bin_path = Some(build_exec_path(file_path));
            Some(value)
        }
        Err(e) => {
            warn!(target: "user", "Unable to parse model from {} because {}", file_path.display(), e);
            None
        }
    }
}

fn build_exec_path(file_path: &Path) -> String {
    let mut paths = vec![file_path.parent().unwrap().display().to_string()];
    for ancestor in file_path.ancestors() {
        let bin_path = ancestor.join("bin");
        if bin_path.exists() {
            paths.push(bin_path.display().to_string());
        }
    }

    paths.push(std::env::var("PATH").unwrap_or_default());

    paths.join(":")
}

fn expand_to_files(paths: &Vec<PathBuf>) -> Vec<PathBuf> {
    let mut config_files = Vec::new();
    for path in paths {
        let expanded_paths = expand_path(path).unwrap_or_else(|e| {
            warn!(target: "user", "Unable to access filesystem because {}", e);
            Vec::new()
        });
        config_files.extend(expanded_paths);
    }

    config_files
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
