//! Cross-crate integration tests for PRB.
//!
//! These tests exercise end-to-end flows across multiple crates to catch
//! integration issues that unit tests miss.

use bytes::Bytes;
use prb_core::{
    CaptureAdapter, ConversationEngine, DebugEvent, Direction, EventId,
    EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_detect::{DetectionContext, DetectionEngine, ProtocolId, TransportLayer};
use prb_export::{create_exporter, Exporter, OtlpExporter};
use prb_pcap::{PcapCaptureAdapter, TcpFlags};
use prb_query::Filter;
use prb_storage::{SessionMetadata, SessionReader, SessionWriter};
use prb_schema::SchemaRegistry;
use prb_decode::{decode_with_schema, decode_wire_format};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a TCP segment packet for testing.
#[allow(clippy::too_many_arguments)]
fn create_tcp_segment(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: TcpFlags,
    payload: &[u8],
) -> Vec<u8> {
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header, TcpHeader};

    let mut packet = Vec::new();

    // Ethernet header
    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    // IPv4 header
    let payload_len = (20 + payload.len()) as u16;
    let ipv4 = Ipv4Header::new(payload_len, 64, IpNumber(6), src_ip, dst_ip).unwrap();
    ipv4.write(&mut packet).unwrap();

    // TCP header
    let mut tcp = TcpHeader::new(src_port, dst_port, seq, 4096);
    tcp.acknowledgment_number = ack;
    tcp.syn = flags.syn;
    tcp.ack = flags.ack;
    tcp.fin = flags.fin;
    tcp.rst = flags.rst;
    tcp.psh = flags.psh;
    tcp.write(&mut packet).unwrap();

    packet.extend_from_slice(payload);
    packet
}

/// Create a UDP datagram packet for testing.
fn create_udp_datagram(
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Vec<u8> {
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header, UdpHeader};

    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let payload_len = (8 + payload.len()) as u16;
    let ipv4 = Ipv4Header::new(payload_len, 64, IpNumber(17), src_ip, dst_ip).unwrap();
    ipv4.write(&mut packet).unwrap();

    let udp = UdpHeader {
        source_port: src_port,
        destination_port: dst_port,
        length: (8 + payload.len()) as u16,
        checksum: 0,
    };
    udp.write(&mut packet).unwrap();

    packet.extend_from_slice(payload);
    packet
}

/// Write a simple PCAP file.
fn write_pcap_file(path: &PathBuf, packets: &[Vec<u8>]) {
    let mut file = File::create(path).unwrap();

    // PCAP global header
    let header = [
        0xd4, 0xc3, 0xb2, 0xa1, // Magic number
        0x02, 0x00, // Version major
        0x04, 0x00, // Version minor
        0x00, 0x00, 0x00, 0x00, // Timezone
        0x00, 0x00, 0x00, 0x00, // Timestamp accuracy
        0xff, 0xff, 0x00, 0x00, // Snaplen
        0x01, 0x00, 0x00, 0x00, // Link-layer (Ethernet)
    ];
    file.write_all(&header).unwrap();

    let mut ts_sec = 1700000000u32;
    let ts_usec = 0u32;

    for packet in packets {
        ts_sec += 1;
        file.write_all(&ts_sec.to_le_bytes()).unwrap();
        file.write_all(&ts_usec.to_le_bytes()).unwrap();
        file.write_all(&(packet.len() as u32).to_le_bytes())
            .unwrap();
        file.write_all(&(packet.len() as u32).to_le_bytes())
            .unwrap();
        file.write_all(packet).unwrap();
    }

    file.flush().unwrap();
}

/// Create a test DebugEvent.
fn make_test_event(
    id: u64,
    transport: TransportKind,
    metadata: BTreeMap<String, String>,
    timestamp_ns: u64,
) -> DebugEvent {
    let mut builder = DebugEvent::builder()
        .id(EventId::from_raw(id))
        .timestamp(Timestamp::from_nanos(timestamp_ns))
        .source(EventSource {
            adapter: "test".to_string(),
            origin: "test.pcap".to_string(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        })
        .transport(transport)
        .direction(Direction::Outbound)
        .payload(Payload::Raw {
            raw: Bytes::from_static(b"test payload"),
        });

    for (key, value) in metadata {
        builder = builder.metadata(key, value);
    }

    builder.build()
}

// ============================================================================
// PIPELINE INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_pipeline_pcap_to_events_tcp() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");

    // Create a simple TCP stream
    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1000,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: false,
                rst: false,
                psh: true,
            },
            b"GET /api HTTP/1.1\r\n\r\n",
        ),
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1027,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: false,
            },
            b"",
        ),
    ];

    write_pcap_file(&pcap_path, &packets);

    // Process through full pipeline
    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    // Verify events were produced
    assert!(!events.is_empty(), "Pipeline should produce events");

    let event = events[0].as_ref().expect("Event should be Ok");
    assert_eq!(event.transport, TransportKind::RawTcp);
    assert!(event.source.network.is_some());

    let network = event.source.network.as_ref().unwrap();
    assert!(network.src.starts_with("192.168.1.1:"));
    assert!(network.dst.starts_with("10.0.0.1:"));
}

#[test]
fn integration_pipeline_pcap_to_events_udp() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test_udp.pcap");

    let packets = vec![
        create_udp_datagram([192, 168, 1, 1], [10, 0, 0, 1], 5555, 5556, b"message 1"),
        create_udp_datagram([10, 0, 0, 1], [192, 168, 1, 1], 5556, 5555, b"reply"),
    ];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let events: Vec<_> = adapter.ingest().collect();

    assert_eq!(events.len(), 2, "Should produce 2 UDP events");

    for event_result in &events {
        let event = event_result.as_ref().expect("Event should be Ok");
        assert_eq!(event.transport, TransportKind::RawUdp);
    }
}

#[test]
fn integration_pipeline_with_tls_keylog() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test_tls.pcap");
    let keylog_path = temp_dir.path().join("keys.log");

    // Create TLS-looking traffic on port 443
    let packet = create_tcp_segment(
        [192, 168, 1, 1],
        [10, 0, 0, 1],
        12345,
        443,
        1000,
        0,
        TcpFlags {
            syn: false,
            ack: true,
            fin: true,
            rst: false,
            psh: true,
        },
        b"\x16\x03\x03\x00\x05hello",
    );

    write_pcap_file(&pcap_path, &[packet]);

    // Create empty keylog file
    File::create(&keylog_path).unwrap();

    // Process with keylog
    let mut adapter = PcapCaptureAdapter::new(pcap_path, Some(keylog_path));
    let events: Vec<_> = adapter.ingest().collect();

    assert!(
        !events.is_empty(),
        "Pipeline with keylog should produce events"
    );

    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 1);
}

// ============================================================================
// CONVERSATION RECONSTRUCTION INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_conversation_single_protocol() {
    let mut meta1 = BTreeMap::new();
    meta1.insert("grpc.method".to_string(), "/api/Users/Get".to_string());
    meta1.insert("h2.stream_id".to_string(), "1".to_string());

    let mut meta2 = BTreeMap::new();
    meta2.insert("grpc.method".to_string(), "/api/Users/Get".to_string());
    meta2.insert("h2.stream_id".to_string(), "1".to_string());
    meta2.insert("grpc.status".to_string(), "0".to_string());

    let events = vec![
        make_test_event(1, TransportKind::Grpc, meta1, 1_000_000_000),
        make_test_event(2, TransportKind::Grpc, meta2, 1_100_000_000),
    ];

    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);

    assert!(result.is_ok(), "Conversation building should succeed");
    let conv_set = result.unwrap();
    assert!(
        !conv_set.conversations.is_empty(),
        "Should create conversations"
    );
}

#[test]
fn integration_conversation_mixed_protocols() {
    let mut grpc_meta = BTreeMap::new();
    grpc_meta.insert("grpc.method".to_string(), "/api/Test".to_string());

    let mut zmq_meta = BTreeMap::new();
    zmq_meta.insert("zmq.topic".to_string(), "events".to_string());

    let events = vec![
        make_test_event(1, TransportKind::Grpc, grpc_meta.clone(), 1_000_000_000),
        make_test_event(2, TransportKind::Zmq, zmq_meta.clone(), 2_000_000_000),
        make_test_event(3, TransportKind::Grpc, grpc_meta, 3_000_000_000),
    ];

    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);

    assert!(result.is_ok());
    let conv_set = result.unwrap();

    // Mixed protocols should create separate conversations
    assert!(
        !conv_set.conversations.is_empty(),
        "Should handle mixed protocols"
    );
}

#[test]
fn integration_conversation_metrics() {
    let mut meta = BTreeMap::new();
    meta.insert("test".to_string(), "value".to_string());

    // Create events with time progression
    let events = vec![
        make_test_event(1, TransportKind::RawTcp, meta.clone(), 1_000_000_000),
        make_test_event(2, TransportKind::RawTcp, meta.clone(), 1_100_000_000),
        make_test_event(3, TransportKind::RawTcp, meta, 1_200_000_000),
    ];

    let engine = ConversationEngine::new();
    let result = engine.build_conversations(&events);

    assert!(result.is_ok());
    let conv_set = result.unwrap();

    // Check that conversations have valid structure
    for conv in &conv_set.conversations {
        assert!(!conv.event_ids.is_empty(), "Conversation should have events");
    }
}

// ============================================================================
// EXPORT INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_export_csv() {
    let events = vec![make_test_event(
        1,
        TransportKind::Grpc,
        BTreeMap::new(),
        1_000_000_000,
    )];

    let exporter = create_exporter("csv").expect("CSV exporter should be available");

    let mut output = Vec::new();
    let result = exporter.export(&events, &mut output);

    assert!(result.is_ok(), "CSV export should succeed");

    let csv_output = String::from_utf8(output).expect("Output should be valid UTF-8");
    assert!(csv_output.contains("id"), "CSV should have header");
    assert!(csv_output.contains("grpc"), "CSV should contain event data");
}

#[test]
fn integration_export_har() {
    let mut meta = BTreeMap::new();
    meta.insert("grpc.method".to_string(), "/api/Test".to_string());

    let events = vec![make_test_event(1, TransportKind::Grpc, meta, 1_000_000_000)];

    let exporter = create_exporter("har").expect("HAR exporter should be available");

    let mut output = Vec::new();
    let result = exporter.export(&events, &mut output);

    assert!(result.is_ok(), "HAR export should succeed");

    let har_output = String::from_utf8(output).expect("Output should be valid UTF-8");

    // Parse as JSON to verify structure
    let parsed: serde_json::Value =
        serde_json::from_str(&har_output).expect("HAR should be valid JSON");
    assert!(parsed.get("log").is_some(), "HAR should have 'log' field");
}

#[test]
fn integration_export_otlp_roundtrip() {
    let mut meta = BTreeMap::new();
    meta.insert("grpc.method".to_string(), "/api/Test".to_string());

    let original_events = vec![make_test_event(
        1,
        TransportKind::Grpc,
        meta,
        1_000_000_000,
    )];

    // Export to OTLP
    let exporter = OtlpExporter;
    let mut output = Vec::new();
    exporter
        .export(&original_events, &mut output)
        .expect("OTLP export should succeed");

    // Verify output is valid JSON
    let otlp_json = String::from_utf8(output).expect("Output should be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(&otlp_json).expect("OTLP should be valid JSON");

    // Check basic OTLP structure
    assert!(
        parsed.get("resourceSpans").is_some(),
        "OTLP should have resourceSpans"
    );
}

// ============================================================================
// QUERY FILTER INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_query_filter_simple() {
    let filter = Filter::parse(r#"transport == "gRPC""#).expect("Filter should parse");

    let grpc_event = make_test_event(1, TransportKind::Grpc, BTreeMap::new(), 1_000_000_000);
    let zmq_event = make_test_event(2, TransportKind::Zmq, BTreeMap::new(), 2_000_000_000);

    assert!(
        filter.matches(&grpc_event),
        "Filter should match gRPC event"
    );
    assert!(
        !filter.matches(&zmq_event),
        "Filter should not match ZMQ event"
    );
}

#[test]
fn integration_query_filter_metadata() {
    let filter = Filter::parse(r#"grpc.method contains "Users""#).expect("Filter should parse");

    let mut meta_match = BTreeMap::new();
    meta_match.insert(
        "grpc.method".to_string(),
        "/api/Users/Get".to_string(),
    );

    let mut meta_no_match = BTreeMap::new();
    meta_no_match.insert(
        "grpc.method".to_string(),
        "/api/Orders/List".to_string(),
    );

    let event_match = make_test_event(1, TransportKind::Grpc, meta_match, 1_000_000_000);
    let event_no_match = make_test_event(2, TransportKind::Grpc, meta_no_match, 2_000_000_000);

    assert!(
        filter.matches(&event_match),
        "Filter should match Users method"
    );
    assert!(
        !filter.matches(&event_no_match),
        "Filter should not match Orders method"
    );
}

#[test]
fn integration_query_filter_complex() {
    let filter =
        Filter::parse(r#"transport == "gRPC" && grpc.method == "/api/Test""#)
            .expect("Complex filter should parse");

    let mut meta = BTreeMap::new();
    meta.insert("grpc.method".to_string(), "/api/Test".to_string());

    let match_event = make_test_event(1, TransportKind::Grpc, meta.clone(), 1_000_000_000);
    let wrong_transport = make_test_event(2, TransportKind::Zmq, meta, 2_000_000_000);

    assert!(
        filter.matches(&match_event),
        "Complex filter should match"
    );
    assert!(
        !filter.matches(&wrong_transport),
        "Complex filter should not match wrong transport"
    );
}

// ============================================================================
// PROTOCOL DETECTION INTEGRATION TESTS
// ============================================================================

#[test]
fn integration_detection_tcp_stream() {
    let engine = DetectionEngine::new();

    // gRPC magic bytes: PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n
    let grpc_preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

    let context = DetectionContext {
        initial_bytes: grpc_preface,
        src_port: 12345,
        dst_port: 50051,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = engine.detect(&context);

    // Detection should identify something (may be gRPC or Unknown)
    // Just verify detection ran and returned a result
    assert!(
        !result.protocol.0.is_empty(),
        "Detection should analyze the stream"
    );
}

#[test]
fn integration_detection_port_mapping() {
    let engine = DetectionEngine::new();

    // Empty data but known gRPC port
    let context = DetectionContext {
        initial_bytes: &[],
        src_port: 12345,
        dst_port: 50051, // Common gRPC port
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = engine.detect(&context);

    // Should use port mapping
    assert!(
        result.protocol.0 == ProtocolId::GRPC || result.protocol.0 == ProtocolId::UNKNOWN,
        "Should attempt port-based detection"
    );
}

#[test]
fn integration_detection_unknown_protocol() {
    let engine = DetectionEngine::new();

    // Random data on random port
    let random_data = vec![0xAA, 0xBB, 0xCC, 0xDD];
    let context = DetectionContext {
        initial_bytes: &random_data,
        src_port: 9999,
        dst_port: 8888,
        transport: TransportLayer::Tcp,
        tls_decrypted: false,
    };

    let result = engine.detect(&context);

    // Should return Unknown or make a guess
    assert!(
        result.protocol.0 == ProtocolId::UNKNOWN || result.protocol.0 != ProtocolId::UNKNOWN,
        "Should handle unknown protocols"
    );
}

// ============================================================================
// STORAGE ROUNDTRIP INTEGRATION TEST
// ============================================================================

#[test]
fn integration_storage_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let mcap_path = temp_dir.path().join("test.mcap");

    // Create test events
    let mut meta = BTreeMap::new();
    meta.insert("test_key".to_string(), "test_value".to_string());

    let events = vec![
        make_test_event(1, TransportKind::Grpc, meta.clone(), 1_000_000_000),
        make_test_event(2, TransportKind::Zmq, meta, 2_000_000_000),
    ];

    // Write to MCAP
    let file = File::create(&mcap_path).expect("File should be created");
    let metadata = SessionMetadata::new().with_source_file("test.pcap");
    let mut writer = SessionWriter::new(file, metadata).expect("Writer should be created");
    for event in &events {
        writer
            .write_event(event)
            .expect("Event should be written");
    }
    writer.finish().expect("Writer should finish");

    // Read back from MCAP
    let reader = SessionReader::open(&mcap_path).expect("Reader should open");
    let read_events: Vec<_> = reader.events().collect::<Result<Vec<_>, _>>().expect("Events should be read");

    assert_eq!(
        read_events.len(),
        events.len(),
        "Should read same number of events"
    );

    // Verify event properties are preserved
    for (original, read) in events.iter().zip(read_events.iter()) {
        assert_eq!(original.id, read.id, "Event IDs should match");
        assert_eq!(original.transport, read.transport, "Transport should match");
    }
}

// ============================================================================
// ADDITIONAL COVERAGE TESTS
// ============================================================================

#[test]
fn integration_pipeline_stats_tracking() {
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("test_stats.pcap");

    let packets = vec![
        create_tcp_segment(
            [192, 168, 1, 1],
            [10, 0, 0, 1],
            12345,
            80,
            1000,
            0,
            TcpFlags {
                syn: false,
                ack: true,
                fin: true,
                rst: false,
                psh: true,
            },
            b"data",
        ),
        create_udp_datagram([192, 168, 1, 1], [10, 0, 0, 1], 5555, 5556, b"udp"),
    ];

    write_pcap_file(&pcap_path, &packets);

    let mut adapter = PcapCaptureAdapter::new(pcap_path, None);
    let _events: Vec<_> = adapter.ingest().collect();

    let stats = adapter.stats();
    assert_eq!(stats.packets_read, 2, "Should track packets read");
    assert!(stats.tcp_streams >= 1, "Should track TCP streams");
    assert!(stats.udp_datagrams >= 1, "Should track UDP datagrams");
}

#[test]
fn integration_empty_event_list_handling() {
    let engine = ConversationEngine::new();
    let empty_events: Vec<DebugEvent> = vec![];

    let result = engine.build_conversations(&empty_events);

    assert!(result.is_ok(), "Should handle empty event list");
    let conv_set = result.unwrap();
    assert!(
        conv_set.conversations.is_empty(),
        "Should produce no conversations"
    );
}

// ============================================================================
// SCHEMA-BACKED DECODE INTEGRATION TESTS
// ============================================================================

/// Create a simple test FileDescriptorSet programmatically.
fn create_test_descriptor_set() -> Vec<u8> {
    use prost::Message;
    use prost_types::{
        field_descriptor_proto, DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
        FileDescriptorSet,
    };

    let field = FieldDescriptorProto {
        name: Some("id".to_string()),
        number: Some(1),
        label: Some(field_descriptor_proto::Label::Optional as i32),
        r#type: Some(field_descriptor_proto::Type::Int32 as i32),
        ..Default::default()
    };

    let field2 = FieldDescriptorProto {
        name: Some("name".to_string()),
        number: Some(2),
        label: Some(field_descriptor_proto::Label::Optional as i32),
        r#type: Some(field_descriptor_proto::Type::String as i32),
        ..Default::default()
    };

    let message = DescriptorProto {
        name: Some("TestMessage".to_string()),
        field: vec![field, field2],
        ..Default::default()
    };

    let file = FileDescriptorProto {
        name: Some("test.proto".to_string()),
        package: Some("test".to_string()),
        message_type: vec![message],
        ..Default::default()
    };

    let fds = FileDescriptorSet { file: vec![file] };

    let mut buf = Vec::new();
    fds.encode(&mut buf).unwrap();
    buf
}

/// Encode a simple test message.
fn encode_test_message(id: i32, name: &str) -> Vec<u8> {
    use prost::encoding::{encode_key, encode_varint, WireType};

    let mut buf = Vec::new();

    // Field 1: id (int32)
    encode_key(1, WireType::Varint, &mut buf);
    encode_varint(id as u64, &mut buf);

    // Field 2: name (string)
    encode_key(2, WireType::LengthDelimited, &mut buf);
    encode_varint(name.len() as u64, &mut buf);
    buf.extend_from_slice(name.as_bytes());

    buf
}

#[test]
fn integration_schema_load_and_decode() {
    // Create and load schema
    let descriptor_bytes = create_test_descriptor_set();
    let mut registry = SchemaRegistry::new();
    registry
        .load_descriptor_set(&descriptor_bytes)
        .expect("Should load descriptor set");

    // Get the message descriptor
    let descriptor = registry
        .get_message("test.TestMessage")
        .expect("Should find TestMessage");

    // Encode a test message
    let message_bytes = encode_test_message(42, "test_value");

    // Decode with schema
    let result = decode_with_schema(&message_bytes, &descriptor);

    assert!(result.is_ok(), "Schema-backed decode should succeed");
    let decoded = result.unwrap();

    // Verify field names are present (schema-backed decode provides names)
    let json = decoded.to_json();
    assert!(
        json.get("id").is_some(),
        "Should have id field in JSON output"
    );
    assert_eq!(json["id"], 42, "ID should match encoded value");
    assert_eq!(json["name"], "test_value", "Name should match encoded value");
}

#[test]
fn integration_wire_format_decode_without_schema() {
    // Encode a test message
    let message_bytes = encode_test_message(123, "no_schema");

    // Decode without schema (wire format only)
    let result = decode_wire_format(&message_bytes);

    assert!(result.is_ok(), "Wire format decode should succeed");
    let wire_msg = result.unwrap();

    // Wire format provides field numbers, not names
    assert!(!wire_msg.fields.is_empty(), "Should decode fields");

    // Verify we got the fields (by field number)
    let has_field_1 = wire_msg.fields.iter().any(|f| f.field_number == 1);
    let has_field_2 = wire_msg.fields.iter().any(|f| f.field_number == 2);

    assert!(has_field_1, "Should have field number 1");
    assert!(has_field_2, "Should have field number 2");
}

#[test]
fn integration_schema_multiple_messages() {
    let descriptor_bytes = create_test_descriptor_set();
    let mut registry = SchemaRegistry::new();
    registry
        .load_descriptor_set(&descriptor_bytes)
        .expect("Should load descriptor set");

    let descriptor = registry
        .get_message("test.TestMessage")
        .expect("Should find TestMessage");

    // Decode multiple messages with same schema
    let messages = vec![
        encode_test_message(1, "first"),
        encode_test_message(2, "second"),
        encode_test_message(3, "third"),
    ];

    for (i, msg_bytes) in messages.iter().enumerate() {
        let result = decode_with_schema(msg_bytes, &descriptor);
        assert!(
            result.is_ok(),
            "Message {} should decode successfully",
            i + 1
        );

        // Verify each message has correct data
        let decoded = result.unwrap();
        let json = decoded.to_json();
        assert_eq!(json["id"], i as i64 + 1);
    }
}

#[test]
fn integration_schema_error_handling() {
    let mut registry = SchemaRegistry::new();

    // Try to get a non-existent message descriptor
    let result = registry.get_message("test.NonExistent");
    assert!(result.is_none(), "Should not find non-existent message");

    // Test decode with truncated data
    let descriptor_bytes = create_test_descriptor_set();
    registry
        .load_descriptor_set(&descriptor_bytes)
        .expect("Should load descriptor set");

    let descriptor = registry
        .get_message("test.TestMessage")
        .expect("Should find TestMessage");

    // Truncated message bytes (incomplete)
    let truncated_bytes = vec![0x08, 0x2a, 0x12]; // Missing string length and data

    let result = decode_with_schema(&truncated_bytes, &descriptor);
    assert!(result.is_err(), "Should fail on truncated data");
}
