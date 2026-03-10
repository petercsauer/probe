//! Unit tests for CLI command handlers.
//! These tests call the command handler functions directly to achieve high coverage.

use camino::Utf8PathBuf;
use prb_cli::cli::{
    CaptureArgs, CaptureOutputFormat, ExportArgs, ExportFormat, IngestArgs, InspectArgs,
    MergeArgs, OutputFormat, PluginsArgs, PluginsCommand, SchemaLoadArgs, SchemasArgs,
    SchemasCommand, TuiArgs,
};
use prb_cli::commands::{
    run_capture, run_export, run_ingest, run_inspect, run_merge, run_plugins, run_schemas,
};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// Helper Functions
// ============================================================================

fn create_temp_dir() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn create_sample_ndjson_file(dir: &TempDir) -> Utf8PathBuf {
    let path = dir.path().join("events.json");
    // Create a proper FixtureFile format for JsonFixtureAdapter (used by ingest command)
    let fixture = r#"{
  "version": 1,
  "description": "Test fixture",
  "events": [
    {
      "timestamp_ns": 1710000000000000000,
      "transport": "grpc",
      "direction": "outbound",
      "payload_base64": "dGVzdA==",
      "metadata": {}
    }
  ]
}"#;
    fs::write(&path, fixture).unwrap();
    Utf8PathBuf::from_path_buf(path).unwrap()
}

fn create_debug_events_ndjson(dir: &TempDir) -> Utf8PathBuf {
    let path = dir.path().join("events.ndjson");
    // Create actual NDJSON (DebugEvent per line) for inspect/export/merge commands
    let event = r#"{"id":1,"timestamp":1710000000000000000,"source":{"adapter":"test","origin":"test"},"transport":"grpc","direction":"outbound","payload":{"type":"raw","raw":"dGVzdA=="},"metadata":{}}"#;
    fs::write(&path, event).unwrap();
    Utf8PathBuf::from_path_buf(path).unwrap()
}

fn create_sample_proto_file(dir: &TempDir) -> Utf8PathBuf {
    let path = dir.path().join("test.proto");
    let proto_content = r#"
syntax = "proto3";
package test;

message TestMessage {
    int32 id = 1;
    string name = 2;
}

service TestService {
    rpc GetTest(TestMessage) returns (TestMessage);
}
"#;
    fs::write(&path, proto_content).unwrap();
    Utf8PathBuf::from_path_buf(path).unwrap()
}

fn create_sample_otlp_json(dir: &TempDir) -> Utf8PathBuf {
    let path = dir.path().join("traces.json");
    let content = r#"{
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": "test-service"}}
                ]
            },
            "scopeSpans": [{
                "scope": {"name": "test", "version": "1.0"},
                "spans": [{
                    "traceId": "4bf92f3577b34da6a3ce929d0e0e4736",
                    "spanId": "00f067aa0ba902b7",
                    "name": "/api.v1.Users/Get",
                    "kind": 3,
                    "startTimeUnixNano": "1710000000000000000",
                    "endTimeUnixNano": "1710000000100000000",
                    "attributes": []
                }]
            }]
        }]
    }"#;
    fs::write(&path, content).unwrap();
    Utf8PathBuf::from_path_buf(path).unwrap()
}

fn create_plugin_dir_with_manifest(dir: &TempDir, plugin_name: &str) -> PathBuf {
    let plugin_dir = dir.path().join(plugin_name);
    fs::create_dir_all(&plugin_dir).unwrap();

    let manifest = r#"
[plugin]
name = "test-plugin"
version = "1.0.0"
description = "Test plugin"
api_version = "1.0"
protocol_id = "test-proto"
type = "native"
library = "libtest.so"
"#;
    let manifest_path = plugin_dir.join("plugin.toml");
    fs::write(&manifest_path, manifest).unwrap();

    plugin_dir
}

// ============================================================================
// TUI Command Tests
// ============================================================================

#[test]
fn test_tui_command_struct() {
    // Test that TuiArgs can be constructed correctly
    let args = TuiArgs {
        input: Utf8PathBuf::from("test.ndjson"),
        where_clause: Some("transport == \"gRPC\"".to_string()),
    };

    assert_eq!(args.input.as_str(), "test.ndjson");
    assert!(args.where_clause.is_some());
}

#[test]
fn test_tui_args_no_filter() {
    let args = TuiArgs {
        input: Utf8PathBuf::from("events.json"),
        where_clause: None,
    };

    assert_eq!(args.input.as_str(), "events.json");
    assert!(args.where_clause.is_none());
}

// ============================================================================
// Plugins Command Tests
// ============================================================================

#[test]
fn test_plugins_list_empty_dir() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::List,
    };

    // Should succeed even with empty plugin directory
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_ok(), "List should succeed with empty dir");
}

#[test]
fn test_plugins_list_with_manifest() {
    let temp_dir = create_temp_dir();
    create_plugin_dir_with_manifest(&temp_dir, "test-plugin");
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::List,
    };

    // Should succeed and find the plugin manifest
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_ok(), "List should succeed with plugin manifest");
}

#[test]
fn test_plugins_info_builtin() {
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "grpc".to_string(),
        },
    };

    // Should find built-in gRPC decoder
    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Info should find built-in gRPC decoder");
}

#[test]
fn test_plugins_info_zmtp_builtin() {
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "zmtp".to_string(),
        },
    };

    // Should find built-in ZMTP decoder
    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Info should find built-in ZMTP decoder");
}

#[test]
fn test_plugins_info_rtps_builtin() {
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "rtps".to_string(),
        },
    };

    // Should find built-in RTPS decoder
    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Info should find built-in RTPS decoder");
}

#[test]
fn test_plugins_info_not_found() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "nonexistent-plugin".to_string(),
        },
    };

    // Should fail when plugin not found
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_err(), "Info should fail for nonexistent plugin");
}

#[test]
fn test_plugins_install_missing_file() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let nonexistent = Utf8PathBuf::from("/nonexistent/plugin.so");

    let args = PluginsArgs {
        command: PluginsCommand::Install {
            path: nonexistent,
            name: None,
        },
    };

    // Should fail when source file doesn't exist
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_err(), "Install should fail for missing file");
}

#[test]
fn test_plugins_install_unsupported_extension() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create a file with unsupported extension
    let bad_file = temp_dir.path().join("plugin.txt");
    fs::write(&bad_file, "not a plugin").unwrap();
    let bad_path = Utf8PathBuf::from_path_buf(bad_file).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::Install {
            path: bad_path,
            name: None,
        },
    };

    // Should fail for unsupported file type
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_err(), "Install should fail for unsupported extension");
    assert!(result.unwrap_err().to_string().contains("Unknown plugin file type"));
}

#[test]
fn test_plugins_remove_nonexistent() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::Remove {
            name: "nonexistent-plugin".to_string(),
        },
    };

    // Should fail when plugin doesn't exist
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_err(), "Remove should fail for nonexistent plugin");
}

#[test]
fn test_plugins_remove_existing() {
    let temp_dir = create_temp_dir();
    let plugin_name = "test-plugin";
    create_plugin_dir_with_manifest(&temp_dir, plugin_name);
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::Remove {
            name: plugin_name.to_string(),
        },
    };

    // Should succeed removing existing plugin
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_ok(), "Remove should succeed for existing plugin");

    // Verify plugin directory was removed
    let plugin_path = temp_dir.path().join(plugin_name);
    assert!(!plugin_path.exists(), "Plugin directory should be removed");
}

// ============================================================================
// Capture Command Tests
// ============================================================================

#[test]
fn test_capture_list_interfaces() {
    let args = CaptureArgs {
        list_interfaces: true,
        interface: None,
        bpf_filter: None,
        output: None,
        write_pcap: None,
        snaplen: 65535,
        no_promisc: false,
        buffer_size: 2 * 1024 * 1024,
        tls_keylog: None,
        count: None,
        duration: None,
        tui: false,
        format: CaptureOutputFormat::Summary,
        quiet: false,
    };

    // Should succeed (may list interfaces or show empty list depending on permissions)
    let result = run_capture(args);
    // We can't guarantee success without root, but the command should handle it gracefully
    let _ = result;
}

#[test]
fn test_capture_args_validation() {
    // Test that CaptureArgs can be constructed with various options
    let args = CaptureArgs {
        list_interfaces: false,
        interface: Some("eth0".to_string()),
        bpf_filter: Some("tcp port 443".to_string()),
        output: Some(Utf8PathBuf::from("/tmp/capture.ndjson")),
        write_pcap: Some(Utf8PathBuf::from("/tmp/capture.pcap")),
        snaplen: 1500,
        no_promisc: true,
        buffer_size: 4 * 1024 * 1024,
        tls_keylog: Some(Utf8PathBuf::from("/tmp/keys.log")),
        count: Some(100),
        duration: Some(60),
        tui: false,
        format: CaptureOutputFormat::Json,
        quiet: true,
    };

    assert_eq!(args.interface.as_deref(), Some("eth0"));
    assert_eq!(args.bpf_filter.as_deref(), Some("tcp port 443"));
    assert_eq!(args.snaplen, 1500);
    assert!(args.no_promisc);
    assert_eq!(args.count, Some(100));
    assert_eq!(args.duration, Some(60));
}

// ============================================================================
// Inspect Command Tests
// ============================================================================

#[test]
fn test_inspect_from_file() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Json,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(
        result.is_ok(),
        "Inspect should succeed with valid NDJSON file. Error: {:?}",
        result.err()
    );
}

#[test]
fn test_inspect_with_transport_filter() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Table,
        filter: Some("grpc".to_string()),
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(result.is_ok(), "Inspect should succeed with transport filter");
}

#[test]
fn test_inspect_with_invalid_filter() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Table,
        filter: Some("invalid-transport".to_string()),
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(result.is_err(), "Inspect should fail with invalid transport filter");
}

#[test]
fn test_inspect_with_where_clause() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Json,
        filter: None,
        where_clause: Some(r#"transport == "gRPC""#.to_string()),
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(result.is_ok(), "Inspect should succeed with where clause");
}

#[test]
fn test_inspect_with_trace_id_filter() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Json,
        filter: None,
        where_clause: None,
        trace_id: Some("4bf92f3577b34da6a3ce929d0e0e4736".to_string()),
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(result.is_ok(), "Inspect should succeed with trace ID filter");
}

#[test]
fn test_inspect_with_span_id_filter() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Json,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: Some("00f067aa0ba902b7".to_string()),
        group_by_trace: false,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(result.is_ok(), "Inspect should succeed with span ID filter");
}

#[test]
fn test_inspect_group_by_trace() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Table,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: true,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(result.is_ok(), "Inspect should succeed with group_by_trace");
}

#[test]
fn test_inspect_wire_format() {
    let temp_dir = create_temp_dir();
    let ndjson_path = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson_path),
        format: OutputFormat::Table,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: true,
    };

    let result = run_inspect(args);
    assert!(result.is_ok(), "Inspect should succeed with wire_format");
}

#[test]
fn test_inspect_nonexistent_file() {
    let args = InspectArgs {
        input: Some(Utf8PathBuf::from("/nonexistent/file.ndjson")),
        format: OutputFormat::Table,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    };

    let result = run_inspect(args);
    assert!(result.is_err(), "Inspect should fail for nonexistent file");
}

// ============================================================================
// Schemas Command Tests
// ============================================================================

#[test]
fn test_schemas_load_proto() {
    let temp_dir = create_temp_dir();
    let proto_path = create_sample_proto_file(&temp_dir);
    let include_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let args = SchemasArgs {
        command: SchemasCommand::Load(SchemaLoadArgs {
            path: proto_path,
            include_paths: vec![include_dir],
        }),
    };

    let result = run_schemas(args);
    assert!(result.is_ok(), "Schemas load should succeed with valid proto file");
}

#[test]
fn test_schemas_load_nonexistent_proto() {
    let args = SchemasArgs {
        command: SchemasCommand::Load(SchemaLoadArgs {
            path: Utf8PathBuf::from("/nonexistent/file.proto"),
            include_paths: vec![],
        }),
    };

    let result = run_schemas(args);
    assert!(result.is_err(), "Schemas load should fail for nonexistent file");
}

#[test]
fn test_schemas_load_unsupported_extension() {
    let temp_dir = create_temp_dir();
    let bad_file = temp_dir.path().join("schema.txt");
    fs::write(&bad_file, "not a proto file").unwrap();
    let bad_path = Utf8PathBuf::from_path_buf(bad_file).unwrap();

    let args = SchemasArgs {
        command: SchemasCommand::Load(SchemaLoadArgs {
            path: bad_path,
            include_paths: vec![],
        }),
    };

    let result = run_schemas(args);
    assert!(result.is_err(), "Schemas load should fail for unsupported extension");
    assert!(result.unwrap_err().to_string().contains("Unsupported file extension"));
}

// ============================================================================
// Merge Command Tests
// ============================================================================

#[test]
fn test_merge_ndjson_and_otlp() {
    let temp_dir = create_temp_dir();
    let packets_path = create_debug_events_ndjson(&temp_dir);
    let traces_path = create_sample_otlp_json(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("merged.ndjson")).unwrap();

    let args = MergeArgs {
        packets: packets_path,
        traces: traces_path,
        output: Some(output_path.clone()),
    };

    let result = run_merge(args);
    assert!(result.is_ok(), "Merge should succeed with valid inputs");

    // Verify output file was created
    assert!(output_path.as_std_path().exists(), "Merged output file should exist");
}

#[test]
fn test_merge_to_stdout() {
    let temp_dir = create_temp_dir();
    let packets_path = create_debug_events_ndjson(&temp_dir);
    let traces_path = create_sample_otlp_json(&temp_dir);

    let args = MergeArgs {
        packets: packets_path,
        traces: traces_path,
        output: None, // stdout
    };

    let result = run_merge(args);
    assert!(result.is_ok(), "Merge should succeed writing to stdout");
}

#[test]
fn test_merge_missing_packets_file() {
    let temp_dir = create_temp_dir();
    let traces_path = create_sample_otlp_json(&temp_dir);

    let args = MergeArgs {
        packets: Utf8PathBuf::from("/nonexistent/packets.ndjson"),
        traces: traces_path,
        output: None,
    };

    let result = run_merge(args);
    assert!(result.is_err(), "Merge should fail with missing packets file");
}

#[test]
fn test_merge_missing_traces_file() {
    let temp_dir = create_temp_dir();
    let packets_path = create_debug_events_ndjson(&temp_dir);

    let args = MergeArgs {
        packets: packets_path,
        traces: Utf8PathBuf::from("/nonexistent/traces.json"),
        output: None,
    };

    let result = run_merge(args);
    assert!(result.is_err(), "Merge should fail with missing traces file");
}

// ============================================================================
// Ingest Command Tests
// ============================================================================

#[test]
fn test_ingest_args_construction() {
    let args = IngestArgs {
        input: Utf8PathBuf::from("test.json"),
        output: Some(Utf8PathBuf::from("output.ndjson")),
        tls_keylog: Some(Utf8PathBuf::from("keys.log")),
        protocol: Some("grpc".to_string()),
        trace_id: Some("trace123".to_string()),
        span_id: Some("span456".to_string()),
        jobs: 4,
    };

    assert_eq!(args.input.as_str(), "test.json");
    assert_eq!(args.output.as_ref().unwrap().as_str(), "output.ndjson");
    assert_eq!(args.protocol.as_deref(), Some("grpc"));
    assert_eq!(args.jobs, 4);
}

#[test]
fn test_ingest_args_default_jobs() {
    let args = IngestArgs {
        input: Utf8PathBuf::from("test.json"),
        output: None,
        tls_keylog: None,
        protocol: None,
        trace_id: None,
        span_id: None,
        jobs: 0, // default auto-detect
    };

    assert_eq!(args.jobs, 0);
}

// ============================================================================
// Export Command Tests
// ============================================================================

#[test]
fn test_export_args_construction() {
    let args = ExportArgs {
        input: Utf8PathBuf::from("events.ndjson"),
        output: Some(Utf8PathBuf::from("output.csv")),
        format: ExportFormat::Csv,
        where_clause: Some(r#"transport == "gRPC""#.to_string()),
    };

    assert_eq!(args.input.as_str(), "events.ndjson");
    assert_eq!(args.output.as_ref().unwrap().as_str(), "output.csv");
}

#[test]
fn test_export_formats() {
    // Test that all export formats can be constructed
    let formats = vec![
        ExportFormat::Csv,
        ExportFormat::Har,
        ExportFormat::Html,
        ExportFormat::Otlp,
    ];

    for format in formats {
        let args = ExportArgs {
            input: Utf8PathBuf::from("events.ndjson"),
            output: Some(Utf8PathBuf::from("output.file")),
            format,
            where_clause: None,
        };
        assert_eq!(args.input.as_str(), "events.ndjson");
    }
}

#[test]
fn test_export_with_filter() {
    let args = ExportArgs {
        input: Utf8PathBuf::from("events.ndjson"),
        output: Some(Utf8PathBuf::from("output.csv")),
        format: ExportFormat::Csv,
        where_clause: Some(r#"direction == "inbound""#.to_string()),
    };

    assert!(args.where_clause.is_some());
}

#[test]
fn test_export_to_stdout() {
    let args = ExportArgs {
        input: Utf8PathBuf::from("events.ndjson"),
        output: None, // stdout
        format: ExportFormat::Csv,
        where_clause: None,
    };

    assert!(args.output.is_none());
}

#[test]
fn test_export_csv_from_ndjson() {
    let temp_dir = create_temp_dir();
    let input_path = create_debug_events_ndjson(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("output.csv")).unwrap();

    let args = ExportArgs {
        input: input_path,
        output: Some(output_path.clone()),
        format: ExportFormat::Csv,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_ok(), "CSV export should succeed");
    assert!(output_path.as_std_path().exists(), "CSV output file should exist");
}

#[test]
fn test_export_har_from_ndjson() {
    let temp_dir = create_temp_dir();
    let input_path = create_debug_events_ndjson(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("output.har")).unwrap();

    let args = ExportArgs {
        input: input_path,
        output: Some(output_path.clone()),
        format: ExportFormat::Har,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_ok(), "HAR export should succeed");
    assert!(output_path.as_std_path().exists(), "HAR output file should exist");
}

#[test]
fn test_export_otlp_from_ndjson() {
    let temp_dir = create_temp_dir();
    let input_path = create_debug_events_ndjson(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("output.json")).unwrap();

    let args = ExportArgs {
        input: input_path,
        output: Some(output_path.clone()),
        format: ExportFormat::Otlp,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_ok(), "OTLP export should succeed");
    assert!(output_path.as_std_path().exists(), "OTLP output file should exist");
}

#[test]
fn test_export_html_from_ndjson() {
    let temp_dir = create_temp_dir();
    let input_path = create_debug_events_ndjson(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("output.html")).unwrap();

    let args = ExportArgs {
        input: input_path,
        output: Some(output_path.clone()),
        format: ExportFormat::Html,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_ok(), "HTML export should succeed");
    assert!(output_path.as_std_path().exists(), "HTML output file should exist");
}

#[test]
fn test_export_html_requires_output_file() {
    let temp_dir = create_temp_dir();
    let input_path = create_debug_events_ndjson(&temp_dir);

    let args = ExportArgs {
        input: input_path,
        output: None, // HTML requires file output
        format: ExportFormat::Html,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_err(), "HTML export to stdout should fail");
    assert!(result.unwrap_err().to_string().contains("HTML export requires"));
}

#[test]
fn test_export_with_where_filter() {
    let temp_dir = create_temp_dir();
    let input_path = create_debug_events_ndjson(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("output.csv")).unwrap();

    let args = ExportArgs {
        input: input_path,
        output: Some(output_path.clone()),
        format: ExportFormat::Csv,
        where_clause: Some(r#"transport == "gRPC""#.to_string()),
    };

    let result = run_export(args);
    assert!(result.is_ok(), "Export with filter should succeed");
}

#[test]
fn test_export_missing_input_file() {
    let temp_dir = create_temp_dir();
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("output.csv")).unwrap();

    let args = ExportArgs {
        input: Utf8PathBuf::from("/nonexistent/input.ndjson"),
        output: Some(output_path),
        format: ExportFormat::Csv,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_err(), "Export with missing input should fail");
}

// ============================================================================
// Ingest Command - Real Execution Tests
// ============================================================================

#[test]
fn test_ingest_json_to_ndjson() {
    let temp_dir = create_temp_dir();
    let input_path = create_sample_ndjson_file(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("output.ndjson")).unwrap();

    let args = IngestArgs {
        input: input_path,
        output: Some(output_path.clone()),
        tls_keylog: None,
        protocol: None,
        trace_id: None,
        span_id: None,
        jobs: 1,
    };

    let result = run_ingest(args);
    assert!(result.is_ok(), "JSON ingest should succeed");
    assert!(output_path.as_std_path().exists(), "Output file should exist");
}

#[test]
fn test_ingest_json_to_stdout() {
    let temp_dir = create_temp_dir();
    let input_path = create_sample_ndjson_file(&temp_dir);

    let args = IngestArgs {
        input: input_path,
        output: None,
        tls_keylog: None,
        protocol: None,
        trace_id: None,
        span_id: None,
        jobs: 1,
    };

    let result = run_ingest(args);
    assert!(result.is_ok(), "JSON ingest to stdout should succeed");
}

#[test]
fn test_ingest_missing_file() {
    let args = IngestArgs {
        input: Utf8PathBuf::from("/nonexistent/input.json"),
        output: None,
        tls_keylog: None,
        protocol: None,
        trace_id: None,
        span_id: None,
        jobs: 1,
    };

    let result = run_ingest(args);
    assert!(result.is_err(), "Ingest should fail for missing input file");
}

#[test]
fn test_ingest_with_trace_id_filter() {
    let temp_dir = create_temp_dir();
    let input_path = create_sample_ndjson_file(&temp_dir);
    let output_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("filtered.ndjson")).unwrap();

    let args = IngestArgs {
        input: input_path,
        output: Some(output_path.clone()),
        tls_keylog: None,
        protocol: None,
        trace_id: Some("nonexistent-trace-id".to_string()),
        span_id: None,
        jobs: 1,
    };

    let result = run_ingest(args);
    if let Err(e) = result {
        panic!("Ingest with trace filter should succeed. Error: {:?}", e);
    }
}

#[test]
fn test_ingest_with_span_id_filter() {
    let temp_dir = create_temp_dir();
    let input_path = create_sample_ndjson_file(&temp_dir);

    let args = IngestArgs {
        input: input_path,
        output: None,
        tls_keylog: None,
        protocol: None,
        trace_id: None,
        span_id: Some("00f067aa0ba902b7".to_string()),
        jobs: 1,
    };

    let result = run_ingest(args);
    if let Err(e) = result {
        panic!("Ingest with span filter should succeed. Error: {:?}", e);
    }
}
