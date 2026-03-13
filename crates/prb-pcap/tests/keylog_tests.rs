//! Comprehensive tests for TLS keylog parsing and edge cases.

use prb_pcap::tls::keylog::{KeyMaterial, TlsKeyLog};
use std::io::Write;
use tempfile::NamedTempFile;

mod corpus;

use proptest::prelude::*;
use rstest::rstest;

#[test]
fn test_keylog_empty_file() {
    let tmpfile = NamedTempFile::new().unwrap();
    let keylog = TlsKeyLog::from_file(tmpfile.path()).unwrap();
    assert_eq!(keylog.len(), 0);
    assert!(keylog.is_empty());
}

#[test]
fn test_keylog_comments_only() {
    let mut tmpfile = NamedTempFile::new().unwrap();
    writeln!(tmpfile, "# This is a comment").unwrap();
    writeln!(tmpfile, "# Another comment").unwrap();
    writeln!(tmpfile).unwrap();
    writeln!(tmpfile, "   # Comment with leading spaces").unwrap();

    let keylog = TlsKeyLog::from_file(tmpfile.path()).unwrap();
    assert_eq!(keylog.len(), 0);
}

#[test]
fn test_keylog_malformed_lines() {
    let mut tmpfile = NamedTempFile::new().unwrap();

    // Valid line first
    writeln!(
        tmpfile,
        "CLIENT_RANDOM {} {}",
        "aa".repeat(32),
        "bb".repeat(48)
    )
    .unwrap();

    // Malformed lines (should be ignored)
    writeln!(
        tmpfile,
        "INVALID_LABEL {} {}",
        "aa".repeat(32),
        "bb".repeat(48)
    )
    .unwrap();
    writeln!(tmpfile, "CLIENT_RANDOM").unwrap(); // Too few fields
    writeln!(tmpfile, "CLIENT_RANDOM {}", "aa".repeat(32)).unwrap(); // Missing key material
    writeln!(tmpfile, "NOT_A_KEY_LOG_LINE").unwrap();

    let keylog = TlsKeyLog::from_file(tmpfile.path()).unwrap();
    assert_eq!(keylog.len(), 1, "Should only parse 1 valid line");
}

#[test]
fn test_keylog_invalid_hex() {
    let mut keylog = TlsKeyLog::new();

    // Invalid hex in client_random
    let result = keylog.parse_line(&format!("CLIENT_RANDOM ZZZZZZ {}", "bb".repeat(48)));
    assert!(result.is_err());

    // Invalid hex in key material
    let result = keylog.parse_line(&format!("CLIENT_RANDOM {} ZZZZZZ", "aa".repeat(32)));
    assert!(result.is_err());
}

#[test]
fn test_keylog_wrong_lengths() {
    let mut keylog = TlsKeyLog::new();

    // client_random too short
    let result = keylog.parse_line(&format!(
        "CLIENT_RANDOM {} {}",
        "aa".repeat(16),
        "bb".repeat(48)
    ));
    assert!(result.is_err());

    // client_random too long
    let result = keylog.parse_line(&format!(
        "CLIENT_RANDOM {} {}",
        "aa".repeat(64),
        "bb".repeat(48)
    ));
    assert!(result.is_err());

    // TLS 1.2 master secret wrong length (not 48 bytes)
    let result = keylog.parse_line(&format!(
        "CLIENT_RANDOM {} {}",
        "aa".repeat(32),
        "bb".repeat(32)
    ));
    assert!(result.is_err());

    // TLS 1.3 traffic secret wrong length (not 32 or 48 bytes)
    let result = keylog.parse_line(&format!(
        "CLIENT_TRAFFIC_SECRET_0 {} {}",
        "aa".repeat(32),
        "bb".repeat(16)
    ));
    assert!(result.is_err());
}

#[test]
fn test_keylog_all_tls13_labels() {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(32);

    // All 4 TLS 1.3 traffic secret types
    keylog
        .parse_line(&format!(
            "CLIENT_HANDSHAKE_TRAFFIC_SECRET {} {}",
            cr,
            "bb".repeat(32)
        ))
        .unwrap();
    keylog
        .parse_line(&format!(
            "SERVER_HANDSHAKE_TRAFFIC_SECRET {} {}",
            cr,
            "cc".repeat(32)
        ))
        .unwrap();
    keylog
        .parse_line(&format!(
            "CLIENT_TRAFFIC_SECRET_0 {} {}",
            cr,
            "dd".repeat(32)
        ))
        .unwrap();
    keylog
        .parse_line(&format!(
            "SERVER_TRAFFIC_SECRET_0 {} {}",
            cr,
            "ee".repeat(32)
        ))
        .unwrap();

    assert_eq!(keylog.len(), 1, "All should map to same client_random");

    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 4);

    // Verify each type exists
    assert!(
        materials
            .iter()
            .any(|m| matches!(m, KeyMaterial::ClientHandshakeTrafficSecret(_)))
    );
    assert!(
        materials
            .iter()
            .any(|m| matches!(m, KeyMaterial::ServerHandshakeTrafficSecret(_)))
    );
    assert!(
        materials
            .iter()
            .any(|m| matches!(m, KeyMaterial::ClientTrafficSecret0(_)))
    );
    assert!(
        materials
            .iter()
            .any(|m| matches!(m, KeyMaterial::ServerTrafficSecret0(_)))
    );
}

#[test]
fn test_keylog_tls13_48byte_secrets() {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(32);

    // TLS 1.3 with 48-byte secrets (for AES-256-GCM)
    keylog
        .parse_line(&format!(
            "CLIENT_TRAFFIC_SECRET_0 {} {}",
            cr,
            "bb".repeat(48)
        ))
        .unwrap();
    keylog
        .parse_line(&format!(
            "SERVER_TRAFFIC_SECRET_0 {} {}",
            cr,
            "cc".repeat(48)
        ))
        .unwrap();

    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 2);
    assert_eq!(materials[0].as_bytes().len(), 48);
    assert_eq!(materials[1].as_bytes().len(), 48);
}

#[test]
fn test_keylog_mixed_tls12_tls13() {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(32);

    // Mix TLS 1.2 and TLS 1.3 keys (unusual but valid)
    keylog
        .parse_line(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(48)))
        .unwrap();
    keylog
        .parse_line(&format!(
            "CLIENT_TRAFFIC_SECRET_0 {} {}",
            cr,
            "cc".repeat(32)
        ))
        .unwrap();

    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 2);
    assert!(
        materials
            .iter()
            .any(prb_pcap::tls::keylog::KeyMaterial::is_tls12)
    );
    assert!(
        materials
            .iter()
            .any(prb_pcap::tls::keylog::KeyMaterial::is_tls13)
    );
}

#[test]
fn test_keylog_lookup_missing() {
    let keylog = TlsKeyLog::new();
    let cr = hex::decode("aa".repeat(32)).unwrap();
    assert!(keylog.lookup(&cr).is_none());
}

#[test]
fn test_keylog_lookup_for_direction() {
    use prb_pcap::tcp::StreamDirection;

    let mut keylog = TlsKeyLog::new();
    let cr_hex = "aa".repeat(32);
    let cr = hex::decode(&cr_hex).unwrap();

    // TLS 1.3 with both client and server secrets
    keylog
        .parse_line(&format!(
            "CLIENT_TRAFFIC_SECRET_0 {} {}",
            cr_hex,
            "bb".repeat(32)
        ))
        .unwrap();
    keylog
        .parse_line(&format!(
            "SERVER_TRAFFIC_SECRET_0 {} {}",
            cr_hex,
            "cc".repeat(32)
        ))
        .unwrap();

    // Lookup by direction
    let client_key = keylog
        .lookup_for_direction(&cr, StreamDirection::ClientToServer)
        .unwrap();
    assert!(matches!(client_key, KeyMaterial::ClientTrafficSecret0(_)));

    let server_key = keylog
        .lookup_for_direction(&cr, StreamDirection::ServerToClient)
        .unwrap();
    assert!(matches!(server_key, KeyMaterial::ServerTrafficSecret0(_)));
}

#[test]
fn test_keylog_lookup_for_direction_tls12() {
    use prb_pcap::tcp::StreamDirection;

    let mut keylog = TlsKeyLog::new();
    let cr_hex = "aa".repeat(32);
    let cr = hex::decode(&cr_hex).unwrap();

    // TLS 1.2 with master secret only
    keylog
        .parse_line(&format!("CLIENT_RANDOM {} {}", cr_hex, "bb".repeat(48)))
        .unwrap();

    // TLS 1.2 master secret works for both directions
    let client_key = keylog
        .lookup_for_direction(&cr, StreamDirection::ClientToServer)
        .unwrap();
    assert!(client_key.is_tls12());

    let server_key = keylog
        .lookup_for_direction(&cr, StreamDirection::ServerToClient)
        .unwrap();
    assert!(server_key.is_tls12());
}

#[test]
fn test_keylog_insert_api() {
    let mut keylog = TlsKeyLog::new();
    let cr = vec![0xaa; 32];

    keylog.insert(cr.clone(), KeyMaterial::MasterSecret(vec![0xbb; 48]));
    keylog.insert(
        cr.clone(),
        KeyMaterial::ClientTrafficSecret0(vec![0xcc; 32]),
    );

    let materials = keylog.lookup(&cr).unwrap();
    assert_eq!(materials.len(), 2);
}

#[test]
fn test_keylog_merge_dsb_empty() {
    let mut keylog = TlsKeyLog::new();
    keylog.merge_dsb_keys(b"").unwrap();
    assert_eq!(keylog.len(), 0);
}

#[test]
fn test_keylog_merge_dsb_with_comments() {
    let mut keylog = TlsKeyLog::new();
    let dsb_data = format!(
        "# TLS Key Log\n\nCLIENT_RANDOM {} {}\n# End\n",
        "aa".repeat(32),
        "bb".repeat(48)
    );
    keylog.merge_dsb_keys(dsb_data.as_bytes()).unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn test_keylog_merge_dsb_invalid_utf8() {
    let mut keylog = TlsKeyLog::new();
    let invalid_utf8 = vec![0xff, 0xfe, 0xfd];
    let result = keylog.merge_dsb_keys(&invalid_utf8);
    assert!(result.is_err());
}

#[test]
fn test_keylog_multiple_client_randoms() {
    let mut keylog = TlsKeyLog::new();

    // Different client randoms
    for i in 0..10 {
        let cr = format!("{i:02x}").repeat(32);
        keylog
            .parse_line(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(48)))
            .unwrap();
    }

    assert_eq!(keylog.len(), 10);
}

#[test]
fn test_keylog_whitespace_handling() {
    let mut keylog = TlsKeyLog::new();

    // Leading/trailing whitespace
    keylog
        .parse_line(&format!(
            "  CLIENT_RANDOM {} {}  ",
            "aa".repeat(32),
            "bb".repeat(48)
        ))
        .unwrap();

    // Tabs
    keylog
        .parse_line(&format!(
            "CLIENT_RANDOM\t{}\t{}",
            "cc".repeat(32),
            "dd".repeat(48)
        ))
        .unwrap();

    assert_eq!(keylog.len(), 2);
}

#[test]
fn test_key_material_as_bytes() {
    let master = KeyMaterial::MasterSecret(vec![0xaa; 48]);
    assert_eq!(master.as_bytes().len(), 48);

    let client_secret = KeyMaterial::ClientTrafficSecret0(vec![0xbb; 32]);
    assert_eq!(client_secret.as_bytes().len(), 32);
}

#[test]
fn test_key_material_type_checks() {
    let master = KeyMaterial::MasterSecret(vec![0xaa; 48]);
    assert!(master.is_tls12());
    assert!(!master.is_tls13());

    let client_secret = KeyMaterial::ClientTrafficSecret0(vec![0xbb; 32]);
    assert!(!client_secret.is_tls12());
    assert!(client_secret.is_tls13());

    let server_secret = KeyMaterial::ServerTrafficSecret0(vec![0xcc; 32]);
    assert!(server_secret.is_tls13());

    let client_hs = KeyMaterial::ClientHandshakeTrafficSecret(vec![0xdd; 32]);
    assert!(client_hs.is_tls13());

    let server_hs = KeyMaterial::ServerHandshakeTrafficSecret(vec![0xee; 32]);
    assert!(server_hs.is_tls13());
}

// =============================================================================
// Parameterized Tests with rstest
// =============================================================================

/// Test all TLS 1.3 key types with 32-byte secrets
#[rstest]
#[case::client_traffic("CLIENT_TRAFFIC_SECRET_0", 32)]
#[case::server_traffic("SERVER_TRAFFIC_SECRET_0", 32)]
#[case::client_handshake("CLIENT_HANDSHAKE_TRAFFIC_SECRET", 32)]
#[case::server_handshake("SERVER_HANDSHAKE_TRAFFIC_SECRET", 32)]
fn keylog_rstest_tls13_32byte(#[case] label: &str, #[case] expected_len: usize) {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(32);
    let key = "bb".repeat(expected_len);

    keylog
        .parse_line(&format!("{} {} {}", label, cr, key))
        .unwrap();

    assert_eq!(keylog.len(), 1);
    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 1);
    assert_eq!(materials[0].as_bytes().len(), expected_len);
    assert!(materials[0].is_tls13());
}

/// Test all TLS 1.3 key types with 48-byte secrets
#[rstest]
#[case::client_traffic("CLIENT_TRAFFIC_SECRET_0", 48)]
#[case::server_traffic("SERVER_TRAFFIC_SECRET_0", 48)]
#[case::client_handshake("CLIENT_HANDSHAKE_TRAFFIC_SECRET", 48)]
#[case::server_handshake("SERVER_HANDSHAKE_TRAFFIC_SECRET", 48)]
fn keylog_rstest_tls13_48byte(#[case] label: &str, #[case] expected_len: usize) {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(32);
    let key = "bb".repeat(expected_len);

    keylog
        .parse_line(&format!("{} {} {}", label, cr, key))
        .unwrap();

    assert_eq!(keylog.len(), 1);
    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 1);
    assert_eq!(materials[0].as_bytes().len(), expected_len);
    assert!(materials[0].is_tls13());
}

/// Test TLS 1.2 master secret (must be exactly 48 bytes)
#[rstest]
#[case::master_secret("CLIENT_RANDOM", 48)]
fn keylog_rstest_tls12(#[case] label: &str, #[case] expected_len: usize) {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(32);
    let key = "bb".repeat(expected_len);

    keylog
        .parse_line(&format!("{} {} {}", label, cr, key))
        .unwrap();

    assert_eq!(keylog.len(), 1);
    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 1);
    assert_eq!(materials[0].as_bytes().len(), expected_len);
    assert!(materials[0].is_tls12());
}

/// Test various invalid key lengths for all key types
#[rstest]
#[case::tls12_wrong_len("CLIENT_RANDOM", 32)] // TLS 1.2 must be 48
#[case::tls12_wrong_len("CLIENT_RANDOM", 64)]
#[case::tls13_wrong_len("CLIENT_TRAFFIC_SECRET_0", 31)] // TLS 1.3 must be 32 or 48
#[case::tls13_wrong_len("CLIENT_TRAFFIC_SECRET_0", 33)]
#[case::tls13_wrong_len("CLIENT_TRAFFIC_SECRET_0", 47)]
#[case::tls13_wrong_len("CLIENT_TRAFFIC_SECRET_0", 49)]
#[case::tls13_wrong_len("SERVER_TRAFFIC_SECRET_0", 16)]
#[case::tls13_wrong_len("CLIENT_HANDSHAKE_TRAFFIC_SECRET", 64)]
#[case::tls13_wrong_len("SERVER_HANDSHAKE_TRAFFIC_SECRET", 100)]
fn keylog_rstest_invalid_lengths(#[case] label: &str, #[case] invalid_len: usize) {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(32);
    let key = "bb".repeat(invalid_len);

    let result = keylog.parse_line(&format!("{} {} {}", label, cr, key));
    assert!(
        result.is_err(),
        "Expected error for {} with {} bytes",
        label,
        invalid_len
    );
}

/// Test invalid client_random lengths
#[rstest]
#[case::too_short(16)]
#[case::too_short(31)]
#[case::too_long(33)]
#[case::too_long(64)]
fn keylog_rstest_invalid_client_random_length(#[case] cr_len: usize) {
    let mut keylog = TlsKeyLog::new();
    let cr = "aa".repeat(cr_len);
    let key = "bb".repeat(48);

    let result = keylog.parse_line(&format!("CLIENT_RANDOM {} {}", cr, key));
    assert!(
        result.is_err(),
        "Expected error for client_random with {} bytes",
        cr_len
    );
}

// =============================================================================
// Property Tests with proptest
// =============================================================================

proptest! {
    /// Property test: Valid TLS 1.2 keylog lines should parse successfully
    #[test]
    fn keylog_property_tls12_roundtrip(
        client_random in "[0-9a-f]{64}",
        master_secret in "[0-9a-f]{96}"
    ) {
        let line = format!("CLIENT_RANDOM {} {}", client_random, master_secret);
        let mut keylog = TlsKeyLog::new();
        let result = keylog.parse_line(&line);

        prop_assert!(result.is_ok(), "Failed to parse valid TLS 1.2 line: {}", line);
        prop_assert_eq!(keylog.len(), 1);

        let cr_bytes = hex::decode(&client_random).unwrap();
        let materials = keylog.lookup(&cr_bytes).unwrap();
        prop_assert_eq!(materials.len(), 1);
        prop_assert!(materials[0].is_tls12());
        prop_assert_eq!(materials[0].as_bytes().len(), 48);
    }

    /// Property test: Valid TLS 1.3 32-byte secrets should parse successfully
    #[test]
    fn keylog_property_tls13_32byte_roundtrip(
        client_random in "[0-9a-f]{64}",
        traffic_secret in "[0-9a-f]{64}"
    ) {
        let line = format!("CLIENT_TRAFFIC_SECRET_0 {} {}", client_random, traffic_secret);
        let mut keylog = TlsKeyLog::new();
        let result = keylog.parse_line(&line);

        prop_assert!(result.is_ok(), "Failed to parse valid TLS 1.3 32-byte line: {}", line);
        prop_assert_eq!(keylog.len(), 1);

        let cr_bytes = hex::decode(&client_random).unwrap();
        let materials = keylog.lookup(&cr_bytes).unwrap();
        prop_assert_eq!(materials.len(), 1);
        prop_assert!(materials[0].is_tls13());
        prop_assert_eq!(materials[0].as_bytes().len(), 32);
    }

    /// Property test: Valid TLS 1.3 48-byte secrets should parse successfully
    #[test]
    fn keylog_property_tls13_48byte_roundtrip(
        client_random in "[0-9a-f]{64}",
        traffic_secret in "[0-9a-f]{96}"
    ) {
        let line = format!("SERVER_TRAFFIC_SECRET_0 {} {}", client_random, traffic_secret);
        let mut keylog = TlsKeyLog::new();
        let result = keylog.parse_line(&line);

        prop_assert!(result.is_ok(), "Failed to parse valid TLS 1.3 48-byte line: {}", line);
        prop_assert_eq!(keylog.len(), 1);

        let cr_bytes = hex::decode(&client_random).unwrap();
        let materials = keylog.lookup(&cr_bytes).unwrap();
        prop_assert_eq!(materials.len(), 1);
        prop_assert!(materials[0].is_tls13());
        prop_assert_eq!(materials[0].as_bytes().len(), 48);
    }

    /// Property test: Invalid hex strings should be rejected
    #[test]
    fn keylog_property_invalid_hex_rejected(
        invalid_hex in "[G-Zg-z]{10,20}",  // Non-hex characters
        valid_client_random in "[0-9a-f]{64}",
        valid_master_secret in "[0-9a-f]{96}"
    ) {
        let mut keylog = TlsKeyLog::new();

        // Invalid client_random with valid key
        let line = format!("CLIENT_RANDOM {} {}", invalid_hex, valid_master_secret);
        let result = keylog.parse_line(&line);
        prop_assert!(result.is_err(), "Should reject invalid hex in client_random");

        // Valid client_random with invalid key material
        let line = format!("CLIENT_RANDOM {} {}", valid_client_random, invalid_hex);
        let result = keylog.parse_line(&line);
        prop_assert!(result.is_err(), "Should reject invalid hex in key material");
    }

    /// Property test: Comments and empty lines should always be ignored
    #[test]
    fn keylog_property_comments_ignored(comment in "[ \t]*#[^\n]*") {
        let mut keylog = TlsKeyLog::new();
        let result = keylog.parse_line(&comment);

        prop_assert!(result.is_ok(), "Comments should not cause errors");
        prop_assert_eq!(keylog.len(), 0, "Comments should not add keys");
    }

    /// Property test: Multiple keys for same client_random should accumulate
    #[test]
    fn keylog_property_multiple_keys_accumulate(
        client_random in "[0-9a-f]{64}",
        secret1 in "[0-9a-f]{64}",
        secret2 in "[0-9a-f]{64}",
        secret3 in "[0-9a-f]{64}"
    ) {
        let mut keylog = TlsKeyLog::new();
        let cr_bytes = hex::decode(&client_random).unwrap();

        // Add three different TLS 1.3 secrets for same client_random
        keylog.parse_line(&format!("CLIENT_TRAFFIC_SECRET_0 {} {}", client_random, secret1)).unwrap();
        keylog.parse_line(&format!("SERVER_TRAFFIC_SECRET_0 {} {}", client_random, secret2)).unwrap();
        keylog.parse_line(&format!("CLIENT_HANDSHAKE_TRAFFIC_SECRET {} {}", client_random, secret3)).unwrap();

        prop_assert_eq!(keylog.len(), 1, "All keys should map to same client_random");

        let materials = keylog.lookup(&cr_bytes).unwrap();
        prop_assert_eq!(materials.len(), 3, "Should have 3 key materials");
    }
}
