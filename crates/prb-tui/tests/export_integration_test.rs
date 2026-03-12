//! Integration tests for export functionality.

use bytes::Bytes;
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufWriter, Write};
use tempfile::TempDir;

fn create_test_events() -> Vec<DebugEvent> {
    vec![
        DebugEvent {
            id: EventId::from_raw(1),
            timestamp: Timestamp::from_nanos(1_000_000_000),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            },
            transport: TransportKind::Grpc,
            direction: Direction::Inbound,
            payload: Payload::Raw {
                raw: Bytes::from_static(b"test payload 1"),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        },
        DebugEvent {
            id: EventId::from_raw(2),
            timestamp: Timestamp::from_nanos(2_000_000_000),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            },
            transport: TransportKind::Zmq,
            direction: Direction::Outbound,
            payload: Payload::Raw {
                raw: Bytes::from_static(b"test payload 2"),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        },
    ]
}

#[test]
fn test_export_json() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("export.json");

    let events = create_test_events();

    // Export to JSON
    let file = fs::File::create(&export_path).unwrap();
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &events).unwrap();
    writer.flush().unwrap();

    // Verify file exists and contains valid JSON
    assert!(export_path.exists());

    let contents = fs::read_to_string(&export_path).unwrap();
    let parsed: Vec<DebugEvent> = serde_json::from_str(&contents).unwrap();

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].id, EventId::from_raw(1));
    assert_eq!(parsed[1].id, EventId::from_raw(2));
}

#[test]
fn test_export_csv() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("export.csv");

    let events = create_test_events();

    // Export to CSV using the prb-export crate
    let exporter = prb_export::create_exporter("csv").unwrap();
    let file = fs::File::create(&export_path).unwrap();
    let mut writer = BufWriter::new(file);
    exporter.export(&events, &mut writer).unwrap();
    writer.flush().unwrap();

    // Verify file exists
    assert!(export_path.exists());

    // Verify CSV has headers and data
    let contents = fs::read_to_string(&export_path).unwrap();
    let lines: Vec<&str> = contents.lines().collect();

    // Should have header + 2 data rows
    assert!(lines.len() >= 3, "CSV should have header and data rows");

    // Header should contain expected columns
    assert!(lines[0].contains("timestamp"));
    assert!(lines[0].contains("transport"));
}

#[test]
fn test_export_har() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("export.har");

    let events = create_test_events();

    // Export to HAR using the prb-export crate
    let exporter = prb_export::create_exporter("har").unwrap();
    let file = fs::File::create(&export_path).unwrap();
    let mut writer = BufWriter::new(file);
    exporter.export(&events, &mut writer).unwrap();
    writer.flush().unwrap();

    // Verify file exists
    assert!(export_path.exists());

    // Verify HAR is valid JSON
    let contents = fs::read_to_string(&export_path).unwrap();
    let har: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // HAR should have log structure
    assert!(har.get("log").is_some());
}

#[test]
fn test_export_otlp() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("export.json");

    let events = create_test_events();

    // Export to OTLP using the prb-export crate
    let exporter = prb_export::create_exporter("otlp").unwrap();
    let file = fs::File::create(&export_path).unwrap();
    let mut writer = BufWriter::new(file);
    exporter.export(&events, &mut writer).unwrap();
    writer.flush().unwrap();

    // Verify file exists
    assert!(export_path.exists());

    // Verify OTLP is valid JSON
    let contents = fs::read_to_string(&export_path).unwrap();
    let otlp: serde_json::Value = serde_json::from_str(&contents).unwrap();

    // OTLP should have resourceSpans structure
    assert!(otlp.get("resourceSpans").is_some());
}

#[test]
fn test_export_html() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("export.html");

    let events = create_test_events();

    // Export to HTML using the prb-export crate
    let exporter = prb_export::create_exporter("html").unwrap();
    let file = fs::File::create(&export_path).unwrap();
    let mut writer = BufWriter::new(file);
    exporter.export(&events, &mut writer).unwrap();
    writer.flush().unwrap();

    // Verify file exists
    assert!(export_path.exists());

    // Verify HTML contains expected content
    let contents = fs::read_to_string(&export_path).unwrap();

    assert!(contents.contains("<!DOCTYPE html>") || contents.contains("<html"));
    assert!(contents.contains("<body>") || contents.contains("</body>"));
}

#[test]
fn test_export_empty_events() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("empty.csv");

    let events: Vec<DebugEvent> = vec![];

    // Export empty events to CSV
    let exporter = prb_export::create_exporter("csv").unwrap();
    let file = fs::File::create(&export_path).unwrap();
    let mut writer = BufWriter::new(file);
    exporter.export(&events, &mut writer).unwrap();
    writer.flush().unwrap();

    // Verify file exists and has at least header
    assert!(export_path.exists());

    let contents = fs::read_to_string(&export_path).unwrap();
    assert!(!contents.is_empty(), "Empty export should still have headers");
}

#[test]
fn test_export_unsupported_format() {
    let result = prb_export::create_exporter("unsupported_format");
    assert!(result.is_err(), "Should fail for unsupported format");
}

#[test]
fn test_all_supported_formats() {
    let formats = prb_export::supported_formats();

    // Verify expected formats are present
    assert!(formats.contains(&"csv"));
    assert!(formats.contains(&"har"));
    assert!(formats.contains(&"otlp"));
    assert!(formats.contains(&"html"));

    // Parquet is feature-gated
    #[cfg(feature = "parquet")]
    assert!(formats.contains(&"parquet"));
}

#[test]
#[cfg(feature = "parquet")]
fn test_export_parquet() {
    let temp_dir = TempDir::new().unwrap();
    let export_path = temp_dir.path().join("export.parquet");

    let events = create_test_events();

    // Export to Parquet using the prb-export crate
    let exporter = prb_export::create_exporter("parquet").unwrap();
    let file = fs::File::create(&export_path).unwrap();
    let mut writer = BufWriter::new(file);
    exporter.export(&events, &mut writer).unwrap();
    writer.flush().unwrap();

    // Verify file exists
    assert!(export_path.exists());

    // Verify file has content (Parquet files have a binary header)
    let metadata = fs::metadata(&export_path).unwrap();
    assert!(metadata.len() > 0, "Parquet file should have content");
}
