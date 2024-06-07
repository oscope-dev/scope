pub mod analyze;
pub mod doctor;
pub mod lint;
pub mod models;
pub mod report;
pub mod shared;

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
