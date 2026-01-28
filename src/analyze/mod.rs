mod cli;
mod error;
pub mod options;

pub mod prelude {
    pub use super::cli::{AnalyzeArgs, analyze_root};
}

// Re-export key types for library usage
pub use options::{AnalyzeInput, AnalyzeOptions};
pub use crate::shared::analyze::{process_lines, AnalyzeStatus, report_result};
