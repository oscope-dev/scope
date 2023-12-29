use clap::{ArgGroup, Parser};
use tracing::{info};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{filter::filter_fn, prelude::*};
use tracing_subscriber::{
    fmt::{
        format::{Format, JsonFields, PrettyFields},
        time,
    },
    layer::SubscriberExt,
    Registry,
};

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

    #[arg(skip = LevelFilter::INFO)]
    default_level: LevelFilter
}

impl LoggingOpts {

    pub fn with_new_default(&self, new_default: LevelFilter) -> Self {
        Self {
            debug: self.debug,
            warn : self.warn,
            error: self.error,
            default_level: new_default,
        }
    }

    pub fn to_level_filter(&self) -> LevelFilter {
        if self.error {
            LevelFilter::ERROR
        } else if self.warn {
            LevelFilter::WARN
        } else if self.debug == 0 {
            self.default_level
        } else if self.debug == 1 {
            LevelFilter::DEBUG
        } else {
            LevelFilter::TRACE
        }
    }

    pub fn configure_logging(&self, prefix: &str) -> tracing_appender::non_blocking::WorkerGuard {
        let id = nanoid::nanoid!(4, &nanoid::alphabet::SAFE);
        let now = chrono::Local::now();
        let current_time = now.format("%Y%m%d");
        let file_name = format!("pity-{}-{}-{}.log", prefix, current_time.to_string(), id);
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
            .with(self.to_level_filter())
            .with(console_output.with_filter(filter_fn(move |metadata| metadata.target() == "user")))
            .with(file_output);

        tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

        info!(target: "user", "More details logs at {}", full_path);

        guard
    }
}