//! ZMTP wire protocol parser.

use crate::error::ZmqError;
use std::collections::HashMap;

/// Maximum frame size to prevent memory exhaustion (16MB).
const MAX_FRAME_SIZE: u64 = 16 * 1024 * 1024;

/// Maximum number of frames in a multipart message.
const MAX_MULTIPART_FRAMES: usize = 1000;

/// ZMTP greeting structure (64 bytes).
#[derive(Debug, Clone, PartialEq)]
pub struct ZmtpGreeting {
    pub major_version: u8,
    pub minor_version: u8,
    pub mechanism: String,
    pub as_server: bool,
}

/// ZMTP command event.
#[derive(Debug, Clone, PartialEq)]
pub struct ZmtpCommand {
    pub name: String,
    pub data: Vec<u8>,
}

/// ZMTP READY command metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ZmtpReady {
    pub properties: HashMap<String, Vec<u8>>,
}

/// ZMTP message (single or multipart).
#[derive(Debug, Clone, PartialEq)]
pub struct ZmtpMessage {
    pub frames: Vec<Vec<u8>>,
}

/// Events produced by ZMTP parser.
#[derive(Debug, Clone, PartialEq)]
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
    pub fn new() -> Self {
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
    pub fn is_degraded(&self) -> bool {
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
            return Ok((0, ZmtpCommand { name: String::new(), data: Vec::new() }));
        }

        let flags = self.buffer[0];
        let is_long = flags & 0x02 != 0;

        let (size_bytes, body_offset) = if is_long {
            if self.buffer.len() < 9 {
                return Ok((0, ZmtpCommand { name: String::new(), data: Vec::new() }));
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
                return Ok((0, ZmtpCommand { name: String::new(), data: Vec::new() }));
            }
            (self.buffer[1] as u64, 2)
        };

        if size_bytes > MAX_FRAME_SIZE {
            return Err(ZmqError::FrameTooLarge(size_bytes));
        }

        let total_size = body_offset + size_bytes as usize;
        if self.buffer.len() < total_size {
            return Ok((0, ZmtpCommand { name: String::new(), data: Vec::new() }));
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

        let name = String::from_utf8(body[1..1 + name_len].to_vec())?;
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
            (self.buffer[1] as u64, 2)
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
