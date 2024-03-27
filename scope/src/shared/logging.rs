use clap::{ArgGroup, Parser};
use indicatif::ProgressStyle;
use lazy_static::lazy_static;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use tracing::level_filters::LevelFilter;
use tracing_indicatif::filter::{hide_indicatif_span_fields, IndicatifFilter};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::fmt::format::DefaultFields;
use tracing_subscriber::{filter::filter_fn, prelude::*};
use tracing_subscriber::{
    fmt::format::{Format, PrettyFields},
    layer::SubscriberExt,
    Registry,
};

pub fn default_progress_bar() -> ProgressStyle {
    ProgressStyle::with_template(
        "{span_child_prefix} {spinner:.green} {wide_msg} {pos:>7}/{len:7} [{elapsed_precise}]",
    )
    .unwrap()
    .progress_chars("##-")
}

pub fn progress_bar_without_pos() -> ProgressStyle {
    ProgressStyle::with_template(
        "{span_child_prefix} {spinner:.green} {wide_msg} [{elapsed_precise}]",
    )
    .unwrap()
    .progress_chars("##-")
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

    #[arg(skip = LevelFilter::INFO)]
    default_level: LevelFilter,
}

lazy_static! {
    pub static ref STDOUT_WRITER: Arc<RwLock<Box<dyn std::io::Write + Sync + Send>>> =
        Arc::new(RwLock::new(Box::new(std::io::stdout())));
    pub static ref STDERR_WRITER: Arc<RwLock<Box<dyn std::io::Write + Sync + Send>>> =
        Arc::new(RwLock::new(Box::new(std::io::stderr())));
}

impl LoggingOpts {
    pub fn with_new_default(&self, new_default: LevelFilter) -> Self {
        Self {
            debug: self.debug,
            warn: self.warn,
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

    pub async fn configure_logging(
        &self,
        run_id: &str,
        prefix: &str,
    ) -> (tracing_appender::non_blocking::WorkerGuard, String) {
        let file_name = format!("scope-{}-{}.log", prefix, run_id);
        let full_file_name = format!("/tmp/scope/{}", file_name);
        std::fs::create_dir_all("/tmp/scope").expect("to be able to create tmp dir");

        let file_path = PathBuf::from(&full_file_name);
        let (non_blocking, guard) = tracing_appender::non_blocking(
            File::create(file_path).expect("to be able to create log file"),
        );

        let file_output = tracing_subscriber::fmt::layer()
            .event_format(Format::default().pretty())
            .with_writer(non_blocking);

        let indicatif_layer = IndicatifLayer::new()
            .with_span_field_formatter(hide_indicatif_span_fields(DefaultFields::new()))
            .with_progress_style(default_progress_bar());
        let indicatif_writer = indicatif_layer.get_stdout_writer();

        *STDOUT_WRITER.write().await = Box::new(indicatif_layer.get_stdout_writer());
        *STDERR_WRITER.write().await = Box::new(indicatif_layer.get_stderr_writer());

        let level_filter = self.to_level_filter();
        let console_output = tracing_subscriber::fmt::layer()
            .event_format(
                Format::default()
                    .with_target(false)
                    .without_time()
                    .compact(),
            )
            .with_writer(indicatif_writer)
            .fmt_fields(PrettyFields::new())
            .with_filter(filter_fn(move |metadata| {
                metadata.target() == "user" && level_filter >= *metadata.level()
                    || metadata.target() == "always"
            }));

        let subscriber = Registry::default()
            .with(console_output)
            .with(indicatif_layer.with_filter(IndicatifFilter::new(false)))
            // .with(console)
            .with(file_output);

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        (guard, full_file_name)
    }
}
