use crate::redact::Redactor;
use chrono::{DateTime, Duration, Utc};
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

pub struct OutputCapture {
    pub working_dir: PathBuf,
    stdout: Vec<(DateTime<Utc>, String)>,
    stderr: Vec<(DateTime<Utc>, String)>,
    pub exit_code: Option<i32>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    pub command: String,
}

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

impl OutputCapture {
    pub async fn capture_output(
        working_dir: &Path,
        args: &[String],
        output_dest: &OutputDestination,
    ) -> Result<Self, CaptureError> {
        check_pre_exec(working_dir, args)?;

        let start_time = Utc::now();
        let mut command = tokio::process::Command::new("/usr/bin/env");
        let mut child = command
            .arg("-S")
            .args(args)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(working_dir)
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout to be available");
        let stderr = child.stderr.take().expect("stdout to be available");

        let stdout = {
            let captured = RwLockOutput::default();
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
            working_dir: working_dir.to_path_buf(),
            stdout: captured_stdout,
            stderr: captured_stderr,
            exit_code: command_result.ok().and_then(|x| x.code()),
            start_time,
            end_time,
            command: args.join(" "),
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

    pub fn create_report_text(&self, title: Option<&str>) -> anyhow::Result<String> {
        let mut f = String::new();
        let title = title.unwrap_or("## Command Results");
        writeln!(&mut f, "{}\n", title)?;
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
        writeln!(&mut f, "### Output")?;
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

fn check_pre_exec(working_dir: &Path, args: &[String]) -> Result<(), CaptureError> {
    let found_binary = match args.join(" ").split(' ').collect::<Vec<_>>().first() {
        None => {
            return Err(CaptureError::MissingShExec {
                name: args.join(" "),
            })
        }
        Some(path) => which_in(path, std::env::var_os("PATH"), working_dir),
    };

    let path = match found_binary {
        Ok(path) => path,
        Err(e) => {
            debug!("Unable to find binary {:?}", e);
            return Err(CaptureError::MissingShExec {
                name: args.join(" "),
            });
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
