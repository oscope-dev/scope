mod capture;
mod config_load;
mod logging;
mod models;
mod report;
mod redact;

pub trait UserListing {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn location(&self) -> String;
}

pub mod prelude {
    pub use crate::capture::{OutputCapture, OutputDestination};
    pub use crate::config_load::{ConfigOptions, FoundConfig};
    pub use crate::logging::LoggingOpts;
    pub use crate::models::{
        parse_config, DoctorExecCheckSpec, ModelMetadata, ModelRoot, ParsedConfig,
    };
    pub use crate::report::ReportBuilder;
}
