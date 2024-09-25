pub mod collector {
    pub mod trace {
        pub mod v1 {
            include!(concat!(
                env!("OUT_DIR"),
                "/opentelemetry.proto.collector.trace.v1.rs"
            ));
        }
    }
}

pub mod common {
    pub mod v1 {
        include!(concat!(
            env!("OUT_DIR"),
            "/opentelemetry.proto.common.v1.rs"
        ));

        impl From<String> for any_value::Value {
            fn from(value: String) -> Self {
                Self::StringValue(value)
            }
        }

        impl From<f64> for any_value::Value {
            fn from(value: f64) -> Self {
                Self::DoubleValue(value)
            }
        }

        impl From<i64> for any_value::Value {
            fn from(value: i64) -> Self {
                Self::IntValue(value)
            }
        }

        impl From<bool> for any_value::Value {
            fn from(value: bool) -> Self {
                Self::BoolValue(value)
            }
        }
    }
}

pub mod resource {
    pub mod v1 {
        include!(concat!(
            env!("OUT_DIR"),
            "/opentelemetry.proto.resource.v1.rs"
        ));
    }
}

pub mod trace {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/opentelemetry.proto.trace.v1.rs"));
    }
}
