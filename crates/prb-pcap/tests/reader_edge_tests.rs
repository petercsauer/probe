//! Edge case tests for PCAP reader.

use prb_pcap::TlsKeyStore;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_tls_keystore_operations() {
    let mut store = TlsKeyStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);

    // Insert a key
    let client_random = vec![0xAA; 32];
    let key_material = vec![0xBB; 48];
    store.insert(client_random.clone(), key_material.clone());

    assert!(!store.is_empty());
    assert_eq!(store.len(), 1);

    // Lookup the key
    let result = store.get(&client_random);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), &key_material[..]);

    // Lookup non-existent key
    let other_random = vec![0xCC; 32];
    assert!(store.get(&other_random).is_none());

    // Test iterator
    let mut count = 0;
    for (cr, km) in store.iter() {
        assert_eq!(cr, &client_random[..]);
        assert_eq!(km, &key_material[..]);
        count += 1;
    }
    assert_eq!(count, 1);

    // Insert another key
    store.insert(other_random.clone(), vec![0xDD; 48]);
    assert_eq!(store.len(), 2);
}

#[test]
fn test_tls_keystore_overwrite() {
    let mut store = TlsKeyStore::new();
    let client_random = vec![0xAA; 32];

    // Insert initial key
    store.insert(client_random.clone(), vec![0xBB; 48]);
    assert_eq!(store.len(), 1);

    // Overwrite with new key material
    store.insert(client_random.clone(), vec![0xCC; 48]);
    assert_eq!(store.len(), 1, "Should still have 1 entry after overwrite");

    // Verify new value
    let result = store.get(&client_random);
    assert_eq!(result.unwrap(), &vec![0xCC; 48][..]);
}

#[test]
fn test_read_pcap_invalid_magic() {
    // Create a file with invalid magic bytes
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("invalid.pcap");

    let mut file = File::create(&pcap_path).unwrap();
    file.write_all(&[0xFF, 0xFF, 0xFF, 0xFF]).unwrap();
    file.flush().unwrap();
    drop(file);

    // Try to open - should fail with unsupported format
    let result = prb_pcap::PcapFileReader::open(pcap_path);
    assert!(
        result.is_err(),
        "Should fail to open file with invalid magic"
    );
}

#[test]
fn test_read_nonexistent_file() {
    // Try to open a file that doesn't exist
    let path = PathBuf::from("/tmp/nonexistent_file_test_12345.pcap");
    let result = prb_pcap::PcapFileReader::open(path);
    assert!(result.is_err(), "Should fail to open nonexistent file");
}

#[test]
fn test_read_empty_file() {
    // Create an empty file
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("empty.pcap");

    File::create(&pcap_path).unwrap();

    // Try to open - should fail
    let result = prb_pcap::PcapFileReader::open(pcap_path);
    assert!(result.is_err(), "Should fail to open empty file");
}

#[test]
fn test_read_truncated_header() {
    // Create a file with only 2 bytes (less than magic bytes needed)
    let temp_dir = TempDir::new().unwrap();
    let pcap_path = temp_dir.path().join("truncated.pcap");

    let mut file = File::create(&pcap_path).unwrap();
    file.write_all(&[0xd4, 0xc3]).unwrap();
    file.flush().unwrap();
    drop(file);

    // Try to open - should fail to read magic bytes
    let result = prb_pcap::PcapFileReader::open(pcap_path);
    assert!(result.is_err(), "Should fail to read truncated header");
}
