//! TLS decryption edge case tests for error paths and boundary conditions.

use prb_pcap::tls::decrypt::TlsDecryptor;
use prb_pcap::tls::keylog::KeyMaterial;
use prb_pcap::tls::session::SessionInfo;
use prb_pcap::{PcapError, tcp::StreamDirection};
use tls_parser::TlsVersion;

/// Test creating decryptor with unsupported cipher suite (non-AEAD).
#[test]
fn test_unsupported_cipher_suite() {
    let session = SessionInfo {
        cipher_suite_id: 0x0035, // TLS_RSA_WITH_AES_256_CBC_SHA (not AEAD)
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    let master_secret = vec![0u8; 48];
    let key_materials = vec![KeyMaterial::MasterSecret(master_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    assert!(result.is_err());
    match result {
        Err(PcapError::TlsKey(msg)) => {
            assert!(msg.contains("unsupported cipher suite"));
        }
        _ => panic!("Expected TlsKey error for unsupported cipher"),
    }
}

/// Test creating TLS 1.3 decryptor with missing client traffic secret.
#[test]
fn test_tls13_missing_client_secret() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301, // TLS_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    // Only provide server secret, no client secret
    let server_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ServerTrafficSecret0(server_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    // Should succeed but use empty keys for client direction
    assert!(result.is_ok());
}

/// Test creating TLS 1.3 decryptor with missing server traffic secret.
#[test]
fn test_tls13_missing_server_secret() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301, // TLS_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    // Only provide client secret, no server secret
    let client_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(client_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    // Should succeed but use empty keys for server direction
    assert!(result.is_ok());
}

/// Test creating TLS 1.3 decryptor with no secrets at all.
#[test]
fn test_tls13_no_secrets() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301, // TLS_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    let key_materials = vec![]; // No secrets

    let result = TlsDecryptor::new(&session, &key_materials);
    // Should succeed but use empty keys for both directions
    assert!(result.is_ok());
}

/// Test creating TLS 1.2 decryptor without master secret.
#[test]
fn test_tls12_missing_master_secret() {
    let session = SessionInfo {
        cipher_suite_id: 0x009C, // TLS_RSA_WITH_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    // Provide wrong key material type (TLS 1.3 key for TLS 1.2 session)
    let client_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(client_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    assert!(result.is_err());
    match result {
        Err(PcapError::TlsKey(msg)) => {
            assert!(msg.contains("TLS 1.2 requires master secret"));
        }
        _ => panic!("Expected TlsKey error for missing master secret"),
    }
}

/// Test creating TLS 1.2 decryptor with empty key materials.
#[test]
fn test_tls12_no_key_materials() {
    let session = SessionInfo {
        cipher_suite_id: 0x009C, // TLS_RSA_WITH_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    let key_materials = vec![];

    let result = TlsDecryptor::new(&session, &key_materials);
    assert!(result.is_err());
    match result {
        Err(PcapError::TlsKey(msg)) => {
            assert!(msg.contains("TLS 1.2 requires master secret"));
        }
        _ => panic!("Expected TlsKey error"),
    }
}

/// Test TLS 1.2 decryption with ciphertext too short for explicit nonce.
#[test]
fn test_tls12_ciphertext_too_short() {
    let session = SessionInfo {
        cipher_suite_id: 0x009C, // TLS_RSA_WITH_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    let master_secret = vec![0u8; 48];
    let key_materials = vec![KeyMaterial::MasterSecret(master_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // Ciphertext with only 7 bytes (need at least 8 for explicit nonce + 16 for tag)
    let short_ciphertext = vec![0u8; 7];

    let result = decryptor.decrypt_aead(
        &short_ciphertext,
        0,
        0x17,   // Application Data
        0x0303, // TLS 1.2
        StreamDirection::ClientToServer,
    );

    assert!(result.is_err());
    match result {
        Err(PcapError::TlsKey(msg)) => {
            assert!(msg.contains("ciphertext too short for TLS 1.2 explicit nonce"));
        }
        _ => panic!("Expected error for short ciphertext"),
    }
}

/// Test TLS 1.3 decryption with wrong key (decryption will fail).
#[test]
fn test_tls13_wrong_key_decryption_failure() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301, // TLS_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    // Use wrong/random secret
    let wrong_secret = vec![0xAA; 32];
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(wrong_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // Create a dummy encrypted record (random data, won't decrypt correctly)
    let fake_ciphertext = vec![0x42; 32]; // 16 bytes encrypted + 16 bytes tag

    let result = decryptor.decrypt_aead(
        &fake_ciphertext,
        0,
        0x17,   // Application Data
        0x0303, // TLS 1.3
        StreamDirection::ClientToServer,
    );

    // Should fail with decryption error
    assert!(result.is_err());
    match result {
        Err(PcapError::TlsKey(msg)) => {
            assert!(msg.contains("decryption failed"));
        }
        _ => panic!("Expected decryption failure"),
    }
}

/// Test TLS 1.2 decryption with corrupted data.
#[test]
fn test_tls12_corrupted_ciphertext() {
    let session = SessionInfo {
        cipher_suite_id: 0x009C, // TLS_RSA_WITH_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    let master_secret = vec![0u8; 48];
    let key_materials = vec![KeyMaterial::MasterSecret(master_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // TLS 1.2: explicit nonce (8 bytes) + encrypted data + tag (16 bytes)
    // Total: 8 + 16 + 16 = 40 bytes of garbage
    let corrupted_ciphertext = vec![0xFF; 40];

    let result = decryptor.decrypt_aead(
        &corrupted_ciphertext,
        0,
        0x17,
        0x0303,
        StreamDirection::ServerToClient,
    );

    assert!(result.is_err());
    match result {
        Err(PcapError::TlsKey(msg)) => {
            assert!(msg.contains("decryption failed"));
        }
        _ => panic!("Expected decryption failure for corrupted data"),
    }
}

/// Test decrypt_stream with malformed TLS records.
#[test]
fn test_decrypt_stream_parse_failure() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301, // TLS_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    let client_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(client_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // Malformed TLS stream (not valid TLS records)
    let malformed_stream = vec![0xFF; 100];

    let result = decryptor.decrypt_stream(&malformed_stream, StreamDirection::ClientToServer);

    // Should return Ok but with empty plaintext (parse failure stops processing)
    assert!(result.is_ok());
    let plaintext = result.unwrap();
    assert_eq!(
        plaintext.len(),
        0,
        "Expected empty plaintext on parse failure"
    );
}

/// Test decrypt_stream with partial TLS record.
#[test]
fn test_decrypt_stream_partial_record() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301,
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    let client_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(client_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // Create a valid TLS record header but truncate the payload
    // TLS record: type (1) + version (2) + length (2) + data
    let mut partial_stream = vec![
        0x17, // Application Data
        0x03, 0x03, // TLS 1.2 version
        0x00, 0x20, // Length: 32 bytes
    ];
    // Add only 10 bytes instead of 32
    partial_stream.extend_from_slice(&[0u8; 10]);

    let result = decryptor.decrypt_stream(&partial_stream, StreamDirection::ClientToServer);

    // Should handle partial record gracefully
    assert!(result.is_ok());
}

/// Test AES-256-GCM cipher suite support.
#[test]
fn test_aes256_gcm_cipher_suite() {
    let session = SessionInfo {
        cipher_suite_id: 0x1302, // TLS_AES_256_GCM_SHA384
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    let client_secret = vec![0u8; 48]; // 384-bit secret for SHA384
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(client_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    assert!(result.is_ok());
}

/// Test ChaCha20-Poly1305 cipher suite support.
#[test]
fn test_chacha20_poly1305_cipher_suite() {
    let session = SessionInfo {
        cipher_suite_id: 0x1303, // TLS_CHACHA20_POLY1305_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    let client_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(client_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    assert!(result.is_ok());
}

/// Test TLS 1.2 cipher suite variants (ECDHE-RSA-AES128-GCM-SHA256).
#[test]
fn test_tls12_ecdhe_cipher_suite() {
    let session = SessionInfo {
        cipher_suite_id: 0xC02F, // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    let master_secret = vec![0u8; 48];
    let key_materials = vec![KeyMaterial::MasterSecret(master_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    assert!(result.is_ok());
}

/// Test TLS 1.2 cipher suite variants (ECDHE-ECDSA-AES256-GCM-SHA384).
#[test]
fn test_tls12_ecdhe_ecdsa_aes256_gcm() {
    let session = SessionInfo {
        cipher_suite_id: 0xC030, // TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    let master_secret = vec![0u8; 48];
    let key_materials = vec![KeyMaterial::MasterSecret(master_secret)];

    let result = TlsDecryptor::new(&session, &key_materials);
    assert!(result.is_ok());
}

/// Test decryption in server-to-client direction.
#[test]
fn test_decrypt_server_to_client_direction() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301,
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    let server_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ServerTrafficSecret0(server_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // Try to decrypt with server key
    let fake_ciphertext = vec![0x42; 32];

    let result = decryptor.decrypt_aead(
        &fake_ciphertext,
        0,
        0x17,
        0x0303,
        StreamDirection::ServerToClient,
    );

    // Will fail due to wrong key, but tests server direction code path
    assert!(result.is_err());
}

/// Test minimum valid ciphertext length for TLS 1.2 (8 + 16 = 24 bytes).
#[test]
fn test_tls12_minimum_ciphertext_length() {
    let session = SessionInfo {
        cipher_suite_id: 0x009C,
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls12,
    };

    let master_secret = vec![0u8; 48];
    let key_materials = vec![KeyMaterial::MasterSecret(master_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // Exactly 24 bytes: 8 (explicit nonce) + 16 (tag), zero encrypted data
    let min_ciphertext = vec![0u8; 24];

    let result = decryptor.decrypt_aead(
        &min_ciphertext,
        0,
        0x17,
        0x0303,
        StreamDirection::ClientToServer,
    );

    // Will fail decryption but validates length check passes
    assert!(result.is_err());
    match result {
        Err(PcapError::TlsKey(msg)) => {
            assert!(msg.contains("decryption failed"));
        }
        _ => panic!("Expected decryption failure"),
    }
}

/// Test that decrypt_stream skips non-ApplicationData records.
#[test]
fn test_decrypt_stream_skips_non_app_data() {
    let session = SessionInfo {
        cipher_suite_id: 0x1301,
        client_random: vec![0u8; 32],
        server_random: vec![0u8; 32],
        version: TlsVersion::Tls13,
    };

    let client_secret = vec![0u8; 32];
    let key_materials = vec![KeyMaterial::ClientTrafficSecret0(client_secret)];

    let decryptor = TlsDecryptor::new(&session, &key_materials).unwrap();

    // Create a Handshake record (type 0x16, not Application Data 0x17)
    let mut handshake_record = vec![
        0x16, // Handshake
        0x03, 0x03, // Version
        0x00, 0x10, // Length: 16
    ];
    handshake_record.extend_from_slice(&[0u8; 16]);

    let result = decryptor.decrypt_stream(&handshake_record, StreamDirection::ClientToServer);

    // Should succeed but return empty plaintext (Handshake records are skipped)
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}
