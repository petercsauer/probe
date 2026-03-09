//! TLS record decryption using AEAD ciphers.
//!
//! Supports:
//! - AES-128-GCM
//! - AES-256-GCM
//! - ChaCha20-Poly1305
//!
//! Per-record nonce construction:
//! - TLS 1.2: explicit nonce from record + implicit IV
//! - TLS 1.3: implicit IV XOR'd with 64-bit sequence number

use crate::error::PcapError;
use crate::tls::kdf::{derive_tls12_keys, derive_tls13_keys};
use crate::tls::keylog::KeyMaterial;
use crate::tls::session::SessionInfo;
use ring::aead::{Aad, BoundKey, Nonce, NonceSequence, OpeningKey, UnboundKey, AES_128_GCM, AES_256_GCM, CHACHA20_POLY1305};
use tls_parser::{parse_tls_plaintext, TlsMessage, TlsRecordType};

/// AEAD cipher algorithms supported for TLS decryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AeadCipher {
    Aes128Gcm,
    Aes256Gcm,
    ChaCha20Poly1305,
}

impl AeadCipher {
    fn algorithm(&self) -> &'static ring::aead::Algorithm {
        match self {
            AeadCipher::Aes128Gcm => &AES_128_GCM,
            AeadCipher::Aes256Gcm => &AES_256_GCM,
            AeadCipher::ChaCha20Poly1305 => &CHACHA20_POLY1305,
        }
    }
}

/// TLS decryptor that handles record-level AEAD decryption.
pub struct TlsDecryptor {
    cipher: AeadCipher,
    client_key: Vec<u8>,
    client_iv: Vec<u8>,
    #[allow(dead_code)] // Will be used for bidirectional decryption
    server_key: Vec<u8>,
    #[allow(dead_code)] // Will be used for bidirectional decryption
    server_iv: Vec<u8>,
    is_tls13: bool,
}

impl TlsDecryptor {
    /// Creates a new TLS decryptor from session info and key material.
    pub fn new(session: &SessionInfo, key_material: &KeyMaterial) -> Result<Self, PcapError> {
        if !session.is_supported() {
            return Err(PcapError::TlsKey(format!(
                "unsupported cipher suite: 0x{:04x}",
                session.cipher_suite_id
            )));
        }

        // Determine cipher algorithm based on cipher suite ID
        let cipher = match session.cipher_suite_id {
            // AES-128-GCM
            0x009C | 0x009E | 0xC02F | 0xC02B | 0x1301 => AeadCipher::Aes128Gcm,
            // AES-256-GCM
            0x009D | 0x009F | 0xC030 | 0xC02C | 0x1302 => AeadCipher::Aes256Gcm,
            // ChaCha20-Poly1305
            0x1303 | 0xCCA8 | 0xCCA9 => AeadCipher::ChaCha20Poly1305,
            _ => {
                return Err(PcapError::TlsKey(format!(
                    "unsupported cipher suite: 0x{:04x}",
                    session.cipher_suite_id
                )))
            }
        };

        let is_tls13 = session.is_tls13();

        // Derive keys based on TLS version
        let (client_key, client_iv, server_key, server_iv) = if is_tls13 {
            // TLS 1.3: use traffic secret to derive keys
            match key_material {
                KeyMaterial::ClientTrafficSecret0(secret) => {
                    let keys =
                        derive_tls13_keys(secret, session.key_len(), session.uses_sha384())?;
                    // For TLS 1.3, we need both client and server secrets
                    // For now, use client secret for both (this is a limitation)
                    (keys.key.clone(), keys.iv.clone(), keys.key, keys.iv)
                }
                KeyMaterial::ServerTrafficSecret0(secret) => {
                    let keys =
                        derive_tls13_keys(secret, session.key_len(), session.uses_sha384())?;
                    // Use server secret for both directions (limitation)
                    (keys.key.clone(), keys.iv.clone(), keys.key, keys.iv)
                }
                KeyMaterial::MasterSecret(_) => {
                    return Err(PcapError::TlsKey(
                        "TLS 1.3 requires traffic secrets, not master secret".to_string(),
                    ))
                }
            }
        } else {
            // TLS 1.2: use master secret to derive keys
            match key_material {
                KeyMaterial::MasterSecret(master_secret) => {
                    let keys = derive_tls12_keys(
                        master_secret,
                        &session.client_random,
                        &session.server_random,
                        session.key_len(),
                        session.iv_len(),
                    )?;
                    (
                        keys.client_write_key,
                        keys.client_write_iv,
                        keys.server_write_key,
                        keys.server_write_iv,
                    )
                }
                _ => {
                    return Err(PcapError::TlsKey(
                        "TLS 1.2 requires master secret, not traffic secrets".to_string(),
                    ))
                }
            }
        };

        Ok(Self {
            cipher,
            client_key,
            client_iv,
            server_key,
            server_iv,
            is_tls13,
        })
    }

    /// Decrypts all TLS records in a stream.
    ///
    /// Returns concatenated plaintext from all Application Data records.
    pub fn decrypt_stream(&self, data: &[u8]) -> Result<Vec<u8>, PcapError> {
        let mut plaintext = Vec::new();
        let mut offset = 0;
        let mut sequence = 0u64;

        while offset < data.len() {
            match parse_tls_plaintext(&data[offset..]) {
                Ok((rem, record)) => {
                    let consumed = data[offset..].len() - rem.len();

                    // Only decrypt Application Data records
                    if record.hdr.record_type == TlsRecordType::ApplicationData {
                        // Extract encrypted payload from record
                        for msg in record.msg {
                            if let TlsMessage::ApplicationData(app_data) = msg {
                                // Decrypt this record
                                let decrypted =
                                    self.decrypt_record(app_data.blob, sequence, &record.hdr)?;
                                plaintext.extend_from_slice(&decrypted);
                                sequence += 1;
                            }
                        }
                    }

                    offset += consumed;
                }
                Err(_) => {
                    // Failed to parse - stop processing
                    break;
                }
            }
        }

        Ok(plaintext)
    }

    /// Decrypts a single TLS record.
    fn decrypt_record(
        &self,
        ciphertext: &[u8],
        sequence: u64,
        record_hdr: &tls_parser::TlsRecordHeader,
    ) -> Result<Vec<u8>, PcapError> {
        // Select key and IV based on direction (heuristic: assume client->server for now)
        // In a real implementation, we'd track connection direction
        let key = &self.client_key;
        let iv = &self.client_iv;

        // Construct nonce
        let nonce = self.construct_nonce(iv, sequence, ciphertext)?;

        // Construct AAD (Additional Authenticated Data)
        let aad = self.construct_aad(record_hdr, ciphertext.len());

        // Decrypt using ring AEAD
        let mut in_out = ciphertext.to_vec();
        let unbound_key = UnboundKey::new(self.cipher.algorithm(), key)
            .map_err(|e| PcapError::TlsKey(format!("failed to create key: {:?}", e)))?;

        let nonce_obj = Nonce::try_assume_unique_for_key(&nonce)
            .map_err(|e| PcapError::TlsKey(format!("invalid nonce: {:?}", e)))?;

        let mut opening_key = OpeningKey::new(unbound_key, FixedNonceSequence(Some(nonce_obj)));

        let plaintext_len = opening_key
            .open_in_place(Aad::from(&aad), &mut in_out)
            .map_err(|e| PcapError::TlsKey(format!("decryption failed: {:?}", e)))?
            .len();

        in_out.truncate(plaintext_len);
        Ok(in_out)
    }

    /// Constructs the nonce for AEAD decryption.
    fn construct_nonce(
        &self,
        iv: &[u8],
        sequence: u64,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, PcapError> {
        if self.is_tls13 {
            // TLS 1.3: XOR IV with sequence number (padded to 12 bytes)
            if iv.len() != 12 {
                return Err(PcapError::TlsKey(format!(
                    "invalid IV length for TLS 1.3: {} (expected 12)",
                    iv.len()
                )));
            }

            let mut nonce = iv.to_vec();
            let seq_bytes = sequence.to_be_bytes();

            // XOR the last 8 bytes of the nonce with the sequence number
            for i in 0..8 {
                nonce[4 + i] ^= seq_bytes[i];
            }

            Ok(nonce)
        } else {
            // TLS 1.2: explicit nonce (first 8 bytes of ciphertext) + implicit IV
            if ciphertext.len() < 8 {
                return Err(PcapError::TlsKey(
                    "ciphertext too short for TLS 1.2 explicit nonce".to_string(),
                ));
            }

            let explicit_nonce = &ciphertext[..8];
            let mut nonce = Vec::with_capacity(12);
            nonce.extend_from_slice(iv); // 4-byte implicit IV
            nonce.extend_from_slice(explicit_nonce); // 8-byte explicit nonce
            Ok(nonce)
        }
    }

    /// Constructs the AAD (Additional Authenticated Data) for AEAD.
    fn construct_aad(&self, record_hdr: &tls_parser::TlsRecordHeader, ciphertext_len: usize) -> Vec<u8> {
        if self.is_tls13 {
            // TLS 1.3 AAD: record header (5 bytes)
            vec![
                u8::from(record_hdr.record_type),
                (record_hdr.version.0 >> 8) as u8,
                (record_hdr.version.0 & 0xff) as u8,
                (ciphertext_len >> 8) as u8,
                (ciphertext_len & 0xff) as u8,
            ]
        } else {
            // TLS 1.2 AAD: record header (5 bytes) but with plaintext length
            // For TLS 1.2, AAD uses the ciphertext length minus explicit nonce and auth tag
            let plaintext_len = ciphertext_len.saturating_sub(8 + 16); // 8-byte nonce, 16-byte tag
            vec![
                u8::from(record_hdr.record_type),
                (record_hdr.version.0 >> 8) as u8,
                (record_hdr.version.0 & 0xff) as u8,
                (plaintext_len >> 8) as u8,
                (plaintext_len & 0xff) as u8,
            ]
        }
    }
}

/// A nonce sequence that returns a single fixed nonce.
struct FixedNonceSequence(Option<Nonce>);

impl NonceSequence for FixedNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        self.0.take().ok_or(ring::error::Unspecified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tls13_nonce_construction() {
        let iv = vec![0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb];
        let sequence = 0x0000000000000001u64;

        // Mock decryptor
        let decryptor = TlsDecryptor {
            cipher: AeadCipher::Aes128Gcm,
            client_key: vec![0u8; 16],
            client_iv: iv.clone(),
            server_key: vec![0u8; 16],
            server_iv: iv.clone(),
            is_tls13: true,
        };

        let nonce = decryptor.construct_nonce(&iv, sequence, &[0u8; 32]).unwrap();

        // Verify nonce is IV XOR'd with sequence number
        assert_eq!(nonce.len(), 12);
        assert_eq!(nonce[0..4], iv[0..4]);
        // Last 8 bytes should be XOR'd
        assert_eq!(nonce[4], iv[4] ^ 0x00);
        assert_eq!(nonce[11], iv[11] ^ 0x01);
    }

    #[test]
    fn test_tls12_nonce_construction() {
        let iv = vec![0x00, 0x11, 0x22, 0x33];
        let ciphertext = vec![0xaa; 32]; // First 8 bytes are explicit nonce

        // Mock decryptor
        let decryptor = TlsDecryptor {
            cipher: AeadCipher::Aes128Gcm,
            client_key: vec![0u8; 16],
            client_iv: iv.clone(),
            server_key: vec![0u8; 16],
            server_iv: iv.clone(),
            is_tls13: false,
        };

        let nonce = decryptor.construct_nonce(&iv, 0, &ciphertext).unwrap();

        // Verify nonce is implicit IV + explicit nonce (first 8 bytes of ciphertext)
        assert_eq!(nonce.len(), 12);
        assert_eq!(&nonce[0..4], &iv[..]);
        assert_eq!(&nonce[4..12], &ciphertext[0..8]);
    }
}
