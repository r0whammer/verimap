//! Runs the schema binary so its `main` is covered. `run` is also unit-tested.

use std::process::Command;

#[test]
fn prints_usage_without_an_output_directory() {
    let output = Command::new(env!("CARGO_BIN_EXE_verimap-schemas"))
        .output()
        .expect("schema binary runs");
    assert!(!output.status.success());
    let err = String::from_utf8(output.stderr).expect("usage is utf-8");
    assert!(err.contains("usage: verimap-schemas"), "{err}");
}

#[test]
fn writes_a_schema_directory() {
    let dir = std::env::temp_dir().join(format!("verimap-schema-bin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let status = Command::new(env!("CARGO_BIN_EXE_verimap-schemas"))
        .arg(&dir)
        .status()
        .expect("schema binary runs");
    assert!(status.success());
    let written = std::fs::read_to_string(dir.join("worker-bridge.schema.json")).unwrap();
    assert!(written.contains("x-verimap-protocol-version"));
    std::fs::remove_dir_all(&dir).unwrap();
}
