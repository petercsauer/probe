//! Property-based tests for TLS decryption using proptest.
//!
//! These tests use randomized inputs to validate:
//! - Encrypt/decrypt roundtrips always succeed
//! - Different keys produce different ciphertexts
//! - Sequence number affects encryption

use prb_pcap::tcp::StreamDirection;
use prb_pcap::tls::decrypt::{AeadCipher, TlsDecryptor};
use proptest::prelude::*;
use ring::aead::{
    AES_128_GCM, AES_256_GCM, Aad, BoundKey, CHACHA20_POLY1305, Nonce, NonceSequence, SealingKey,
    UnboundKey,
};

struct FixedNonceSequence(Option<Nonce>);

impl NonceSequence for FixedNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0.take().ok_or(ring::error::Unspecified)
    }
}

fn encrypt_tls13_aead(
    algorithm: &'static ring::aead::Algorithm,
    key: &[u8],
    iv: &[u8],
    sequence: u64,
    plaintext: &[u8],
) -> Vec<u8> {
    // Construct TLS 1.3 nonce: IV XOR sequence number
    let mut nonce_bytes = iv.to_vec();
    let seq_bytes = sequence.to_be_bytes();
    for i in 0..8 {
        nonce_bytes[4 + i] ^= seq_bytes[i];
    }

    let unbound_key = UnboundKey::new(algorithm, key).expect("valid key");
    let nonce = Nonce::try_assume_unique_for_key(&nonce_bytes).expect("valid nonce");
    let mut sealing_key = SealingKey::new(unbound_key, FixedNonceSequence(Some(nonce)));

    // TLS 1.3 AAD
    let aad_bytes = vec![
        0x17, // ApplicationData
        0x03,
        0x03, // TLS 1.2 (wire version)
        ((plaintext.len() + algorithm.tag_len()) >> 8) as u8,
        ((plaintext.len() + algorithm.tag_len()) & 0xff) as u8,
    ];

    let mut in_out = plaintext.to_vec();
    sealing_key
        .seal_in_place_append_tag(Aad::from(&aad_bytes), &mut in_out)
        .expect("encryption succeeds");

    in_out
}

proptest! {
    #[test]
    fn prop_aes128_encrypt_decrypt_roundtrip(
        key in prop::collection::vec(any::<u8>(), 16..=16),
        iv in prop::collection::vec(any::<u8>(), 12..=12),
        plaintext in prop::collection::vec(any::<u8>(), 0..1000),
        sequence in any::<u64>(),
    ) {
        let decryptor = TlsDecryptor::new_for_test(
            AeadCipher::Aes128Gcm,
            key.clone(),
            iv.clone(),
            vec![0u8; 16],
            vec![0u8; 12],
            true,
        );

        let ciphertext = encrypt_tls13_aead(&AES_128_GCM, &key, &iv, sequence, &plaintext);

        let decrypted = decryptor
            .decrypt_aead(
                &ciphertext,
                sequence,
                0x17,
                0x0303,
                StreamDirection::ClientToServer,
            )
            .expect("decryption succeeds");

        prop_assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn prop_aes256_encrypt_decrypt_roundtrip(
        key in prop::collection::vec(any::<u8>(), 32..=32),
        iv in prop::collection::vec(any::<u8>(), 12..=12),
        plaintext in prop::collection::vec(any::<u8>(), 0..500),
        sequence in any::<u64>(),
    ) {
        let decryptor = TlsDecryptor::new_for_test(
            AeadCipher::Aes256Gcm,
            key.clone(),
            iv.clone(),
            vec![0u8; 32],
            vec![0u8; 12],
            true,
        );

        let ciphertext = encrypt_tls13_aead(&AES_256_GCM, &key, &iv, sequence, &plaintext);

        let decrypted = decryptor
            .decrypt_aead(
                &ciphertext,
                sequence,
                0x17,
                0x0303,
                StreamDirection::ClientToServer,
            )
            .expect("decryption succeeds");

        prop_assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn prop_chacha20_encrypt_decrypt_roundtrip(
        key in prop::collection::vec(any::<u8>(), 32..=32),
        iv in prop::collection::vec(any::<u8>(), 12..=12),
        plaintext in prop::collection::vec(any::<u8>(), 0..500),
        sequence in any::<u64>(),
    ) {
        let decryptor = TlsDecryptor::new_for_test(
            AeadCipher::ChaCha20Poly1305,
            key.clone(),
            iv.clone(),
            vec![0u8; 32],
            vec![0u8; 12],
            true,
        );

        let ciphertext = encrypt_tls13_aead(&CHACHA20_POLY1305, &key, &iv, sequence, &plaintext);

        let decrypted = decryptor
            .decrypt_aead(
                &ciphertext,
                sequence,
                0x17,
                0x0303,
                StreamDirection::ClientToServer,
            )
            .expect("decryption succeeds");

        prop_assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn prop_different_sequences_different_ciphertexts(
        key in prop::collection::vec(any::<u8>(), 16..=16),
        iv in prop::collection::vec(any::<u8>(), 12..=12),
        plaintext in prop::collection::vec(any::<u8>(), 16..100),
        seq1 in any::<u64>(),
        seq2 in any::<u64>(),
    ) {
        // Skip if sequences are the same
        prop_assume!(seq1 != seq2);

        let ct1 = encrypt_tls13_aead(&AES_128_GCM, &key, &iv, seq1, &plaintext);
        let ct2 = encrypt_tls13_aead(&AES_128_GCM, &key, &iv, seq2, &plaintext);

        // Different sequences should produce different ciphertexts
        prop_assert_ne!(ct1, ct2, "Different sequences must produce different ciphertexts");
    }

    #[test]
    fn prop_wrong_sequence_fails_decryption(
        key in prop::collection::vec(any::<u8>(), 16..=16),
        iv in prop::collection::vec(any::<u8>(), 12..=12),
        plaintext in prop::collection::vec(any::<u8>(), 16..100),
        encrypt_seq in any::<u64>(),
        decrypt_seq in any::<u64>(),
    ) {
        // Skip if sequences match
        prop_assume!(encrypt_seq != decrypt_seq);

        let ciphertext = encrypt_tls13_aead(&AES_128_GCM, &key, &iv, encrypt_seq, &plaintext);

        let decryptor = TlsDecryptor::new_for_test(
            AeadCipher::Aes128Gcm,
            key.clone(),
            iv.clone(),
            vec![0u8; 16],
            vec![0u8; 12],
            true,
        );

        let result = decryptor.decrypt_aead(
            &ciphertext,
            decrypt_seq,
            0x17,
            0x0303,
            StreamDirection::ClientToServer,
        );

        // Decryption with wrong sequence should fail
        prop_assert!(result.is_err(), "Decryption with wrong sequence must fail");
    }
}
