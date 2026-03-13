//! Comprehensive malformed input corpus for TLS keylog parser.
//!
//! This test suite contains 50+ malformed input cases to verify that the keylog
//! parser handles invalid input gracefully and returns errors instead of panicking.

use prb_pcap::tls::keylog::TlsKeyLog;

/// Helper to assert that a line parsing returns an error (not a panic)
fn assert_parse_error(line: &str) {
    let mut keylog = TlsKeyLog::new();
    let result = keylog.parse_line(line);
    assert!(
        result.is_err(),
        "Expected error for line: {}, got: {:?}",
        line,
        result
    );
}

/// Helper to assert that a line is silently ignored (returns Ok but stores nothing)
fn assert_parse_ignored(line: &str) {
    let mut keylog = TlsKeyLog::new();
    let result = keylog.parse_line(line);
    assert!(result.is_ok(), "Expected Ok for line: {}", line);
    assert_eq!(
        keylog.len(),
        0,
        "Expected no keys stored for line: {}",
        line
    );
}

#[test]
fn malformed_invalid_hex_encoding() {
    // Odd-length hex strings
    assert_parse_error(&format!(
        "CLIENT_RANDOM {} {}",
        "a".repeat(63),
        "bb".repeat(48)
    ));
    assert_parse_error(&format!(
        "CLIENT_RANDOM {} {}",
        "aa".repeat(32),
        "b".repeat(95)
    ));

    // Non-hex characters
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "GGGGGGGG", "bb".repeat(48)));
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(32), "ZZZZZZZZ"));
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "!!@@##$$", "bb".repeat(48)));
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(32), "xyz123=="));

    // Mixed case (should work, but let's test edge cases)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!(
            "CLIENT_RANDOM {} {}",
            "AA".repeat(32),
            "BB".repeat(48)
        ))
        .unwrap();
    assert_eq!(keylog.len(), 1);

    // Hex with spaces
    assert_parse_error(&format!(
        "CLIENT_RANDOM {} {}",
        "aa aa".repeat(16),
        "bb".repeat(48)
    ));
    assert_parse_error(&format!(
        "CLIENT_RANDOM {} {}",
        "aa".repeat(32),
        "bb bb".repeat(24)
    ));

    // Hex with dashes or colons
    assert_parse_error(&format!(
        "CLIENT_RANDOM {}-{} {}",
        "aa".repeat(16),
        "aa".repeat(16),
        "bb".repeat(48)
    ));
    assert_parse_error(&format!(
        "CLIENT_RANDOM {}:{} {}",
        "aa".repeat(16),
        "aa".repeat(16),
        "bb".repeat(48)
    ));
}

#[test]
fn malformed_wrong_key_lengths() {
    let cr = "aa".repeat(32);

    // CLIENT_RANDOM (TLS 1.2) - must be exactly 48 bytes
    // 0 bytes would result in malformed line (< 3 fields) - ignored
    assert_parse_ignored(&format!("CLIENT_RANDOM {} {}", cr, ""));
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb")); // 1 byte
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(16))); // 16 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(31))); // 31 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(32))); // 32 bytes (wrong)
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(47))); // 47 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(49))); // 49 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(64))); // 64 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(100))); // 100 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", cr, "bb".repeat(500))); // 500 bytes (huge)

    // TLS 1.3 traffic secrets - must be 32 or 48 bytes
    // 0 bytes would result in malformed line (< 3 fields) - ignored
    assert_parse_ignored(&format!("CLIENT_TRAFFIC_SECRET_0 {} {}", cr, ""));
    assert_parse_error(&format!("CLIENT_TRAFFIC_SECRET_0 {} {}", cr, "bb")); // 1 byte
    assert_parse_error(&format!(
        "CLIENT_TRAFFIC_SECRET_0 {} {}",
        cr,
        "bb".repeat(16)
    )); // 16 bytes
    assert_parse_error(&format!(
        "CLIENT_TRAFFIC_SECRET_0 {} {}",
        cr,
        "bb".repeat(31)
    )); // 31 bytes
    // 32 bytes OK
    assert_parse_error(&format!(
        "CLIENT_TRAFFIC_SECRET_0 {} {}",
        cr,
        "bb".repeat(33)
    )); // 33 bytes
    assert_parse_error(&format!(
        "CLIENT_TRAFFIC_SECRET_0 {} {}",
        cr,
        "bb".repeat(47)
    )); // 47 bytes
    // 48 bytes OK
    assert_parse_error(&format!(
        "CLIENT_TRAFFIC_SECRET_0 {} {}",
        cr,
        "bb".repeat(49)
    )); // 49 bytes
    assert_parse_error(&format!(
        "CLIENT_TRAFFIC_SECRET_0 {} {}",
        cr,
        "bb".repeat(64)
    )); // 64 bytes

    // All TLS 1.3 secret types should have same length requirements
    assert_parse_error(&format!(
        "SERVER_TRAFFIC_SECRET_0 {} {}",
        cr,
        "bb".repeat(31)
    ));
    assert_parse_error(&format!(
        "CLIENT_HANDSHAKE_TRAFFIC_SECRET {} {}",
        cr,
        "bb".repeat(31)
    ));
    assert_parse_error(&format!(
        "SERVER_HANDSHAKE_TRAFFIC_SECRET {} {}",
        cr,
        "bb".repeat(31)
    ));
}

#[test]
fn malformed_client_random_lengths() {
    let key = "bb".repeat(48);

    // client_random must be exactly 32 bytes
    // 0 bytes would result in malformed line (< 3 fields) - ignored
    assert_parse_ignored(&format!("CLIENT_RANDOM {} {}", "", key));

    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa", key)); // 1 byte
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(16), key)); // 16 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(31), key)); // 31 bytes
    // 32 bytes OK
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(33), key)); // 33 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(48), key)); // 48 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(64), key)); // 64 bytes
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(100), key)); // 100 bytes
}

#[test]
fn malformed_missing_fields() {
    // Lines with too few fields (should be ignored, not error)
    assert_parse_ignored("");
    assert_parse_ignored("CLIENT_RANDOM");
    assert_parse_ignored(&format!("CLIENT_RANDOM {}", "aa".repeat(32)));
    assert_parse_ignored("SINGLE_FIELD");
    assert_parse_ignored("TWO FIELDS");
}

#[test]
fn malformed_invalid_label_names() {
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    // Unknown labels should be silently ignored (not errors)
    assert_parse_ignored(&format!("INVALID_LABEL {} {}", cr, key));
    assert_parse_ignored(&format!("UNKNOWN_SECRET {} {}", cr, key));
    assert_parse_ignored(&format!("CLIENT_SECRET {} {}", cr, key));
    assert_parse_ignored(&format!("SERVER_SECRET {} {}", cr, key));
    assert_parse_ignored(&format!("MASTER_SECRET {} {}", cr, key));
    assert_parse_ignored(&format!("TRAFFIC_SECRET {} {}", cr, key));
    assert_parse_ignored(&format!("random_label {} {}", cr, key)); // lowercase
    assert_parse_ignored(&format!("123NUMERIC {} {}", cr, key));
    assert_parse_ignored(&format!("LABEL-WITH-DASHES {} {}", cr, key));
    assert_parse_ignored(&format!("LABEL.WITH.DOTS {} {}", cr, key));
}

#[test]
fn malformed_whitespace_variations() {
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    // Multiple spaces between fields (should work - split_whitespace handles it)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!("CLIENT_RANDOM    {}    {}", cr, key))
        .unwrap();
    assert_eq!(keylog.len(), 1);

    // Tabs (should work)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!("CLIENT_RANDOM\t{}\t{}", cr, key))
        .unwrap();
    assert_eq!(keylog.len(), 1);

    // Mixed tabs and spaces (should work)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!("CLIENT_RANDOM \t {} \t {}", cr, key))
        .unwrap();
    assert_eq!(keylog.len(), 1);

    // Leading/trailing whitespace (should work - trim handles it)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!("   CLIENT_RANDOM {} {}   ", cr, key))
        .unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn malformed_comments_and_empty_lines() {
    // These should all be silently ignored
    assert_parse_ignored("");
    assert_parse_ignored("   ");
    assert_parse_ignored("\t\t");
    assert_parse_ignored("# This is a comment");
    assert_parse_ignored("# Comment with special chars: !@#$%^&*()");
    assert_parse_ignored("   # Comment with leading spaces");
    assert_parse_ignored("\t# Comment with leading tab");
    assert_parse_ignored("## Double hash comment");
    assert_parse_ignored("### Triple hash comment");
}

#[test]
fn malformed_extra_fields() {
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    // Extra fields after the key material (should be ignored, takes first 3 fields)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!("CLIENT_RANDOM {} {} extra_field", cr, key))
        .unwrap();
    assert_eq!(keylog.len(), 1);

    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!(
            "CLIENT_RANDOM {} {} extra1 extra2 extra3",
            cr, key
        ))
        .unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn malformed_special_characters() {
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    // Special characters in label (should be ignored as unknown label)
    assert_parse_ignored(&format!("CLIENT@RANDOM {} {}", cr, key));
    assert_parse_ignored(&format!("CLIENT#RANDOM {} {}", cr, key));
    assert_parse_ignored(&format!("CLIENT$RANDOM {} {}", cr, key));
    assert_parse_ignored(&format!("CLIENT%RANDOM {} {}", cr, key));
    assert_parse_ignored(&format!("CLIENT&RANDOM {} {}", cr, key));
}

#[test]
fn malformed_case_sensitivity() {
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    // Labels are case-sensitive - lowercase should be ignored
    assert_parse_ignored(&format!("client_random {} {}", cr, key));
    assert_parse_ignored(&format!("Client_Random {} {}", cr, key));
    assert_parse_ignored(&format!("CLIENT_random {} {}", cr, key));
    assert_parse_ignored(&format!(
        "client_traffic_secret_0 {} {}",
        cr,
        "bb".repeat(32)
    ));
}

#[test]
fn malformed_unicode_and_non_utf8() {
    // Unicode in label (should be ignored as unknown label)
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    assert_parse_ignored(&format!("CLIENT_RANDOM_🔑 {} {}", cr, key));
    assert_parse_ignored(&format!("クライアント {} {}", cr, key));
    assert_parse_ignored(&format!("КЛИЕНТ_РАНДОМ {} {}", cr, key));
}

#[test]
fn malformed_duplicate_entries() {
    let cr = "aa".repeat(32);
    let key1 = "bb".repeat(48);
    let key2 = "cc".repeat(48);

    // Duplicate client_random with same label (both should be stored)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!("CLIENT_RANDOM {} {}", cr, key1))
        .unwrap();
    keylog
        .parse_line(&format!("CLIENT_RANDOM {} {}", cr, key2))
        .unwrap();
    assert_eq!(keylog.len(), 1); // Same client_random
    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 2); // But two key materials
}

#[test]
fn malformed_boundary_values() {
    // Empty hex strings (< 3 fields, should be ignored)
    assert_parse_ignored("CLIENT_RANDOM  ");
    assert_parse_ignored("CLIENT_RANDOM ");

    // Single character hex (invalid - need pairs)
    assert_parse_error(&format!("CLIENT_RANDOM a {}", "bb".repeat(48)));
    assert_parse_error(&format!("CLIENT_RANDOM {} b", "aa".repeat(32)));

    // Very long lines (stress test)
    let huge_cr = "aa".repeat(1000);
    let huge_key = "bb".repeat(1000);
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", huge_cr, huge_key));
}

#[test]
fn malformed_newline_handling() {
    // Line with embedded newline characters should not crash
    // (In practice, lines() iterator won't produce these, but test anyway)
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    // These would be split by lines() in real usage, but test parse_line directly
    let mut keylog = TlsKeyLog::new();
    // This should work - split_whitespace will treat \n as whitespace
    keylog
        .parse_line(&format!("CLIENT_RANDOM\n{}\n{}", cr, key))
        .unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn malformed_mixed_tls_versions() {
    // Mix TLS 1.2 and 1.3 for same client_random (unusual but valid)
    let cr = "aa".repeat(32);

    let mut keylog = TlsKeyLog::new();
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
    keylog
        .parse_line(&format!(
            "SERVER_TRAFFIC_SECRET_0 {} {}",
            cr,
            "dd".repeat(32)
        ))
        .unwrap();
    keylog
        .parse_line(&format!(
            "CLIENT_HANDSHAKE_TRAFFIC_SECRET {} {}",
            cr,
            "ee".repeat(32)
        ))
        .unwrap();
    keylog
        .parse_line(&format!(
            "SERVER_HANDSHAKE_TRAFFIC_SECRET {} {}",
            cr,
            "ff".repeat(32)
        ))
        .unwrap();

    assert_eq!(keylog.len(), 1); // Same client_random
    let materials = keylog.lookup(&hex::decode(&cr).unwrap()).unwrap();
    assert_eq!(materials.len(), 5); // 1 TLS 1.2 + 4 TLS 1.3 secrets
}

#[test]
fn malformed_all_zeros() {
    // All zeros should be valid (it's legitimate hex)
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!(
            "CLIENT_RANDOM {} {}",
            "00".repeat(32),
            "00".repeat(48)
        ))
        .unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn malformed_all_ones() {
    // All 0xFF should be valid
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!(
            "CLIENT_RANDOM {} {}",
            "ff".repeat(32),
            "ff".repeat(48)
        ))
        .unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn malformed_binary_data() {
    // Test with raw binary data that's not hex-encoded
    // Use embedded null bytes and control characters
    assert_parse_error(&format!(
        "CLIENT_RANDOM \u{0000}\u{0001}\u{0002}\u{0003} {}",
        "bb".repeat(48)
    ));

    // Use invalid UTF-8 sequences via String::from_utf8_lossy
    let invalid_bytes = vec![0xFF, 0xFE, 0xFD];
    let invalid_str = String::from_utf8_lossy(&invalid_bytes);
    assert_parse_error(&format!(
        "CLIENT_RANDOM {} {}",
        "aa".repeat(32),
        invalid_str
    ));
}

#[test]
fn malformed_leading_zeros() {
    // Leading zeros in hex should be fine
    let mut keylog = TlsKeyLog::new();
    keylog
        .parse_line(&format!(
            "CLIENT_RANDOM {} {}",
            "00aa".repeat(16),
            "00bb".repeat(24)
        ))
        .unwrap();
    assert_eq!(keylog.len(), 1);
}

#[test]
fn malformed_quoted_strings() {
    let cr = "aa".repeat(32);
    let key = "bb".repeat(48);

    // Quoted client_random or key should fail (quotes aren't valid hex)
    assert_parse_error(&format!("CLIENT_RANDOM \"{}\" {}", cr, key));
    assert_parse_error(&format!("CLIENT_RANDOM {} \"{}\"", cr, key));

    // Quoted label is unknown label - should be ignored
    assert_parse_ignored(&format!("\"CLIENT_RANDOM\" {} {}", cr, key));
}

#[test]
fn malformed_url_encoded() {
    // URL-encoded data should fail
    assert_parse_error(&format!("CLIENT_RANDOM %20%20%20 {}", "bb".repeat(48)));
    assert_parse_error(&format!("CLIENT_RANDOM {} %20%20%20", "aa".repeat(32)));
}

#[test]
fn malformed_base64_instead_of_hex() {
    // Base64-encoded data should fail (not hex)
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "QUFBQQ==", "bb".repeat(48)));
    assert_parse_error(&format!("CLIENT_RANDOM {} {}", "aa".repeat(32), "QkJCQg=="));
}
