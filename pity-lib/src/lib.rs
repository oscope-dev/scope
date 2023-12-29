mod logging;
mod capture;
mod report;

pub mod prelude {
    pub use crate::logging::{LoggingOpts};
    pub use crate::capture::{OutputCapture, OutputDestination};
    pub use crate::report::write_to_report_file;
}