use std::time::Duration;

use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::{
    Resource, metrics::SdkMeterProvider, propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use tracing_subscriber::{
    EnvFilter,
    filter::filter_fn,
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
};

use crate::config::ObservabilityConfig;

pub struct ObservabilityGuard {
    tracer_provider: Option<SdkTracerProvider>,
    meter_provider: Option<SdkMeterProvider>,
}

impl ObservabilityGuard {
    pub fn shutdown(self) {
        if let Some(meter_provider) = self.meter_provider
            && let Err(error) = meter_provider.shutdown()
        {
            tracing::warn!(%error, "failed to flush metrics");
        }
        if let Some(tracer_provider) = self.tracer_provider
            && let Err(error) = tracer_provider.shutdown()
        {
            tracing::warn!(%error, "failed to flush traces");
        }
    }
}

pub fn init(
    config: &ObservabilityConfig,
) -> Result<ObservabilityGuard, opentelemetry_otlp::ExporterBuildError> {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));

    let Some(endpoint) = config.otlp_endpoint.as_deref() else {
        init_subscriber(config, filter, None);
        return Ok(ObservabilityGuard {
            tracer_provider: None,
            meter_provider: None,
        });
    };

    let resource = Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.name", config.service_name.clone()),
            KeyValue::new("service.version", config.service_version.clone()),
            KeyValue::new(
                "deployment.environment.name",
                config.deployment_environment.clone(),
            ),
        ])
        .build();
    let trace_endpoint = signal_endpoint(endpoint, "traces");
    let metric_endpoint = signal_endpoint(endpoint, "metrics");

    let tracer_exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(trace_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()?;
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(tracer_exporter)
        .with_resource(resource.clone())
        .build();
    let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(metric_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()?;
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(metric_exporter)
        .with_resource(resource)
        .build();

    global::set_meter_provider(meter_provider.clone());
    global::set_tracer_provider(tracer_provider.clone());
    init_subscriber(config, filter, Some(&tracer_provider));

    Ok(ObservabilityGuard {
        tracer_provider: Some(tracer_provider),
        meter_provider: Some(meter_provider),
    })
}

fn init_subscriber(
    config: &ObservabilityConfig,
    filter: EnvFilter,
    tracer_provider: Option<&SdkTracerProvider>,
) {
    if config.deployment_environment == "local" {
        let telemetry = tracer_provider.map(|provider| {
            tracing_opentelemetry::layer().with_tracer(provider.tracer("rust-tmpl"))
        });
        tracing_subscriber::registry()
            .with(telemetry)
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_ansi(true)
                    .with_target(false)
                    .with_filter(filter_fn(is_event)),
            )
            .with(filter)
            .init();
    } else {
        let telemetry = tracer_provider.map(|provider| {
            tracing_opentelemetry::layer().with_tracer(provider.tracer("rust-tmpl"))
        });
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(false),
            )
            .with(telemetry)
            .init();
    }
}

fn is_event(metadata: &tracing::Metadata<'_>) -> bool {
    metadata.is_event()
}

fn signal_endpoint(base_endpoint: &str, signal: &str) -> String {
    format!("{}/v1/{signal}", base_endpoint.trim_end_matches('/'))
}
