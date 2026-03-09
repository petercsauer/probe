//! CLI integration test for wire-format protobuf decoding.

use std::io::Write;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_cli_inspect_wire_format() {
    // Create temp directory
    let temp_dir = TempDir::new().unwrap();

    // Create a simple protobuf message: field 1 = varint 42, field 2 = string "hello"
    let mut payload = Vec::new();
    payload.push(0x08); // field 1, wire type 0 (varint)
    payload.push(42); // value 42
    payload.push(0x12); // field 2, wire type 2 (length-delimited)
    payload.push(5); // length 5
    payload.extend_from_slice(b"hello");

    // Create NDJSON event
    let payload_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &payload);
    let event_json = format!(
        r#"{{"id":1,"timestamp":1000000000,"source":{{"adapter":"test","origin":"test.json"}},"transport":"grpc","direction":"inbound","payload":{{"type":"raw","raw":"{}"}}}}"#,
        payload_b64
    );

    let event_path = temp_dir.path().join("events.ndjson");
    let mut event_file = std::fs::File::create(&event_path).unwrap();
    writeln!(event_file, "{}", event_json).unwrap();
    drop(event_file);

    // Run prb inspect events.ndjson --wire-format
    let output = Command::new(env!("CARGO_BIN_EXE_prb"))
        .arg("inspect")
        .arg(event_path.to_str().unwrap())
        .arg("--wire-format")
        .output()
        .expect("failed to execute prb");

    assert!(output.status.success(),
        "prb inspect --wire-format should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify wire-format output
    assert!(stdout.contains("WIRE FORMAT DECODE"),
        "Output should indicate wire-format decode: {}", stdout);
    assert!(stdout.contains("field 1:"),
        "Output should contain field 1: {}", stdout);
    assert!(stdout.contains("42"),
        "Output should contain value 42: {}", stdout);
    assert!(stdout.contains("field 2:"),
        "Output should contain field 2: {}", stdout);
    assert!(stdout.contains("hello"),
        "Output should contain string 'hello': {}", stdout);
}
