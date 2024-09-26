//! This crate provides a `tracing` implementation for the OpenTelemetry protocol (OTLP),
//! specifically on top of http/protobuf. It is based on `distributed-tracing` in order
//! to allow for multi-process tracing.

use std::{
    str::FromStr,
    sync::mpsc::{channel, Sender},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::prost::{common::v1::any_value::Value, trace::v1::span};
use tracing_distributed::{Telemetry, TraceCtxError};
use url::Url;
use worker::Worker;

use crate::prost::trace::v1::Span;

pub use builder::Builder;
pub use id::SpanId;
pub use id::TraceId;
pub use visitor::Visitor;

mod builder;
mod id;

pub mod prost;
mod visitor;
mod worker;

/// Register the current span as the local root of a distributed trace.
///
/// Specialized to the OTLP SpanId and TraceId provided by this crate.
pub fn register_dist_tracing_root(
    trace_id: TraceId,
    remote_parent_span: Option<SpanId>,
) -> Result<(), TraceCtxError> {
    tracing_distributed::register_dist_tracing_root(trace_id, remote_parent_span)
}

/// Retrieve the distributed trace context associated with the current span.
///
/// Returns the `TraceId`, if any, that the current span is associated with along with
/// the `SpanId` belonging to the current span.
///
/// Specialized to the OTLP SpanId and TraceId provided by this crate.
pub fn current_dist_trace_ctx() -> Result<(TraceId, SpanId), TraceCtxError> {
    tracing_distributed::current_dist_trace_ctx()
}

/// OpenTelemetry protocol implementation of [`Telemetry`]. Use [`Builder`] to instantiate this.
pub struct Otlp {
    tx: Sender<Span>,
}

impl Otlp {
    pub(crate) fn new(
        endpoint: &str,
        send_interval: Duration,
        resource_attributes: Vec<(String, Value)>,
        http_headers: Vec<(String, String)>,
    ) -> Result<Self, url::ParseError> {
        let (tx, rx) = channel();

        let endpoint = Url::from_str(endpoint)?;

        let mut worker = Worker::new(
            send_interval,
            endpoint.join("/v1/traces")?,
            rx,
            resource_attributes,
            http_headers,
        );

        thread::Builder::new()
            .name("OTLP worker".to_string())
            .spawn(move || {
                worker.run_loop();
            })
            .expect("Spawning worker should not fail");

        Ok(Self { tx })
    }
}

impl Telemetry for Otlp {
    type Visitor = Visitor;

    type TraceId = TraceId;

    type SpanId = SpanId;

    fn mk_visitor(&self) -> Self::Visitor {
        Default::default()
    }

    fn report_span(
        &self,
        span: tracing_distributed::Span<Self::Visitor, Self::SpanId, Self::TraceId>,
        events: Vec<tracing_distributed::Event<Self::Visitor, Self::SpanId, Self::TraceId>>,
    ) {
        let events = events
            .into_iter()
            .map(|ev| span::Event {
                time_unix_nano: system_time_to_unix_nanos(&ev.initialized_at),
                name: "event".to_string(),
                attributes: ev.values.0,
                dropped_attributes_count: 0,
            })
            .collect();

        let span = Span {
            trace_id: span.trace_id.0.to_le_bytes().to_vec(),
            span_id: span.id.0.to_le_bytes().to_vec(),
            trace_state: "".to_string(),
            parent_span_id: span
                .parent_id
                .map(|pid| pid.0.to_le_bytes().to_vec())
                .unwrap_or_default(),
            flags: 0,
            name: span.name,
            kind: 0,
            start_time_unix_nano: system_time_to_unix_nanos(&span.initialized_at),
            end_time_unix_nano: system_time_to_unix_nanos(&span.completed_at),
            attributes: span.values.0,
            dropped_attributes_count: 0,
            events,
            dropped_events_count: 0,
            links: vec![],
            dropped_links_count: 0,
            status: None,
        };

        self.tx.send(span).expect("Worker thread should not crash")
    }

    fn report_event(
        &self,
        _event: tracing_distributed::Event<Self::Visitor, Self::SpanId, Self::TraceId>,
    ) {
    }
}

fn system_time_to_unix_nanos(t: &SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| {
            eprintln!("Time went backwards while calculating OTLP timestamp");
            Duration::ZERO
        })
        .as_nanos() as u64
}
