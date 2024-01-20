mod cli;
mod error;

pub mod prelude {
    pub use crate::cli::{analyze_root, AnalyzeArgs};
}
