use crate::telemetry::Telemetry;
use crate::trace;
use std::marker::PhantomData;
use std::time::SystemTime;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Subscriber};
use tracing_subscriber::{layer::Context, registry, Layer};

/// A `tracing_subscriber::Layer` that publishes events and spans to some backend
/// using the provided `Telemetry` capability.
pub struct TelemetryLayer<Telemetry, SpanId, TraceId> {
    service_name: &'static str,
    pub(crate) telemetry: Telemetry,
    promote_span_id: Box<dyn 'static + Send + Sync + Fn(Id) -> SpanId>,
    _ttype: PhantomData<TraceId>,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub(crate) struct TraceCtx<SpanId, TraceId> {
    pub(crate) parent_span: Option<SpanId>,
    pub(crate) trace_id: TraceId,
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub(crate) struct PromotedSpanId<SpanId>(pub(crate) SpanId);

impl<T, SpanId, TraceId> TelemetryLayer<T, SpanId, TraceId>
where
    SpanId: 'static + Clone + Send + Sync,
    TraceId: 'static + Clone + Send + Sync,
{
    /// Construct a new TelemetryLayer using the provided `Telemetry` capability.
    /// Uses the provided function, `F`, to promote `tracing::span::Id` instances to the
    /// `SpanId` type associated with the provided `Telemetry` instance.
    pub fn new<F: 'static + Send + Sync + Fn(Id) -> SpanId>(
        service_name: &'static str,
        telemetry: T,
        promote_span_id: F,
    ) -> Self {
        TelemetryLayer {
            service_name,
            telemetry,
            promote_span_id: Box::new(promote_span_id),
            _ttype: Default::default(),
        }
    }
}

impl<S, TraceId, SpanId, V, T> Layer<S> for TelemetryLayer<T, SpanId, TraceId>
where
    S: Subscriber + for<'a> registry::LookupSpan<'a>,
    TraceId: 'static + Clone + Eq + Send + Sync,
    SpanId: 'static + Clone + Eq + Send + Sync,
    V: 'static + tracing::field::Visit + Send + Sync,
    T: 'static + Telemetry<Visitor = V, TraceId = TraceId, SpanId = SpanId>,
{
    fn on_new_span(&self, attrs: &Attributes, id: &Id, ctx: Context<S>) {
        let span = ctx.span(id).expect("span data not found during new_span");

        let tid = span.parent().and_then(|p| {
            p.extensions()
                .get::<TraceCtx<SpanId, TraceId>>()
                .map(|t| t.trace_id.clone())
        });

        let mut extensions_mut = span.extensions_mut();
        extensions_mut.insert(SpanInitAt::new());
        extensions_mut.insert(PromotedSpanId((self.promote_span_id)(id.clone())));
        let mut visitor: V = self.telemetry.mk_visitor();
        attrs.record(&mut visitor);
        extensions_mut.insert::<V>(visitor);
        extensions_mut.insert::<Vec<trace::Event<V, SpanId, TraceId>>>(Default::default());

        // If parent is part of a trace, then make this span part of the trace too.
        if let Some(tid) = tid {
            extensions_mut.insert(TraceCtx {
                trace_id: tid,
                parent_span: Some((self.promote_span_id)(
                    span.parent()
                        .expect("Span parent should not disappear")
                        .id(),
                )),
            })
        }
    }

    fn on_record(&self, id: &Id, values: &Record, ctx: Context<S>) {
        let span = ctx.span(id).expect("span data not found during on_record");
        let mut extensions_mut = span.extensions_mut();
        let visitor: &mut V = extensions_mut
            .get_mut()
            .expect("fields extension not found during on_record");
        values.record(visitor);
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let parent_id = if let Some(parent_id) = event.parent() {
            // explicit parent
            Some(parent_id.clone())
        } else if event.is_root() {
            // don't bother checking thread local if span is explicitly root according to this fn
            None
        } else {
            // implicit parent from threadlocal ctx, or root span if none
            ctx.current_span().id().cloned()
        };

        let initialized_at = SystemTime::now();

        let mut visitor = self.telemetry.mk_visitor();
        event.record(&mut visitor);

        match parent_id {
            None => {
                let event = trace::Event {
                    trace_id: None,
                    parent_id: None,
                    initialized_at,
                    meta: event.metadata(),
                    service_name: self.service_name,
                    values: visitor,
                };

                self.telemetry.report_event(event);
            }
            Some(parent_id) => {
                // only report event if its parent span is part of a trace
                if let Some(parent_trace_ctx) = ctx
                    .span(&parent_id)
                    .and_then(|s| s.extensions().get::<TraceCtx<SpanId, TraceId>>().cloned())
                {
                    let span = ctx
                        .span(&parent_id)
                        .expect("Parent span id should be in the context");
                    let event = trace::Event {
                        trace_id: Some(parent_trace_ctx.trace_id),
                        parent_id: Some((self.promote_span_id)(parent_id)),
                        initialized_at,
                        meta: event.metadata(),
                        service_name: self.service_name,
                        values: visitor,
                    };
                    let mut extensions = span.extensions_mut();
                    extensions
                        .get_mut::<Vec<trace::Event<V, SpanId, TraceId>>>()
                        .expect("List of events should have been added to span")
                        .push(event);
                }
            }
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("span data not found during on_close");

        let mut extensions_mut = span.extensions_mut();

        // if span's enclosing ctx has a trace id, eval & use to report telemetry
        if let Some(trace_ctx) = extensions_mut.remove::<TraceCtx<SpanId, TraceId>>() {
            let TraceCtx {
                parent_span,
                trace_id,
            } = trace_ctx;

            let visitor: V = extensions_mut
                .remove()
                .expect("should be present on all spans");
            let SpanInitAt(initialized_at) = extensions_mut
                .remove()
                .expect("should be present on all spans");

            let events = extensions_mut
                .remove::<Vec<trace::Event<V, SpanId, TraceId>>>()
                .expect("List of events should have been added to span");

            let completed_at = SystemTime::now();

            let parent_id = match parent_span {
                None => span
                    .parent()
                    .map(|parent_ref| (self.promote_span_id)(parent_ref.id())),
                Some(parent_span) => Some(parent_span),
            };

            let span = trace::Span {
                id: (self.promote_span_id)(span.id()),
                name: span.name().to_string(),
                meta: span.metadata(),
                parent_id,
                initialized_at,
                trace_id,
                completed_at,
                service_name: self.service_name,
                values: visitor,
            };

            self.telemetry.report_span(span, events);
        };
    }
}

struct SpanInitAt(SystemTime);

impl SpanInitAt {
    fn new() -> Self {
        let initialized_at = SystemTime::now();

        Self(initialized_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::test::{SpanId, TestTelemetry, TraceId};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::runtime::Runtime;
    use tracing::instrument;
    use tracing_subscriber::layer::Layer;

    fn explicit_trace_id() -> TraceId {
        135
    }

    fn explicit_parent_span_id() -> SpanId {
        Id::from_u64(246)
    }

    #[test]
    fn test_instrument() {
        with_test_scenario_runner(|| {
            #[instrument]
            fn f(ns: Vec<u64>) {
                trace::register_dist_tracing_root(
                    explicit_trace_id(),
                    Some(explicit_parent_span_id()),
                )
                .unwrap();
                for n in ns {
                    g(format!("{}", n));
                }
            }

            #[instrument]
            fn g(_s: String) {
                let use_of_reserved_word = "duration-value";
                tracing::event!(
                    tracing::Level::INFO,
                    duration_ms = use_of_reserved_word,
                    foo = "bar"
                );

                assert_eq!(
                    trace::current_dist_trace_ctx::<SpanId, TraceId>()
                        .map(|x| x.0)
                        .unwrap(),
                    explicit_trace_id(),
                );
            }

            f(vec![1, 2, 3]);
        });
    }

    // run async fn (with multiple entry and exit for each span due to delay) with test scenario
    #[test]
    fn test_async_instrument() {
        with_test_scenario_runner(|| {
            #[instrument]
            async fn f(ns: Vec<u64>) {
                trace::register_dist_tracing_root(
                    explicit_trace_id(),
                    Some(explicit_parent_span_id()),
                )
                .unwrap();
                for n in ns {
                    g(format!("{}", n)).await;
                }
            }

            #[instrument]
            async fn g(s: String) {
                // delay to force multiple span entry
                tokio::time::delay_for(Duration::from_millis(100)).await;
                let use_of_reserved_word = "duration-value";
                tracing::event!(
                    tracing::Level::INFO,
                    duration_ms = use_of_reserved_word,
                    foo = "bar"
                );

                assert_eq!(
                    trace::current_dist_trace_ctx::<SpanId, TraceId>()
                        .map(|x| x.0)
                        .unwrap(),
                    explicit_trace_id(),
                );
            }

            let mut rt = Runtime::new().unwrap();
            rt.block_on(f(vec![1, 2, 3]));
        });
    }

    fn with_test_scenario_runner<F>(f: F)
    where
        F: Fn(),
    {
        let spans = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::new()));
        let cap: TestTelemetry = TestTelemetry::new(spans.clone(), events.clone());
        let layer = TelemetryLayer::new("test_svc_name", cap, |x| x);

        let subscriber = layer.with_subscriber(registry::Registry::default());
        tracing::subscriber::with_default(subscriber, f);

        let spans = spans.lock().unwrap();
        let events = events.lock().unwrap();

        // root span is exited (and reported) last
        let root_span = &spans[3];
        let child_spans = &spans[0..3];

        let expected_trace_id = explicit_trace_id();

        assert_eq!(root_span.parent_id, Some(explicit_parent_span_id()));
        assert_eq!(root_span.trace_id, expected_trace_id);

        for (span, event) in child_spans.iter().zip(events.iter()) {
            // confirm parent and trace ids are as expected
            assert_eq!(span.parent_id, Some(root_span.id.clone()));
            assert_eq!(event.parent_id, Some(span.id.clone()));
            assert_eq!(span.trace_id, explicit_trace_id());
            assert_eq!(event.trace_id, Some(explicit_trace_id()));
        }
    }
}
