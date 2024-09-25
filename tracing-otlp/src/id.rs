/// Unique Span identifier.
///
/// Wraps a `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpanId(pub u64);

impl From<u64> for SpanId {
    fn from(value: u64) -> Self {
        SpanId(value)
    }
}

impl From<SpanId> for u64 {
    fn from(value: SpanId) -> u64 {
        value.0
    }
}

/// Uniquely identifies a single distributed trace.
///
/// Wraps a u128, and can be generated new from a UUID V4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TraceId(pub u128);

impl Default for TraceId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().as_u128())
    }
}

impl TraceId {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<u128> for TraceId {
    fn from(value: u128) -> Self {
        TraceId(value)
    }
}

impl From<TraceId> for u128 {
    fn from(value: TraceId) -> u128 {
        value.0
    }
}
