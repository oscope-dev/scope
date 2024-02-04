mod cli;
mod error;

pub mod prelude {
    pub use super::cli::{analyze_root, AnalyzeArgs};
}
