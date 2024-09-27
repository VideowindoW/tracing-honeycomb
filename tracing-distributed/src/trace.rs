use crate::telemetry_layer::{PromotedSpanId, TraceCtx};
use std::time::SystemTime;
use tracing_subscriber::registry::LookupSpan;

/// Register the current span as the local root of a distributed trace.
pub fn register_dist_tracing_root<SpanId, TraceId>(
    trace_id: TraceId,
    remote_parent_span: Option<SpanId>,
) -> Result<(), TraceCtxError>
where
    SpanId: 'static + Clone + Send + Sync,
    TraceId: 'static + Clone + Send + Sync,
{
    let span = tracing::Span::current();

    span.with_subscriber(|(current_span_id, dispatch)| {
        let registry = dispatch
            .downcast_ref::<tracing_subscriber::Registry>()
            .ok_or(TraceCtxError::RegistrySubscriberNotRegistered)?;

        registry
            .span(current_span_id)
            .expect("Span should be present in registry")
            .extensions_mut()
            .replace(TraceCtx {
                parent_span: remote_parent_span,
                trace_id,
            });
        Ok(())
    })
    .ok_or(TraceCtxError::NoEnabledSpan)?
}

/// Retrieve the distributed trace context associated with the current span. Returns the
/// `TraceId`, if any, that the current span is associated with along with the `SpanId`
/// belonging to the current span.
pub fn current_dist_trace_ctx<SpanId, TraceId>() -> Result<(TraceId, SpanId), TraceCtxError>
where
    SpanId: 'static + Clone + Send + Sync,
    TraceId: 'static + Clone + Send + Sync,
{
    let span = tracing::Span::current();
    span.with_subscriber(|(current_span_id, dispatch)| {
        let registry = dispatch
            .downcast_ref::<tracing_subscriber::Registry>()
            .ok_or(TraceCtxError::RegistrySubscriberNotRegistered)?;

        let trace_id = registry
            .span(current_span_id)
            .and_then(|s| {
                s.extensions()
                    .get::<TraceCtx<SpanId, TraceId>>()
                    .map(|x| x.trace_id.clone())
            })
            .ok_or(TraceCtxError::NoParentNodeHasTraceCtx)?;

        let span_id = registry
            .span(current_span_id)
            .and_then(|s| {
                s.extensions()
                    .get::<PromotedSpanId<SpanId>>()
                    .map(|x| x.0.clone())
            })
            .ok_or(TraceCtxError::NoParentNodeHasTraceCtx)?;

        Ok((trace_id, span_id))
    })
    .ok_or(TraceCtxError::NoEnabledSpan)?
}

/// Errors that can occur while registering the current span as a distributed trace root or
/// attempting to retrieve the current trace context.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
#[non_exhaustive]
pub enum TraceCtxError {
    /// Expected a `TelemetryLayer` to be registered as a subscriber associated with the current Span.
    TelemetryLayerNotRegistered,
    /// Expected a `tracing_subscriber::Registry` to be registered as a subscriber associated with the current Span.
    RegistrySubscriberNotRegistered,
    /// Expected the span returned by `tracing::Span::current()` to be enabled, with an associated subscriber.
    NoEnabledSpan,
    /// Attempted to evaluate the current distributed trace context but none was found. If this occurs, you should check to make sure that `register_dist_tracing_root` is called in some parent of the current span.
    NoParentNodeHasTraceCtx,
}

impl std::fmt::Display for TraceCtxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TraceCtxError::*;
        write!(f, "{}",
            match self {
                TelemetryLayerNotRegistered => "`TelemetryLayer` is not a registered subscriber of the current Span",
                RegistrySubscriberNotRegistered => "no `tracing_subscriber::Registry` is a registered subscriber of the current Span",
                NoEnabledSpan => "the span is not enabled with an associated subscriber",
                NoParentNodeHasTraceCtx => "unable to evaluate trace context; assert `register_dist_tracing_root` is called in some parent span",
            })
    }
}

impl std::error::Error for TraceCtxError {}

/// A `Span` holds ready-to-publish information gathered during the lifetime of a `tracing::Span`.
#[derive(Debug, Clone)]
pub struct Span<Visitor, SpanId, TraceId> {
    /// id identifying this span
    pub id: SpanId,
    /// Name of the span
    pub name: String,
    /// `TraceId` identifying the trace to which this span belongs
    pub trace_id: TraceId,
    /// optional parent span id
    pub parent_id: Option<SpanId>,
    /// UTC time at which this span was initialized
    pub initialized_at: SystemTime,
    /// `chrono::Duration` elapsed between the time this span was initialized and the time it was completed
    pub completed_at: SystemTime,
    /// `tracing::Metadata` for this span
    pub meta: &'static tracing::Metadata<'static>,
    /// name of the service on which this span occured
    pub service_name: &'static str,
    /// values accumulated by visiting fields observed by the `tracing::Span` this span was derived from
    pub values: Visitor,
}

/// An `Event` holds ready-to-publish information derived from a `tracing::Event`.
#[derive(Clone, Debug)]
pub struct Event<Visitor, SpanId, TraceId> {
    /// `TraceId` identifying the trace to which this event belongs, it it is part of a trace.
    pub trace_id: Option<TraceId>,
    /// optional parent span id
    pub parent_id: Option<SpanId>,
    /// UTC time at which this event was initialized
    pub initialized_at: SystemTime,
    /// `tracing::Metadata` for this event
    pub meta: &'static tracing::Metadata<'static>,
    /// name of the service on which this event occured
    pub service_name: &'static str,
    /// values accumulated by visiting the fields of the `tracing::Event` this event was derived from
    pub values: Visitor,
}
