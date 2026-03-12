//! Real-world capture tests for export formats (CSV, HAR, OTLP).
//!
//! Tests export functionality with real protocol data to ensure
//! spec compliance and round-trip validity.

use prb_core::{CaptureAdapter, DebugEvent};
use prb_export::{CsvExporter, Exporter, HarExporter, OtlpExporter};
use prb_pcap::PcapCaptureAdapter;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/captures")
}

fn collect_ok_events(adapter: &mut PcapCaptureAdapter) -> Vec<DebugEvent> {
    adapter
        .ingest()
        .filter_map(|r| r.ok())
        .collect()
}

#[test]
fn test_csv_export_from_http_capture() {
    // HTTP capture → pipeline → CSV export
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    assert!(
        capture_path.exists(),
        "HTTP capture required: {:?}",
        capture_path
    );

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty(), "Should produce events");

    // Export to CSV
    let exporter = CsvExporter;
    let mut output = Vec::new();
    let result = exporter.export(&events, &mut output);

    assert!(result.is_ok(), "CSV export should succeed");
    assert!(!output.is_empty(), "CSV output should not be empty");

    // Parse back to verify it's valid CSV
    let csv_string = String::from_utf8(output).expect("CSV should be UTF-8");

    // Should have header row
    assert!(
        csv_string.contains("timestamp"),
        "CSV should have timestamp column"
    );
    assert!(csv_string.contains("src_addr"), "CSV should have src_addr column");
    assert!(csv_string.contains("dst_addr"), "CSV should have dst_addr column");
    assert!(
        csv_string.contains("transport"),
        "CSV should have transport column"
    );

    // Count rows (header + data)
    let line_count = csv_string.lines().count();
    assert!(line_count > 1, "CSV should have header + at least one data row");

    // Verify CSV is parseable
    let mut reader = csv::ReaderBuilder::new()
        .from_reader(csv_string.as_bytes());

    let headers = reader.headers().expect("Should have headers");
    assert!(!headers.is_empty(), "Should have columns");

    let record_count = reader.records().count();
    assert!(
        record_count > 0,
        "Should have at least one record (got {})",
        record_count
    );
}

#[test]
fn test_csv_export_field_escaping() {
    // Use DNS capture which may have various field values
    let capture_path = fixtures_dir().join("dns/dns.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let exporter = CsvExporter;
    let mut output = Vec::new();
    let result = exporter.export(&events, &mut output);

    assert!(result.is_ok(), "CSV export should handle special characters");

    // Verify CSV is parseable (csv crate handles escaping)
    let csv_string = String::from_utf8(output).unwrap();
    let mut reader = csv::ReaderBuilder::new().from_reader(csv_string.as_bytes());

    // Should parse without errors
    let records: Vec<_> = reader.records().collect();
    assert!(records.iter().all(|r| r.is_ok()), "All records should parse");
}

#[test]
fn test_har_export_from_http_capture() {
    // HTTP capture → pipeline → HAR export
    // NOTE: HAR exporter only exports gRPC events. Raw pcap events
    // are RawTcp/RawUdp and won't appear in HAR unless decoded as gRPC.
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let exporter = HarExporter;
    let mut output = Vec::new();
    let result = exporter.export(&events, &mut output);

    assert!(result.is_ok(), "HAR export should succeed");
    assert!(!output.is_empty(), "HAR output should not be empty");

    // Parse as JSON to verify structure
    let har_string = String::from_utf8(output).expect("HAR should be UTF-8");
    let har_json: serde_json::Value =
        serde_json::from_str(&har_string).expect("HAR should be valid JSON");

    // Verify HAR structure
    assert!(har_json.is_object(), "HAR should be an object");
    assert!(
        har_json.get("log").is_some(),
        "HAR should have 'log' field"
    );

    let log = har_json.get("log").unwrap();
    assert!(log.get("version").is_some(), "HAR log should have version");
    assert!(log.get("creator").is_some(), "HAR log should have creator");
    assert!(log.get("entries").is_some(), "HAR log should have entries");

    let entries = log.get("entries").unwrap();
    assert!(entries.is_array(), "HAR entries should be an array");
    // Entries may be empty for non-gRPC captures
}

#[test]
fn test_har_export_entry_structure() {
    // Verify HAR entries have required fields (when gRPC events exist)
    // NOTE: Raw pcap events won't be exported unless decoded as gRPC
    let capture_path = fixtures_dir().join("http/http_with_jpegs.cap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let exporter = HarExporter;
    let mut output = Vec::new();
    exporter.export(&events, &mut output).unwrap();

    let har_string = String::from_utf8(output).unwrap();
    let har_json: serde_json::Value = serde_json::from_str(&har_string).unwrap();

    let entries = har_json["log"]["entries"].as_array().unwrap();

    // If no gRPC events, entries will be empty - that's expected
    if entries.is_empty() {
        return;
    }

    // Check first entry has required HAR fields
    let first_entry = &entries[0];
    assert!(
        first_entry.get("startedDateTime").is_some(),
        "Entry should have startedDateTime"
    );
    assert!(first_entry.get("time").is_some(), "Entry should have time");
    assert!(
        first_entry.get("request").is_some(),
        "Entry should have request"
    );
    assert!(
        first_entry.get("response").is_some(),
        "Entry should have response"
    );
    assert!(
        first_entry.get("timings").is_some(),
        "Entry should have timings"
    );

    // Check request structure
    let request = first_entry.get("request").unwrap();
    assert!(
        request.get("method").is_some(),
        "Request should have method"
    );
    assert!(request.get("url").is_some(), "Request should have url");
    assert!(
        request.get("headers").is_some(),
        "Request should have headers"
    );
}

#[test]
fn test_har_roundtrip_validity() {
    // Export to HAR → parse back → verify structure
    let capture_path = fixtures_dir().join("dns/dns.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let exporter = HarExporter;
    let mut output = Vec::new();
    exporter.export(&events, &mut output).unwrap();

    // Parse back
    let har_string = String::from_utf8(output).unwrap();
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&har_string);

    assert!(
        parsed.is_ok(),
        "Exported HAR should be valid JSON: {:?}",
        parsed.err()
    );

    let har = parsed.unwrap();
    // Verify required HAR 1.2 fields exist
    assert!(har["log"].is_object());
    assert!(har["log"]["version"].is_string());
    assert!(har["log"]["creator"].is_object());
    assert!(har["log"]["entries"].is_array());
}

#[test]
fn test_otlp_export_from_http_capture() {
    // HTTP capture → pipeline → OTLP span export
    let capture_path = fixtures_dir().join("http/http-chunked-gzip.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let exporter = OtlpExporter;
    let mut output = Vec::new();
    let result = exporter.export(&events, &mut output);

    assert!(result.is_ok(), "OTLP export should succeed");
    assert!(!output.is_empty(), "OTLP output should not be empty");

    // Parse as JSON to verify structure
    let otlp_string = String::from_utf8(output).expect("OTLP should be UTF-8");
    let otlp_json: serde_json::Value =
        serde_json::from_str(&otlp_string).expect("OTLP should be valid JSON");

    // Verify OTLP structure
    assert!(otlp_json.is_object(), "OTLP should be an object");
    assert!(
        otlp_json.get("resourceSpans").is_some(),
        "OTLP should have resourceSpans"
    );

    let resource_spans = otlp_json.get("resourceSpans").unwrap();
    assert!(
        resource_spans.is_array(),
        "resourceSpans should be an array"
    );
    assert!(
        !resource_spans.as_array().unwrap().is_empty(),
        "Should have at least one resource span"
    );
}

#[test]
fn test_otlp_span_structure() {
    // Verify OTLP spans have valid fields
    let capture_path = fixtures_dir().join("dns/dns.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let exporter = OtlpExporter;
    let mut output = Vec::new();
    exporter.export(&events, &mut output).unwrap();

    let otlp_string = String::from_utf8(output).unwrap();
    let otlp_json: serde_json::Value = serde_json::from_str(&otlp_string).unwrap();

    let resource_spans = otlp_json["resourceSpans"].as_array().unwrap();
    assert!(!resource_spans.is_empty());

    let scope_spans = resource_spans[0]["scopeSpans"].as_array().unwrap();
    assert!(!scope_spans.is_empty());

    let spans = scope_spans[0]["spans"].as_array().unwrap();
    assert!(!spans.is_empty());

    // Check first span has required OTLP fields
    let first_span = &spans[0];
    assert!(
        first_span.get("traceId").is_some(),
        "Span should have traceId"
    );
    assert!(first_span.get("spanId").is_some(), "Span should have spanId");
    assert!(first_span.get("name").is_some(), "Span should have name");
    assert!(
        first_span.get("startTimeUnixNano").is_some(),
        "Span should have startTimeUnixNano"
    );
    assert!(
        first_span.get("endTimeUnixNano").is_some(),
        "Span should have endTimeUnixNano"
    );

    // Verify trace IDs are valid hex strings
    let trace_id = first_span["traceId"].as_str().unwrap();
    assert!(
        !trace_id.is_empty() && trace_id.chars().all(|c| c.is_ascii_hexdigit()),
        "traceId should be hex string"
    );

    let span_id = first_span["spanId"].as_str().unwrap();
    assert!(
        !span_id.is_empty() && span_id.chars().all(|c| c.is_ascii_hexdigit()),
        "spanId should be hex string"
    );
}

#[test]
fn test_otlp_span_timing() {
    // Verify span timing is consistent
    let capture_path = fixtures_dir().join("http/http_with_jpegs.cap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let exporter = OtlpExporter;
    let mut output = Vec::new();
    exporter.export(&events, &mut output).unwrap();

    let otlp_string = String::from_utf8(output).unwrap();
    let otlp_json: serde_json::Value = serde_json::from_str(&otlp_string).unwrap();

    let spans = otlp_json["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array()
        .unwrap();

    for span in spans {
        let start_str = span["startTimeUnixNano"].as_str().unwrap();
        let end_str = span["endTimeUnixNano"].as_str().unwrap();

        let start: u64 = start_str.parse().expect("Start time should be numeric");
        let end: u64 = end_str.parse().expect("End time should be numeric");

        assert!(
            start <= end,
            "Span start time should be <= end time (start: {}, end: {})",
            start,
            end
        );
    }
}

#[test]
fn test_csv_and_har_event_count_consistency() {
    // Same capture → export to both CSV and HAR
    // NOTE: CSV exports all events, HAR only exports gRPC events
    let capture_path = fixtures_dir().join("dns/dns.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path.clone(), None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    let event_count = events.len();

    // Export to CSV - should export all events
    let csv_exporter = CsvExporter;
    let mut csv_output = Vec::new();
    csv_exporter.export(&events, &mut csv_output).unwrap();

    let csv_string = String::from_utf8(csv_output).unwrap();
    let csv_data_rows = csv_string.lines().count() - 1; // Subtract header

    // Export to HAR - only exports gRPC events
    let har_exporter = HarExporter;
    let mut har_output = Vec::new();
    har_exporter.export(&events, &mut har_output).unwrap();

    let har_string = String::from_utf8(har_output).unwrap();
    let har_json: serde_json::Value = serde_json::from_str(&har_string).unwrap();
    let har_entries = har_json["log"]["entries"].as_array().unwrap().len();

    // CSV should reflect all events
    assert_eq!(
        csv_data_rows, event_count,
        "CSV row count should match event count"
    );

    // HAR may have fewer entries (only gRPC)
    assert!(
        har_entries <= event_count,
        "HAR entry count should be <= event count (HAR only exports gRPC)"
    );
}

#[test]
fn test_smb_export_to_all_formats() {
    // Test that SMB/enterprise traffic exports cleanly to all formats
    let capture_path = fixtures_dir().join("smb/smb2-peter.pcap");
    assert!(capture_path.exists());

    let mut adapter = PcapCaptureAdapter::new(capture_path, None);
    let events = collect_ok_events(&mut adapter);
    assert!(!events.is_empty());

    // CSV export
    let csv_exporter = CsvExporter;
    let mut csv_output = Vec::new();
    assert!(
        csv_exporter.export(&events, &mut csv_output).is_ok(),
        "CSV export should handle SMB data"
    );
    assert!(!csv_output.is_empty());

    // HAR export
    let har_exporter = HarExporter;
    let mut har_output = Vec::new();
    assert!(
        har_exporter.export(&events, &mut har_output).is_ok(),
        "HAR export should handle SMB data"
    );
    assert!(!har_output.is_empty());

    // OTLP export
    let otlp_exporter = OtlpExporter;
    let mut otlp_output = Vec::new();
    assert!(
        otlp_exporter.export(&events, &mut otlp_output).is_ok(),
        "OTLP export should handle SMB data"
    );
    assert!(!otlp_output.is_empty());
}
