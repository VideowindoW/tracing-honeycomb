//! Example connecting to a locally running OTLP server. To test with Jaeger, first run:
//! ```
//! docker run --rm --name jaeger -p 16686:16686 -p 4318:4318 jaegertracing/all-in-one:1.61.0
//! ```

use std::{thread, time::Duration};

pub use tracing;
use tracing::{span, Level};
use tracing_otlp::{current_dist_trace_ctx, register_dist_tracing_root, Builder, TraceId};
pub use tracing_subscriber;
use tracing_subscriber::layer::SubscriberExt;

pub fn main() {
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(
            Builder::new()
                .service_name("test".to_string())
                .build("http://127.0.0.1:4318")
                .unwrap(),
        ),
    )
    .unwrap();

    span!(Level::INFO, "Main thread").in_scope(|| {
        register_dist_tracing_root(TraceId::new(), None).unwrap();

        for i in 0..5 {
            let ctx = current_dist_trace_ctx().unwrap();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(2));
                span!(Level::INFO, "Child thread", i = i).in_scope(|| {
                    register_dist_tracing_root(ctx.0, Some(ctx.1)).unwrap();
                    thread::sleep(Duration::from_secs(3));
                })
            });
        }

        thread::sleep(Duration::from_secs(1));
    });

    // Sleep to give worker a chance to send all traces
    thread::sleep(Duration::from_secs(6));
}
