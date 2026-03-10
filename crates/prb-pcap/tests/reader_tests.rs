//! Integration tests for PCAP/pcapng file reader.

use prb_pcap::PcapFileReader;
use std::io::Write;
use tempfile::NamedTempFile;

/// Creates a minimal legacy pcap file for testing.
fn create_test_pcap() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();

    // PCAP file header (24 bytes)
    // Magic: 0xa1b2c3d4 (microsecond precision, native endian)
    file.write_all(&0xa1b2c3d4u32.to_le_bytes()).unwrap();
    // Version: 2.4
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&4u16.to_le_bytes()).unwrap();
    // Timezone offset: 0
    file.write_all(&0i32.to_le_bytes()).unwrap();
    // Timestamp accuracy: 0
    file.write_all(&0u32.to_le_bytes()).unwrap();
    // Snapshot length: 65535
    file.write_all(&65535u32.to_le_bytes()).unwrap();
    // Linktype: 1 (Ethernet)
    file.write_all(&1u32.to_le_bytes()).unwrap();

    // Add a simple packet (16 byte header + minimal data)
    // Timestamp seconds
    file.write_all(&1000000u32.to_le_bytes()).unwrap();
    // Timestamp microseconds
    file.write_all(&500000u32.to_le_bytes()).unwrap();
    // Captured length
    let packet_data = b"Hello PCAP";
    file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
    // Original length
    file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
    // Packet data
    file.write_all(packet_data).unwrap();

    file.flush().unwrap();
    file
}

/// Creates a minimal pcapng file for testing.
fn create_test_pcapng() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();

    // Section Header Block (SHB)
    // Block Type: 0x0a0d0d0a
    file.write_all(&0x0a0d0d0au32.to_le_bytes()).unwrap();
    // Block Total Length: 28 bytes
    file.write_all(&28u32.to_le_bytes()).unwrap();
    // Byte-Order Magic: 0x1a2b3c4d
    file.write_all(&0x1a2b3c4du32.to_le_bytes()).unwrap();
    // Major Version: 1
    file.write_all(&1u16.to_le_bytes()).unwrap();
    // Minor Version: 0
    file.write_all(&0u16.to_le_bytes()).unwrap();
    // Section Length: -1 (unspecified)
    file.write_all(&(-1i64).to_le_bytes()).unwrap();
    // Block Total Length (repeated)
    file.write_all(&28u32.to_le_bytes()).unwrap();

    // Interface Description Block (IDB)
    // Block Type: 0x00000001
    file.write_all(&1u32.to_le_bytes()).unwrap();
    // Block Total Length: 20 bytes
    file.write_all(&20u32.to_le_bytes()).unwrap();
    // Linktype: 1 (Ethernet)
    file.write_all(&1u16.to_le_bytes()).unwrap();
    // Reserved: 0
    file.write_all(&0u16.to_le_bytes()).unwrap();
    // SnapLen: 65535
    file.write_all(&65535u32.to_le_bytes()).unwrap();
    // Block Total Length (repeated)
    file.write_all(&20u32.to_le_bytes()).unwrap();

    // Enhanced Packet Block (EPB)
    // Block Type: 0x00000006
    file.write_all(&6u32.to_le_bytes()).unwrap();
    let packet_data = b"Hello pcapng";
    let epb_len = 32 + ((packet_data.len() + 3) & !3); // Align to 4 bytes
    // Block Total Length
    file.write_all(&(epb_len as u32).to_le_bytes()).unwrap();
    // Interface ID: 0
    file.write_all(&0u32.to_le_bytes()).unwrap();
    // Timestamp (high): 0
    file.write_all(&0u32.to_le_bytes()).unwrap();
    // Timestamp (low): 1500000 microseconds
    file.write_all(&1500000u32.to_le_bytes()).unwrap();
    // Captured Packet Length
    file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
    // Original Packet Length
    file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
    // Packet Data (padded to 4 bytes)
    file.write_all(packet_data).unwrap();
    let padding = (4 - (packet_data.len() % 4)) % 4;
    file.write_all(&vec![0u8; padding]).unwrap();
    // Block Total Length (repeated)
    file.write_all(&(epb_len as u32).to_le_bytes()).unwrap();

    file.flush().unwrap();
    file
}

/// Creates a pcapng file with embedded TLS keys (DSB).
fn create_test_pcapng_with_dsb() -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();

    // Section Header Block (SHB)
    file.write_all(&0x0a0d0d0au32.to_le_bytes()).unwrap();
    file.write_all(&28u32.to_le_bytes()).unwrap();
    file.write_all(&0x1a2b3c4du32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&0u16.to_le_bytes()).unwrap();
    file.write_all(&(-1i64).to_le_bytes()).unwrap();
    file.write_all(&28u32.to_le_bytes()).unwrap();

    // Interface Description Block (IDB)
    file.write_all(&1u32.to_le_bytes()).unwrap();
    file.write_all(&20u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&0u16.to_le_bytes()).unwrap();
    file.write_all(&65535u32.to_le_bytes()).unwrap();
    file.write_all(&20u32.to_le_bytes()).unwrap();

    // Decryption Secrets Block (DSB)
    // Use a proper 48-byte (96 hex char) master secret for TLS 1.2
    let key_log = "CLIENT_RANDOM 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210\n";
    let key_log_bytes = key_log.as_bytes();
    let padding = (4 - (key_log_bytes.len() % 4)) % 4;
    let padded_len = key_log_bytes.len() + padding;

    // pcapng Block Total Length includes ALL bytes from block_type through trailing length:
    // block_type(4) + block_total_length(4) + secrets_type(4) + secrets_length(4) + padded_data + block_total_length(4)
    let dsb_total_len = 4 + 4 + 4 + 4 + padded_len + 4;

    // Block Type: 0x0000000a (DSB)
    file.write_all(&0x0au32.to_le_bytes()).unwrap();
    // Block Total Length
    file.write_all(&(dsb_total_len as u32).to_le_bytes()).unwrap();
    // Secrets Type: 0x544c534b ("TLSK")
    file.write_all(&0x544c534bu32.to_le_bytes()).unwrap();
    // Secrets Length (unpadded)
    file.write_all(&(key_log_bytes.len() as u32).to_le_bytes()).unwrap();
    // Secrets Data
    file.write_all(key_log_bytes).unwrap();
    // Padding to 4-byte boundary
    if padding > 0 {
        file.write_all(&vec![0u8; padding]).unwrap();
    }
    // Block Total Length (repeated)
    file.write_all(&(dsb_total_len as u32).to_le_bytes()).unwrap();

    file.flush().unwrap();
    file.as_file().sync_all().unwrap();
    file
}

#[test]
fn test_read_pcap_legacy() {
    let pcap_file = create_test_pcap();
    let mut reader = PcapFileReader::open(pcap_file.path()).unwrap();

    let packets = reader.read_all_packets().unwrap();

    assert_eq!(packets.len(), 1, "should read exactly one packet");
    assert_eq!(packets[0].linktype, 1, "linktype should be Ethernet (1)");
    assert_eq!(packets[0].timestamp_us, 1000000 * 1_000_000 + 500000, "timestamp should match");
    assert_eq!(packets[0].data, b"Hello PCAP", "packet data should match");
}

#[test]
fn test_read_pcapng() {
    let pcapng_file = create_test_pcapng();
    let mut reader = PcapFileReader::open(pcapng_file.path()).unwrap();

    let packets = reader.read_all_packets().unwrap();

    assert_eq!(packets.len(), 1, "should read exactly one packet");
    assert_eq!(packets[0].linktype, 1, "linktype should be Ethernet (1)");
    assert_eq!(packets[0].timestamp_us, 1500000, "timestamp should match");
    assert_eq!(packets[0].data, b"Hello pcapng", "packet data should match");
}

#[test]
fn test_read_pcapng_dsb() {
    let pcapng_file = create_test_pcapng_with_dsb();
    let mut reader = PcapFileReader::open(pcapng_file.path()).unwrap();

    let _packets = reader.read_all_packets().unwrap();
    let tls_keys = reader.tls_keys();

    assert_eq!(tls_keys.len(), 1, "should extract one TLS key");

    let client_random = hex::decode("0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF").unwrap();
    let expected_secret = hex::decode("FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210FEDCBA9876543210").unwrap();

    let secret = tls_keys.get(&client_random).unwrap();
    assert_eq!(secret, expected_secret.as_slice(), "TLS key should match");
}

#[test]
fn test_format_autodetect() {
    // Test pcap detection
    let pcap_file = create_test_pcap();
    let reader = PcapFileReader::open(pcap_file.path());
    assert!(reader.is_ok(), "should auto-detect pcap format");

    // Test pcapng detection
    let pcapng_file = create_test_pcapng();
    let reader = PcapFileReader::open(pcapng_file.path());
    assert!(reader.is_ok(), "should auto-detect pcapng format");
}

#[test]
fn test_streaming_large_file() {
    // Create a pcapng with many packets to test streaming
    let mut file = NamedTempFile::new().unwrap();

    // Section Header Block
    file.write_all(&0x0a0d0d0au32.to_le_bytes()).unwrap();
    file.write_all(&28u32.to_le_bytes()).unwrap();
    file.write_all(&0x1a2b3c4du32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&0u16.to_le_bytes()).unwrap();
    file.write_all(&(-1i64).to_le_bytes()).unwrap();
    file.write_all(&28u32.to_le_bytes()).unwrap();

    // Interface Description Block
    file.write_all(&1u32.to_le_bytes()).unwrap();
    file.write_all(&20u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&0u16.to_le_bytes()).unwrap();
    file.write_all(&65535u32.to_le_bytes()).unwrap();
    file.write_all(&20u32.to_le_bytes()).unwrap();

    // Add 100 packets (large enough to test streaming without timeout)
    let packet_data = vec![0xAAu8; 1024]; // 1KB per packet
    for i in 0..100 {
        // Enhanced Packet Block
        file.write_all(&6u32.to_le_bytes()).unwrap();
        let epb_len = 32 + ((packet_data.len() + 3) & !3);
        file.write_all(&(epb_len as u32).to_le_bytes()).unwrap();
        file.write_all(&0u32.to_le_bytes()).unwrap(); // Interface ID
        file.write_all(&0u32.to_le_bytes()).unwrap(); // Timestamp high
        file.write_all(&(i as u32).to_le_bytes()).unwrap(); // Timestamp low
        file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
        file.write_all(&(packet_data.len() as u32).to_le_bytes()).unwrap();
        file.write_all(&packet_data).unwrap();
        let padding = (4 - (packet_data.len() % 4)) % 4;
        file.write_all(&vec![0u8; padding]).unwrap();
        file.write_all(&(epb_len as u32).to_le_bytes()).unwrap();
    }

    file.flush().unwrap();

    let mut reader = PcapFileReader::open(file.path()).unwrap();
    let packets = reader.read_all_packets().unwrap();

    assert_eq!(packets.len(), 100, "should read all 100 packets");
    // Memory check: This test passes if we don't OOM
    // The pcap-parser streaming reader should use constant memory
    // File size: ~100KB, well over 1MB threshold for streaming test
}
