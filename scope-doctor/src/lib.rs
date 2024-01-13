mod check;
mod cli;
mod commands;
mod error;
mod file_cache;

pub mod prelude {
    pub use crate::cli::doctor_root;
    pub use crate::cli::DoctorArgs;
}
