use async_trait::async_trait;
use pity_lib::prelude::{
    CaptureError, DoctorExecCheckSpec, ModelRoot, OutputCapture, OutputDestination,
};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Unable to prcess file. {error:?}")]
    IoError {
        #[from]
        error: std::io::Error,
    },
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
    #[error(transparent)]
    CaptureError(#[from] CaptureError),
}

pub struct RuntimeResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait CheckRuntime {
    async fn exec(&self, working_dir: &Path) -> Result<RuntimeResult, RuntimeError>;
    fn description(&self) -> String;
    fn help_text(&self) -> String;
    fn name(&self) -> String;
}

#[async_trait]
impl CheckRuntime for ModelRoot<DoctorExecCheckSpec> {
    async fn exec(&self, working_dir: &Path) -> Result<RuntimeResult, RuntimeError> {
        let args = vec![self.spec.check_exec.clone()];
        let output =
            OutputCapture::capture_output(working_dir, &args, &OutputDestination::Null).await?;

        Ok(RuntimeResult {
            success: output.exit_code == Some(0),
            stdout: output.get_stdout(),
            stderr: output.get_stderr(),
        })
    }

    fn description(&self) -> String {
        self.spec.description.to_owned()
    }
    fn help_text(&self) -> String {
        self.spec.help_text.to_owned()
    }
    fn name(&self) -> String {
        self.metadata.name.to_owned()
    }
}
