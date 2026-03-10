//! TLS offline decryption for PRB.
//!
//! This module provides TLS 1.2 and 1.3 decryption using key material from:
//! - SSLKEYLOGFILE (NSS key log format)
//! - pcapng Decryption Secrets Blocks (DSB)
//!
//! Supports AEAD cipher suites:
//! - AES-128-GCM
//! - AES-256-GCM
//! - ChaCha20-Poly1305
//!
//! Architecture follows `pcapsql-core` reference implementation:
//! - keylog: parse SSLKEYLOGFILE + DSB keys
//! - session: parse TLS handshake, extract client_random/server_random/cipher
//! - kdf: TLS 1.2 PRF and TLS 1.3 HKDF-Expand-Label
//! - decrypt: AEAD decryption with per-record nonce construction

pub mod decrypt;
pub mod kdf;
pub mod keylog;
pub mod session;

pub use decrypt::TlsDecryptor;
pub use kdf::{derive_tls12_keys, derive_tls13_keys};
pub use keylog::TlsKeyLog;
pub use session::TlsSession;

use crate::error::PcapError;
use crate::tcp::ReassembledStream;

/// A decrypted TLS stream.
#[derive(Debug, Clone)]
pub struct DecryptedStream {
    /// Original connection 4-tuple (client perspective).
    pub src_ip: std::net::IpAddr,
    pub src_port: u16,
    pub dst_ip: std::net::IpAddr,
    pub dst_port: u16,
    /// Direction of this stream segment.
    pub direction: crate::tcp::StreamDirection,
    /// Decrypted application data (concatenated from all records).
    pub data: Vec<u8>,
    /// Whether decryption was successful.
    pub encrypted: bool,
    /// Whether the stream is complete (FIN or RST seen).
    pub is_complete: bool,
}

/// TLS stream processor that combines session parsing, key derivation, and decryption.
pub struct TlsStreamProcessor {
    keylog: TlsKeyLog,
}

impl TlsStreamProcessor {
    /// Creates a new TLS stream processor with an empty key log.
    pub fn new() -> Self {
        Self {
            keylog: TlsKeyLog::new(),
        }
    }

    /// Creates a new TLS stream processor with an existing key log.
    pub fn with_keylog(keylog: TlsKeyLog) -> Self {
        Self { keylog }
    }

    /// Returns a mutable reference to the key log for adding keys.
    pub fn keylog_mut(&mut self) -> &mut TlsKeyLog {
        &mut self.keylog
    }

    /// Processes a reassembled TCP stream and attempts TLS decryption.
    ///
    /// If the stream contains a TLS handshake and matching key material is available,
    /// returns a decrypted stream. Otherwise, returns the stream as encrypted.
    pub fn process_stream(&mut self, stream: ReassembledStream) -> Result<DecryptedStream, PcapError> {
        // Try to parse TLS session from stream data
        let session_result = TlsSession::from_stream(&stream.data);

        let (data, encrypted) = match session_result {
            Ok(session) => {
                // Look up key material using client_random
                if let Some(key_materials) = self.keylog.lookup(&session.client_random) {
                    // Create decryptor based on cipher suite
                    let decryptor = TlsDecryptor::new(&session, key_materials)?;

                    // Decrypt all records in the stream
                    match decryptor.decrypt_stream(&stream.data, stream.direction) {
                        Ok(decrypted) => (decrypted, false),
                        Err(_) => {
                            // Decryption failed - pass through as encrypted
                            (stream.data, true)
                        }
                    }
                } else {
                    // No key material - pass through as encrypted
                    (stream.data, true)
                }
            }
            Err(_) => {
                // Not a TLS stream or handshake not found - pass through as encrypted
                (stream.data, true)
            }
        };

        Ok(DecryptedStream {
            src_ip: stream.src_ip,
            src_port: stream.src_port,
            dst_ip: stream.dst_ip,
            dst_port: stream.dst_port,
            direction: stream.direction,
            data,
            encrypted,
            is_complete: stream.is_complete,
        })
    }
}

impl Default for TlsStreamProcessor {
    fn default() -> Self {
        Self::new()
    }
}
