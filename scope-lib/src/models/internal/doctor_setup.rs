use std::path::PathBuf;

#[derive(Debug, PartialEq, Clone)]
pub enum DoctorSetupSpecExec {
    Exec(Vec<String>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorSetupSpecCachePath {
    pub(crate) paths: Vec<String>,
    pub(crate) base_path: PathBuf,
}

#[derive(Debug, PartialEq, Clone)]
pub enum DoctorSetupSpecCache {
    Paths(DoctorSetupSpecCachePath),
}

#[derive(Debug, PartialEq, Clone)]
pub struct DoctorSetupSpec {
    pub cache: DoctorSetupSpecCache,
    pub exec: DoctorSetupSpecExec,
    pub description: String,
    pub help_text: String,
}
