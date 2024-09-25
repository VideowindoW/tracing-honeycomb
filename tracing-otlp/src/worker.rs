use std::{
    sync::mpsc::{Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

use prost::Message;
use ureq::Agent;
use url::Url;

use crate::prost::{
    collector::trace::v1::{ExportTraceServiceRequest, ExportTraceServiceResponse},
    common::v1::{any_value::Value, AnyValue, KeyValue},
    resource::v1::Resource,
    trace::v1::{ResourceSpans, ScopeSpans, Span},
};

pub struct Worker {
    send_interval: Duration,
    endpoint_trace: Url,
    rx: Receiver<Span>,
    resource: Resource,
    agent: Agent,
    last_send: Instant,
    http_headers: Vec<(String, String)>,
}

impl Worker {
    pub fn new(
        send_interval: Duration,
        endpoint_trace: Url,
        rx: Receiver<Span>,
        resource_attributes: Vec<(String, Value)>,
        http_headers: Vec<(String, String)>,
    ) -> Self {
        Self {
            send_interval,
            endpoint_trace,
            rx,
            resource: Resource {
                attributes: resource_attributes
                    .into_iter()
                    .map(|(key, v)| KeyValue {
                        key,
                        value: Some(AnyValue { value: v.into() }),
                    })
                    .collect(),
                dropped_attributes_count: 0,
            },
            agent: Agent::new(),
            last_send: Instant::now(),
            http_headers,
        }
    }

    pub fn run_loop(&mut self) {
        let mut spans = Vec::new();
        loop {
            // Receive spans at most until the interval is up
            match self.rx.recv_timeout(self.duration_to_next_send()) {
                Ok(span) => spans.push(span),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }

            // Send spans on the given interval
            if self.last_send.elapsed() >= self.send_interval {
                self.last_send = Instant::now();

                // Only send spans if we have any to send
                if spans.is_empty() {
                    continue;
                }

                let mut protobuf_req = ExportTraceServiceRequest {
                    resource_spans: vec![ResourceSpans {
                        resource: Some(self.resource.clone()),
                        scope_spans: vec![ScopeSpans {
                            scope: None,
                            spans: std::mem::take(&mut spans),
                            schema_url: "".to_string(),
                        }],
                        schema_url: "".to_string(),
                    }],
                };

                let encoded = protobuf_req.encode_to_vec();

                let mut req = self
                    .agent
                    .request_url("POST", &self.endpoint_trace)
                    .set("Content-Type", "application/x-protobuf");

                // Set the HTTP headers passed by the user
                req = self.http_headers.iter().fold(req, |r, (k, v)| r.set(k, v));
                // Send the traces to the server
                match req.send_bytes(&encoded) {
                    Ok(res) => {
                        if let Some("application/x-protobuf") = res.header("content-type") {
                            let mut buf: Vec<u8> = Vec::new();
                            if let Err(err) = res.into_reader().read_to_end(&mut buf) {
                                eprintln!("Protobuf response interrupted: {err}")
                            }
                            match ExportTraceServiceResponse::decode(&*buf) {
                                Ok(res) => {
                                    if let Some(err) = res.partial_success {
                                        if !err.error_message.is_empty() || err.rejected_spans != 0
                                        {
                                            eprintln!("Server returned protobuf error: {:?}", err)
                                        }
                                    }
                                }
                                Err(err) => {
                                    eprintln!("Could not decode protobuf response: {err:?}")
                                }
                            }
                        }
                    }
                    Err(err) => {
                        // Sending failed, so put spans back into vec
                        spans = std::mem::take(
                            &mut protobuf_req.resource_spans[0].scope_spans[0].spans,
                        );
                        eprintln!("Error sending spans to {}: {:?}", &self.endpoint_trace, err)
                    }
                }
            }
        }
    }

    fn instant_next_send(&self) -> Instant {
        self.last_send + self.send_interval
    }

    fn duration_to_next_send(&self) -> Duration {
        self.instant_next_send() - Instant::now()
    }
}
