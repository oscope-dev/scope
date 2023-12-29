mod error;
mod config;
mod cli;

pub mod prelude {
    pub use crate::cli::ReportArgs;
    pub use crate::cli::report_root;
}
