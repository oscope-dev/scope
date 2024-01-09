use async_trait::async_trait;
use scope_lib::prelude::{
    CaptureError, CaptureOpts, DoctorExec, FoundConfig, ModelRoot, OutputCapture,
    OutputDestination,
};
use thiserror::Error;

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Unable to process file. {error:?}")]
    IoError {
        #[from]
        error: std::io::Error,
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
    async fn exec(&self, found_config: &FoundConfig) -> Result<RuntimeResult, RuntimeError>;
    fn description(&self) -> String;
    fn help_text(&self) -> String;
}

#[async_trait]
impl CheckRuntime for ModelRoot<DoctorExec> {
    async fn exec(&self, found_config: &FoundConfig) -> Result<RuntimeResult, RuntimeError> {
        let args = vec![self.spec.check_exec.clone()];
        let output = OutputCapture::capture_output(CaptureOpts {
            working_dir: &found_config.working_dir,
            args: &args,
            output_dest: OutputDestination::Null,
            path: &found_config.bin_path,
            env_vars: Default::default(),
        })
        .await?;

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
}
