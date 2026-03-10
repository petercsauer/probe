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
    #[allow(deprecated)]
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
fn test_cli_jobs_flag_parsing() {
    // Test that --jobs flag is accepted with various values
    let fixture = fixtures_dir().join("sample.json");

    // --jobs 1 (sequential)
    prb()
        .arg("ingest")
        .arg(&fixture)
        .arg("--jobs")
        .arg("1")
        .assert()
        .success();

    // --jobs 4 (parallel)
    prb()
        .arg("ingest")
        .arg(&fixture)
        .arg("--jobs")
        .arg("4")
        .assert()
        .success();

    // -j 2 (short form)
    prb()
        .arg("ingest")
        .arg(&fixture)
        .arg("-j")
        .arg("2")
        .assert()
        .success();
}

#[test]
fn test_cli_jobs_default_zero() {
    // Default should be 0 (auto-detect)
    // This is implicit - if no --jobs is specified, it should still work
    let fixture = fixtures_dir().join("sample.json");

    prb()
        .arg("ingest")
        .arg(&fixture)
        .assert()
        .success();
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

// Helper to create a simple PCAP file
fn create_test_pcap(path: &std::path::Path, include_tls: bool) {
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header, TcpHeader};
    use std::io::Write;

    let mut file = fs::File::create(path).unwrap();

    // PCAP global header
    let header = [
        0xd4, 0xc3, 0xb2, 0xa1, // Magic number (little-endian)
        0x02, 0x00, // Version major
        0x04, 0x00, // Version minor
        0x00, 0x00, 0x00, 0x00, // Timezone offset
        0x00, 0x00, 0x00, 0x00, // Timestamp accuracy
        0xff, 0xff, 0x00, 0x00, // Snaplen (65535)
        0x01, 0x00, 0x00, 0x00, // Link-layer type (Ethernet)
    ];
    file.write_all(&header).unwrap();

    // Create a simple TCP packet
    let payload = if include_tls {
        // TLS Client Hello-like header (not valid, just for testing)
        b"\x16\x03\x03\x00\x05hello".to_vec()
    } else {
        b"GET / HTTP/1.1\r\n\r\n".to_vec()
    };

    let mut packet = Vec::new();

    // Ethernet header
    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800), // IPv4
    };
    eth.write(&mut packet).unwrap();

    // IPv4 header
    let payload_len = (20 + payload.len()) as u16; // TCP header (20) + payload
    let ipv4 = Ipv4Header::new(payload_len, 64, IpNumber(6), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.write(&mut packet).unwrap();

    // TCP header
    let mut tcp = TcpHeader::new(12345, if include_tls { 443 } else { 80 }, 1000, 4096);
    tcp.acknowledgment_number = 0;
    tcp.syn = false;
    tcp.ack = true;
    tcp.fin = true;
    tcp.rst = false;
    tcp.psh = true;
    tcp.write(&mut packet).unwrap();

    // Payload
    packet.extend_from_slice(&payload);

    // Packet header
    let ts_sec = 1700000000u32;
    let ts_usec = 0u32;
    file.write_all(&ts_sec.to_le_bytes()).unwrap(); // Timestamp seconds
    file.write_all(&ts_usec.to_le_bytes()).unwrap(); // Timestamp microseconds
    file.write_all(&(packet.len() as u32).to_le_bytes()).unwrap(); // Included length
    file.write_all(&(packet.len() as u32).to_le_bytes()).unwrap(); // Original length

    // Packet data
    file.write_all(&packet).unwrap();

    file.flush().unwrap();
}

#[test]
fn test_cli_ingest_pcap() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");
    create_test_pcap(&pcap_path, false);

    prb()
        .arg("ingest")
        .arg(&pcap_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport":"raw-tcp""#));
}

#[test]
fn test_cli_ingest_pcap_tls() {
    let temp_dir = tempfile::tempdir().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");
    let keylog_path = temp_dir.path().join("keys.log");

    create_test_pcap(&pcap_path, true);

    // Create an empty keylog file
    fs::File::create(&keylog_path).unwrap();

    prb()
        .arg("ingest")
        .arg(&pcap_path)
        .arg("--tls-keylog")
        .arg(&keylog_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport":"raw-tcp""#));
}

#[test]
fn test_cli_format_autodetect() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Test .json fixture
    let json_fixture = fixtures_dir().join("sample.json");
    prb()
        .arg("ingest")
        .arg(&json_fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport":"grpc""#));

    // Test .pcap
    let pcap_path = temp_dir.path().join("test.pcap");
    create_test_pcap(&pcap_path, false);
    prb()
        .arg("ingest")
        .arg(&pcap_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport":"raw-tcp""#));

    // Test unsupported extension
    let unsupported = temp_dir.path().join("test.txt");
    fs::File::create(&unsupported).unwrap();
    prb()
        .arg("ingest")
        .arg(&unsupported)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unsupported input format"));
}

// WS-5.1: CLI command coverage

#[test]
fn test_cli_ingest_magic_bytes_detection() {
    // WS-5.1: Rename .pcap to .bin, ingest still works (after WS-2.5)
    let temp_dir = tempfile::tempdir().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");
    create_test_pcap(&pcap_path, false);

    // Rename to .bin
    let bin_path = temp_dir.path().join("test.bin");
    fs::rename(&pcap_path, &bin_path).unwrap();

    // Should still detect PCAP format via magic bytes
    prb()
        .arg("ingest")
        .arg(&bin_path)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""transport":"raw-tcp""#));
}

#[test]
fn test_cli_error_messages() {
    // WS-5.1: Nonexistent file, bad format → human-readable errors

    // Nonexistent file
    prb()
        .arg("ingest")
        .arg("/nonexistent/file.json")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed").or(predicate::str::contains("No such file")));

    // Bad format
    let temp_dir = tempfile::tempdir().unwrap();
    let bad_file = temp_dir.path().join("bad.json");
    fs::write(&bad_file, "not valid json {][").unwrap();

    prb()
        .arg("ingest")
        .arg(&bad_file)
        .assert()
        .failure();
}

#[test]
fn test_cli_empty_input() {
    // WS-5.1: Empty JSON file → zero events, exit 0
    let temp_dir = tempfile::tempdir().unwrap();
    let empty_json = temp_dir.path().join("empty.json");
    fs::write(&empty_json, r#"{"version": 1, "events": []}"#).unwrap();

    let output = prb()
        .arg("ingest")
        .arg(&empty_json)
        .output()
        .unwrap();

    assert!(output.status.success(), "Should exit 0 for empty input");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output should be empty (no events)
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 0, "Should produce zero output lines");
}

#[test]
fn test_cli_large_input_streaming() {
    // WS-5.1: Large JSON fixture (>100 events) → all emitted
    use std::io::Write;

    let temp_dir = tempfile::tempdir().unwrap();
    let large_json = temp_dir.path().join("large.json");

    // Create JSON with 150 events
    let mut file = fs::File::create(&large_json).unwrap();
    write!(file, r#"{{"version": 1, "events": ["#).unwrap();

    for i in 0..150 {
        if i > 0 {
            write!(file, ",").unwrap();
        }
        write!(
            file,
            r#"{{
                "timestamp_ns": {},
                "transport": "grpc",
                "direction": "outbound",
                "payload_base64": "dGVzdA=="
            }}"#,
            1700000000000000000u64 + i
        )
        .unwrap();
    }

    writeln!(file, "]}}").unwrap();
    drop(file);

    // Ingest and count output lines
    let output = prb()
        .arg("ingest")
        .arg(&large_json)
        .output()
        .unwrap();

    assert!(output.status.success(), "Should process large input");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 150, "Should emit all 150 events");
}

#[test]
fn test_cli_ingest_stdin_ndjson() {
    // WS-5.1: Pipe NDJSON to `prb inspect` via stdin (duplicate of existing test, kept for WS-5 completeness)
    let fixture = fixtures_dir().join("sample.json");

    // First ingest to get NDJSON
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    assert!(ingest_output.status.success());

    // Then pipe to inspect via stdin (no argument needed, stdin is default)
    prb()
        .arg("inspect")
        .write_stdin(ingest_output.stdout)
        .assert()
        .success()
        .stdout(predicate::str::contains("TIMESTAMP"))
        .stdout(predicate::str::contains("grpc"));
}

#[test]
fn test_cli_ingest_pcap_to_mcap() {
    // WS-5.1: .pcap → .mcap output file creation
    let temp_dir = tempfile::tempdir().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");
    let mcap_path = temp_dir.path().join("output.mcap");

    create_test_pcap(&pcap_path, false);

    // Ingest PCAP to MCAP
    prb()
        .arg("ingest")
        .arg(&pcap_path)
        .arg("--output")
        .arg(&mcap_path)
        .assert()
        .success();

    // Verify MCAP file was created and is non-empty
    assert!(mcap_path.exists(), "MCAP file should be created");
    let metadata = fs::metadata(&mcap_path).unwrap();
    assert!(metadata.len() > 0, "MCAP file should not be empty");

    // Note: MCAP inspect might not be implemented in Phase 1
    // This test verifies MCAP output creation works
}

#[test]
fn test_cli_inspect_with_where_filter() {
    // S7.3: Test --where flag for CLI filtering
    let fixture = fixtures_dir().join("multi_transport.json");

    // First ingest to get NDJSON
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    assert!(ingest_output.status.success());

    // Inspect with --where filter for gRPC only
    let output = prb()
        .arg("inspect")
        .arg("--where-clause")
        .arg(r#"transport == "gRPC""#)
        .write_stdin(ingest_output.stdout)
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("grpc"));

    // Verify no zmq events in output (they should be filtered out)
    let lines: Vec<&str> = stdout.lines().skip(2).collect(); // Skip header
    for line in lines {
        if !line.trim().is_empty() && line.contains("│") {
            // Check that it's a data row and contains grpc
            assert!(
                line.to_lowercase().contains("grpc"),
                "All events should be gRPC after filter"
            );
        }
    }
}

#[test]
fn test_cli_inspect_with_metadata_filter() {
    // S7.3: Test --where with metadata field matching
    let fixture = fixtures_dir().join("grpc_sample.json");

    // First ingest to get NDJSON
    let ingest_output = prb()
        .arg("ingest")
        .arg(&fixture)
        .output()
        .unwrap();

    assert!(ingest_output.status.success());

    // Inspect with metadata filter
    let output = prb()
        .arg("inspect")
        .arg("--where-clause")
        .arg(r#"direction == "inbound""#)
        .write_stdin(ingest_output.stdout)
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should only show inbound events
    assert!(!stdout.is_empty());
}

#[test]
fn test_cli_tui_help() {
    // S7.3: Verify tui command exists and has help
    prb()
        .arg("tui")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Open interactive TUI"))
        .stdout(predicate::str::contains("--where-clause"));
}

#[test]
fn test_cli_capture_help() {
    // S14: Verify capture command exists and has help
    prb()
        .arg("capture")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Capture live network traffic"))
        .stdout(predicate::str::contains("--interface"))
        .stdout(predicate::str::contains("--filter"))
        .stdout(predicate::str::contains("--list-interfaces"));
}

#[test]
fn test_cli_capture_list_interfaces() {
    // S14: Test --list-interfaces succeeds and shows interface table
    prb()
        .arg("capture")
        .arg("--list-interfaces")
        .assert()
        .success()
        .stdout(predicate::str::contains("Interface"))
        .stdout(predicate::str::contains("Status"))
        .stdout(predicate::str::contains("Addresses"));
}
