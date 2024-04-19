use clap::{ArgGroup, Parser, ValueEnum};
use gethostname::gethostname;
use indicatif::ProgressStyle;
use lazy_static::lazy_static;
use opentelemetry::KeyValue;
use opentelemetry_otlp::{TonicExporterBuilder, WithExportConfig};
use opentelemetry_sdk::metrics::reader::{DefaultAggregationSelector, DefaultTemporalitySelector};
use opentelemetry_sdk::{
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use std::fs::File;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tonic::metadata::MetadataMap;

use tracing::level_filters::LevelFilter;
use tracing_indicatif::filter::{hide_indicatif_span_fields, IndicatifFilter};
use tracing_indicatif::IndicatifLayer;
use tracing_opentelemetry::{MetricsLayer, OpenTelemetryLayer};
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
    #[arg(short, long, action = clap::ArgAction::Count, global(true))]
    pub verbose: u8,

    #[arg(
        long,
        global(true),
        default_value = "auto",
        env = "SCOPE_OUTPUT_PROGRESS"
    )]
    /// Set the progress output. Use plain to disable updating UI.
    pub progress: LoggingProgress,

    #[arg(skip = LevelFilter::WARN)]
    default_level: LevelFilter,

    /// When set metrics will be sent to an otel collector at the endpoint provided
    #[clap(long = "otel-collector", env = "SCOPE_OTEL_ENDPOINT", global(true))]
    otel_collector: Option<String>,
}

#[derive(ValueEnum, Debug, Copy, Clone)]
pub enum LoggingProgress {
    /// Determine output format based on execution context
    Auto,
    /// Standard output, no progress bar, no auto-updating output.
    Plain,
    /// Use progress bar
    Tty,
}

impl LoggingProgress {
    fn is_tty(&self) -> bool {
        match self {
            LoggingProgress::Auto => std::io::stdout().is_terminal(),
            LoggingProgress::Plain => false,
            LoggingProgress::Tty => true,
        }
    }
}

lazy_static! {
    pub static ref STDOUT_WRITER: Arc<RwLock<Box<dyn Write + Sync + Send>>> =
        Arc::new(RwLock::new(Box::new(std::io::stdout())));
    pub static ref STDERR_WRITER: Arc<RwLock<Box<dyn Write + Sync + Send>>> =
        Arc::new(RwLock::new(Box::new(std::io::stderr())));
}

impl LoggingOpts {
    pub fn with_new_default(&self, new_default: LevelFilter) -> Self {
        Self {
            verbose: self.verbose,
            progress: self.progress,
            default_level: new_default,
            otel_collector: self.otel_collector.clone(),
        }
    }

    pub fn to_level_filter(&self) -> LevelFilter {
        match self.verbose {
            0 => self.default_level,
            1 => LevelFilter::INFO,
            2 => LevelFilter::DEBUG,
            _ => LevelFilter::TRACE,
        }
    }

    fn make_exporter(&self, id: &str) -> TonicExporterBuilder {
        let endpoint = self.otel_collector.clone().unwrap();
        let mut map = MetadataMap::with_capacity(2);

        map.insert(
            "x-host",
            gethostname()
                .into_string()
                .unwrap_or_else(|_| "unknown".to_string())
                .parse()
                .unwrap(),
        );
        map.insert("x-id", id.parse().unwrap());

        opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(3))
            .with_metadata(map)
    }

    // this is a complicated type, but is only used in one place
    #[allow(clippy::type_complexity)]
    fn setup_otel<TL, ML>(
        &self,
        run_id: &str,
    ) -> Result<
        (
            Option<OpenTelemetryLayer<TL, trace::Tracer>>,
            Option<MetricsLayer<ML>>,
        ),
        anyhow::Error,
    >
    where
        TL: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
        ML: for<'span> tracing_subscriber::registry::LookupSpan<'span> + tracing::Subscriber,
    {
        let mut otel_tracer_layer = None;
        let mut otel_metrics_layer = None;
        if self.otel_collector.is_some() {
            let tracer = opentelemetry_otlp::new_pipeline()
                .tracing()
                .with_exporter(self.make_exporter(run_id))
                .with_trace_config(
                    trace::config()
                        .with_sampler(Sampler::AlwaysOn)
                        .with_id_generator(RandomIdGenerator::default())
                        .with_max_events_per_span(64)
                        .with_max_attributes_per_span(16)
                        .with_max_events_per_span(16)
                        .with_resource(Resource::new(vec![KeyValue::new("service.name", "scope")])),
                )
                .install_batch(opentelemetry_sdk::runtime::Tokio)?;

            otel_tracer_layer = Some(tracing_opentelemetry::layer().with_tracer(tracer));

            let metrics = opentelemetry_otlp::new_pipeline()
                .metrics(opentelemetry_sdk::runtime::Tokio)
                .with_exporter(self.make_exporter(run_id))
                .with_resource(Resource::new(vec![KeyValue::new(
                    "service.name",
                    "example",
                )]))
                .with_period(Duration::from_secs(3))
                .with_timeout(Duration::from_secs(10))
                .with_aggregation_selector(DefaultAggregationSelector::new())
                .with_temporality_selector(DefaultTemporalitySelector::new())
                .build()?;

            otel_metrics_layer = Some(MetricsLayer::new(metrics));
        }

        Ok((otel_tracer_layer, otel_metrics_layer))
    }

    pub async fn configure_logging(
        &self,
        run_id: &str,
        prefix: &str,
    ) -> (tracing_appender::non_blocking::WorkerGuard, String) {
        let file_name = format!("scope-{}-{}.log", prefix, run_id);
        let full_file_name = format!("/tmp/scope/{}", file_name);
        std::fs::create_dir_all("/tmp/scope").expect("to be able to create tmp dir");

        let (otel_tracer_layer, otel_metrics_layer) = self.setup_otel(run_id).unwrap_or_else(|e| {
            println!(
                "opentelemetry configuration failed. Events will not be sent. {:?}",
                e
            );
            (None, None)
        });

        let file_path = PathBuf::from(&full_file_name);
        let (non_blocking, guard) =
            tracing_appender::non_blocking(strip_ansi_escapes::Writer::new(
                File::create(file_path).expect("to be able to create log file"),
            ));

        let file_output = tracing_subscriber::fmt::layer()
            .event_format(Format::default().pretty())
            .with_ansi(false)
            .with_writer(non_blocking);

        let indicatif_layer = IndicatifLayer::new()
            .with_span_field_formatter(hide_indicatif_span_fields(DefaultFields::new()))
            .with_progress_style(default_progress_bar());
        let indicatif_writer = indicatif_layer.get_stdout_writer();

        *STDOUT_WRITER.write().await = Box::new(indicatif_layer.get_stdout_writer());
        *STDERR_WRITER.write().await = Box::new(indicatif_layer.get_stderr_writer());

        let is_tty_output = self.progress.is_tty();

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
            .with_filter(filter_fn(move |metadata| match metadata.target() {
                "user" => level_filter >= *metadata.level(),
                "always" => true,
                "progress" => !is_tty_output,
                "stdout" => false,
                _ => false,
            }));

        let progress_layer = if is_tty_output {
            Some(indicatif_layer.with_filter(IndicatifFilter::new(false)))
        } else {
            None
        };

        let subscriber = Registry::default()
            .with(console_output)
            .with(progress_layer)
            .with(otel_metrics_layer)
            .with(otel_tracer_layer)
            .with(file_output);

        tracing::subscriber::set_global_default(subscriber)
            .expect("setting default subscriber failed");

        (guard, full_file_name)
    }
}
