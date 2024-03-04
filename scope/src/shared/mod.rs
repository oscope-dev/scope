use colored::Colorize;

use crate::models::HelpMetadata;
use std::cmp::max;
use std::path::Path;
use tracing::info;

mod capture;
mod config_load;
mod logging;
// mod models_bck;
mod models;
mod redact;
mod report;

pub const CONFIG_FILE_PATH_ENV: &str = "SCOPE_CONFIG_JSON";
pub const RUN_ID_ENV_VAR: &str = "SCOPE_RUN_ID";

pub mod prelude {
    pub use super::capture::{
        CaptureError, CaptureOpts, DefaultExecutionProvider, ExecutionProvider,
        MockExecutionProvider, OutputCapture, OutputCaptureBuilder, OutputDestination,
    };
    pub use super::config_load::{build_config_path, ConfigOptions, FoundConfig};
    pub use super::logging::LoggingOpts;
    pub use super::models::prelude::*;
    pub use super::print_details;
    pub use super::report::ReportBuilder;
    pub use super::{CONFIG_FILE_PATH_ENV, RUN_ID_ENV_VAR};
}

pub(crate) fn convert_to_string(input: Vec<&str>) -> Vec<String> {
    input.iter().map(|x| x.to_string()).collect()
}

pub fn print_details<T>(working_dir: &Path, config: &Vec<T>)
where
    T: HelpMetadata,
{
    let max_name_length = config
        .iter()
        .map(|x| x.full_name().len())
        .max()
        .unwrap_or(20);
    let max_name_length = max(max_name_length, 20) + 2;

    info!(target: "user", "  {:max_name_length$}{:60}{}", "Name".white().bold(), "Description".white().bold(), "Path".white().bold());
    for resource in config {
        let mut description = resource.description().to_string();
        if description.len() > 55 {
            description.truncate(55);
            description = format!("{}...", description);
        }

        let mut loc = resource.metadata().file_path();
        let diff_path = pathdiff::diff_paths(&loc, working_dir);
        if let Some(diff) = diff_path {
            loc = diff.display().to_string();
        } else if loc.len() > 35 {
            loc = format!("...{}", loc.split_off(loc.len() - 35));
        }

        info!(target: "user", "- {:max_name_length$}{:60}{}", resource.full_name(), description, loc);
    }
}
