use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Histogram, UpDownCounter},
    propagation::Extractor,
    trace::TraceContextExt,
};
use poem::{
    Endpoint, IntoResponse, Middleware, PathPattern, Request, Response, Result,
    http::{HeaderValue, header},
};
use tracing::{Instrument, field};
use tracing_opentelemetry::OpenTelemetrySpanExt;

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub struct HttpObservability {
    requests: Counter<u64>,
    errors: Counter<u64>,
    duration: Histogram<f64>,
    in_flight: UpDownCounter<i64>,
}

impl HttpObservability {
    pub fn new() -> Self {
        let meter = global::meter("rust-tmpl.http");
        Self {
            requests: meter
                .u64_counter("http.server.request.count")
                .with_description("Completed HTTP requests")
                .build(),
            errors: meter
                .u64_counter("http.server.error.count")
                .with_description("HTTP requests completed with a 5xx status")
                .build(),
            duration: meter
                .f64_histogram("http.server.request.duration")
                .with_unit("s")
                .with_description("HTTP request duration")
                .build(),
            in_flight: meter
                .i64_up_down_counter("http.server.active_requests")
                .with_description("HTTP requests currently in flight")
                .build(),
        }
    }
}

impl Default for HttpObservability {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Endpoint> Middleware<E> for HttpObservability {
    type Output = HttpObservabilityEndpoint<E>;

    fn transform(&self, ep: E) -> Self::Output {
        HttpObservabilityEndpoint {
            requests: self.requests.clone(),
            errors: self.errors.clone(),
            duration: self.duration.clone(),
            in_flight: self.in_flight.clone(),
            ep,
        }
    }
}

pub struct HttpObservabilityEndpoint<E> {
    requests: Counter<u64>,
    errors: Counter<u64>,
    duration: Histogram<f64>,
    in_flight: UpDownCounter<i64>,
    ep: E,
}

impl<E: Endpoint> Endpoint for HttpObservabilityEndpoint<E> {
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output> {
        let request_id = req
            .header("x-request-id")
            .filter(|value| value.len() <= 128)
            .map_or_else(next_request_id, ToString::to_string);
        let method = req.method().to_string();
        let parent_context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderMapExtractor(req.headers()))
        });
        let span = tracing::info_span!(
            "http.request",
            request_id = %request_id,
            method = %method,
            route = field::Empty,
            status = field::Empty,
            duration_ms = field::Empty,
            trace_id = field::Empty,
        );
        span.set_parent(parent_context);
        let trace_id = span.context().span().span_context().trace_id().to_string();
        span.record("trace_id", field::display(trace_id));

        let requests = self.requests.clone();
        let errors = self.errors.clone();
        let duration = self.duration.clone();
        let in_flight = self.in_flight.clone();
        let span_for_request = span.clone();

        async move {
            let started_at = Instant::now();
            in_flight.add(1, &[]);
            let result = self.ep.call(req).await.map(IntoResponse::into_response);
            in_flight.add(-1, &[]);
            let elapsed = started_at.elapsed();
            let elapsed_ms = duration_millis(elapsed);

            match result {
                Ok(mut response) => {
                    let route = response
                        .data::<PathPattern>()
                        .map_or("unmatched", |pattern| pattern.0.as_ref());
                    let status = response.status();
                    let attributes = request_attributes(&method, route, status.as_u16());
                    requests.add(1, &attributes);
                    duration.record(elapsed.as_secs_f64(), &attributes);
                    if status.is_server_error() {
                        errors.add(1, &attributes);
                    }
                    span_for_request.record("route", route);
                    span_for_request.record("status", status.as_u16());
                    span_for_request.record("duration_ms", elapsed_ms);
                    response.headers_mut().insert(
                        header::HeaderName::from_static("x-request-id"),
                        HeaderValue::from_str(&request_id)
                            .expect("request IDs only contain safe ASCII characters"),
                    );
                    tracing::info!(status = status.as_u16(), duration_ms = elapsed_ms, "request completed");
                    Ok(response)
                }
                Err(error) => {
                    let status = error.status();
                    let attributes = request_attributes(&method, "unmatched", status.as_u16());
                    requests.add(1, &attributes);
                    duration.record(elapsed.as_secs_f64(), &attributes);
                    errors.add(1, &attributes);
                    span_for_request.record("route", "unmatched");
                    span_for_request.record("status", status.as_u16());
                    span_for_request.record("duration_ms", elapsed_ms);
                    tracing::warn!(status = status.as_u16(), duration_ms = elapsed_ms, error = %error, "request failed");
                    Err(error)
                }
            }
        }
        .instrument(span)
        .await
    }
}

fn next_request_id() -> String {
    format!("{:016x}", NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed))
}

fn request_attributes(method: &str, route: &str, status: u16) -> [KeyValue; 3] {
    [
        KeyValue::new("http.request.method", method.to_string()),
        KeyValue::new("http.route", route.to_string()),
        KeyValue::new("http.response.status_code", i64::from(status)),
    ]
}

fn duration_millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

struct HeaderMapExtractor<'a>(&'a header::HeaderMap);

impl Extractor for HeaderMapExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(header::HeaderName::as_str).collect()
    }
}
