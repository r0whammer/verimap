//! Writes the protocol's JSON Schemas: `verimap-schemas <out-dir>`.
//!
//! `just gen` runs this and then generates TypeScript from the output;
//! `just check-generated` runs it into a temporary directory and compares.

use std::path::PathBuf;
use std::process::ExitCode;

use verimap_protocol::PROTOCOL_VERSION;
use verimap_protocol::bridge::WorkerBridgeSchema;

fn main() -> ExitCode {
    run(std::env::args_os().skip(1))
}

fn run(mut args: impl Iterator<Item = std::ffi::OsString>) -> ExitCode {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_the_schema_and_rejects_bad_usage() {
        assert_eq!(run(std::iter::empty()), ExitCode::from(2));
        assert_eq!(run(["a".into(), "b".into()].into_iter()), ExitCode::from(2));
        let dir = std::env::temp_dir().join(format!("verimap-schema-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(run(std::iter::once(dir.clone().into())), ExitCode::SUCCESS);
        let written = std::fs::read_to_string(dir.join("worker-bridge.schema.json")).unwrap();
        assert!(written.contains("x-verimap-protocol-version"));
        // A file is not a directory, so the write fails and the process
        // reports it instead of claiming the schema was saved.
        let file = dir.join("not-a-directory");
        std::fs::write(&file, b"x").unwrap();
        assert_eq!(run(std::iter::once(file.into())), ExitCode::FAILURE);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
