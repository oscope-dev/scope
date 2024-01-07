mod capture;
mod config_load;
mod logging;
mod models;
mod redact;
mod report;

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
