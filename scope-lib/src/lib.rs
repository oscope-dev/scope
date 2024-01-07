mod capture;
mod config_load;
mod logging;
mod models;
mod redact;
mod report;

pub const FILE_PATH_ANNOTATION: &str = "scope.github.com/file-path";
pub const CONFIG_FILE_PATH_ENV: &str = "SCOPE_CONFIG_JSON";
pub const RUN_ID_ENV_VAR: &str = "SCOPE_RUN_ID";

pub trait HelpMetadata {
    fn description(&self) -> &str;
}

pub mod prelude {
    pub use crate::capture::{CaptureError, CaptureOpts, OutputCapture, OutputDestination};
    pub use crate::config_load::{build_config_path, ConfigOptions, FoundConfig};
    pub use crate::logging::LoggingOpts;
    pub use crate::models::{DoctorExecCheckSpec, ModelMetadata, ModelRoot, ParsedConfig};
    pub use crate::report::ReportBuilder;
}
