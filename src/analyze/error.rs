use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnalyzeError {
    #[error("Unable to find/open {file_name}")]
    FileNotFound { file_name: String },
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}
