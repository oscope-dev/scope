use anyhow::Result;
use clap::{ArgGroup, Args, Parser, Subcommand};
use human_panic::setup_panic;
use tracing::error;
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

#[derive(Args, Debug)]
#[clap(group = ArgGroup::new("logging"))]
pub struct LoggingOpts {
    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, parse(from_occurrences), global(true), group = "logging")]
    pub debug: u64,

    /// Enable warn logging
    #[clap(short, long, global(true), group = "logging")]
    pub warn: bool,

    /// Disable everything but error logging
    #[clap(short, long, global(true), group = "logging")]
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
}

#[tokio::main]
async fn main() {
    setup_panic!();
    dotenv::dotenv().ok();
    let opts = Cli::parse();

    let _gaurd = configure_logging(&opts.logging);
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
    let file_appender = tracing_appender::rolling::hourly("/tmp/pity", "doctor.log");
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

    guard
}

async fn handle_commands(command: &Command) -> Result<()> {
    match command {
        Command::Doctor(args) => doctor_root(args).await,
    }
}
