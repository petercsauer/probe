//! ZMTP wire protocol parser.

use crate::error::ZmqError;
use std::collections::HashMap;

/// Maximum frame size to prevent memory exhaustion (16MB).
const MAX_FRAME_SIZE: u64 = 16 * 1024 * 1024;

/// Maximum number of frames in a multipart message.
const MAX_MULTIPART_FRAMES: usize = 1000;

/// ZMTP greeting structure (64 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZmtpGreeting {
    pub major_version: u8,
    pub minor_version: u8,
    pub mechanism: String,
    pub as_server: bool,
}

/// ZMTP command event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZmtpCommand {
    pub name: String,
    pub data: Vec<u8>,
}

/// ZMTP READY command metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZmtpReady {
    pub properties: HashMap<String, Vec<u8>>,
}

/// ZMTP message (single or multipart).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZmtpMessage {
    pub frames: Vec<Vec<u8>>,
}

/// Events produced by ZMTP parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZmtpEvent {
    Greeting(ZmtpGreeting),
    Ready(ZmtpReady),
    Message(ZmtpMessage),
    Command(ZmtpCommand),
}

/// Parser state.
#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    AwaitingGreeting,
    AwaitingHandshake,
    Traffic,
    Degraded,
}

/// ZMTP wire protocol parser.
///
/// Implements stateful parsing of ZMTP 3.0/3.1 streams with support for:
/// - Greeting detection and parsing
/// - NULL security handshake (READY command)
/// - Multipart message reassembly
/// - Mid-stream heuristic detection
pub struct ZmtpParser {
    state: State,
    buffer: Vec<u8>,
    partial_frames: Vec<Vec<u8>>,
    degraded: bool,
}

impl ZmtpParser {
    /// Create a new ZMTP parser.
    pub const fn new() -> Self {
        Self {
            state: State::AwaitingGreeting,
            buffer: Vec::new(),
            partial_frames: Vec::new(),
            degraded: false,
        }
    }

    /// Feed data into the parser and extract complete events.
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<ZmtpEvent>, ZmqError> {
        self.buffer.extend_from_slice(data);
        let mut events = Vec::new();

        loop {
            let consumed = match self.state {
                State::AwaitingGreeting => self.try_parse_greeting(&mut events)?,
                State::AwaitingHandshake => self.try_parse_handshake(&mut events)?,
                State::Traffic => self.try_parse_traffic(&mut events)?,
                State::Degraded => self.try_parse_degraded(&mut events)?,
            };

            if consumed == 0 {
                break;
            }

            self.buffer.drain(..consumed);
        }

        Ok(events)
    }

    /// Check if parser is in degraded mode (mid-stream capture).
    pub const fn is_degraded(&self) -> bool {
        self.degraded
    }

    /// Try to parse a ZMTP greeting (64 bytes).
    fn try_parse_greeting(&mut self, events: &mut Vec<ZmtpEvent>) -> Result<usize, ZmqError> {
        // Check for greeting signature as soon as we have enough bytes
        if self.buffer.len() >= 10 && !Self::has_valid_greeting_signature(&self.buffer) {
            tracing::warn!("ZMTP greeting not detected, entering degraded mode");
            self.state = State::Degraded;
            self.degraded = true;
            return Ok(0);
        }

        if self.buffer.len() < 64 {
            return Ok(0);
        }

        // Check greeting signature
        if !Self::has_valid_greeting_signature(&self.buffer) {
            return Err(ZmqError::InvalidGreetingSignature);
        }

        // Parse greeting
        let major_version = self.buffer[10];
        let minor_version = self.buffer[11];

        // Only support ZMTP 3.0 and 3.1
        if major_version != 3 {
            return Err(ZmqError::UnsupportedVersion {
                major: major_version,
                minor: minor_version,
            });
        }

        // Parse mechanism (20 bytes, null-padded ASCII)
        let mechanism_bytes = &self.buffer[12..32];
        let mechanism_end = mechanism_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(mechanism_bytes.len());
        let mechanism = String::from_utf8(mechanism_bytes[..mechanism_end].to_vec())?;

        // Parse as-server flag
        let as_server = self.buffer[32] != 0;

        events.push(ZmtpEvent::Greeting(ZmtpGreeting {
            major_version,
            minor_version,
            mechanism,
            as_server,
        }));

        self.state = State::AwaitingHandshake;
        Ok(64)
    }

    /// Check if buffer starts with valid ZMTP greeting signature.
    fn has_valid_greeting_signature(buf: &[u8]) -> bool {
        buf.len() >= 10 && buf[0] == 0xFF && buf[9] == 0x7F
    }

    /// Try to parse handshake commands (READY, ERROR).
    fn try_parse_handshake(&mut self, events: &mut Vec<ZmtpEvent>) -> Result<usize, ZmqError> {
        let (consumed, cmd) = self.try_parse_command()?;
        if consumed == 0 {
            return Ok(0);
        }

        if cmd.name == "READY" {
            // Parse READY metadata
            let ready = Self::parse_ready_metadata(&cmd.data)?;
            events.push(ZmtpEvent::Ready(ready));
            self.state = State::Traffic;
        } else {
            // Other commands (ERROR, etc.)
            events.push(ZmtpEvent::Command(cmd));
        }

        Ok(consumed)
    }

    /// Try to parse traffic frames (messages and commands).
    fn try_parse_traffic(&mut self, events: &mut Vec<ZmtpEvent>) -> Result<usize, ZmqError> {
        if self.buffer.is_empty() {
            return Ok(0);
        }

        let flags = self.buffer[0];

        // Validate flag byte (bits 7-3 must be zero)
        if flags & 0xF8 != 0 {
            return Err(ZmqError::InvalidFlagByte(flags));
        }

        let is_command = flags & 0x04 != 0;

        if is_command {
            let (consumed, cmd) = self.try_parse_command()?;
            if consumed > 0 {
                events.push(ZmtpEvent::Command(cmd));
            }
            Ok(consumed)
        } else {
            self.try_parse_message_frame(events)
        }
    }

    /// Try to parse a command frame.
    fn try_parse_command(&self) -> Result<(usize, ZmtpCommand), ZmqError> {
        if self.buffer.is_empty() {
            return Ok((
                0,
                ZmtpCommand {
                    name: String::new(),
                    data: Vec::new(),
                },
            ));
        }

        let flags = self.buffer[0];
        let is_long = flags & 0x02 != 0;

        let (size_bytes, body_offset) = if is_long {
            if self.buffer.len() < 9 {
                return Ok((
                    0,
                    ZmtpCommand {
                        name: String::new(),
                        data: Vec::new(),
                    },
                ));
            }
            let size = u64::from_be_bytes([
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
                self.buffer[4],
                self.buffer[5],
                self.buffer[6],
                self.buffer[7],
                self.buffer[8],
            ]);
            (size, 9)
        } else {
            if self.buffer.len() < 2 {
                return Ok((
                    0,
                    ZmtpCommand {
                        name: String::new(),
                        data: Vec::new(),
                    },
                ));
            }
            (u64::from(self.buffer[1]), 2)
        };

        if size_bytes > MAX_FRAME_SIZE {
            return Err(ZmqError::FrameTooLarge(size_bytes));
        }

        let total_size = body_offset + size_bytes as usize;
        if self.buffer.len() < total_size {
            return Ok((
                0,
                ZmtpCommand {
                    name: String::new(),
                    data: Vec::new(),
                },
            ));
        }

        // Parse command body
        let body = &self.buffer[body_offset..total_size];
        if body.is_empty() {
            return Ok((
                total_size,
                ZmtpCommand {
                    name: String::new(),
                    data: Vec::new(),
                },
            ));
        }

        let name_len = body[0] as usize;
        if name_len == 0 || body.len() < 1 + name_len {
            return Err(ZmqError::InvalidCommandNameLength(body[0]));
        }

        let name = String::from_utf8(body[1..=name_len].to_vec())?;
        let data = body[1 + name_len..].to_vec();

        Ok((total_size, ZmtpCommand { name, data }))
    }

    /// Try to parse a message frame.
    fn try_parse_message_frame(&mut self, events: &mut Vec<ZmtpEvent>) -> Result<usize, ZmqError> {
        if self.buffer.is_empty() {
            return Ok(0);
        }

        let flags = self.buffer[0];
        let is_long = flags & 0x02 != 0;
        let has_more = flags & 0x01 != 0;

        let (size_bytes, body_offset) = if is_long {
            if self.buffer.len() < 9 {
                return Ok(0);
            }
            let size = u64::from_be_bytes([
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
                self.buffer[4],
                self.buffer[5],
                self.buffer[6],
                self.buffer[7],
                self.buffer[8],
            ]);
            (size, 9)
        } else {
            if self.buffer.len() < 2 {
                return Ok(0);
            }
            (u64::from(self.buffer[1]), 2)
        };

        if size_bytes > MAX_FRAME_SIZE {
            return Err(ZmqError::FrameTooLarge(size_bytes));
        }

        let total_size = body_offset + size_bytes as usize;
        if self.buffer.len() < total_size {
            return Ok(0);
        }

        // Extract frame body
        let body = self.buffer[body_offset..total_size].to_vec();

        if has_more {
            // Accumulate frame
            self.partial_frames.push(body);
            if self.partial_frames.len() > MAX_MULTIPART_FRAMES {
                return Err(ZmqError::TooManyFrames(self.partial_frames.len()));
            }
        } else {
            // Final frame - emit complete message
            self.partial_frames.push(body);
            let frames = std::mem::take(&mut self.partial_frames);
            events.push(ZmtpEvent::Message(ZmtpMessage { frames }));
        }

        Ok(total_size)
    }

    /// Try to parse frames in degraded mode (heuristic detection).
    fn try_parse_degraded(&mut self, events: &mut Vec<ZmtpEvent>) -> Result<usize, ZmqError> {
        // In degraded mode, scan for valid flag bytes
        if self.buffer.is_empty() {
            return Ok(0);
        }

        // Look for a plausible flag byte (bits 7-3 zero)
        for i in 0..self.buffer.len() {
            let flags = self.buffer[i];
            if flags & 0xF8 == 0 {
                // Found potential frame start
                if i > 0 {
                    // Skip to this position
                    return Ok(i);
                }

                // Try to parse as a message frame
                let result = self.try_parse_message_frame(events);
                if let Ok(consumed) = result
                    && consumed > 0
                {
                    return Ok(consumed);
                }

                // Can't parse, skip this byte
                return Ok(1);
            }
        }

        // No valid flag bytes found, skip all buffered data
        let len = self.buffer.len();
        Ok(len)
    }

    /// Parse READY command metadata.
    fn parse_ready_metadata(data: &[u8]) -> Result<ZmtpReady, ZmqError> {
        let mut properties = HashMap::new();
        let mut offset = 0;

        while offset < data.len() {
            // Read property name length
            if offset >= data.len() {
                break;
            }
            let name_len = data[offset] as usize;
            offset += 1;

            if name_len == 0 || offset + name_len > data.len() {
                return Err(ZmqError::InvalidPropertyMetadata(
                    "invalid name length".to_string(),
                ));
            }

            // Read property name
            let name = String::from_utf8(data[offset..offset + name_len].to_vec())?;
            offset += name_len;

            // Read property value length (4 bytes, network order)
            if offset + 4 > data.len() {
                return Err(ZmqError::InvalidPropertyMetadata(
                    "missing value length".to_string(),
                ));
            }
            let value_len = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + value_len > data.len() {
                return Err(ZmqError::InvalidPropertyMetadata(
                    "truncated value".to_string(),
                ));
            }

            // Read property value
            let value = data[offset..offset + value_len].to_vec();
            offset += value_len;

            properties.insert(name, value);
        }

        Ok(ZmtpReady { properties })
    }
}

impl Default for ZmtpParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_greeting(major: u8, minor: u8, mechanism: &str, as_server: bool) -> Vec<u8> {
        let mut greeting = vec![0xFF; 10]; // Signature
        greeting[0] = 0xFF;
        greeting[9] = 0x7F;
        greeting.push(major); // Version major
        greeting.push(minor); // Version minor
        // Mechanism (20 bytes, padded with zeros)
        let mut mech_bytes = mechanism.as_bytes().to_vec();
        mech_bytes.resize(20, 0);
        greeting.extend_from_slice(&mech_bytes);
        // as-server flag
        greeting.push(u8::from(as_server));
        // Padding to 64 bytes
        greeting.resize(64, 0);
        greeting
    }

    #[test]
    fn test_zmtp_incremental_greeting() {
        // WS-3.2: Feed greeting 1 byte at a time, assert greeting event only after byte 64
        let mut parser = ZmtpParser::new();
        let greeting = create_greeting(3, 0, "NULL", false);

        // Feed 63 bytes - should not produce event
        for i in 0..63 {
            let events = parser.feed(&greeting[i..=i]).unwrap();
            assert_eq!(events.len(), 0, "Should not emit event before byte 64");
        }

        // Feed byte 64 - should produce greeting event
        let events = parser.feed(&greeting[63..64]).unwrap();
        assert_eq!(events.len(), 1, "Should emit greeting event on byte 64");

        match &events[0] {
            ZmtpEvent::Greeting(g) => {
                assert_eq!(g.major_version, 3);
                assert_eq!(g.minor_version, 0);
                assert_eq!(g.mechanism, "NULL");
                assert!(!g.as_server);
            }
            _ => panic!("Expected Greeting event"),
        }
    }

    #[test]
    fn test_zmtp_frame_boundary_split() {
        // WS-3.2: Message frame header in one feed, body in next
        let mut parser = ZmtpParser::new();

        // Feed greeting first
        let greeting = create_greeting(3, 0, "NULL", false);
        parser.feed(&greeting).unwrap();

        // Feed READY command to complete handshake
        let mut ready_cmd = vec![0x04, 0x06, 0x05];
        ready_cmd.extend_from_slice(b"READY");
        parser.feed(&ready_cmd).unwrap();

        // Create message frame split: header in first feed, body in second
        let body = b"test_message";
        let frame_header = vec![
            0x00,             // flags: no more frames
            body.len() as u8, // size
        ];

        // Feed header only
        let events = parser.feed(&frame_header).unwrap();
        assert_eq!(events.len(), 0, "Should not emit message without body");

        // Feed body
        let events = parser.feed(body).unwrap();
        assert_eq!(events.len(), 1, "Should emit message after body");

        match &events[0] {
            ZmtpEvent::Message(msg) => {
                assert_eq!(msg.frames.len(), 1);
                assert_eq!(&msg.frames[0], body);
            }
            _ => panic!("Expected Message event"),
        }
    }

    #[test]
    fn test_zmtp_max_multipart_limit() {
        // WS-3.2: >1000 frames triggers TooManyFrames error
        let mut parser = ZmtpParser::new();

        // Feed greeting and READY
        let greeting = create_greeting(3, 0, "NULL", false);
        parser.feed(&greeting).unwrap();
        let mut ready_cmd = vec![0x04, 0x06, 0x05];
        ready_cmd.extend_from_slice(b"READY");
        parser.feed(&ready_cmd).unwrap();

        // Feed 1001 frames (all with "more" flag set)
        for i in 0..1001 {
            let frame = vec![
                0x01, // flags: more frames
                0x01, // size
                i as u8,
            ];
            let result = parser.feed(&frame);

            if i < 1000 {
                assert!(result.is_ok(), "Should accept frame {i}");
            } else {
                // Frame 1001 should trigger error
                assert!(result.is_err(), "Should reject frame 1001");
                match result.unwrap_err() {
                    ZmqError::TooManyFrames(count) => {
                        assert_eq!(count, 1001, "Error should report 1001 frames");
                    }
                    _ => panic!("Expected TooManyFrames error"),
                }
                break;
            }
        }
    }

    #[test]
    fn test_zmtp_long_command_frame() {
        // WS-3.2: Command with 8-byte length encoding
        let mut parser = ZmtpParser::new();

        // Feed greeting
        let greeting = create_greeting(3, 0, "NULL", false);
        parser.feed(&greeting).unwrap();

        // Create command frame with long size (requires 8-byte encoding)
        // Build command body: name_length + name + data
        let mut body = Vec::new();
        body.push(7); // "TESTCMD" length
        body.extend_from_slice(b"TESTCMD");
        body.extend_from_slice(&vec![0x42; 300]); // 300 bytes of data

        // Build frame with long flag
        let mut frame = vec![
            0x06, // flags: command (0x04) + long (0x02)
        ];
        frame.extend_from_slice(&(body.len() as u64).to_be_bytes()); // 8-byte size
        frame.extend_from_slice(&body);

        let events = parser.feed(&frame).unwrap();
        assert!(!events.is_empty(), "Should parse long command frame");

        // Find command event
        let cmd_event = events.iter().find(|e| matches!(e, ZmtpEvent::Command(_)));
        assert!(cmd_event.is_some(), "Should emit Command event");
    }

    #[test]
    fn test_zmtp_degraded_message_extraction() {
        // WS-3.2: Mid-stream data → degraded mode → parse valid frames
        let mut parser = ZmtpParser::new();

        // Feed invalid greeting data (at least 10 bytes, wrong signature)
        // This triggers degraded mode detection
        let invalid_greeting = vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        parser.feed(&invalid_greeting).unwrap();

        // Verify parser entered degraded mode
        assert!(parser.is_degraded(), "Parser should be in degraded mode");

        // Now feed a valid message frame
        let body = b"recovered_message";
        let frame = vec![
            0x00,             // flags: no more frames
            body.len() as u8, // size
        ];
        let mut frame_data = frame;
        frame_data.extend_from_slice(body);

        let events = parser.feed(&frame_data).unwrap();

        // In degraded mode, parser should recover and extract message
        // Check if we got a message event
        let has_message = events.iter().any(|e| matches!(e, ZmtpEvent::Message(_)));
        assert!(has_message, "Should recover message in degraded mode");
    }

    #[test]
    fn test_zmtp_ready_empty_properties() {
        // WS-3.2: READY with zero properties
        let mut parser = ZmtpParser::new();

        // Feed greeting
        let greeting = create_greeting(3, 0, "NULL", false);
        parser.feed(&greeting).unwrap();

        // Feed READY command with no properties
        // Format: flags + body_length + command_name_length + command_name
        let mut ready_cmd = vec![
            0x04, // flags: command frame, no more
            0x06, // body size: 1 (name length) + 5 (name "READY")
            0x05, // command name length
        ];
        ready_cmd.extend_from_slice(b"READY");

        let events = parser.feed(&ready_cmd).unwrap();
        assert_eq!(events.len(), 1, "Should emit READY event");

        match &events[0] {
            ZmtpEvent::Ready(ready) => {
                assert_eq!(ready.properties.len(), 0, "Should have zero properties");
            }
            _ => panic!("Expected Ready event"),
        }
    }
}
