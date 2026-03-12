//! TLS session parsing and cipher identification.
//!
//! Extracts session information from TLS handshake:
//! - Client Random (32 bytes)
//! - Server Random (32 bytes)
//! - Cipher suite
//! - TLS version

use crate::error::PcapError;
use tls_parser::{TlsMessage, TlsMessageHandshake, TlsVersion, parse_tls_plaintext};

/// TLS session information extracted from handshake.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Client random (32 bytes).
    pub client_random: Vec<u8>,
    /// Server random (32 bytes).
    pub server_random: Vec<u8>,
    /// Negotiated cipher suite ID.
    pub cipher_suite_id: u16,
    /// TLS protocol version.
    pub version: TlsVersion,
}

impl SessionInfo {
    /// Returns the key length in bytes for the cipher suite.
    #[must_use] 
    pub const fn key_len(&self) -> usize {
        match self.cipher_suite_id {
            // AES-128-GCM (16 bytes)
            0x009C | 0x009E | 0xC02F | 0xC02B | 0x1301 => 16,
            // AES-256-GCM (32 bytes)
            0x009D | 0x009F | 0xC030 | 0xC02C | 0x1302 => 32,
            // ChaCha20-Poly1305 (32 bytes)
            0x1303 | 0xCCA8 | 0xCCA9 => 32,
            _ => 0, // Unknown or unsupported cipher
        }
    }

    /// Returns the fixed IV length in bytes for TLS 1.2 AEAD ciphers.
    ///
    /// TLS 1.2 uses 4-byte implicit IV (fixed IV).
    /// TLS 1.3 uses 12-byte IV derived via HKDF.
    #[must_use] 
    pub const fn iv_len(&self) -> usize {
        match self.version {
            TlsVersion::Tls12 => 4,  // TLS 1.2 uses 4-byte fixed IV
            TlsVersion::Tls13 => 12, // TLS 1.3 uses 12-byte IV
            _ => 0,
        }
    }

    /// Returns true if this is a TLS 1.3 cipher suite.
    #[must_use] 
    pub fn is_tls13(&self) -> bool {
        // TLS 1.3 cipher suites: 0x1301-0x1305
        (0x1301..=0x1305).contains(&self.cipher_suite_id)
    }

    /// Returns true if the cipher suite uses SHA384 (for TLS 1.3 HKDF selection).
    #[must_use] 
    pub const fn uses_sha384(&self) -> bool {
        matches!(
            self.cipher_suite_id,
            0x009D | 0x009F | 0xC030 | 0xC02C | 0x1302
        )
    }

    /// Returns true if this cipher suite is supported for decryption.
    #[must_use] 
    pub const fn is_supported(&self) -> bool {
        self.key_len() > 0
    }
}

/// TLS session parser.
pub struct TlsSession;

impl TlsSession {
    /// Parses a TLS stream and extracts session information from handshake.
    ///
    /// Looks for `ClientHello` and `ServerHello` messages to extract randoms and cipher suite.
    pub fn from_stream(data: &[u8]) -> Result<SessionInfo, PcapError> {
        let mut client_random: Option<Vec<u8>> = None;
        let mut server_random: Option<Vec<u8>> = None;
        let mut cipher_suite_id: Option<u16> = None;
        let mut version: Option<TlsVersion> = None;

        // Parse TLS records from stream
        let mut offset = 0;
        while offset < data.len() {
            match parse_tls_plaintext(&data[offset..]) {
                Ok((rem, record)) => {
                    // Calculate consumed bytes
                    let consumed = data[offset..].len() - rem.len();

                    // Process each message in the record
                    for msg in record.msg {
                        if let TlsMessage::Handshake(handshake) = msg {
                            match handshake {
                                TlsMessageHandshake::ClientHello(ch) => {
                                    // Extract client random (32 bytes)
                                    if ch.random.len() == 32 {
                                        client_random = Some(ch.random.to_vec());
                                    }
                                    // Record TLS version from ClientHello
                                    if version.is_none() {
                                        version = Some(ch.version);
                                    }
                                }
                                TlsMessageHandshake::ServerHello(sh) => {
                                    // Extract server random (32 bytes)
                                    if sh.random.len() == 32 {
                                        server_random = Some(sh.random.to_vec());
                                    }
                                    // Extract negotiated cipher suite (as u16)
                                    cipher_suite_id = Some(sh.cipher.0);
                                    // Update version from ServerHello (authoritative)
                                    version = Some(sh.version);
                                }
                                _ => {}
                            }
                        }
                    }

                    offset += consumed;

                    // If we have all required fields, we can return early
                    if client_random.is_some()
                        && server_random.is_some()
                        && cipher_suite_id.is_some()
                        && version.is_some()
                    {
                        break;
                    }
                }
                Err(_) => {
                    // Failed to parse - might not be a TLS stream or incomplete handshake
                    break;
                }
            }
        }

        // Validate that we have all required fields
        let client_random =
            client_random.ok_or_else(|| PcapError::TlsKey("ClientHello not found".to_string()))?;
        let server_random =
            server_random.ok_or_else(|| PcapError::TlsKey("ServerHello not found".to_string()))?;
        let cipher_suite_id = cipher_suite_id
            .ok_or_else(|| PcapError::TlsKey("cipher suite not found".to_string()))?;
        let version =
            version.ok_or_else(|| PcapError::TlsKey("TLS version not found".to_string()))?;

        Ok(SessionInfo {
            client_random,
            server_random,
            cipher_suite_id,
            version,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_info_aes128_gcm() {
        let session = SessionInfo {
            client_random: vec![0u8; 32],
            server_random: vec![0u8; 32],
            cipher_suite_id: 0x1301, // TLS_AES_128_GCM_SHA256
            version: TlsVersion::Tls13,
        };

        assert_eq!(session.key_len(), 16);
        assert_eq!(session.iv_len(), 12);
        assert!(session.is_tls13());
        assert!(session.is_supported());
        assert!(!session.uses_sha384());
    }

    #[test]
    fn test_session_info_aes256_gcm() {
        let session = SessionInfo {
            client_random: vec![0u8; 32],
            server_random: vec![0u8; 32],
            cipher_suite_id: 0xC030, // TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
            version: TlsVersion::Tls12,
        };

        assert_eq!(session.key_len(), 32);
        assert_eq!(session.iv_len(), 4);
        assert!(!session.is_tls13());
        assert!(session.uses_sha384());
        assert!(session.is_supported());
    }

    #[test]
    fn test_session_info_chacha20() {
        let session = SessionInfo {
            client_random: vec![0u8; 32],
            server_random: vec![0u8; 32],
            cipher_suite_id: 0x1303, // TLS_CHACHA20_POLY1305_SHA256
            version: TlsVersion::Tls13,
        };

        assert_eq!(session.key_len(), 32);
        assert_eq!(session.iv_len(), 12);
        assert!(session.is_tls13());
        assert!(session.is_supported());
    }
}
