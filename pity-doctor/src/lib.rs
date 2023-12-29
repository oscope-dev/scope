mod check;
mod cli;
mod commands;
mod error;

pub mod prelude {
    pub use crate::cli::doctor_root;
    pub use crate::cli::DoctorArgs;
}
