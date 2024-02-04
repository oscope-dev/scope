use std::cmp::max;
use crate::models::{ModelRoot, ScopeModel};
use colored::Colorize;
use std::path::Path;
use tracing::info;

mod capture;
mod config_load;
mod logging;
// mod models_bck;
mod models;
mod redact;
mod report;

pub const FILE_PATH_ANNOTATION: &str = "scope.github.com/file-path";
pub const FILE_DIR_ANNOTATION: &str = "scope.github.com/file-dir";
pub const FILE_EXEC_PATH_ANNOTATION: &str = "scope.github.com/bin-path";
pub const CONFIG_FILE_PATH_ENV: &str = "SCOPE_CONFIG_JSON";
pub const RUN_ID_ENV_VAR: &str = "SCOPE_RUN_ID";

pub trait HelpMetadata {
    fn description(&self) -> &str;
}

pub mod prelude {
    pub use crate::capture::{
        CaptureError, CaptureOpts, DefaultExecutionProvider, ExecutionProvider,
        MockExecutionProvider, OutputCapture, OutputCaptureBuilder, OutputDestination,
    };
    pub use crate::config_load::{build_config_path, ConfigOptions, FoundConfig};
    pub use crate::logging::LoggingOpts;
    pub use crate::models::prelude::*;
    pub use crate::print_details;
    pub use crate::report::ReportBuilder;
    pub use crate::HelpMetadata;
}

pub(crate) fn convert_to_string(input: Vec<&str>) -> Vec<String> {
    input.iter().map(|x| x.to_string()).collect()
}

pub fn print_details<T>(working_dir: &Path, config: Vec<&ModelRoot<T>>)
where
    T: HelpMetadata,
{
    let max_name_length = config.iter().map(|x| x.name().len()).max().unwrap_or(20);
    let max_name_length = max(max_name_length, 20) + 2;

    info!(target: "user", "{:max_name_length$}{:60}{}", "Name".white().bold(), "Description".white().bold(), "Path".white().bold());
    for check in config {
        let mut description = check.spec.description().to_string();
        if description.len() > 55 {
            description.truncate(55);
            description = format!("{}...", description);
        }

        let mut loc = check.file_path();
        let diff_path = pathdiff::diff_paths(&loc, working_dir);
        if let Some(diff) = diff_path {
            loc = diff.display().to_string();
        } else if loc.len() > 35 {
            loc = format!("...{}", loc.split_off(loc.len() - 35));
        }

        info!(target: "user", "{:max_name_length$}{:60}{}", check.name().white().bold(), description, loc);
    }
}
