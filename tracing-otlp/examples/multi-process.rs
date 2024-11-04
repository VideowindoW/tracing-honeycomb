//! Multi-process example connecting to a locally running OTLP server. To test with Jaeger, first run:
//! ```
//! docker run --rm --name jaeger -p 16686:16686 -p 4318:4318 jaegertracing/all-in-one:1.61.0
//! ```

use std::{thread, time::Duration};

use tracing::{event, span, Level};
use tracing_otlp::{current_dist_trace_ctx, register_dist_tracing_root, Builder, TraceId};
use tracing_subscriber::layer::SubscriberExt;

pub fn main() {
    procspawn::init();

    init_tracing("main".to_string());
    span!(Level::INFO, "Main function").in_scope(|| {
        register_dist_tracing_root(TraceId::new(), None).unwrap();
        span!(Level::INFO, "Main process").in_scope(|| {
            register_dist_tracing_root(TraceId::new(), None).unwrap();
            for i in 0..5 {
                let ctx = current_dist_trace_ctx().unwrap();
                procspawn::spawn((ctx.0 .0, ctx.1 .0, i), |(trace_id, span_id, i)| {
                    init_tracing("child".to_string());

                    span!(Level::INFO, "Subprocess", i = i).in_scope(|| {
                        register_dist_tracing_root(trace_id.into(), Some(span_id.into())).unwrap();
                        span!(Level::INFO, "Subprocess child", i = i).in_scope(|| {
                            event!(Level::INFO, i, "event");
                            thread::sleep(Duration::from_millis(50))
                        });
                    });
                    thread::sleep(Duration::from_secs(3))
                });
            }
            thread::sleep(Duration::from_millis(100))
        });
    });
    thread::sleep(Duration::from_secs(3))
}

pub fn init_tracing(service: String) {
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(
            Builder::new()
                .service_name(service)
                .build("http://127.0.0.1:4318")
                .unwrap(),
        ),
    )
    .unwrap();
}
