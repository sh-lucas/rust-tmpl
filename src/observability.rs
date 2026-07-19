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

pub fn init(config: &ObservabilityConfig) -> ObservabilityGuard {
    global::set_text_map_propagator(TraceContextPropagator::new());
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn"));

    let Some(endpoint) = config.otlp_endpoint.as_deref() else {
        init_subscriber(config, filter, None);
        return ObservabilityGuard {
            tracer_provider: None,
            meter_provider: None,
        };
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

    let tracer_provider = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(trace_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()
        .ok()
        .map(|exporter| {
            SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(resource.clone())
                .build()
        });
    let meter_provider = opentelemetry_otlp::MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(metric_endpoint)
        .with_timeout(Duration::from_secs(3))
        .build()
        .ok()
        .map(|exporter| {
            SdkMeterProvider::builder()
                .with_periodic_exporter(exporter)
                .with_resource(resource)
                .build()
        });

    if let Some(provider) = &meter_provider {
        global::set_meter_provider(provider.clone());
    }

    if let Some(provider) = &tracer_provider {
        global::set_tracer_provider(provider.clone());
    }
    init_subscriber(config, filter, tracer_provider.as_ref());

    if tracer_provider.is_none() {
        tracing::warn!("OTLP configuration is invalid; remote telemetry disabled");
    }

    ObservabilityGuard {
        tracer_provider,
        meter_provider,
    }
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
