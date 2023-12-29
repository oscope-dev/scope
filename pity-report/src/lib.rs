mod cli;
mod config;
mod error;

pub mod prelude {
    pub use crate::cli::report_root;
    pub use crate::cli::ReportArgs;
}
