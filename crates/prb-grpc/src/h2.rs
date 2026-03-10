//! HTTP/2 frame parsing with h2-sans-io and graceful HPACK degradation.
//!
//! This module wraps h2-sans-io to provide HTTP/2 frame parsing for offline
//! PCAP analysis. It implements graceful degradation for mid-stream captures
//! where HPACK dynamic table context is unavailable.

use crate::error::GrpcError;
use bytes::Bytes;
use std::collections::HashMap;

/// HTTP/2 stream state.
#[derive(Debug)]
pub struct StreamState {
    /// Request headers (method, authority, path, etc.).
    pub request_headers: HashMap<String, String>,
    /// Response headers.
    pub response_headers: HashMap<String, String>,
    /// Trailing headers (grpc-status, grpc-message).
    pub trailers: HashMap<String, String>,
    /// Whether the stream has seen the initial request headers.
    pub saw_request_headers: bool,
    /// Whether the stream has seen response headers.
    pub saw_response_headers: bool,
    /// Whether the stream is closed.
    pub closed: bool,
}

impl StreamState {
    fn new() -> Self {
        Self {
            request_headers: HashMap::new(),
            response_headers: HashMap::new(),
            trailers: HashMap::new(),
            saw_request_headers: false,
            saw_response_headers: false,
            closed: false,
        }
    }
}

/// HTTP/2 event parsed from the stream.
#[derive(Debug)]
pub enum H2Event {
    /// HEADERS frame received.
    Headers {
        stream_id: u32,
        headers: HashMap<String, String>,
        end_stream: bool,
    },
    /// DATA frame received.
    Data {
        stream_id: u32,
        data: Bytes,
        end_stream: bool,
    },
    /// SETTINGS frame received.
    Settings,
    /// RST_STREAM frame received.
    RstStream { stream_id: u32 },
    /// GOAWAY frame received.
    GoAway,
    /// HPACK degradation warning.
    HpackDegraded { reason: String },
}

/// HTTP/2 codec for parsing frames from a byte stream.
pub struct H2Codec {
    /// Per-stream state tracking.
    streams: HashMap<u32, StreamState>,
    /// Buffer for accumulating partial frames.
    buffer: Vec<u8>,
    /// Whether HPACK degradation has occurred.
    hpack_degraded: bool,
    /// Preface sent flag (HTTP/2 connection preface).
    preface_seen: bool,
    /// Buffer for accumulating header block fragments (HEADERS + CONTINUATION).
    header_block_buffer: Option<(u32, Vec<u8>, bool)>, // (stream_id, buffer, end_stream)
}

impl H2Codec {
    /// Create a new HTTP/2 codec.
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
            buffer: Vec::new(),
            hpack_degraded: false,
            preface_seen: false,
            header_block_buffer: None,
        }
    }

    /// Process a chunk of bytes and return parsed events.
    ///
    /// This method feeds bytes into the HTTP/2 frame parser and returns
    /// zero or more events. Partial frames are buffered internally.
    pub fn process(&mut self, data: &[u8]) -> Result<Vec<H2Event>, GrpcError> {
        // For now, implement a basic frame parser
        // TODO: Replace with h2-sans-io integration when API is confirmed

        self.buffer.extend_from_slice(data);
        let mut events = Vec::new();

        // Parse HTTP/2 connection preface if not seen yet
        if !self.preface_seen && self.buffer.len() >= 24 {
            const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
            if self.buffer.starts_with(PREFACE) {
                self.buffer.drain(0..24);
                self.preface_seen = true;
            }
        }

        // Parse frames
        while self.buffer.len() >= 9 {
            // HTTP/2 frame header: 9 bytes
            // - 3 bytes: length (24-bit big-endian)
            // - 1 byte: type
            // - 1 byte: flags
            // - 4 bytes: stream ID (31-bit, reserved bit must be 0)

            let length = u32::from_be_bytes([0, self.buffer[0], self.buffer[1], self.buffer[2]]) as usize;
            let frame_type = self.buffer[3];
            let flags = self.buffer[4];
            let stream_id = u32::from_be_bytes([
                self.buffer[5] & 0x7F, // Clear reserved bit
                self.buffer[6],
                self.buffer[7],
                self.buffer[8],
            ]);

            // Check if we have the complete frame
            if self.buffer.len() < 9 + length {
                break; // Wait for more data
            }

            // Extract frame payload (copy to avoid borrow issues)
            let payload = self.buffer[9..9 + length].to_vec();

            // Parse frame based on type
            match frame_type {
                0x00 => {
                    // DATA frame
                    let end_stream = (flags & 0x01) != 0;
                    events.push(H2Event::Data {
                        stream_id,
                        data: Bytes::copy_from_slice(&payload),
                        end_stream,
                    });
                }
                0x01 => {
                    // HEADERS frame
                    let end_stream = (flags & 0x01) != 0;
                    let end_headers = (flags & 0x04) != 0;

                    if !end_headers {
                        // Start accumulating header block fragments
                        self.header_block_buffer = Some((stream_id, payload.clone(), end_stream));
                    } else {
                        // Complete header block in one frame
                        let headers = match self.parse_hpack_headers(&payload) {
                            Ok(h) => h,
                            Err(e) => {
                                // HPACK degradation - log warning and continue
                                if !self.hpack_degraded {
                                    self.hpack_degraded = true;
                                    tracing::warn!("HPACK degradation: {}", e);
                                    events.push(H2Event::HpackDegraded {
                                        reason: e.to_string(),
                                    });
                                }
                                HashMap::new()
                            }
                        };

                        events.push(H2Event::Headers {
                            stream_id,
                            headers,
                            end_stream,
                        });
                    }
                }
                0x09 => {
                    // CONTINUATION frame
                    let end_headers = (flags & 0x04) != 0;

                    if let Some((buffered_stream_id, mut buffer, buffered_end_stream)) =
                        self.header_block_buffer.take()
                    {
                        if buffered_stream_id == stream_id {
                            // Append to buffer
                            buffer.extend_from_slice(&payload);

                            if end_headers {
                                // Complete header block - parse it
                                let headers = match self.parse_hpack_headers(&buffer) {
                                    Ok(h) => h,
                                    Err(e) => {
                                        if !self.hpack_degraded {
                                            self.hpack_degraded = true;
                                            tracing::warn!("HPACK degradation: {}", e);
                                            events.push(H2Event::HpackDegraded {
                                                reason: e.to_string(),
                                            });
                                        }
                                        HashMap::new()
                                    }
                                };

                                events.push(H2Event::Headers {
                                    stream_id,
                                    headers,
                                    end_stream: buffered_end_stream,
                                });
                            } else {
                                // Still accumulating - put buffer back
                                self.header_block_buffer = Some((buffered_stream_id, buffer, buffered_end_stream));
                            }
                        } else {
                            // Stream ID mismatch - protocol error, clear buffer
                            tracing::warn!(
                                "CONTINUATION frame stream ID mismatch: expected {}, got {}",
                                buffered_stream_id,
                                stream_id
                            );
                        }
                    } else {
                        // CONTINUATION without HEADERS - protocol error, ignore
                        tracing::warn!("CONTINUATION frame without preceding HEADERS");
                    }
                }
                0x04 => {
                    // SETTINGS frame
                    events.push(H2Event::Settings);
                }
                0x03 => {
                    // RST_STREAM frame
                    events.push(H2Event::RstStream { stream_id });
                }
                0x07 => {
                    // GOAWAY frame
                    events.push(H2Event::GoAway);
                }
                _ => {
                    // Unknown frame type - skip
                    tracing::debug!("Skipping unknown HTTP/2 frame type: 0x{:02x}", frame_type);
                }
            }

            // Remove parsed frame from buffer
            self.buffer.drain(0..9 + length);
        }

        Ok(events)
    }

    /// Parse HPACK-encoded headers.
    ///
    /// This is a simplified implementation that handles literal headers without indexing.
    /// In production, this should use h2-sans-io's HPACK decoder.
    fn parse_hpack_headers(&mut self, data: &[u8]) -> Result<HashMap<String, String>, GrpcError> {
        let mut headers = HashMap::new();
        let mut offset = 0;

        while offset < data.len() {
            let byte = data[offset];

            // Check for indexed header (0x80 prefix, 7-bit index)
            if (byte & 0x80) != 0 {
                let (index, index_bytes) = self.parse_integer(&data[offset..], 7)?;
                offset += index_bytes;

                // Static table lookup
                if let Some((name, value)) = self.static_table_lookup(index) {
                    headers.insert(name.to_string(), value.to_string());
                } else {
                    // Dynamic table reference - requires context we may not have
                    return Err(GrpcError::HpackError(format!(
                        "Dynamic table reference {} not available (mid-stream capture)",
                        index
                    )));
                }
            }
            // Check for literal header with incremental indexing (0x40 prefix, 6-bit index)
            else if (byte & 0x40) != 0 {
                let (name_index, name_index_bytes) = self.parse_integer(&data[offset..], 6)?;
                offset += name_index_bytes;

                // If name_index == 0, name is literal; otherwise it's from table
                let name = if name_index == 0 {
                    // Literal name
                    if offset >= data.len() {
                        break;
                    }
                    let (name_len, name_len_bytes) = self.parse_integer(&data[offset..], 7)?;
                    offset += name_len_bytes;
                    if offset + name_len > data.len() {
                        break;
                    }
                    let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
                    offset += name_len;
                    name
                } else {
                    // Indexed name from static table
                    self.static_table_lookup(name_index)
                        .map(|(n, _)| n.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                };

                // Parse value (always literal)
                if offset >= data.len() {
                    break;
                }
                let (value_len, value_len_bytes) = self.parse_integer(&data[offset..], 7)?;
                offset += value_len_bytes;
                if offset + value_len > data.len() {
                    break;
                }
                let value = String::from_utf8_lossy(&data[offset..offset + value_len]).to_string();
                offset += value_len;

                headers.insert(name, value);
            }
            // Check for dynamic table size update (0x20 prefix, 5-bit value)
            else if (byte & 0xE0) == 0x20 {
                let (_new_size, size_bytes) = self.parse_integer(&data[offset..], 5)?;
                offset += size_bytes;
                // We don't maintain a dynamic table, so just consume the bytes
            }
            // Check for literal header never indexed (0x10 prefix, 4-bit index)
            else if (byte & 0x10) != 0 {
                let (name_index, name_index_bytes) = self.parse_integer(&data[offset..], 4)?;
                offset += name_index_bytes;

                // Same parsing as literal-with-indexing
                let name = if name_index == 0 {
                    if offset >= data.len() {
                        break;
                    }
                    let (name_len, name_len_bytes) = self.parse_integer(&data[offset..], 7)?;
                    offset += name_len_bytes;
                    if offset + name_len > data.len() {
                        break;
                    }
                    let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
                    offset += name_len;
                    name
                } else {
                    self.static_table_lookup(name_index)
                        .map(|(n, _)| n.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                };

                if offset >= data.len() {
                    break;
                }
                let (value_len, value_len_bytes) = self.parse_integer(&data[offset..], 7)?;
                offset += value_len_bytes;
                if offset + value_len > data.len() {
                    break;
                }
                let value = String::from_utf8_lossy(&data[offset..offset + value_len]).to_string();
                offset += value_len;

                headers.insert(name, value);
            }
            // Check for literal header without indexing (0x00 prefix, 4-bit index)
            else {
                let (name_index, name_index_bytes) = self.parse_integer(&data[offset..], 4)?;
                offset += name_index_bytes;

                let name = if name_index == 0 {
                    if offset >= data.len() {
                        break;
                    }
                    let (name_len, name_len_bytes) = self.parse_integer(&data[offset..], 7)?;
                    offset += name_len_bytes;
                    if offset + name_len > data.len() {
                        break;
                    }
                    let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
                    offset += name_len;
                    name
                } else {
                    self.static_table_lookup(name_index)
                        .map(|(n, _)| n.to_string())
                        .unwrap_or_else(|| "unknown".to_string())
                };

                if offset >= data.len() {
                    break;
                }
                let (value_len, value_len_bytes) = self.parse_integer(&data[offset..], 7)?;
                offset += value_len_bytes;
                if offset + value_len > data.len() {
                    break;
                }
                let value = String::from_utf8_lossy(&data[offset..offset + value_len]).to_string();
                offset += value_len;

                headers.insert(name, value);
            }
        }

        Ok(headers)
    }

    /// Parse HPACK integer with N-bit prefix.
    /// Returns (value, bytes_consumed).
    fn parse_integer(&self, data: &[u8], n: u8) -> Result<(usize, usize), GrpcError> {
        if data.is_empty() {
            return Err(GrpcError::HpackError("Unexpected end of data".to_string()));
        }

        let mask = (1u8 << n) - 1;
        let mut value = (data[0] & mask) as usize;

        if value < mask as usize {
            return Ok((value, 1));
        }

        // Multi-byte integer
        let mut offset = 1;
        let mut m = 0;
        loop {
            if offset >= data.len() {
                return Err(GrpcError::HpackError("Unexpected end of data".to_string()));
            }

            let byte = data[offset];
            value += ((byte & 0x7F) as usize) << m;
            m += 7;
            offset += 1;

            if (byte & 0x80) == 0 {
                break;
            }
        }

        Ok((value, offset))
    }

    /// Lookup in HTTP/2 static table (RFC 7541 Appendix A).
    fn static_table_lookup(&self, index: usize) -> Option<(&'static str, &'static str)> {
        match index {
            1 => Some((":authority", "")),
            2 => Some((":method", "GET")),
            3 => Some((":method", "POST")),
            4 => Some((":path", "/")),
            5 => Some((":path", "/index.html")),
            6 => Some((":scheme", "http")),
            7 => Some((":scheme", "https")),
            8 => Some((":status", "200")),
            9 => Some((":status", "204")),
            10 => Some((":status", "206")),
            11 => Some((":status", "304")),
            12 => Some((":status", "400")),
            13 => Some((":status", "404")),
            14 => Some((":status", "500")),
            15 => Some(("accept-charset", "")),
            16 => Some(("accept-encoding", "gzip, deflate")),
            17 => Some(("accept-language", "")),
            18 => Some(("accept-ranges", "")),
            19 => Some(("accept", "")),
            20 => Some(("access-control-allow-origin", "")),
            21 => Some(("age", "")),
            22 => Some(("allow", "")),
            23 => Some(("authorization", "")),
            24 => Some(("cache-control", "")),
            25 => Some(("content-disposition", "")),
            26 => Some(("content-encoding", "")),
            27 => Some(("content-language", "")),
            28 => Some(("content-length", "")),
            29 => Some(("content-location", "")),
            30 => Some(("content-range", "")),
            31 => Some(("content-type", "")),
            32 => Some(("cookie", "")),
            33 => Some(("date", "")),
            34 => Some(("etag", "")),
            35 => Some(("expect", "")),
            36 => Some(("expires", "")),
            37 => Some(("from", "")),
            38 => Some(("host", "")),
            39 => Some(("if-match", "")),
            40 => Some(("if-modified-since", "")),
            41 => Some(("if-none-match", "")),
            42 => Some(("if-range", "")),
            43 => Some(("if-unmodified-since", "")),
            44 => Some(("last-modified", "")),
            45 => Some(("link", "")),
            46 => Some(("location", "")),
            47 => Some(("max-forwards", "")),
            48 => Some(("proxy-authenticate", "")),
            49 => Some(("proxy-authorization", "")),
            50 => Some(("range", "")),
            51 => Some(("referer", "")),
            52 => Some(("refresh", "")),
            53 => Some(("retry-after", "")),
            54 => Some(("server", "")),
            55 => Some(("set-cookie", "")),
            56 => Some(("strict-transport-security", "")),
            57 => Some(("transfer-encoding", "")),
            58 => Some(("user-agent", "")),
            59 => Some(("vary", "")),
            60 => Some(("via", "")),
            61 => Some(("www-authenticate", "")),
            _ => None,
        }
    }

    /// Get the state for a stream, creating it if it doesn't exist.
    pub fn get_stream(&mut self, stream_id: u32) -> &mut StreamState {
        self.streams.entry(stream_id).or_insert_with(StreamState::new)
    }

    /// Check if HPACK degradation has occurred.
    pub fn is_hpack_degraded(&self) -> bool {
        self.hpack_degraded
    }
}

impl Default for H2Codec {
    fn default() -> Self {
        Self::new()
    }
}
