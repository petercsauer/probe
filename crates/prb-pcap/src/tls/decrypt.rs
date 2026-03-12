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
use ring::aead::{
    AES_128_GCM, AES_256_GCM, Aad, BoundKey, CHACHA20_POLY1305, Nonce, NonceSequence, OpeningKey,
    UnboundKey,
};
use tls_parser::{TlsMessage, TlsRecordType, parse_tls_plaintext};

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
    server_key: Vec<u8>,
    server_iv: Vec<u8>,
    is_tls13: bool,
}

impl TlsDecryptor {
    /// Creates a new TLS decryptor from session info and key materials.
    ///
    /// For TLS 1.2, only the first key material (master secret) is used.
    /// For TLS 1.3, searches for both client and server traffic secrets.
    pub fn new(session: &SessionInfo, key_materials: &[KeyMaterial]) -> Result<Self, PcapError> {
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
                )));
            }
        };

        let is_tls13 = session.is_tls13();

        // Derive keys based on TLS version
        let (client_key, client_iv, server_key, server_iv) = if is_tls13 {
            // TLS 1.3: find both client and server traffic secrets
            let client_secret = key_materials
                .iter()
                .find(|m| matches!(m, KeyMaterial::ClientTrafficSecret0(_)));
            let server_secret = key_materials
                .iter()
                .find(|m| matches!(m, KeyMaterial::ServerTrafficSecret0(_)));

            let (client_key, client_iv) =
                if let Some(KeyMaterial::ClientTrafficSecret0(secret)) = client_secret {
                    let keys = derive_tls13_keys(secret, session.key_len(), session.uses_sha384())?;
                    (keys.key, keys.iv)
                } else {
                    // No client secret - use empty keys (decryption will fail gracefully)
                    (vec![0u8; session.key_len()], vec![0u8; 12])
                };

            let (server_key, server_iv) =
                if let Some(KeyMaterial::ServerTrafficSecret0(secret)) = server_secret {
                    let keys = derive_tls13_keys(secret, session.key_len(), session.uses_sha384())?;
                    (keys.key, keys.iv)
                } else {
                    // No server secret - use empty keys (decryption will fail gracefully)
                    (vec![0u8; session.key_len()], vec![0u8; 12])
                };

            (client_key, client_iv, server_key, server_iv)
        } else {
            // TLS 1.2: use master secret to derive keys
            let master_secret = key_materials
                .iter()
                .find(|m| matches!(m, KeyMaterial::MasterSecret(_)));

            match master_secret {
                Some(KeyMaterial::MasterSecret(master_secret)) => {
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
                        "TLS 1.2 requires master secret".to_string(),
                    ));
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

    /// Decrypts a single AEAD-encrypted payload directly (without TLS record framing).
    ///
    /// For TLS 1.3: `ciphertext` is the raw encrypted content + auth tag.
    /// For TLS 1.2: `ciphertext` is explicit_nonce (8 bytes) + encrypted content + auth tag.
    pub fn decrypt_aead(
        &self,
        ciphertext: &[u8],
        sequence: u64,
        content_type: u8,
        version: u16,
        direction: crate::tcp::StreamDirection,
    ) -> Result<Vec<u8>, PcapError> {
        let hdr = tls_parser::TlsRecordHeader {
            record_type: tls_parser::TlsRecordType(content_type),
            version: tls_parser::TlsVersion(version),
            len: ciphertext.len() as u16,
        };
        self.decrypt_record(ciphertext, sequence, &hdr, direction)
    }

    /// Decrypts all TLS records in a stream.
    ///
    /// Returns concatenated plaintext from all Application Data records.
    pub fn decrypt_stream(
        &self,
        data: &[u8],
        direction: crate::tcp::StreamDirection,
    ) -> Result<Vec<u8>, PcapError> {
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
                                let decrypted = self.decrypt_record(
                                    app_data.blob,
                                    sequence,
                                    &record.hdr,
                                    direction,
                                )?;
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
        direction: crate::tcp::StreamDirection,
    ) -> Result<Vec<u8>, PcapError> {
        let (key, iv) = match direction {
            crate::tcp::StreamDirection::ClientToServer => (&self.client_key, &self.client_iv),
            crate::tcp::StreamDirection::ServerToClient => (&self.server_key, &self.server_iv),
        };

        let nonce = self.construct_nonce(iv, sequence, ciphertext)?;
        let aad = self.construct_aad(record_hdr, ciphertext.len(), sequence);

        // For TLS 1.2 GCM, the record payload is: explicit_nonce (8) + encrypted + tag (16).
        // ring expects only encrypted + tag, so strip the 8-byte explicit nonce prefix.
        let decrypt_input = if self.is_tls13 {
            ciphertext
        } else {
            if ciphertext.len() < 8 {
                return Err(PcapError::TlsKey(
                    "ciphertext too short for TLS 1.2 explicit nonce".to_string(),
                ));
            }
            &ciphertext[8..]
        };

        let mut in_out = decrypt_input.to_vec();
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
    ///
    /// TLS 1.2 (RFC 5246 Section 6.2.3.3): seq_num (8) + type (1) + version (2) + length (2) = 13 bytes
    /// TLS 1.3 (RFC 8446 Section 5.2): type (1) + version (2) + length (2) = 5 bytes
    fn construct_aad(
        &self,
        record_hdr: &tls_parser::TlsRecordHeader,
        ciphertext_len: usize,
        sequence: u64,
    ) -> Vec<u8> {
        if self.is_tls13 {
            vec![
                u8::from(record_hdr.record_type),
                (record_hdr.version.0 >> 8) as u8,
                (record_hdr.version.0 & 0xff) as u8,
                (ciphertext_len >> 8) as u8,
                (ciphertext_len & 0xff) as u8,
            ]
        } else {
            // TLS 1.2 AEAD AAD: seq_num (8 bytes) + content_type + version + plaintext_length
            let plaintext_len = ciphertext_len.saturating_sub(8 + 16); // 8-byte explicit nonce, 16-byte tag
            let seq_bytes = sequence.to_be_bytes();
            let mut aad = Vec::with_capacity(13);
            aad.extend_from_slice(&seq_bytes);
            aad.push(u8::from(record_hdr.record_type));
            aad.push((record_hdr.version.0 >> 8) as u8);
            aad.push((record_hdr.version.0 & 0xff) as u8);
            aad.push((plaintext_len >> 8) as u8);
            aad.push((plaintext_len & 0xff) as u8);
            aad
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
        let iv = vec![
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb,
        ];
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

        let nonce = decryptor
            .construct_nonce(&iv, sequence, &[0u8; 32])
            .unwrap();

        // Verify nonce is IV XOR'd with sequence number
        assert_eq!(nonce.len(), 12);
        assert_eq!(nonce[0..4], iv[0..4]);
        // Last 8 bytes should be XOR'd with sequence number (1)
        assert_eq!(nonce[4], iv[4]); // High bytes of seq number (0x00)
        assert_eq!(nonce[11], iv[11] ^ 0x01); // Low byte of seq number
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
