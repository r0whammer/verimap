//! Writes the protocol's JSON Schemas: `verimap-schemas <out-dir>`.
//!
//! `just gen` runs this and then generates TypeScript from the output;
//! `just check-generated` runs it into a temporary directory and compares.

use std::path::PathBuf;
use std::process::ExitCode;

use verimap_protocol::PROTOCOL_VERSION;
use verimap_protocol::bridge::WorkerBridgeSchema;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(out_dir), None) = (args.next(), args.next()) else {
        eprintln!("usage: verimap-schemas <out-dir>");
        return ExitCode::from(2);
    };
    match write_schemas(PathBuf::from(out_dir)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("verimap-schemas: {e}");
            ExitCode::FAILURE
        }
    }
}

fn write_schemas(out_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut schema = schemars::schema_for!(WorkerBridgeSchema);
    // The protocol version travels with the schema so generated TypeScript
    // carries the same constant as this build of the core.
    schema.insert(
        "x-verimap-protocol-version".into(),
        serde_json::to_value(PROTOCOL_VERSION)?,
    );
    std::fs::create_dir_all(&out_dir)?;
    let mut json = serde_json::to_string_pretty(&schema)?;
    json.push('\n');
    std::fs::write(out_dir.join("worker-bridge.schema.json"), json)?;
    Ok(())
}
