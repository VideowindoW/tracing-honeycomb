use std::io::Result;
fn main() -> Result<()> {
    println!("cargo::rerun-if-changed=opentelemetry-proto/");

    prost_build::compile_protos(
        &["opentelemetry-proto/opentelemetry/proto/collector/trace/v1/trace_service.proto"],
        &["opentelemetry-proto"],
    )?;
    Ok(())
}
