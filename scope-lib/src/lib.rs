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
    pub use crate::capture::{CaptureError, OutputCapture, OutputDestination};
    pub use crate::config_load::{ConfigOptions, FoundConfig};
    pub use crate::logging::LoggingOpts;
    pub use crate::models::{
        parse_config, DoctorExecCheckSpec, ModelMetadata, ModelRoot, ParsedConfig,
    };
    pub use crate::report::ReportBuilder;
}
