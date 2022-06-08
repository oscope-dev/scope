use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Unable to prcess file. {error:?}")]
    IoError {
        #[from]
        error: std::io::Error,
    },
    #[error("File {name} was not executable or it did not exist.")]
    MissingShExec { name: String },
    #[error("Unable to persist temp file. {error:?}")]
    UnableToWriteFile {
        #[from]
        error: tempfile::PersistError,
    },
    #[error("Unable to parse UTF-8 output. {error:?}")]
    FromUtf8Error {
        #[from]
        error: std::string::FromUtf8Error,
    },
}

pub struct RuntimeResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait CheckRuntime {
    async fn exec(&self) -> Result<RuntimeResult, RuntimeError>;
    fn description(&self) -> String;
    fn help_text(&self) -> String;
    fn name(&self) -> String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ExecCheck {
    target: String,
    description: String,
    help_text: String,
    name: String,
}

#[async_trait]
impl CheckRuntime for ExecCheck {
    async fn exec(&self) -> Result<RuntimeResult, RuntimeError> {
        let path = PathBuf::from(&self.target);
        if !path.exists() {
            return Err(RuntimeError::MissingShExec {
                name: self.target.to_owned(),
            });
        }
        let metadata = std::fs::metadata(path)?;
        let permissions = metadata.permissions().mode();
        if permissions & 0x700 == 0 {
            return Err(RuntimeError::MissingShExec {
                name: self.target.to_owned(),
            });
        }

        let output = Command::new(&self.target).output()?;

        Ok(RuntimeResult {
            success: output.status.success(),
            stdout: String::from_utf8(output.stdout)?,
            stderr: String::from_utf8(output.stderr)?,
        })
    }

    fn description(&self) -> String {
        self.description.to_owned()
    }
    fn help_text(&self) -> String {
        self.help_text.to_owned()
    }
    fn name(&self) -> String {
        self.name.to_owned()
    }
}
