//! Integration tests for prb-cli.

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
    let mut cmd = Command::cargo_bin("prb").unwrap();
    cmd.current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap());
    cmd
}

#[test]
fn test_cli_help() {
    prb()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Universal message debugger"));
}

#[test]
fn test_cli_version() {
    prb()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("prb"));
}

#[test]
fn test_cli_ingest_fixture_to_stdout() {
    let fixture = fixtures_dir().join("sample.json");

    prb()
        .arg("ingest")
        .arg(&fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport":"grpc""#))
        .stdout(predicate::str::contains(r#""direction":"inbound""#));
}

#[test]
fn test_cli_ingest_fixture_to_file() {
    let fixture = fixtures_dir().join("sample.json");
    let temp_dir = tempfile::tempdir().unwrap();
    let output = temp_dir.path().join("output.ndjson");

    prb()
        .arg("ingest")
        .arg(&fixture)
        .arg("--output")
        .arg(&output)
        .assert()
        .success();

    // Verify output file exists and contains expected data
    let content = fs::read_to_string(&output).unwrap();
    assert!(content.contains(r#""transport":"grpc""#));
    assert!(content.contains(r#""direction":"inbound""#));
}

#[test]
fn test_cli_inspect_from_stdin() {
    let fixture = fixtures_dir().join("sample.json");

    // First ingest to get NDJSON
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    // Then inspect from stdin
    prb()
        .arg("inspect")
        .write_stdin(ingest_output.stdout)
        .assert()
        .success()
        .stdout(predicate::str::contains("TIMESTAMP"))
        .stdout(predicate::str::contains("TRANSPORT"))
        .stdout(predicate::str::contains("grpc"));
}

#[test]
fn test_cli_inspect_from_file() {
    let fixture = fixtures_dir().join("sample.json");
    let temp_dir = tempfile::tempdir().unwrap();
    let ndjson_file = temp_dir.path().join("events.ndjson");

    // First ingest to file
    prb()
        .arg("ingest")
        .arg(&fixture)
        .arg("--output")
        .arg(&ndjson_file)
        .assert()
        .success();

    // Then inspect from file
    prb()
        .arg("inspect")
        .arg(&ndjson_file)
        .assert()
        .success()
        .stdout(predicate::str::contains("TIMESTAMP"))
        .stdout(predicate::str::contains("grpc"));
}

#[test]
fn test_cli_inspect_json_format() {
    let fixture = fixtures_dir().join("sample.json");

    // First ingest to get NDJSON
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    // Then inspect with JSON format
    prb()
        .arg("inspect")
        .arg("--format")
        .arg("json")
        .write_stdin(ingest_output.stdout)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport": "grpc""#))
        .stdout(predicate::str::starts_with("["));
}

#[test]
fn test_cli_ingest_nonexistent_file() {
    prb()
        .arg("ingest")
        .arg("/nonexistent/file.json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed"));
}

#[test]
fn test_cli_ingest_malformed() {
    let fixture = fixtures_dir().join("malformed.json");

    prb()
        .arg("ingest")
        .arg(&fixture)
        .assert()
        .failure();
}

#[test]
fn test_cli_inspect_filter_transport() {
    let fixture = fixtures_dir().join("multi_transport.json");

    // First ingest
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    // Inspect with grpc filter
    let output = prb()
        .arg("inspect")
        .arg("--filter")
        .arg("grpc")
        .write_stdin(ingest_output.stdout.clone())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("grpc"));
    // Should not contain zmq if filtering for grpc
    let lines: Vec<&str> = stdout.lines().skip(2).collect(); // Skip header
    for line in lines {
        if !line.trim().is_empty() {
            assert!(line.contains("grpc"));
        }
    }
}

#[test]
fn test_cli_pipe_end_to_end() {
    let fixture = fixtures_dir().join("grpc_sample.json");

    // Ingest
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    assert!(ingest_output.status.success());

    // Inspect table format
    let inspect_output = prb()
        .arg("inspect")
        .write_stdin(ingest_output.stdout)
        .output()
        .unwrap();

    assert!(inspect_output.status.success());

    let stdout = String::from_utf8_lossy(&inspect_output.stdout);
    insta::assert_snapshot!(stdout);
}

#[test]
fn test_cli_schemas_load_proto() {
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let proto_path = temp_dir.path().join("test.proto");

    // Write a simple .proto file
    let proto_content = r#"
syntax = "proto3";
package test;

message TestMessage {
    int32 id = 1;
    string name = 2;
}
"#;
    let mut file = fs::File::create(&proto_path).unwrap();
    file.write_all(proto_content.as_bytes()).unwrap();
    drop(file);

    prb()
        .arg("schemas")
        .arg("load")
        .arg(&proto_path)
        .arg("--include-path")
        .arg(temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully loaded schema"))
        .stdout(predicate::str::contains("test.TestMessage"));
}

#[test]
fn test_cli_schemas_list() {
    use prost::Message as ProstMessage;
    use std::io::Write;

    let temp_dir = tempfile::tempdir().unwrap();
    let session_path = temp_dir.path().join("session.mcap");

    // Create a test schema using prost_types
    let file_desc = prost_types::FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![prost_types::DescriptorProto {
            name: Some("TestMessage".to_string()),
            field: vec![prost_types::FieldDescriptorProto {
                name: Some("id".to_string()),
                number: Some(1),
                label: Some(prost_types::field_descriptor_proto::Label::Optional as i32),
                r#type: Some(prost_types::field_descriptor_proto::Type::Int32 as i32),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let fds = prost_types::FileDescriptorSet {
        file: vec![file_desc],
    };

    let mut fds_bytes = Vec::new();
    fds.encode(&mut fds_bytes).unwrap();

    // Create a session with embedded schema
    use prb_schema::SchemaRegistry;
    use prb_storage::{SessionWriter, SessionMetadata};

    let mut registry = SchemaRegistry::new();
    registry.load_descriptor_set(&fds_bytes).unwrap();

    let file = fs::File::create(&session_path).unwrap();
    let mut writer = SessionWriter::new(file, SessionMetadata::new()).unwrap();
    writer.embed_schemas(&registry).unwrap();
    writer.finish().unwrap();

    // Test list command
    prb()
        .arg("schemas")
        .arg("list")
        .arg(&session_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("test.TestMessage"));
}
