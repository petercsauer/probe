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
use std::sync::Arc;

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
    /// Capture timestamp of the first packet in this stream (microseconds since epoch).
    pub timestamp_us: u64,
}

/// TLS stream processor that combines session parsing, key derivation, and decryption.
pub struct TlsStreamProcessor {
    keylog: Arc<TlsKeyLog>,
}

impl TlsStreamProcessor {
    /// Creates a new TLS stream processor with an empty key log.
    pub fn new() -> Self {
        Self {
            keylog: Arc::new(TlsKeyLog::new()),
        }
    }

    /// Creates a new TLS stream processor with an existing key log.
    pub fn with_keylog(keylog: TlsKeyLog) -> Self {
        Self {
            keylog: Arc::new(keylog),
        }
    }

    /// Creates a new TLS stream processor with a shared keylog reference.
    ///
    /// This is useful for parallel processing where multiple shards need to
    /// share the same keylog.
    pub fn with_keylog_ref(keylog: Arc<TlsKeyLog>) -> Self {
        Self { keylog }
    }

    /// Returns a mutable reference to the key log for adding keys.
    ///
    /// Note: If the Arc has multiple references, this will clone the keylog
    /// to maintain uniqueness. For parallel processing, prefer using a shared
    /// keylog via `with_keylog_ref`.
    pub fn keylog_mut(&mut self) -> &mut TlsKeyLog {
        Arc::make_mut(&mut self.keylog)
    }

    /// Decrypts a reassembled TCP stream using TLS keys.
    ///
    /// This method is thread-safe (&self) and can be called concurrently from
    /// multiple threads sharing the same keylog via Arc.
    ///
    /// If the stream contains a TLS handshake and matching key material is available,
    /// returns a decrypted stream. Otherwise, returns the stream as encrypted.
    pub fn decrypt_stream(&self, stream: ReassembledStream) -> Result<DecryptedStream, PcapError> {
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
            timestamp_us: stream.timestamp_us,
        })
    }
}

impl Default for TlsStreamProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tcp::StreamDirection;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn test_tls_decrypt_is_send_sync() {
        // Static assertion that TlsStreamProcessor implements Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TlsStreamProcessor>();
    }

    #[test]
    fn test_tls_decrypt_parallel_same_keylog() {
        use std::sync::Arc;
        use std::thread;

        // Create a shared keylog
        let keylog = Arc::new(TlsKeyLog::new());

        // Create multiple processors sharing the same keylog
        let processor1 = TlsStreamProcessor::with_keylog_ref(Arc::clone(&keylog));
        let processor2 = TlsStreamProcessor::with_keylog_ref(Arc::clone(&keylog));

        // Create test streams
        let stream1 = ReassembledStream {
            src_ip: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            src_port: 12345,
            dst_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            dst_port: 443,
            direction: StreamDirection::ClientToServer,
            data: vec![0x16, 0x03, 0x03, 0x00, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05], // Invalid TLS, but doesn't matter
            is_complete: true,
            missing_ranges: vec![],
            timestamp_us: 1000,
        };

        let stream2 = stream1.clone();

        // Process streams in parallel threads
        let handle1 = thread::spawn(move || {
            processor1.decrypt_stream(stream1)
        });

        let handle2 = thread::spawn(move || {
            processor2.decrypt_stream(stream2)
        });

        // Both should complete without panic
        let result1 = handle1.join().expect("Thread 1 panicked");
        let result2 = handle2.join().expect("Thread 2 panicked");

        // Both should succeed (no keys means encrypted passthrough)
        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert!(result1.unwrap().encrypted);
        assert!(result2.unwrap().encrypted);
    }
}
