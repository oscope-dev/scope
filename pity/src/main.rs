use anyhow::Result;
use clap::{ArgGroup, Parser, Subcommand};
use human_panic::setup_panic;
use tracing::{error, info};
use tracing_subscriber::{filter::filter_fn, prelude::*};
use tracing_subscriber::{
    fmt::{
        format::{Format, JsonFields, PrettyFields},
        time,
    },
    layer::SubscriberExt,
    Registry,
};
use pity_doctor::prelude::*;
use pity_report::prelude::{report_root, ReportArgs};

/// Pity the Fool
///
/// Pity is a tool to enable teams to manage local machine
/// checks. An example would be a team that supports other
/// engineers may want to verify that the engineer asking
/// for support's machine is setup correctly.
#[derive(Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(flatten)]
    logging: LoggingOpts,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Parser, Debug)]
#[clap(group = ArgGroup::new("logging"))]
pub struct LoggingOpts {
    /// A level of verbosity, and can be used multiple times
    #[arg(short, long, action = clap::ArgAction::Count, global(true), group = "logging")]
    pub debug: u8,

    /// Enable warn logging
    #[arg(short, long, global(true), group = "logging")]
    pub warn: bool,

    /// Disable everything but error logging
    #[arg(short, long, global(true), group = "logging")]
    pub error: bool,
}

impl LoggingOpts {
    pub fn to_level_filter(&self) -> tracing::level_filters::LevelFilter {
        use tracing::level_filters::LevelFilter;

        if self.error {
            LevelFilter::ERROR
        } else if self.warn {
            LevelFilter::WARN
        } else if self.debug == 0 {
            LevelFilter::INFO
        } else if self.debug == 1 {
            LevelFilter::DEBUG
        } else {
            LevelFilter::TRACE
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run checks that will "checkup" your machine.
    Doctor(DoctorArgs),
    /// Generate a bug report based from a command that was ran
    Report(ReportArgs)
}

#[tokio::main]
async fn main() {
    setup_panic!();
    dotenv::dotenv().ok();
    let opts = Cli::parse();

    let _guard = configure_logging(&opts.logging);
    let error_code = match handle_commands(&opts.command).await {
        Ok(_) => 0,
        Err(e) => {
            error!("Critical Error. {}", e);
            1
        }
    };

    std::process::exit(error_code);
}

fn configure_logging(logging_opts: &LoggingOpts) -> tracing_appender::non_blocking::WorkerGuard {

    let id = nanoid::nanoid!(4, &nanoid::alphabet::SAFE);
    let now = chrono::Local::now();
    let current_time = now.format("%Y%m%d");
    let file_name = format!("pity-{}-{}.log", current_time.to_string(), id);
    let full_path = format!("/tmp/pity/{}", file_name);
    let file_appender = tracing_appender::rolling::never("/tmp/pity", file_name);
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_output = tracing_subscriber::fmt::layer()
        .event_format(Format::default().json().flatten_event(true))
        .fmt_fields(JsonFields::new())
        .with_writer(non_blocking);

    let console_output = tracing_subscriber::fmt::layer()
        .with_timer(time::LocalTime::rfc_3339())
        .event_format(Format::default().with_target(false).compact())
        .fmt_fields(PrettyFields::new());

    let subscriber = Registry::default()
        .with(logging_opts.to_level_filter())
        .with(console_output.with_filter(filter_fn(move |metadata| metadata.target() == "user")))
        .with(file_output);

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    info!(target: "user", "More details logs at {}", full_path);

    guard
}

async fn handle_commands(command: &Command) -> Result<()> {
    match command {
        Command::Doctor(args) => doctor_root(args).await,
        Command::Report(args) => report_root(args).await,
    }
}
