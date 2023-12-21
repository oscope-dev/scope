use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use clap::{Args};
use sysinfo::{System, SystemExt};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};
use std::process::{Stdio};
use tokio::{io::{AsyncBufReadExt, BufReader}};
// use futures::StreamExt;

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Where the report will be generated, if not set a location will be determined at runtime.
    #[arg(long, short = 'o')]
    report_location: Option<String>,

    /// The command that should be run and reported on
    #[arg(last = true, required = true)]
    command: Vec<String>,
}

pub async fn report_root(_args: &ReportArgs) -> Result<()> {
    let mut sys = System::new_all();
    let report_builder = crate::report::DataCapture::new(&sys);
    let token = CancellationToken::new();

    let report_builder = Arc::new(report_builder);

    {
        let report_builder = report_builder.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let report_builder = report_builder.clone();
            while !token.is_cancelled() {
                sys.refresh_processes();
                for (_pid, process) in sys.processes() {
                    report_builder.handle_process(process).await;
                }
                sleep(Duration::from_millis(10)).await;
            }
        });
    }

    let mut command = tokio::process::Command::new("sh");
    let mut child = command.args(["-c", "ps"])
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().expect("stdout to be available");
    let stderr = child.stderr.take().expect("stdout to be available");

    let stdout = {
        let report_builder = report_builder.clone();
        async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Some(line) = reader.next_line().await? {
                report_builder.add_stdout(&line).await;
                info!(target:   "user", "{}", line);
            }

            Ok::<(), anyhow::Error>(())
        }
    };

    let stderr = {
        let report_builder = report_builder.clone();
        async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Some(line) = reader.next_line().await? {
                report_builder.add_stderr(&line).await;
                error!(target:"user", "{}", line);
            }

            Ok::<(), anyhow::Error>(())
        }
    };

    let (command_result, _, _) = tokio::join!(child.wait(), stdout, stderr);
    debug!("join result {:?}", command_result);

    token.cancel();

    report_builder.make_report().await?;

    Ok(())
}

