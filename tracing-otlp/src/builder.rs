use std::time::Duration;

use tracing_distributed::TelemetryLayer;

use crate::{prost::common::v1::any_value::Value, Otlp, SpanId, TraceId};

/// Builder for the [`crate::Otlp`] `tracing` layer.
///
/// Use the [`Builder`] in order to set configuration for the layer and its endpoint.
pub struct Builder {
    send_interval: Duration,
    resource_attributes: Vec<(String, Value)>,
    headers: Vec<(String, String)>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            send_interval: Duration::from_secs(1),
            resource_attributes: Default::default(),
            headers: Default::default(),
        }
    }
}

impl Builder {
    pub fn new() -> Builder {
        Self::default()
    }

    /// Configures the interval at which traces are reported to the OTLP endpoint
    pub fn send_interval(mut self, interval: Duration) -> Self {
        self.send_interval = interval;
        self
    }

    /// Sets the name of this service.
    ///
    /// See: [https://opentelemetry.io/docs/languages/sdk-configuration/general/#otel_service_name]
    pub fn service_name(mut self, service_name: String) -> Self {
        self.resource_attributes
            .push(("service.name".to_string(), service_name.into()));
        self
    }

    /// Adds an attribute for this OpenTelemetry resource.
    ///
    /// This may be an attribute such as rust version, program version, MAC address, etc.
    pub fn resource_attribute(mut self, key: String, value: impl Into<Value>) -> Self {
        self.resource_attributes.push((key, value.into()));
        self
    }

    /// Sets the HTTP headers to be added to OTLP requests.
    ///
    /// The headers are given in the form of a tuple, with the first value
    /// the key and the second the value.
    pub fn http_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// Builds a [`TelemetryLayer`] based on [`Otlp`] the settings provided.
    ///
    /// The `endpoint` given should be an HTTP URL.
    ///
    /// # Examples
    /// ```
    /// Builder::new().build("http://127.0.0.1:4318");
    /// ```
    pub fn build(
        self,
        endpoint: &str,
    ) -> Result<TelemetryLayer<Otlp, SpanId, TraceId>, url::ParseError> {
        Ok(TelemetryLayer::new(
            "",
            Otlp::new(
                endpoint,
                self.send_interval,
                self.resource_attributes,
                self.headers,
            )?,
            move |tracing_id| SpanId(tracing_id.into_u64()),
        ))
    }
}
