//! Comprehensive cipher suite coverage tests for TLS decryption.
//!
//! Tests all 13 supported cipher suite IDs across AES-128-GCM, AES-256-GCM,
//! and ChaCha20-Poly1305 algorithms for both TLS 1.2 and TLS 1.3.

use prb_pcap::tcp::StreamDirection;
use prb_pcap::tls::decrypt::TlsDecryptor;
use prb_pcap::tls::keylog::KeyMaterial;
use prb_pcap::tls::session::SessionInfo;
use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, SealingKey, UnboundKey};
use rstest::rstest;
use tls_parser::TlsVersion;

/// Helper: One-time nonce for sealing operations.
struct OneNonce(Option<Nonce>);

impl NonceSequence for OneNonce {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0.take().ok_or(ring::error::Unspecified)
    }
}

/// Helper: seal with an AEAD algorithm using ring.
fn aead_seal(
    algo: &'static ring::aead::Algorithm,
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    plaintext: &[u8],
) -> Vec<u8> {
    let unbound = UnboundKey::new(algo, key).unwrap();
    let nonce_val = Nonce::try_assume_unique_for_key(nonce).unwrap();
    let mut sealing_key = SealingKey::new(unbound, OneNonce(Some(nonce_val)));
    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::from(aad), &mut in_out)
        .unwrap();
    in_out
}

/// Comprehensive cipher suite test matrix covering all 13 supported cipher suite IDs.
///
/// Tests cover:
/// - AES-128-GCM: 5 cipher suite IDs (4 TLS 1.2 + 1 TLS 1.3)
/// - AES-256-GCM: 5 cipher suite IDs (4 TLS 1.2 + 1 TLS 1.3)
/// - ChaCha20-Poly1305: 3 cipher suite IDs (2 TLS 1.2 + 1 TLS 1.3)
#[rstest]
// AES-128-GCM TLS 1.2 variants
#[case::aes128_gcm_0x009c(
    0x009C,
    "TLS_RSA_WITH_AES_128_GCM_SHA256",
    &ring::aead::AES_128_GCM,
    16,
    TlsVersion::Tls12,
    false
)]
#[case::aes128_gcm_0x009e(
    0x009E,
    "TLS_DHE_RSA_WITH_AES_128_GCM_SHA256",
    &ring::aead::AES_128_GCM,
    16,
    TlsVersion::Tls12,
    false
)]
#[case::aes128_gcm_0xc02f(
    0xC02F,
    "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    &ring::aead::AES_128_GCM,
    16,
    TlsVersion::Tls12,
    false
)]
#[case::aes128_gcm_0xc02b(
    0xC02B,
    "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    &ring::aead::AES_128_GCM,
    16,
    TlsVersion::Tls12,
    false
)]
// AES-128-GCM TLS 1.3
#[case::aes128_gcm_0x1301(
    0x1301,
    "TLS_AES_128_GCM_SHA256",
    &ring::aead::AES_128_GCM,
    16,
    TlsVersion::Tls13,
    false
)]
// AES-256-GCM TLS 1.2 variants
#[case::aes256_gcm_0x009d(
    0x009D,
    "TLS_RSA_WITH_AES_256_GCM_SHA384",
    &ring::aead::AES_256_GCM,
    32,
    TlsVersion::Tls12,
    true
)]
#[case::aes256_gcm_0x009f(
    0x009F,
    "TLS_DHE_RSA_WITH_AES_256_GCM_SHA384",
    &ring::aead::AES_256_GCM,
    32,
    TlsVersion::Tls12,
    true
)]
#[case::aes256_gcm_0xc030(
    0xC030,
    "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    &ring::aead::AES_256_GCM,
    32,
    TlsVersion::Tls12,
    true
)]
#[case::aes256_gcm_0xc02c(
    0xC02C,
    "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    &ring::aead::AES_256_GCM,
    32,
    TlsVersion::Tls12,
    true
)]
// AES-256-GCM TLS 1.3
#[case::aes256_gcm_0x1302(
    0x1302,
    "TLS_AES_256_GCM_SHA384",
    &ring::aead::AES_256_GCM,
    32,
    TlsVersion::Tls13,
    true
)]
// ChaCha20-Poly1305 TLS 1.3
#[case::chacha20_0x1303(
    0x1303,
    "TLS_CHACHA20_POLY1305_SHA256",
    &ring::aead::CHACHA20_POLY1305,
    32,
    TlsVersion::Tls13,
    false
)]
// ChaCha20-Poly1305 TLS 1.2 variants
#[case::chacha20_0xcca8(
    0xCCA8,
    "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
    &ring::aead::CHACHA20_POLY1305,
    32,
    TlsVersion::Tls12,
    false
)]
#[case::chacha20_0xcca9(
    0xCCA9,
    "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    &ring::aead::CHACHA20_POLY1305,
    32,
    TlsVersion::Tls12,
    false
)]
fn test_cipher_suite_comprehensive(
    #[case] cipher_id: u16,
    #[case] cipher_name: &str,
    #[case] ring_algo: &'static ring::aead::Algorithm,
    #[case] key_len: usize,
    #[case] tls_version: TlsVersion,
    #[case] uses_sha384: bool,
) {
    // Create session info for this cipher suite
    let client_random = vec![0x11; 32];
    let server_random = vec![0x22; 32];

    let session = SessionInfo {
        client_random: client_random.clone(),
        server_random: server_random.clone(),
        cipher_suite_id: cipher_id,
        version: tls_version,
    };

    // Verify session properties match expectations
    assert_eq!(
        session.key_len(),
        key_len,
        "Cipher {} (0x{:04X}) should have key length {}",
        cipher_name,
        cipher_id,
        key_len
    );
    assert_eq!(
        session.uses_sha384(),
        uses_sha384,
        "Cipher {} (0x{:04X}) SHA384 flag mismatch",
        cipher_name,
        cipher_id
    );
    assert!(
        session.is_supported(),
        "Cipher {} (0x{:04X}) should be supported",
        cipher_name,
        cipher_id
    );

    // Test encryption and decryption based on TLS version
    match tls_version {
        TlsVersion::Tls13 => {
            test_tls13_cipher_suite(
                &session,
                ring_algo,
                key_len,
                uses_sha384,
                cipher_name,
                cipher_id,
            );
        }
        TlsVersion::Tls12 => {
            test_tls12_cipher_suite(
                &session,
                &client_random,
                &server_random,
                ring_algo,
                key_len,
                cipher_name,
                cipher_id,
            );
        }
        _ => panic!("Unsupported TLS version for cipher {}", cipher_name),
    }
}

/// Test TLS 1.3 cipher suite end-to-end.
fn test_tls13_cipher_suite(
    session: &SessionInfo,
    ring_algo: &'static ring::aead::Algorithm,
    key_len: usize,
    uses_sha384: bool,
    cipher_name: &str,
    cipher_id: u16,
) {
    let traffic_secret = vec![0xaa; if uses_sha384 { 48 } else { 32 }];

    // Create decryptor with TLS 1.3 traffic secrets
    let decryptor = TlsDecryptor::new(
        session,
        &[
            KeyMaterial::ClientTrafficSecret0(traffic_secret.clone()),
            KeyMaterial::ServerTrafficSecret0(traffic_secret.clone()),
        ],
    )
    .unwrap_or_else(|e| {
        panic!(
            "Failed to create decryptor for {} (0x{:04X}): {}",
            cipher_name, cipher_id, e
        )
    });

    // Derive keys using TLS 1.3 KDF
    let keys = prb_pcap::tls::kdf::derive_tls13_keys(&traffic_secret, key_len, uses_sha384)
        .unwrap_or_else(|e| {
            panic!(
                "Failed to derive TLS 1.3 keys for {} (0x{:04X}): {}",
                cipher_name, cipher_id, e
            )
        });

    // Create test plaintext
    let plaintext = format!("Hello, TLS 1.3 cipher suite: {}!", cipher_name);
    let plaintext_bytes = plaintext.as_bytes();

    // TLS 1.3 uses sequence number 0 XOR with IV for first record
    let nonce = keys.iv.clone();

    // TLS 1.3 AAD: content_type(1) + version(2) + ciphertext_length(2)
    let ct_len = plaintext_bytes.len() + 16; // plaintext + 16-byte auth tag
    let aad = vec![
        0x17, // ApplicationData
        0x03,
        0x03,                // TLS 1.2 legacy version
        (ct_len >> 8) as u8, // length high byte
        ct_len as u8,        // length low byte
    ];

    // Encrypt using ring
    let ciphertext = aead_seal(ring_algo, &keys.key, &nonce, &aad, plaintext_bytes);

    // Decrypt using TlsDecryptor
    let result = decryptor
        .decrypt_aead(
            &ciphertext,
            0,      // sequence number
            0x17,   // ApplicationData
            0x0303, // TLS 1.2 legacy version
            StreamDirection::ClientToServer,
        )
        .unwrap_or_else(|e| {
            panic!(
                "Failed to decrypt for {} (0x{:04X}): {}",
                cipher_name, cipher_id, e
            )
        });

    assert_eq!(
        result, plaintext_bytes,
        "Plaintext mismatch for {} (0x{:04X})",
        cipher_name, cipher_id
    );
}

/// Test TLS 1.2 cipher suite end-to-end.
fn test_tls12_cipher_suite(
    session: &SessionInfo,
    client_random: &[u8],
    server_random: &[u8],
    ring_algo: &'static ring::aead::Algorithm,
    key_len: usize,
    cipher_name: &str,
    cipher_id: u16,
) {
    let master_secret = vec![0x33; 48];

    // Create decryptor with TLS 1.2 master secret
    let decryptor = TlsDecryptor::new(session, &[KeyMaterial::MasterSecret(master_secret.clone())])
        .unwrap_or_else(|e| {
            panic!(
                "Failed to create decryptor for {} (0x{:04X}): {}",
                cipher_name, cipher_id, e
            )
        });

    // Derive keys using TLS 1.2 KDF
    let keys = prb_pcap::tls::kdf::derive_tls12_keys(
        &master_secret,
        client_random,
        server_random,
        key_len,
        4, // TLS 1.2 uses 4-byte implicit IV
    )
    .unwrap_or_else(|e| {
        panic!(
            "Failed to derive TLS 1.2 keys for {} (0x{:04X}): {}",
            cipher_name, cipher_id, e
        )
    });

    // Create test plaintext
    let plaintext = format!("Hello, TLS 1.2 cipher suite: {}!", cipher_name);
    let plaintext_bytes = plaintext.as_bytes();

    // TLS 1.2 explicit nonce (8 bytes)
    let explicit_nonce: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

    // 12-byte nonce: implicit_iv(4) + explicit_nonce(8)
    let mut nonce = keys.client_write_iv.clone();
    nonce.extend_from_slice(&explicit_nonce);

    // TLS 1.2 AAD: seq_num(8) + content_type(1) + version(2) + plaintext_length(2)
    let seq: u64 = 0;
    let mut aad = Vec::with_capacity(13);
    aad.extend_from_slice(&seq.to_be_bytes());
    aad.push(0x17); // ApplicationData
    aad.extend_from_slice(&[0x03, 0x03]); // TLS 1.2
    aad.extend_from_slice(&(plaintext_bytes.len() as u16).to_be_bytes());

    // Encrypt using ring
    let ciphertext_and_tag = aead_seal(
        ring_algo,
        &keys.client_write_key,
        &nonce,
        &aad,
        plaintext_bytes,
    );

    // TLS 1.2 record payload: explicit_nonce(8) + ciphertext + tag
    let mut record_payload = Vec::new();
    record_payload.extend_from_slice(&explicit_nonce);
    record_payload.extend_from_slice(&ciphertext_and_tag);

    // Decrypt using TlsDecryptor
    let result = decryptor
        .decrypt_aead(
            &record_payload,
            0,      // sequence number
            0x17,   // ApplicationData
            0x0303, // TLS 1.2
            StreamDirection::ClientToServer,
        )
        .unwrap_or_else(|e| {
            panic!(
                "Failed to decrypt for {} (0x{:04X}): {}",
                cipher_name, cipher_id, e
            )
        });

    assert_eq!(
        result, plaintext_bytes,
        "Plaintext mismatch for {} (0x{:04X})",
        cipher_name, cipher_id
    );
}
