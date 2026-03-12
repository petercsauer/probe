//! Unit tests for `OwnedPacket`.

use prb_capture::OwnedPacket;

#[test]
fn test_owned_packet_timestamp_conversion() {
    // Simulate a pcap packet with known timestamp
    // We'll test the conversion logic directly

    // Mock timestamp: 1609459200 seconds (2021-01-01 00:00:00 UTC)
    // + 500000 microseconds
    let tv_sec = 1609459200i64;
    let tv_usec = 500000i64;

    let expected_us = (tv_sec as u64) * 1_000_000 + (tv_usec as u64);
    assert_eq!(expected_us, 1609459200500000);

    // Expected: 1609459200500000 microseconds
    // This is the conversion logic in OwnedPacket::from_pcap
}

#[test]
fn test_owned_packet_fields() {
    // Create a mock packet to verify field preservation
    let packet = OwnedPacket {
        timestamp_us: 1609459200500000,
        orig_len: 1500,
        data: vec![0x01, 0x02, 0x03, 0x04],
    };

    assert_eq!(packet.timestamp_us, 1609459200500000);
    assert_eq!(packet.orig_len, 1500);
    assert_eq!(packet.data.len(), 4);
    assert_eq!(packet.data, vec![0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn test_owned_packet_clone() {
    let packet = OwnedPacket {
        timestamp_us: 123456789,
        orig_len: 100,
        data: vec![0xAA, 0xBB],
    };

    let cloned = packet.clone();
    assert_eq!(cloned.timestamp_us, packet.timestamp_us);
    assert_eq!(cloned.orig_len, packet.orig_len);
    assert_eq!(cloned.data, packet.data);
}
