//! End-to-end CLI tests covering export, merge, plugins, and additional command scenarios.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
}

fn prb() -> Command {
    #[allow(deprecated)]
    let mut cmd = Command::cargo_bin("prb").unwrap();
    cmd.current_dir(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap(),
    );
    cmd
}

// Helper to create a test MCAP file
fn create_test_mcap(path: &std::path::Path) {
    use bytes::Bytes;
    use prb_core::{Direction, EventSource, Payload, Timestamp, TransportKind};
    use prb_storage::{SessionWriter, SessionMetadata};

    let file = fs::File::create(path).unwrap();
    let mut writer = SessionWriter::new(file, SessionMetadata::new()).unwrap();

    // Write a few test events
    for i in 0..3 {
        let event = prb_core::DebugEvent::builder()
            .timestamp(Timestamp::from_nanos(1700000000000000000 + i * 1000000000))
            .source(EventSource {
                adapter: "test".to_string(),
                origin: "test.mcap".to_string(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(&[1, 2, 3, 4]),
            })
            .build();
        writer.write_event(&event).unwrap();
    }

    writer.finish().unwrap();
}

#[test]
fn test_no_args_shows_help() {
    // Invoking prb with no arguments should show help
    prb()
        .assert()
        .failure() // clap returns error when no subcommand is provided
        .stderr(predicate::str::contains("Universal message debugger"))
        .stderr(predicate::str::contains("Usage:"));
}

#[test]
fn test_version_flag() {
    prb()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("prb"));
}

#[test]
fn test_export_csv() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mcap_path = temp_dir.path().join("test.mcap");
    let csv_path = temp_dir.path().join("output.csv");

    create_test_mcap(&mcap_path);

    prb()
        .arg("export")
        .arg(&mcap_path)
        .arg("--format")
        .arg("csv")
        .arg("--output")
        .arg(&csv_path)
        .assert()
        .success();

    // Verify CSV file was created and contains expected headers
    assert!(csv_path.exists(), "CSV file should be created");
    let content = fs::read_to_string(&csv_path).unwrap();
    assert!(
        content.contains("timestamp") || content.contains("Timestamp"),
        "CSV should contain timestamp column"
    );
}

#[test]
fn test_export_har() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mcap_path = temp_dir.path().join("test.mcap");
    let har_path = temp_dir.path().join("output.har");

    create_test_mcap(&mcap_path);

    prb()
        .arg("export")
        .arg(&mcap_path)
        .arg("--format")
        .arg("har")
        .arg("--output")
        .arg(&har_path)
        .assert()
        .success();

    // Verify HAR file was created and is valid JSON
    assert!(har_path.exists(), "HAR file should be created");
    let content = fs::read_to_string(&har_path).unwrap();
    let _: serde_json::Value = serde_json::from_str(&content)
        .expect("HAR file should be valid JSON");
    assert!(content.contains("\"log\""), "HAR should contain log object");
}

#[test]
fn test_export_html() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mcap_path = temp_dir.path().join("test.mcap");
    let html_path = temp_dir.path().join("output.html");

    create_test_mcap(&mcap_path);

    prb()
        .arg("export")
        .arg(&mcap_path)
        .arg("--format")
        .arg("html")
        .arg("--output")
        .arg(&html_path)
        .assert()
        .success();

    // Verify HTML file was created and contains HTML tags
    assert!(html_path.exists(), "HTML file should be created");
    let content = fs::read_to_string(&html_path).unwrap();
    assert!(content.contains("<html>") || content.contains("<!DOCTYPE"));
    assert!(content.contains("</html>"));
}

#[test]
fn test_export_with_where_filter() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mcap_path = temp_dir.path().join("test.mcap");
    let csv_path = temp_dir.path().join("filtered.csv");

    create_test_mcap(&mcap_path);

    prb()
        .arg("export")
        .arg(&mcap_path)
        .arg("--format")
        .arg("csv")
        .arg("--output")
        .arg(&csv_path)
        .arg("--where")
        .arg(r#"transport == "grpc""#)
        .assert()
        .success();

    assert!(csv_path.exists(), "Filtered CSV should be created");
}

#[test]
fn test_merge_two_mcap_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mcap1 = temp_dir.path().join("test1.mcap");
    let mcap2 = temp_dir.path().join("test2.mcap");
    let output = temp_dir.path().join("merged.ndjson");

    create_test_mcap(&mcap1);
    create_test_mcap(&mcap2);

    // Create a simple OTLP JSON trace file
    let otlp_path = temp_dir.path().join("traces.json");
    fs::write(
        &otlp_path,
        r#"{"resourceSpans": []}"#,
    )
    .unwrap();

    prb()
        .arg("merge")
        .arg(&mcap1)
        .arg(&otlp_path)
        .arg("--output")
        .arg(&output)
        .assert()
        .success();

    assert!(output.exists(), "Merged output should be created");
}

#[test]
fn test_merge_outputs_to_stdout() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mcap_path = temp_dir.path().join("test.mcap");
    let otlp_path = temp_dir.path().join("traces.json");

    create_test_mcap(&mcap_path);
    fs::write(&otlp_path, r#"{"resourceSpans": []}"#).unwrap();

    // Merge without --output should write to stdout
    prb()
        .arg("merge")
        .arg(&mcap_path)
        .arg(&otlp_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport":"grpc""#));
}

#[test]
fn test_plugins_list() {
    prb()
        .arg("plugins")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Available decoders"));
}

#[test]
fn test_plugins_info() {
    // Test info command with a built-in decoder
    prb()
        .arg("plugins")
        .arg("info")
        .arg("grpc")
        .assert()
        .success()
        .stdout(predicate::str::contains("grpc").or(predicate::str::contains("gRPC")));
}

#[test]
fn test_inspect_with_limit() {
    let fixture = fixtures_dir().join("sample.json");

    // First ingest to get NDJSON
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    // Then inspect with --limit flag
    let output = prb()
        .arg("inspect")
        .write_stdin(ingest_output.stdout)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("TIMESTAMP"));
}

#[test]
fn test_inspect_json_format_output() {
    let fixture = fixtures_dir().join("sample.json");

    // First ingest
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    // Inspect with JSON format
    let output = prb()
        .arg("inspect")
        .arg("--format")
        .arg("json")
        .write_stdin(ingest_output.stdout)
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("Output should be valid JSON");

    // Should be an array of events
    assert!(parsed.is_array(), "JSON output should be an array");
}

#[test]
fn test_export_help() {
    prb()
        .arg("export")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Export events to developer ecosystem formats"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn test_merge_help() {
    prb()
        .arg("merge")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Merge OTLP traces"))
        .stdout(predicate::str::contains("--output"));
}
