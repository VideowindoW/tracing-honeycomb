use tracing::field::{Field, Visit};

use crate::prost::common::v1::{any_value::Value, AnyValue, KeyValue};

#[derive(Default, Clone, Debug)]
pub struct Visitor(pub Vec<KeyValue>);

impl Visit for Visitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push(KeyValue::new(
            field.to_string(),
            format!("{:?}", value).into(),
        ))
    }
    // TODO: This may allow hashmaps to be used as attributes
    // fn record_value(&mut self, field: &Field, value: Value<'_>) {
    //     todo!()
    // }
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.0.push(KeyValue::new(field.to_string(), value.into()))
    }
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.0.push(KeyValue::new(field.to_string(), value.into()))
    }
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.0.push(KeyValue::new(field.to_string(), value.into()))
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        self.0
            .push(KeyValue::new(field.to_string(), value.to_string().into()))
    }
}

impl KeyValue {
    pub fn new(key: String, value: Value) -> Self {
        Self {
            key,
            value: Some(AnyValue { value: Some(value) }),
        }
    }
}
