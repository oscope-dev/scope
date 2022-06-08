use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Unable to find a config file {path}")]
    UnableToFindConfigFile { path: String },
    #[error("Unable to prcess file. {error:?}")]
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