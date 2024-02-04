mod check;
mod cli;
mod commands;
mod error;
mod file_cache;
mod runner;
#[cfg(test)]
mod tests;

pub mod prelude {
    pub use crate::cli::doctor_root;
    pub use crate::cli::DoctorArgs;
    pub use crate::commands::generate_doctor_list;
}
