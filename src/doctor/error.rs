use thiserror::Error;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum FileCacheError {
    #[error("Unable to access filesystem to do cache operations.")]
    FsError,
    #[error("Unable to write to cache. {0:?}")]
    WriteIoError(std::io::Error),
    #[error("Error deserializing cache data. {0:?}")]
    SerializationError(serde_json::Error),
    #[error("IoError {0:?}")]
    IoError(#[from] std::io::Error),
}
