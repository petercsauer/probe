//! gRPC Length-Prefixed-Message (LPM) parsing.
//!
//! gRPC uses a 5-byte header followed by the message payload:
//! - 1 byte: compressed flag (0 = uncompressed, 1 = compressed)
//! - 4 bytes: message length (big-endian u32)
//! - N bytes: message payload
//!
//! Messages may span multiple HTTP/2 DATA frames.

use crate::error::GrpcError;
use bytes::Bytes;
use std::io::Read;

/// gRPC compression algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// No compression (identity).
    Identity,
    /// gzip compression.
    Gzip,
    /// deflate compression.
    Deflate,
}

impl CompressionAlgorithm {
    /// Parse compression algorithm from gRPC `grpc-encoding` header value.
    pub fn from_header(value: &str) -> Self {
        match value {
            "gzip" => Self::Gzip,
            "deflate" => Self::Deflate,
            _ => Self::Identity,
        }
    }
}

/// A parsed gRPC message.
#[derive(Debug, Clone)]
pub struct GrpcMessage {
    /// The decompressed message payload.
    pub payload: Bytes,
}

/// Parser for gRPC Length-Prefixed-Messages.
///
/// Handles message accumulation across multiple DATA frames.
pub struct LpmParser {
    /// Buffer for accumulating partial message data.
    buffer: Vec<u8>,
    /// Expected compression algorithm from grpc-encoding header.
    compression: CompressionAlgorithm,
}

impl LpmParser {
    /// Create a new LPM parser with the given compression algorithm.
    pub const fn new(compression: CompressionAlgorithm) -> Self {
        Self {
            buffer: Vec::new(),
            compression,
        }
    }

    /// Feed data into the parser and extract complete messages.
    ///
    /// Returns a vector of complete messages. Incomplete messages are buffered
    /// internally and will be returned on subsequent calls.
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<GrpcMessage>, GrpcError> {
        // Append new data to buffer
        self.buffer.extend_from_slice(data);

        let mut messages = Vec::new();

        // Parse complete messages from buffer
        loop {
            // Need at least 5 bytes for the LPM header
            if self.buffer.len() < 5 {
                break;
            }

            // Read LPM header
            let compressed_flag = self.buffer[0];
            let message_length = u32::from_be_bytes([
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
                self.buffer[4],
            ]) as usize;

            // Check if we have the complete message
            let total_length = 5 + message_length;
            if self.buffer.len() < total_length {
                // Message incomplete, wait for more data
                break;
            }

            // Extract message payload
            let payload_bytes = self.buffer[5..total_length].to_vec();

            // Remove parsed message from buffer
            self.buffer.drain(0..total_length);

            // Decompress if needed
            let payload = if compressed_flag == 1 {
                self.decompress(&payload_bytes)?
            } else {
                Bytes::from(payload_bytes)
            };

            messages.push(GrpcMessage { payload });
        }

        Ok(messages)
    }

    /// Decompress a payload using the configured compression algorithm.
    fn decompress(&self, data: &[u8]) -> Result<Bytes, GrpcError> {
        match self.compression {
            CompressionAlgorithm::Identity => Ok(Bytes::copy_from_slice(data)),
            CompressionAlgorithm::Gzip => {
                let mut decoder = flate2::read::GzDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(|e| GrpcError::DecompressionError(e.to_string()))?;
                Ok(Bytes::from(decompressed))
            }
            CompressionAlgorithm::Deflate => {
                // gRPC "deflate" is actually zlib (RFC 1950), not raw deflate (RFC 1951)
                let mut decoder = flate2::read::ZlibDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder
                    .read_to_end(&mut decompressed)
                    .map_err(|e| GrpcError::DecompressionError(e.to_string()))?;
                Ok(Bytes::from(decompressed))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lpm_single_message() {
        let mut parser = LpmParser::new(CompressionAlgorithm::Identity);

        // Create a test message: compressed=0, length=5, payload="hello"
        let mut data = vec![0u8]; // compressed flag
        data.extend_from_slice(&5u32.to_be_bytes()); // length
        data.extend_from_slice(b"hello"); // payload

        let messages = parser.feed(&data).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(&messages[0].payload[..], b"hello");
    }

    #[test]
    fn test_lpm_multi_frame() {
        let mut parser = LpmParser::new(CompressionAlgorithm::Identity);

        // Create a test message split across two frames
        let mut frame1 = vec![0u8]; // compressed flag
        frame1.extend_from_slice(&10u32.to_be_bytes()); // length
        frame1.extend_from_slice(b"hello"); // partial payload

        let frame2 = b"world"; // rest of payload

        // Feed first frame - should not produce a message yet
        let messages = parser.feed(&frame1).unwrap();
        assert_eq!(messages.len(), 0);

        // Feed second frame - should produce complete message
        let messages = parser.feed(frame2).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(&messages[0].payload[..], b"helloworld");
    }

    #[test]
    fn test_lpm_multiple_messages() {
        let mut parser = LpmParser::new(CompressionAlgorithm::Identity);

        // Create two messages in one buffer
        let mut data = Vec::new();

        // Message 1: "hi"
        data.push(0u8);
        data.extend_from_slice(&2u32.to_be_bytes());
        data.extend_from_slice(b"hi");

        // Message 2: "bye"
        data.push(0u8);
        data.extend_from_slice(&3u32.to_be_bytes());
        data.extend_from_slice(b"bye");

        let messages = parser.feed(&data).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(&messages[0].payload[..], b"hi");
        assert_eq!(&messages[1].payload[..], b"bye");
    }
}
