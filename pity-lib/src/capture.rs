use chrono::{DateTime, Duration, Utc};
use std::fmt::Write;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use crate::redact::Redactor;

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
    stdout: Vec<(DateTime<Utc>, String)>,
    stderr: Vec<(DateTime<Utc>, String)>,
    pub exit_code: Option<i32>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    command: String,
}

pub enum OutputDestination {
    StandardOut,
    Logging,
    Null,
}


impl OutputCapture {
    pub async fn capture_output(
        args: &[String],
        output: &OutputDestination,
    ) -> anyhow::Result<Self> {
        let start_time = Utc::now();
        let mut command = tokio::process::Command::new("/usr/bin/env");
        let mut child = command
            .arg("-S")
            .args(args)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().expect("stdout to be available");
        let stderr = child.stderr.take().expect("stdout to be available");

        let stdout = {
            let captured = RwLockOutput::default();
            async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Some(line) = reader.next_line().await? {
                    captured.add_line(&line).await;
                    match output {
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
                    match output {
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
