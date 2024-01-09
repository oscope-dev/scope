use std::path::PathBuf;

#[derive(Debug, PartialEq, Clone)]
pub enum DoctorSetupExec {
    Exec(Vec<String>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorSetupCachePath {
    pub(crate) paths: Vec<String>,
    pub(crate) base_path: PathBuf,
}

#[derive(Debug, PartialEq, Clone)]
pub enum DoctorSetupCache {
    Paths(DoctorSetupCachePath),
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorSetup {
    pub cache: DoctorSetupCache,
    pub exec: DoctorSetupExec,
    pub description: String,
    pub help_text: String,
}
