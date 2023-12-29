mod check;
mod error;
mod cli;
mod commands;

pub mod prelude {
    pub use crate::cli::DoctorArgs;
    pub use crate::cli::doctor_root;
}
