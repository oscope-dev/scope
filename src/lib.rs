//! # dev-scope Library
//!
//! This library provides the core functionality for the `scope` tool,
//! designed for library-first usage with a thin CLI wrapper.
//!
//! ## Key Features
//!
//! - **Analyze**: Detect known errors in command output and log files
//! - **Doctor**: Run health checks with automatic fixes
//! - **Config**: Load and manage scope configuration files
//!
//! ## Library Usage
//!
//! The library can be used programmatically without CLI dependencies:
//!
//! ```rust,ignore
//! use dx_scope::{
//!     analyze::{AnalyzeOptions, process_lines, AnalyzeStatus},
//!     doctor::{DoctorRunOptions, PathRunResult},
//!     internal::prompts::{AutoApprove, DenyAll, UserInteraction},
//!     internal::progress::{NoOpProgress, ProgressReporter},
//!     shared::config::ConfigLoadOptions,
//! };
//!
//! // Run analysis with auto-approve
//! let options = AnalyzeOptions::default();
//! let interaction = AutoApprove;
//! // ... process_lines(&known_errors, &working_dir, input, &interaction).await
//! ```
//!
//! ## Modules
//!
//! - [`analyze`] - Log and output analysis for known errors
//! - [`doctor`] - Health checks and automatic fixes
//! - [`cli`] - CLI-specific implementations (InquireInteraction)
//! - [`internal`] - Abstraction traits (UserInteraction, ProgressReporter)
//! - [`shared`] - Shared utilities and configuration loading
//! - [`models`] - Data model definitions

pub mod analyze;
pub mod cli;
pub mod doctor;
pub mod internal;
pub mod lint;
pub mod models;
pub mod report;
pub mod shared;

// Re-export key types at crate root for convenience
// Analyze module
pub use analyze::{AnalyzeInput, AnalyzeOptions, AnalyzeStatus};

// Doctor module
pub use doctor::{DoctorRunOptions, PathRunResult};

// Config module
pub use shared::config::ConfigLoadOptions;
pub use shared::prelude::FoundConfig;

// Internal abstractions (for library implementors)
pub use internal::progress::{NoOpProgress, ProgressReporter};
pub use internal::prompts::{AutoApprove, DenyAll, UserInteraction};

// CLI implementations
pub use cli::InquireInteraction;

/// Prelude module for convenient glob imports.
///
/// This module re-exports commonly used types from all submodules.
/// For new code, prefer explicit imports from the crate root or specific modules.
pub mod prelude {
    pub use crate::analyze::prelude::*;
    pub use crate::doctor::prelude::*;
    pub use crate::lint::prelude::*;
    pub use crate::models::prelude::*;
    pub use crate::report::prelude::*;
    pub use crate::shared::prelude::*;
}

/// Preferred way to output data to users. This macro will write the output to tracing for debugging
/// and to stdout using the global stdout writer. Because we use the stdout writer, the calls
/// will all be async.
#[macro_export]
macro_rules! report_stdout {
    ($($arg:tt)*) => {
        tracing::info!(target="stdout", $($arg)*);
        writeln!($crate::prelude::STDOUT_WRITER.write().await, $($arg)*).ok()
    };
}
