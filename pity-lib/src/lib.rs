mod capture;
mod config;
mod logging;
mod report;

pub mod prelude {
    pub use crate::capture::{OutputCapture, OutputDestination};
    pub use crate::config::{parse_config, ExecCheck, ModelMetadata, ModelRoot, ParsedConfig};
    pub use crate::logging::LoggingOpts;
    pub use crate::report::write_to_report_file;
}
