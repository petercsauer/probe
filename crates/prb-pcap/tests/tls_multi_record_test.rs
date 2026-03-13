//! Multi-record TLS decryption tests with sequence number progression.
//!
//! Tests cover:
//! - Sequence number progression through multiple records
//! - Sequence number wrap at boundaries
//! - Multiple records in a single stream
//! - Out-of-order record handling

use prb_pcap::tcp::StreamDirection;
use prb_pcap::tls::decrypt::{AeadCipher, TlsDecryptor};
use ring::aead::{AES_128_GCM, Aad, BoundKey, Nonce, NonceSequence, SealingKey, UnboundKey};

/// Helper to create a simple sealing key for test encryption
struct FixedNonceSequence(Option<Nonce>);

impl NonceSequence for FixedNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0.take().ok_or(ring::error::Unspecified)
    }
}

fn encrypt_tls13_record(key: &[u8], iv: &[u8], sequence: u64, plaintext: &[u8]) -> Vec<u8> {
    // Construct TLS 1.3 nonce: IV XOR sequence number
    let mut nonce_bytes = iv.to_vec();
    let seq_bytes = sequence.to_be_bytes();
    for i in 0..8 {
        nonce_bytes[4 + i] ^= seq_bytes[i];
    }

    // Encrypt using ring
    let unbound_key = UnboundKey::new(&AES_128_GCM, key).expect("valid key");
    let nonce = Nonce::try_assume_unique_for_key(&nonce_bytes).expect("valid nonce");
    let mut sealing_key = SealingKey::new(unbound_key, FixedNonceSequence(Some(nonce)));

    // TLS 1.3 AAD: type (1) + version (2) + length (2)
    let aad_bytes = vec![
        0x17, // ApplicationData
        0x03,
        0x03, // TLS 1.2 (wire version)
        ((plaintext.len() + 16) >> 8) as u8,
        ((plaintext.len() + 16) & 0xff) as u8,
    ];

    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::from(&aad_bytes), &mut in_out)
        .expect("encryption succeeds");

    in_out
}

#[test]
fn test_multi_record_sequence_progression() {
    // Test decrypting 100 records with sequence 0..100
    let key = vec![0x03; 16];
    let iv = vec![0x04; 12];

    // Create decryptor with known keys for testing
    let decryptor = TlsDecryptor::new_for_test(
        AeadCipher::Aes128Gcm,
        key.clone(),
        iv.clone(),
        vec![0x06; 16],
        vec![0x07; 12],
        true,
    );

    // Encrypt and decrypt 100 records
    for seq in 0..100u64 {
        let plaintext = format!("Record {seq}").into_bytes();
        let ciphertext = encrypt_tls13_record(&key, &iv, seq, &plaintext);

        let decrypted = decryptor
            .decrypt_aead(
                &ciphertext,
                seq,
                0x17, // ApplicationData
                0x0303,
                StreamDirection::ClientToServer,
            )
            .expect("decryption succeeds");

        assert_eq!(
            decrypted, plaintext,
            "Plaintext mismatch for sequence {seq}"
        );
    }

    println!("✓ Successfully decrypted 100 records with sequence progression");
}

#[test]
fn test_sequence_number_boundaries() {
    // Test sequence numbers near boundaries
    let key = vec![0x01; 16];
    let iv = vec![0x02; 12];

    let decryptor = TlsDecryptor::new_for_test(
        AeadCipher::Aes128Gcm,
        key.clone(),
        iv.clone(),
        vec![0x03; 16],
        vec![0x04; 12],
        true,
    );

    // Test sequences: 0, 1, 255, 256, 65535, 65536, u32::MAX, u64::MAX - 1
    let test_sequences = vec![
        0u64,
        1u64,
        255u64,
        256u64,
        65535u64,
        65536u64,
        u32::MAX as u64,
        u64::MAX - 1,
    ];

    for seq in test_sequences {
        let plaintext = format!("Seq {seq}").into_bytes();
        let ciphertext = encrypt_tls13_record(&key, &iv, seq, &plaintext);

        let decrypted = decryptor
            .decrypt_aead(
                &ciphertext,
                seq,
                0x17,
                0x0303,
                StreamDirection::ClientToServer,
            )
            .expect(&format!("decryption succeeds for sequence {seq}"));

        assert_eq!(
            decrypted, plaintext,
            "Plaintext mismatch for sequence {seq}"
        );
    }

    println!("✓ Sequence number boundary tests passed");
}

#[test]
fn test_out_of_order_records_fail() {
    // Test that using wrong sequence number causes decryption failure
    let key = vec![0x01; 16];
    let iv = vec![0x02; 12];

    let decryptor = TlsDecryptor::new_for_test(
        AeadCipher::Aes128Gcm,
        key.clone(),
        iv.clone(),
        vec![0x03; 16],
        vec![0x04; 12],
        true,
    );

    // Encrypt with sequence 5
    let plaintext = b"Test data";
    let ciphertext = encrypt_tls13_record(&key, &iv, 5, plaintext);

    // Try to decrypt with wrong sequence numbers (should fail due to nonce mismatch)
    for wrong_seq in [0, 1, 4, 6, 10] {
        let result = decryptor.decrypt_aead(
            &ciphertext,
            wrong_seq,
            0x17,
            0x0303,
            StreamDirection::ClientToServer,
        );

        assert!(
            result.is_err(),
            "Decryption should fail with wrong sequence {wrong_seq}"
        );
    }

    // Decrypt with correct sequence (should succeed)
    let decrypted = decryptor
        .decrypt_aead(
            &ciphertext,
            5,
            0x17,
            0x0303,
            StreamDirection::ClientToServer,
        )
        .expect("decryption succeeds with correct sequence");

    assert_eq!(decrypted, plaintext);
    println!("✓ Out-of-order record detection works");
}

#[test]
fn test_multiple_records_stream() {
    // Test decrypting multiple records from a stream
    let key = vec![0x01; 16];
    let iv = vec![0x02; 12];

    let decryptor = TlsDecryptor::new_for_test(
        AeadCipher::Aes128Gcm,
        key.clone(),
        iv.clone(),
        vec![0x03; 16],
        vec![0x04; 12],
        true,
    );

    // Create 10 records
    let records: Vec<_> = (0..10)
        .map(|seq| {
            let plaintext = format!("Record {seq}").into_bytes();
            let ciphertext = encrypt_tls13_record(&key, &iv, seq, &plaintext);
            (plaintext, ciphertext)
        })
        .collect();

    // Decrypt each record with correct sequence
    for (seq, (plaintext, ciphertext)) in records.iter().enumerate() {
        let decrypted = decryptor
            .decrypt_aead(
                ciphertext,
                seq as u64,
                0x17,
                0x0303,
                StreamDirection::ClientToServer,
            )
            .expect("decryption succeeds");

        assert_eq!(&decrypted, plaintext, "Mismatch for record {seq}");
    }

    println!("✓ Multiple records in stream decrypt correctly");
}

#[test]
fn test_zero_length_record() {
    // Test decrypting a zero-length record
    let key = vec![0x01; 16];
    let iv = vec![0x02; 12];

    let decryptor = TlsDecryptor::new_for_test(
        AeadCipher::Aes128Gcm,
        key.clone(),
        iv.clone(),
        vec![0x03; 16],
        vec![0x04; 12],
        true,
    );

    let plaintext = b"";
    let ciphertext = encrypt_tls13_record(&key, &iv, 0, plaintext);

    let decrypted = decryptor
        .decrypt_aead(
            &ciphertext,
            0,
            0x17,
            0x0303,
            StreamDirection::ClientToServer,
        )
        .expect("decryption of empty record succeeds");

    assert_eq!(decrypted, plaintext);
    println!("✓ Zero-length record decryption works");
}
