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
