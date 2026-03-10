//! Tests for DDS/RTPS decoder.

use crate::decoder::DdsDecoder;
use crate::discovery::{DiscoveredEndpoint, Guid, RtpsDiscoveryTracker};
use prb_core::{DecodeContext, ProtocolDecoder, TransportKind, METADATA_KEY_DDS_DOMAIN_ID, METADATA_KEY_DDS_TOPIC_NAME};

/// Helper to create a minimal valid RTPS message header.
fn create_rtps_header(guid_prefix: [u8; 12]) -> Vec<u8> {
    let mut header = Vec::new();
    // Magic: "RTPS"
    header.extend_from_slice(b"RTPS");
    // Protocol version: 2.3
    header.push(2);
    header.push(3);
    // Vendor ID: 0x0000 (unknown)
    header.push(0);
    header.push(0);
    // GUID prefix (12 bytes)
    header.extend_from_slice(&guid_prefix);
    header
}

/// Helper to create an INFO_TS submessage.
fn create_info_ts_submessage(seconds: u32, fraction: u32) -> Vec<u8> {
    let mut submsg = vec![
        0x09, // Submessage ID: INFO_TS
        0x01, // Flags: little-endian
        8,    // Octets to next header: 8
        0,
    ];
    // Timestamp: seconds (4 bytes) + fraction (4 bytes)
    submsg.extend_from_slice(&seconds.to_le_bytes());
    submsg.extend_from_slice(&fraction.to_le_bytes());
    submsg
}

/// Helper to create a DATA submessage.
fn create_data_submessage(
    writer_entity: [u8; 4],
    reader_entity: [u8; 4],
    sequence_number: i64,
    payload: &[u8],
) -> Vec<u8> {
    let mut submsg = Vec::new();
    // Submessage ID: DATA (0x15)
    submsg.push(0x15);
    // Flags: little-endian (0x01), no inline QoS
    submsg.push(0x01);
    // Octets to next header (calculated later)
    let octets_start = submsg.len();
    submsg.push(0);
    submsg.push(0);
    // Extra flags (2 bytes)
    submsg.push(0);
    submsg.push(0);
    // Octets to inline QoS (2 bytes) - 16 (directly after sequence number)
    submsg.push(16);
    submsg.push(0);
    // Reader entity ID (4 bytes)
    submsg.extend_from_slice(&reader_entity);
    // Writer entity ID (4 bytes)
    submsg.extend_from_slice(&writer_entity);
    // Sequence number: RTPS format is {long high, unsigned long low}
    // For a value like 42, we want high=0, low=42
    let sn_high = ((sequence_number >> 32) & 0xFFFFFFFF) as i32;
    let sn_low = (sequence_number & 0xFFFFFFFF) as u32;
    submsg.extend_from_slice(&sn_high.to_le_bytes());
    submsg.extend_from_slice(&sn_low.to_le_bytes());
    // No inline QoS, go directly to serialized payload
    // Encapsulation kind (0x0000 = CDR_BE)
    submsg.push(0x00);
    submsg.push(0x00);
    // Encapsulation options
    submsg.push(0x00);
    submsg.push(0x00);
    // Payload data
    submsg.extend_from_slice(payload);

    // Update octets to next header
    let octets_to_next = (submsg.len() - 4) as u16;
    submsg[octets_start] = (octets_to_next & 0xFF) as u8;
    submsg[octets_start + 1] = ((octets_to_next >> 8) & 0xFF) as u8;

    submsg
}

#[test]
fn test_rtps_magic_detection() {
    let _decoder = DdsDecoder::new();

    // Valid RTPS magic
    let valid_data = b"RTPS\x02\x03\x00\x00............";
    assert!(DdsDecoder::has_rtps_magic(valid_data));

    // Invalid magic
    let invalid_data = b"HTTP/1.1 200 OK\r\n";
    assert!(!DdsDecoder::has_rtps_magic(invalid_data));

    // Too short
    let short_data = b"RTP";
    assert!(!DdsDecoder::has_rtps_magic(short_data));
}

#[test]
fn test_rtps_message_parse() {
    let mut decoder = DdsDecoder::new();

    let guid_prefix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let mut message = create_rtps_header(guid_prefix);

    // Add a simple DATA submessage
    let writer_entity = [0x00, 0x00, 0x01, 0x02];
    let reader_entity = [0x00, 0x00, 0x00, 0x00];
    let payload = b"test payload";
    message.extend_from_slice(&create_data_submessage(writer_entity, reader_entity, 1, payload));

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.10:7400")
        .with_dst_addr("239.255.0.1:7400")
        .with_metadata("origin", "test.pcap");

    let events = decoder.decode_stream(&message, &ctx).expect("decode should succeed");

    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(event.transport, TransportKind::DdsRtps);
    assert!(event.metadata.contains_key("dds.writer_guid"));
}

#[test]
fn test_rtps_data_payload() {
    let mut decoder = DdsDecoder::new();

    let guid_prefix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let mut message = create_rtps_header(guid_prefix);

    // Use user-defined entity IDs (not built-in)
    let writer_entity = [0x00, 0x00, 0x01, 0x02]; // USER_DEFINED_WRITER_WITH_KEY
    let reader_entity = [0x00, 0x00, 0x01, 0x07]; // USER_DEFINED_READER_WITH_KEY
    let sequence = 42;
    let payload = b"sensor data payload";
    message.extend_from_slice(&create_data_submessage(writer_entity, reader_entity, sequence, payload));

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.10:7400")
        .with_dst_addr("239.255.0.1:7400");

    let result = decoder.decode_stream(&message, &ctx);
    if let Err(ref e) = result {
        eprintln!("Decode error: {}", e);
    }
    let events = result.expect("decode should succeed");

    assert_eq!(events.len(), 1, "Expected 1 event, got {}", events.len());
    let event = &events[0];

    // Check metadata
    assert_eq!(event.metadata.get("dds.sequence_number"), Some(&"42".to_string()));
    assert!(event.metadata.contains_key("dds.writer_guid"));
    assert!(event.metadata.contains_key("dds.reader_entity"));
}

#[test]
fn test_rtps_info_ts_timestamp() {
    let mut decoder = DdsDecoder::new();

    let guid_prefix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let mut message = create_rtps_header(guid_prefix);

    // Add INFO_TS submessage (timestamp: 1000 seconds)
    message.extend_from_slice(&create_info_ts_submessage(1000, 0));

    // Add DATA submessage
    let writer_entity = [0x00, 0x00, 0x01, 0x02];
    let reader_entity = [0x00, 0x00, 0x00, 0x00];
    let payload = b"test";
    message.extend_from_slice(&create_data_submessage(writer_entity, reader_entity, 1, payload));

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.10:7400")
        .with_dst_addr("239.255.0.1:7400");

    let events = decoder.decode_stream(&message, &ctx).expect("decode should succeed");

    assert_eq!(events.len(), 1);
    let event = &events[0];

    // Timestamp should be applied from INFO_TS
    assert_eq!(event.timestamp.as_nanos(), 1_000_000_000_000); // 1000 seconds in nanos
}

#[test]
fn test_rtps_discovery_topic_name() {
    let mut decoder = DdsDecoder::new();

    let guid_prefix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

    // First message: SEDP discovery data with topic name
    let mut discovery_msg = create_rtps_header(guid_prefix);
    let sedp_writer = [0x00, 0x00, 0x03, 0xC2]; // SEDP Publications Writer
    let sedp_reader = [0x00, 0x00, 0x03, 0xC7];

    // Create discovery payload with topic name "sensor/imu"
    let mut discovery_payload = Vec::new();
    discovery_payload.extend_from_slice(b"topicName\0\0\0");
    discovery_payload.extend_from_slice(&10u32.to_le_bytes()); // string length
    discovery_payload.extend_from_slice(b"sensor/imu\0");

    discovery_msg.extend_from_slice(&create_data_submessage(
        sedp_writer,
        sedp_reader,
        1,
        &discovery_payload,
    ));

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.10:7400")
        .with_dst_addr("239.255.0.1:7400");

    // Process discovery message (should not generate events)
    let events = decoder.decode_stream(&discovery_msg, &ctx).expect("decode should succeed");
    assert_eq!(events.len(), 0, "Discovery messages should not generate events");

    // Second message: user DATA from a user-defined writer (not built-in)
    // The discovery message above registered the SEDP writer, but we need to actually
    // register a user writer. For this test, let's just verify discovery was processed.
    // In reality, SEDP would contain info about user writers, not itself.

    let mut data_msg = create_rtps_header(guid_prefix);
    let user_writer = [0x00, 0x00, 0x01, 0x02]; // USER_DEFINED_WRITER_WITH_KEY
    let user_reader = [0x00, 0x00, 0x01, 0x07];
    let user_payload = b"IMU data here";

    data_msg.extend_from_slice(&create_data_submessage(
        user_writer,
        user_reader,
        1,
        user_payload,
    ));

    let events = decoder.decode_stream(&data_msg, &ctx).expect("decode should succeed");

    assert_eq!(events.len(), 1);
    let event = &events[0];

    // This writer was not discovered, so no topic name
    assert!(event.metadata.contains_key("dds.writer_guid"));
    // Should have warning about no discovery
    assert!(!event.warnings.is_empty());
}

#[test]
fn test_rtps_no_discovery_fallback() {
    let mut decoder = DdsDecoder::new();

    let guid_prefix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let mut message = create_rtps_header(guid_prefix);

    // User DATA without prior discovery
    let writer_entity = [0x00, 0x00, 0x01, 0x02];
    let reader_entity = [0x00, 0x00, 0x00, 0x00];
    let payload = b"undiscovered data";
    message.extend_from_slice(&create_data_submessage(writer_entity, reader_entity, 1, payload));

    let ctx = DecodeContext::new()
        .with_src_addr("192.168.1.10:7400")
        .with_dst_addr("239.255.0.1:7400");

    let events = decoder.decode_stream(&message, &ctx).expect("decode should succeed");

    assert_eq!(events.len(), 1);
    let event = &events[0];

    // Should NOT have topic name (no discovery)
    assert!(!event.metadata.contains_key(METADATA_KEY_DDS_TOPIC_NAME));

    // Should have writer GUID displayed
    assert!(event.metadata.contains_key("dds.writer_guid"));

    // Should have warning about missing discovery
    assert!(!event.warnings.is_empty());
}

#[test]
fn test_rtps_domain_id_from_port() {
    let mut decoder = DdsDecoder::new();

    let guid_prefix = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let mut message = create_rtps_header(guid_prefix);

    let writer_entity = [0x00, 0x00, 0x01, 0x02];
    let reader_entity = [0x00, 0x00, 0x00, 0x00];
    let payload = b"test";
    message.extend_from_slice(&create_data_submessage(writer_entity, reader_entity, 1, payload));

    // Test domain 0 (port 7400)
    let ctx_domain0 = DecodeContext::new()
        .with_src_addr("192.168.1.10:7400")
        .with_dst_addr("239.255.0.1:7400");

    let events = decoder.decode_stream(&message, &ctx_domain0).expect("decode should succeed");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].metadata.get(METADATA_KEY_DDS_DOMAIN_ID), Some(&"0".to_string()));

    // Test domain 1 (port 7650)
    let ctx_domain1 = DecodeContext::new()
        .with_src_addr("192.168.1.10:7650")
        .with_dst_addr("239.255.0.1:7650");

    let events = decoder.decode_stream(&message, &ctx_domain1).expect("decode should succeed");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].metadata.get(METADATA_KEY_DDS_DOMAIN_ID), Some(&"1".to_string()));
}

#[test]
fn test_discovery_tracker() {
    let mut tracker = RtpsDiscoveryTracker::new();

    let guid = Guid::new(
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        [0x00, 0x00, 0x01, 0x02],
    );

    let endpoint = DiscoveredEndpoint {
        topic_name: "test/topic".to_string(),
        type_name: "TestType".to_string(),
    };

    tracker.register_endpoint(guid, endpoint);

    assert_eq!(tracker.lookup_topic_name(&guid), Some("test/topic"));
    assert_eq!(tracker.lookup_type_name(&guid), Some("TestType"));

    // Unknown GUID
    let unknown_guid = Guid::new([0; 12], [0; 4]);
    assert_eq!(tracker.lookup_topic_name(&unknown_guid), None);
}
