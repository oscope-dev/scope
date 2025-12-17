mod cli;
mod error;

pub mod prelude {
    pub use super::cli::{AnalyzeArgs, analyze_root};
}
