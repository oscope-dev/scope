use std::process::Stdio;
use chrono::{DateTime, Duration, Utc};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use std::fmt::Write;

#[derive(Debug, Default)]
struct RwLockOutput {
    output: RwLock<Vec<(DateTime<Utc>, String)>>
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
    exit_code: Option<i32>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    command: String,
}

pub enum OutputDestination {
    StandardOut,
    Logging
}

impl OutputCapture {
    pub async fn capture_output(args: &[String], output: &OutputDestination) -> anyhow::Result<Self> {
        let start_time = Utc::now();
        let mut command = tokio::process::Command::new("/usr/bin/env");
        let mut child = command.arg("-S").args(args)
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
                        OutputDestination::StandardOut => println!("{}", line)
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
                        OutputDestination::StandardOut => eprintln!("{}", line)
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
            exit_code: command_result.ok().map(|x| x.code()).flatten(),
            start_time,
            end_time,
            command: args.join(" "),
        })
    }

    pub fn generate_output(&self) -> String {
        let stdout: Vec<_> = self.stdout.iter().map(|(time, line)| {
            let offset: Duration = *time - self.start_time;
            (time.clone(), format!("{} OUT: {}", offset, line))
        }).collect();

        let stderr: Vec<_> = self.stderr.iter().map(|(time, line)| {
            let offset: Duration = *time - self.start_time;
            (time.clone(), format!("{} ERR: {}", offset, line))
        }).collect();

        let mut output = Vec::new();
        output.extend(stdout);
        output.extend(stderr);

        output.sort_by(|(l_time, _), (r_time, _)| l_time.cmp(&r_time));

        let text: String = output.iter().map(|(_, line)| line.clone()).collect::<Vec<_>>().join("\n");
        text
    }

    pub fn create_report_text(&self) -> anyhow::Result<String> {
        let mut f = String::new();
            write!(&mut f, "= Command Results\n")?;
            write!(&mut f, "Ran command `/usr/bin/env -S {}`\n", self.command)?;
            write!(&mut f, "Execution started: {}; finished: {}\n", self.start_time, self.end_time)?;
            write!(&mut f, "Result of command: {}\n", self.exit_code.unwrap_or_else(|| -1))?;
            write!(&mut f, "\n== Output\n")?;
            write!(&mut f, "\n[source,text]\n")?;
            write!(&mut f, "....\n")?;
            write!(&mut f, "{}\n", self.generate_output())?;
            write!(&mut f, "....\n")?;
            write!(&mut f, "\n")?;
            Ok(f)
    }

    pub fn get_stdout(&self) -> String {
        self.stdout.iter().map(|(_, line)| line.clone()).collect::<Vec<_>>().join("\n")
    }

    pub fn get_stderr(&self) -> String {
        self.stderr.iter().map(|(_, line)| line.clone()).collect::<Vec<_>>().join("\n")
    }
}
