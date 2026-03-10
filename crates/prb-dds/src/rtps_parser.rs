//! Minimal RTPS message parser for Phase 1.
//!
//! Parses RTPS messages and submessages according to DDSI-RTPS v2.3 spec.

use crate::error::DdsError;

/// RTPS protocol magic.
pub const RTPS_MAGIC: &[u8; 4] = b"RTPS";

/// INFO_TS submessage ID.
pub const SUBMESSAGE_INFO_TS: u8 = 0x09;

/// DATA submessage ID.
pub const SUBMESSAGE_DATA: u8 = 0x15;

/// DATA_FRAG submessage ID.
pub const SUBMESSAGE_DATA_FRAG: u8 = 0x16;

/// RTPS message header (20 bytes).
#[derive(Debug, Clone, Copy)]
pub struct RtpsHeader {
    #[allow(dead_code)]
    pub protocol_version: [u8; 2],
    #[allow(dead_code)]
    pub vendor_id: [u8; 2],
    pub guid_prefix: [u8; 12],
}

impl RtpsHeader {
    /// Parse RTPS header from bytes.
    pub fn parse(data: &[u8]) -> Result<Self, DdsError> {
        if data.len() < 20 {
            return Err(DdsError::RtpsParse(format!(
                "Header too short: {} bytes",
                data.len()
            )));
        }

        if &data[0..4] != RTPS_MAGIC {
            return Err(DdsError::InvalidMagic([
                data[0], data[1], data[2], data[3],
            ]));
        }

        Ok(Self {
            protocol_version: [data[4], data[5]],
            vendor_id: [data[6], data[7]],
            guid_prefix: [
                data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
                data[16], data[17], data[18], data[19],
            ],
        })
    }
}

/// RTPS submessage header (4 bytes).
#[derive(Debug, Clone, Copy)]
pub struct SubmessageHeader {
    pub submessage_id: u8,
    pub flags: u8,
    pub octets_to_next_header: u16,
}

impl SubmessageHeader {
    /// Parse submessage header from bytes.
    pub fn parse(data: &[u8]) -> Result<Self, DdsError> {
        if data.len() < 4 {
            return Err(DdsError::RtpsParse(format!(
                "Submessage header too short: {} bytes",
                data.len()
            )));
        }

        let submessage_id = data[0];
        let flags = data[1];
        let endianness_flag = (flags & 0x01) != 0;

        let octets_to_next_header = if endianness_flag {
            u16::from_le_bytes([data[2], data[3]])
        } else {
            u16::from_be_bytes([data[2], data[3]])
        };

        Ok(Self {
            submessage_id,
            flags,
            octets_to_next_header,
        })
    }

    /// Check if little-endian flag is set.
    pub fn is_little_endian(&self) -> bool {
        (self.flags & 0x01) != 0
    }
}

/// INFO_TS submessage.
#[derive(Debug, Clone, Copy)]
pub struct InfoTsSubmessage {
    pub seconds: u32,
    pub fraction: u32,
}

impl InfoTsSubmessage {
    /// Parse INFO_TS submessage (8 bytes timestamp after header).
    pub fn parse(data: &[u8], header: &SubmessageHeader) -> Result<Self, DdsError> {
        let invalidate_flag = (header.flags & 0x02) != 0;

        if invalidate_flag {
            // Timestamp is invalid
            return Ok(Self {
                seconds: 0,
                fraction: 0,
            });
        }

        if data.len() < 8 {
            return Err(DdsError::RtpsParse(format!(
                "INFO_TS payload too short: {} bytes",
                data.len()
            )));
        }

        let (seconds, fraction) = if header.is_little_endian() {
            (
                u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
                u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            )
        } else {
            (
                u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
                u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            )
        };

        Ok(Self { seconds, fraction })
    }
}

/// DATA submessage.
#[derive(Debug, Clone)]
pub struct DataSubmessage<'a> {
    pub reader_id: [u8; 4],
    pub writer_id: [u8; 4],
    pub writer_sn: i64,
    pub serialized_payload: &'a [u8],
}

impl<'a> DataSubmessage<'a> {
    /// Parse DATA submessage.
    pub fn parse(data: &'a [u8], header: &SubmessageHeader) -> Result<Self, DdsError> {
        if data.len() < 20 {
            return Err(DdsError::RtpsParse(format!(
                "DATA payload too short: {} bytes",
                data.len()
            )));
        }

        // Skip extra flags (2 bytes) and octetsToInlineQos (2 bytes)
        let reader_id = [data[4], data[5], data[6], data[7]];
        let writer_id = [data[8], data[9], data[10], data[11]];

        // Sequence number: high (4 bytes) + low (4 bytes)
        let (sn_high, sn_low) = if header.is_little_endian() {
            (
                i32::from_le_bytes([data[12], data[13], data[14], data[15]]),
                u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
            )
        } else {
            (
                i32::from_be_bytes([data[12], data[13], data[14], data[15]]),
                u32::from_be_bytes([data[16], data[17], data[18], data[19]]),
            )
        };

        let writer_sn = ((sn_high as i64) << 32) | (sn_low as i64);

        // Find serialized payload after parameter list
        // For simplicity, assume no inline QoS and look for encapsulation header
        let mut payload_offset = 20;

        // Check for inline QoS flag (bit 1)
        let inline_qos_flag = (header.flags & 0x02) != 0;
        if inline_qos_flag {
            // Skip inline QoS parameter list (find PID_SENTINEL = 0x0001)
            while payload_offset + 4 <= data.len() {
                let pid = if header.is_little_endian() {
                    u16::from_le_bytes([data[payload_offset], data[payload_offset + 1]])
                } else {
                    u16::from_be_bytes([data[payload_offset], data[payload_offset + 1]])
                };

                let length = if header.is_little_endian() {
                    u16::from_le_bytes([data[payload_offset + 2], data[payload_offset + 3]])
                } else {
                    u16::from_be_bytes([data[payload_offset + 2], data[payload_offset + 3]])
                };

                payload_offset += 4;

                if pid == 0x0001 {
                    // PID_SENTINEL - end of parameter list
                    break;
                }

                payload_offset += length as usize;
                // Align to 4-byte boundary
                payload_offset = (payload_offset + 3) & !3;
            }
        }

        // Serialized payload starts here (with encapsulation header)
        let serialized_payload = &data[payload_offset..];

        Ok(Self {
            reader_id,
            writer_id,
            writer_sn,
            serialized_payload,
        })
    }
}

/// RTPS message parser.
pub struct RtpsMessage<'a> {
    pub header: RtpsHeader,
    data: &'a [u8],
}

impl<'a> RtpsMessage<'a> {
    /// Parse RTPS message from bytes.
    pub fn parse(data: &'a [u8]) -> Result<Self, DdsError> {
        let header = RtpsHeader::parse(data)?;
        Ok(Self { header, data })
    }

    /// Iterate over submessages.
    pub fn submessages(&self) -> SubmessageIterator<'a> {
        SubmessageIterator {
            data: &self.data[20..], // Skip header
            offset: 0,
        }
    }
}

/// Iterator over RTPS submessages.
pub struct SubmessageIterator<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Iterator for SubmessageIterator<'a> {
    type Item = (SubmessageHeader, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset + 4 > self.data.len() {
            return None;
        }

        let header = match SubmessageHeader::parse(&self.data[self.offset..]) {
            Ok(h) => h,
            Err(_) => return None,
        };

        let payload_start = self.offset + 4;
        let payload_len = header.octets_to_next_header as usize;
        let payload_end = payload_start + payload_len;

        if payload_end > self.data.len() {
            return None;
        }

        let payload = &self.data[payload_start..payload_end];

        self.offset = payload_end;

        Some((header, payload))
    }
}
