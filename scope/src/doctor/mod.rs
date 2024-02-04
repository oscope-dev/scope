mod check;
mod cli;
mod commands;
mod error;
mod file_cache;
mod runner;
#[cfg(test)]
mod tests;

pub mod prelude {
    pub use super::cli::doctor_root;
    pub use super::cli::DoctorArgs;
    pub use super::commands::generate_doctor_list;
}
