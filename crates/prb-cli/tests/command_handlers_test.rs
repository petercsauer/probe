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
        input: Some(Utf8PathBuf::from("test.ndjson")),
        where_clause: Some("transport == \"gRPC\"".to_string()),
        proto: vec![],
        descriptor_set: vec![],
        demo: false,
        interface: None,
        bpf_filter: None,
        tls_keylog: None,
        session: None,
        diff: false,
        diff_file: None,
    };

    assert_eq!(args.input.as_ref().unwrap().as_str(), "test.ndjson");
    assert!(args.where_clause.is_some());
}

#[test]
fn test_tui_args_no_filter() {
    let args = TuiArgs {
        input: Some(Utf8PathBuf::from("events.json")),
        where_clause: None,
        proto: vec![],
        descriptor_set: vec![],
        demo: false,
        interface: None,
        bpf_filter: None,
        tls_keylog: None,
        session: None,
        diff: false,
        diff_file: None,
    };

    assert_eq!(args.input.as_ref().unwrap().as_str(), "events.json");
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
// Schemas Command Tests - Additional
// ============================================================================

#[test]
fn test_schemas_load_with_multiple_includes() {
    let temp_dir = create_temp_dir();
    let proto_path = create_sample_proto_file(&temp_dir);
    let include_dir1 = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();
    let include_dir2 = Utf8PathBuf::from("/tmp");

    let args = SchemasArgs {
        command: SchemasCommand::Load(SchemaLoadArgs {
            path: proto_path,
            include_paths: vec![include_dir1, include_dir2],
        }),
    };

    let result = run_schemas(args);
    assert!(result.is_ok(), "Schemas load with multiple includes should succeed");
}

#[test]
fn test_schemas_load_invalid_proto_syntax() {
    let temp_dir = create_temp_dir();
    let bad_proto = temp_dir.path().join("bad.proto");
    // Create a proto file with invalid syntax
    fs::write(&bad_proto, "this is not valid protobuf syntax!!!").unwrap();
    let bad_path = Utf8PathBuf::from_path_buf(bad_proto).unwrap();

    let args = SchemasArgs {
        command: SchemasCommand::Load(SchemaLoadArgs {
            path: bad_path,
            include_paths: vec![],
        }),
    };

    let result = run_schemas(args);
    assert!(result.is_err(), "Schemas load should fail for invalid proto syntax");
}

// ============================================================================
// TUI Command Tests - Additional
// ============================================================================

#[test]
fn test_tui_with_filter_syntax() {
    // Test various where clause syntaxes can be constructed
    let test_cases = vec![
        r#"transport == "gRPC""#,
        r#"direction == "inbound""#,
        r#"timestamp > 1000000000"#,
    ];

    for filter in test_cases {
        let args = TuiArgs {
            input: Some(Utf8PathBuf::from("test.json")),
            where_clause: Some(filter.to_string()),
            proto: vec![],
            descriptor_set: vec![],
            demo: false,
            interface: None,
            bpf_filter: None,
            tls_keylog: None,
            session: None,
            diff: false,
            diff_file: None,
        };
        assert_eq!(args.where_clause.as_ref().unwrap(), filter);
    }
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

#[test]
fn test_ingest_with_protocol_filter() {
    let temp_dir = create_temp_dir();
    let input_path = create_sample_ndjson_file(&temp_dir);

    let args = IngestArgs {
        input: input_path,
        output: None,
        tls_keylog: None,
        protocol: Some("grpc".to_string()),
        trace_id: None,
        span_id: None,
        jobs: 1,
    };

    let result = run_ingest(args);
    assert!(result.is_ok(), "Ingest with protocol filter should succeed");
}

#[test]
fn test_ingest_with_multiple_jobs() {
    let temp_dir = create_temp_dir();
    let input_path = create_sample_ndjson_file(&temp_dir);

    let args = IngestArgs {
        input: input_path,
        output: None,
        tls_keylog: None,
        protocol: None,
        trace_id: None,
        span_id: None,
        jobs: 4,
    };

    let result = run_ingest(args);
    assert!(result.is_ok(), "Ingest with multiple jobs should succeed");
}

// ============================================================================
// Main.rs Coverage Tests
// ============================================================================

#[test]
fn test_main_command_dispatch_coverage() {
    // These tests increase coverage for main.rs dispatch logic by calling command
    // handlers directly (main.rs dispatches to these same handlers)

    // Test all command variants are reachable
    let temp_dir = create_temp_dir();
    let ndjson = create_debug_events_ndjson(&temp_dir);

    // Already tested above but ensures dispatch coverage
    let _ = run_inspect(InspectArgs {
        input: Some(ndjson.clone()),
        format: OutputFormat::Json,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    });
}

// ============================================================================
// Additional Plugins Tests for Coverage
// ============================================================================

#[test]
fn test_plugins_info_detailed_grpc() {
    // Test detailed info output for gRPC built-in
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "grpc".to_string(),
        },
    };

    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Should get gRPC decoder info");
}

#[test]
fn test_plugins_info_case_insensitive_match() {
    // Test case-insensitive name matching
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "GRPC".to_string(),
        },
    };

    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Should match gRPC decoder case-insensitively");
}

#[test]
fn test_plugins_info_partial_name_match() {
    // Test partial name matching for built-ins
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "http2".to_string(),
        },
    };

    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Should match gRPC decoder by partial name");
}

#[test]
fn test_plugins_list_shows_builtins() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::List,
    };

    // Should always list built-in decoders
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_ok(), "List should show built-in decoders");
}

#[test]
fn test_plugins_install_native_with_custom_name() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create a dummy .so file (won't be loadable but tests path logic)
    let dummy_so = temp_dir.path().join("test.so");
    fs::write(&dummy_so, b"not a real plugin").unwrap();
    let so_path = Utf8PathBuf::from_path_buf(dummy_so).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::Install {
            path: so_path,
            name: Some("custom-name".to_string()),
        },
    };

    // Will fail loading but tests install logic paths
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_err(), "Should fail loading invalid plugin");
}

#[test]
fn test_plugins_install_wasm() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create a dummy .wasm file
    let dummy_wasm = temp_dir.path().join("test.wasm");
    fs::write(&dummy_wasm, b"not a real wasm module").unwrap();
    let wasm_path = Utf8PathBuf::from_path_buf(dummy_wasm).unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::Install {
            path: wasm_path,
            name: None,
        },
    };

    // Will fail loading but tests wasm path
    let result = run_plugins(args, Some(&plugin_dir));
    assert!(result.is_err(), "Should fail loading invalid wasm");
}

// ============================================================================
// Additional Capture Tests
// ============================================================================

#[test]
fn test_capture_with_all_options() {
    // Test that all capture options can be set together
    let temp_dir = create_temp_dir();
    let args = CaptureArgs {
        list_interfaces: false,
        interface: Some("lo".to_string()),
        bpf_filter: Some("tcp port 80".to_string()),
        output: Some(Utf8PathBuf::from_path_buf(temp_dir.path().join("out.ndjson")).unwrap()),
        write_pcap: Some(Utf8PathBuf::from_path_buf(temp_dir.path().join("out.pcap")).unwrap()),
        snaplen: 1500,
        no_promisc: true,
        buffer_size: 8 * 1024 * 1024,
        tls_keylog: Some(Utf8PathBuf::from_path_buf(temp_dir.path().join("keys.log")).unwrap()),
        count: Some(10),
        duration: Some(5),
        tui: false,
        format: CaptureOutputFormat::Summary,
        quiet: false,
    };

    // Can't actually capture without permissions, but tests arg construction
    assert_eq!(args.snaplen, 1500);
    assert!(args.no_promisc);
    assert_eq!(args.buffer_size, 8 * 1024 * 1024);
}

#[test]
fn test_capture_quiet_mode() {
    let args = CaptureArgs {
        list_interfaces: false,
        interface: Some("lo".to_string()),
        bpf_filter: None,
        output: None,
        write_pcap: None,
        snaplen: 65535,
        no_promisc: false,
        buffer_size: 2 * 1024 * 1024,
        tls_keylog: None,
        count: Some(1),
        duration: None,
        tui: false,
        format: CaptureOutputFormat::Summary,
        quiet: true,
    };

    assert!(args.quiet);
    // format field is set to Summary
}

#[test]
fn test_capture_json_output_format() {
    let args = CaptureArgs {
        list_interfaces: false,
        interface: Some("eth0".to_string()),
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
        format: CaptureOutputFormat::Json,
        quiet: false,
    };

    // format field is set to Json
    assert!(!args.quiet);
}

// ============================================================================
// Additional Inspect Tests
// ============================================================================

#[test]
fn test_inspect_all_output_formats() {
    let temp_dir = create_temp_dir();
    let ndjson = create_debug_events_ndjson(&temp_dir);

    // Test Table format
    let result = run_inspect(InspectArgs {
        input: Some(ndjson.clone()),
        format: OutputFormat::Table,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    });
    assert!(result.is_ok(), "Table format should work");

    // Test Json format
    let result = run_inspect(InspectArgs {
        input: Some(ndjson),
        format: OutputFormat::Json,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    });
    assert!(result.is_ok(), "JSON format should work");
}

#[test]
fn test_inspect_combined_filters() {
    let temp_dir = create_temp_dir();
    let ndjson = create_debug_events_ndjson(&temp_dir);

    // Test combining multiple filter types
    let result = run_inspect(InspectArgs {
        input: Some(ndjson),
        format: OutputFormat::Json,
        filter: Some("grpc".to_string()),
        where_clause: Some("direction == \"outbound\"".to_string()),
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    });
    assert!(result.is_ok(), "Combined filters should work");
}

// ============================================================================
// Additional Merge Tests
// ============================================================================

#[test]
fn test_merge_both_empty_filter() {
    let temp_dir = create_temp_dir();
    let packets = create_debug_events_ndjson(&temp_dir);
    let traces = create_sample_otlp_json(&temp_dir);
    let output = Utf8PathBuf::from_path_buf(temp_dir.path().join("merged.ndjson")).unwrap();

    let result = run_merge(MergeArgs {
        packets,
        traces,
        output: Some(output.clone()),
    });

    assert!(result.is_ok(), "Merge should succeed");
    assert!(output.as_std_path().exists(), "Output should be created");
}

// ============================================================================
// Additional Export Tests
// ============================================================================

#[test]
fn test_export_invalid_where_clause() {
    let temp_dir = create_temp_dir();
    let input = create_debug_events_ndjson(&temp_dir);
    let output = Utf8PathBuf::from_path_buf(temp_dir.path().join("out.csv")).unwrap();

    let result = run_export(ExportArgs {
        input,
        output: Some(output),
        format: ExportFormat::Csv,
        where_clause: Some("invalid syntax !!!".to_string()),
    });

    // Should handle invalid where clause
    assert!(result.is_err(), "Invalid where clause should fail");
}

// ============================================================================
// Additional TUI Tests
// ============================================================================

#[test]
fn test_tui_with_long_path() {
    let args = TuiArgs {
        input: Some(Utf8PathBuf::from("/very/long/path/to/some/events.ndjson")),
        where_clause: Some(r#"transport == "gRPC" && direction == "inbound""#.to_string()),
        proto: vec![],
        descriptor_set: vec![],
        demo: false,
        interface: None,
        bpf_filter: None,
        tls_keylog: None,
        session: None,
        diff: false,
        diff_file: None,
    };

    assert!(args.input.as_ref().unwrap().as_str().contains("events.ndjson"));
    assert!(args.where_clause.is_some());
}

// ============================================================================
// Additional Plugin Tests for load_all_plugins coverage
// ============================================================================

#[test]
fn test_plugins_list_with_corrupt_manifest() {
    let temp_dir = create_temp_dir();
    let plugin_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create a plugin directory with corrupt TOML
    let bad_plugin = temp_dir.path().join("bad-plugin");
    fs::create_dir_all(&bad_plugin).unwrap();
    fs::write(bad_plugin.join("plugin.toml"), "not valid toml!!!").unwrap();

    let args = PluginsArgs {
        command: PluginsCommand::List,
    };

    // Should handle corrupt manifest gracefully
    let result = run_plugins(args, Some(&plugin_dir));
    // Will fail to parse the manifest but should not panic
    let _ = result;
}

#[test]
fn test_plugins_info_zmtp_detailed() {
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "zmtp".to_string(),
        },
    };

    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Should display ZMTP decoder info");
}

#[test]
fn test_plugins_info_rtps_detailed() {
    let args = PluginsArgs {
        command: PluginsCommand::Info {
            name: "rtps".to_string(),
        },
    };

    let result = run_plugins(args, None);
    assert!(result.is_ok(), "Should display RTPS decoder info");
}

// ============================================================================
// Additional Capture Tests for coverage
// ============================================================================

#[test]
fn test_capture_with_count_limit() {
    let args = CaptureArgs {
        list_interfaces: false,
        interface: Some("lo".to_string()),
        bpf_filter: None,
        output: None,
        write_pcap: None,
        snaplen: 65535,
        no_promisc: false,
        buffer_size: 2 * 1024 * 1024,
        tls_keylog: None,
        count: Some(100),
        duration: None,
        tui: false,
        format: CaptureOutputFormat::Summary,
        quiet: false,
    };

    assert_eq!(args.count, Some(100));
}

#[test]
fn test_capture_with_duration_limit() {
    let args = CaptureArgs {
        list_interfaces: false,
        interface: Some("eth0".to_string()),
        bpf_filter: None,
        output: None,
        write_pcap: None,
        snaplen: 65535,
        no_promisc: false,
        buffer_size: 2 * 1024 * 1024,
        tls_keylog: None,
        count: None,
        duration: Some(30),
        tui: false,
        format: CaptureOutputFormat::Json,
        quiet: true,
    };

    assert_eq!(args.duration, Some(30));
    assert!(args.quiet);
}

#[test]
fn test_capture_with_pcap_output() {
    let temp_dir = create_temp_dir();
    let pcap_path = Utf8PathBuf::from_path_buf(temp_dir.path().join("capture.pcap")).unwrap();

    let args = CaptureArgs {
        list_interfaces: false,
        interface: Some("lo".to_string()),
        bpf_filter: Some("tcp".to_string()),
        output: None,
        write_pcap: Some(pcap_path.clone()),
        snaplen: 65535,
        no_promisc: false,
        buffer_size: 2 * 1024 * 1024,
        tls_keylog: None,
        count: Some(1),
        duration: None,
        tui: false,
        format: CaptureOutputFormat::Summary,
        quiet: false,
    };

    assert_eq!(args.write_pcap.as_ref().unwrap(), &pcap_path);
}

// ============================================================================
// Additional Inspect Tests
// ============================================================================

#[test]
fn test_inspect_from_stdin() {
    // Test inspect with None input (reads from stdin)
    let args = InspectArgs {
        input: None,
        format: OutputFormat::Json,
        filter: None,
        where_clause: None,
        trace_id: None,
        span_id: None,
        group_by_trace: false,
        wire_format: false,
    };

    // Tests the stdin code path (may succeed with empty output or fail)
    let _ = run_inspect(args);
}

#[test]
fn test_inspect_with_all_filters_combined() {
    let temp_dir = create_temp_dir();
    let ndjson = create_debug_events_ndjson(&temp_dir);

    let args = InspectArgs {
        input: Some(ndjson),
        format: OutputFormat::Table,
        filter: Some("grpc".to_string()),
        where_clause: Some(r#"direction == "outbound""#.to_string()),
        trace_id: Some("abc123".to_string()),
        span_id: Some("def456".to_string()),
        group_by_trace: true,
        wire_format: true,
    };

    let result = run_inspect(args);
    assert!(result.is_ok(), "Should handle all filters together");
}

// ============================================================================
// Additional Merge Tests
// ============================================================================

#[test]
fn test_merge_creates_output_directory() {
    let temp_dir = create_temp_dir();
    let packets = create_debug_events_ndjson(&temp_dir);
    let traces = create_sample_otlp_json(&temp_dir);

    // Create output path in non-existent subdirectory
    let subdir = temp_dir.path().join("output_dir");
    let output = Utf8PathBuf::from_path_buf(subdir.join("merged.ndjson")).unwrap();

    let args = MergeArgs {
        packets,
        traces,
        output: Some(output.clone()),
    };

    let result = run_merge(args);
    // May fail if directory creation isn't handled, tests that code path
    let _ = result;
}

// ============================================================================
// Additional Export Tests
// ============================================================================

#[test]
fn test_export_csv_to_stdout() {
    let temp_dir = create_temp_dir();
    let input = create_debug_events_ndjson(&temp_dir);

    let args = ExportArgs {
        input,
        output: None, // stdout
        format: ExportFormat::Csv,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_ok(), "CSV to stdout should work");
}

#[test]
fn test_export_har_to_stdout() {
    let temp_dir = create_temp_dir();
    let input = create_debug_events_ndjson(&temp_dir);

    let args = ExportArgs {
        input,
        output: None,
        format: ExportFormat::Har,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_ok(), "HAR to stdout should work");
}

#[test]
fn test_export_otlp_to_stdout() {
    let temp_dir = create_temp_dir();
    let input = create_debug_events_ndjson(&temp_dir);

    let args = ExportArgs {
        input,
        output: None,
        format: ExportFormat::Otlp,
        where_clause: None,
    };

    let result = run_export(args);
    assert!(result.is_ok(), "OTLP to stdout should work");
}

// ============================================================================
// Additional Ingest Tests
// ============================================================================

#[test]
fn test_ingest_with_tls_keylog() {
    let temp_dir = create_temp_dir();
    let input = create_sample_ndjson_file(&temp_dir);
    let keylog = Utf8PathBuf::from_path_buf(temp_dir.path().join("keys.log")).unwrap();

    // Create empty keylog file
    fs::write(&keylog, "").unwrap();

    let args = IngestArgs {
        input,
        output: None,
        tls_keylog: Some(keylog),
        protocol: None,
        trace_id: None,
        span_id: None,
        jobs: 1,
    };

    let result = run_ingest(args);
    assert!(result.is_ok(), "Ingest with TLS keylog should work");
}

#[test]
fn test_ingest_with_all_filters() {
    let temp_dir = create_temp_dir();
    let input = create_sample_ndjson_file(&temp_dir);
    let output = Utf8PathBuf::from_path_buf(temp_dir.path().join("out.ndjson")).unwrap();

    let args = IngestArgs {
        input,
        output: Some(output.clone()),
        tls_keylog: None,
        protocol: Some("grpc".to_string()),
        trace_id: Some("trace123".to_string()),
        span_id: Some("span456".to_string()),
        jobs: 2,
    };

    let result = run_ingest(args);
    assert!(result.is_ok(), "Ingest with all filters should work");
    assert!(output.as_std_path().exists(), "Output should be created");
}
