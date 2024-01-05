use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Unable to process file. {error:?}")]
    IoError {
        #[from]
        error: std::io::Error,
    },
    #[error("Unable to parse config file. {error:?}")]
    SerdeError {
        #[from]
        error: serde_yaml::Error,
    },
}
