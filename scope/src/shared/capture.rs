use super::redact::Redactor;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use derive_builder::Builder;
use mockall::automock;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use which::which_in;

#[derive(Debug, Default)]
struct RwLockOutput {
    output: RwLock<Vec<(DateTime<Utc>, String)>>,
}

impl RwLockOutput {
    async fn add_line(&self, line: &str) {
        let mut stdout = self.output.write().await;
        stdout.push((Utc::now(), line.to_string()));
    }
}

#[derive(Default, Builder)]
#[builder(setter(into))]
pub struct OutputCapture {
    #[builder(default)]
    pub working_dir: PathBuf,
    #[builder(default)]
    stdout: Vec<(DateTime<Utc>, String)>,
    #[builder(default)]
    stderr: Vec<(DateTime<Utc>, String)>,
    #[builder(default)]
    pub exit_code: Option<i32>,
    #[builder(default)]
    start_time: DateTime<Utc>,
    #[builder(default)]
    end_time: DateTime<Utc>,
    #[builder(default)]
    pub command: String,
}

#[derive(Clone, Debug)]
pub enum OutputDestination {
    StandardOut,
    Logging,
    Null,
}

#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("Unable to process file. {error:?}")]
    IoError {
        #[from]
        error: std::io::Error,
    },
    #[error("File {name} was not executable or it did not exist.")]
    MissingShExec { name: String },
    #[error("Unable to parse UTF-8 output. {error:?}")]
    FromUtf8Error {
        #[from]
        error: std::string::FromUtf8Error,
    },
}

#[automock]
#[async_trait]
pub trait ExecutionProvider: Send + Sync {
    async fn run_command<'a>(&self, opts: CaptureOpts<'a>) -> Result<OutputCapture, CaptureError>;
}

#[derive(Default, Debug)]
pub struct DefaultExecutionProvider {}

#[async_trait]
impl ExecutionProvider for DefaultExecutionProvider {
    async fn run_command<'a>(&self, opts: CaptureOpts<'a>) -> Result<OutputCapture, CaptureError> {
        OutputCapture::capture_output(opts).await
    }
}

pub struct CaptureOpts<'a> {
    pub working_dir: &'a Path,
    pub env_vars: BTreeMap<String, String>,
    pub path: &'a str,
    pub args: &'a [String],
    pub output_dest: OutputDestination,
}

impl<'a> CaptureOpts<'a> {
    fn command(&self) -> String {
        self.args.join(" ")
    }
}

impl OutputCapture {
    pub async fn capture_output(opts: CaptureOpts<'_>) -> Result<Self, CaptureError> {
        check_pre_exec(&opts)?;
        let args = opts.args.to_vec();

        debug!("Executing PATH={} {:?}", &opts.path, &args);

        let start_time = Utc::now();
        let mut command = tokio::process::Command::new("/usr/bin/env");
        let mut child = command
            .arg("-S")
            .args(args)
            .env("PATH", opts.path)
            .envs(&opts.env_vars)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(opts.working_dir)
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout to be available");
        let stderr = child.stderr.take().expect("stdout to be available");

        let stdout = {
            let captured = RwLockOutput::default();
            let output_dest = opts.output_dest.clone();
            async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Some(line) = reader.next_line().await? {
                    captured.add_line(&line).await;
                    match output_dest {
                        OutputDestination::Logging => info!("{}", line),
                        OutputDestination::StandardOut => println!("{}", line),
                        OutputDestination::Null => {}
                    }
                }

                Ok::<_, anyhow::Error>(captured.output.into_inner())
            }
        };

        let stderr = {
            let captured = RwLockOutput::default();
            let output_dest = opts.output_dest.clone();
            async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Some(line) = reader.next_line().await? {
                    captured.add_line(&line).await;
                    match output_dest {
                        OutputDestination::Logging => error!("{}", line),
                        OutputDestination::StandardOut => eprintln!("{}", line),
                        OutputDestination::Null => {}
                    }
                }

                Ok::<_, anyhow::Error>(captured.output.into_inner())
            }
        };

        let (command_result, wait_stdout, wait_stderr) = tokio::join!(child.wait(), stdout, stderr);
        let end_time = Utc::now();
        debug!("join result {:?}", command_result);

        let captured_stdout = wait_stdout.unwrap_or_default();
        let captured_stderr = wait_stderr.unwrap_or_default();

        Ok(Self {
            working_dir: opts.working_dir.to_path_buf(),
            stdout: captured_stdout,
            stderr: captured_stderr,
            exit_code: command_result.ok().and_then(|x| x.code()),
            start_time,
            end_time,
            command: opts.command(),
        })
    }

    pub fn generate_output(&self) -> String {
        let stdout: Vec<_> = self
            .stdout
            .iter()
            .map(|(time, line)| {
                let offset: Duration = *time - self.start_time;
                (*time, format!("{} OUT: {}", offset, line))
            })
            .collect();

        let stderr: Vec<_> = self
            .stderr
            .iter()
            .map(|(time, line)| {
                let offset: Duration = *time - self.start_time;
                (*time, format!("{} ERR: {}", offset, line))
            })
            .collect();

        let mut output = Vec::new();
        output.extend(stdout);
        output.extend(stderr);

        output.sort_by(|(l_time, _), (r_time, _)| l_time.cmp(r_time));

        let text: String = output
            .iter()
            .map(|(_, line)| line.clone())
            .collect::<Vec<_>>()
            .join("\n");

        Redactor::new().redact_text(&text).to_string()
    }

    pub fn create_report_text(&self) -> anyhow::Result<String> {
        let mut f = String::new();
        writeln!(&mut f, "### Command Results\n")?;
        writeln!(&mut f, "Ran command `/usr/bin/env -S {}`", self.command)?;
        writeln!(
            &mut f,
            "Execution started: {}; finished: {}",
            self.start_time, self.end_time
        )?;
        writeln!(
            &mut f,
            "Result of command: {}",
            self.exit_code.unwrap_or(-1)
        )?;
        writeln!(&mut f)?;
        writeln!(&mut f, "#### Output")?;
        writeln!(&mut f)?;
        writeln!(&mut f, "```text")?;
        writeln!(&mut f, "{}", self.generate_output().trim())?;
        writeln!(&mut f, "```")?;
        writeln!(&mut f)?;
        Ok(f)
    }

    pub fn get_stdout(&self) -> String {
        self.stdout
            .iter()
            .map(|(_, line)| line.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn get_stderr(&self) -> String {
        self.stderr
            .iter()
            .map(|(_, line)| line.clone())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn check_pre_exec(opts: &CaptureOpts) -> Result<(), CaptureError> {
    let command = opts.command();
    let found_binary = match command.split(' ').collect::<Vec<_>>().first() {
        None => return Err(CaptureError::MissingShExec { name: command }),
        Some(path) => which_in(path, Some(OsString::from(opts.path)), opts.working_dir),
    };

    let path = match found_binary {
        Ok(path) => path,
        Err(e) => {
            debug!("Unable to find binary {:?}", e);
            return Err(CaptureError::MissingShExec { name: command });
        }
    };

    if !path.exists() {
        return Err(CaptureError::MissingShExec {
            name: path.display().to_string(),
        });
    }
    let metadata = std::fs::metadata(&path)?;
    let permissions = metadata.permissions().mode();
    if permissions & 0x700 == 0 {
        return Err(CaptureError::MissingShExec {
            name: path.display().to_string(),
        });
    }

    Ok(())
}
