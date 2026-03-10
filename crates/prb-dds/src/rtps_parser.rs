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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtps_bad_magic() {
        // WS-3.3: Non-RTPS data returns InvalidMagic error
        let bad_data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
                            0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
                            0x10, 0x11, 0x12, 0x13];

        let result = RtpsHeader::parse(&bad_data);
        assert!(result.is_err(), "Should reject non-RTPS magic");

        match result.unwrap_err() {
            DdsError::InvalidMagic(magic) => {
                assert_eq!(magic, [0x00, 0x01, 0x02, 0x03]);
            }
            _ => panic!("Expected InvalidMagic error"),
        }
    }

    #[test]
    fn test_rtps_truncated_header() {
        // WS-3.3: <20 bytes returns appropriate error
        let truncated = vec![b'R', b'T', b'P', b'S', 0x02, 0x03, 0x01, 0x0F];

        let result = RtpsHeader::parse(&truncated);
        assert!(result.is_err(), "Should reject truncated header");

        match result.unwrap_err() {
            DdsError::RtpsParse(msg) => {
                assert!(msg.contains("too short") || msg.contains("8 bytes"));
            }
            _ => panic!("Expected RtpsParse error"),
        }
    }

    #[test]
    fn test_rtps_data_frag_submessage() {
        // WS-3.3: DATA_FRAG is silently skipped (not an error)
        let mut rtps_msg = Vec::new();
        rtps_msg.extend_from_slice(RTPS_MAGIC);
        rtps_msg.extend_from_slice(&[0x02, 0x03]); // Protocol version
        rtps_msg.extend_from_slice(&[0x01, 0x0F]); // Vendor ID
        rtps_msg.extend_from_slice(&[0x00; 12]); // GUID prefix

        // Add DATA_FRAG submessage
        rtps_msg.push(SUBMESSAGE_DATA_FRAG); // submessage_id
        rtps_msg.push(0x01); // flags (little-endian)
        rtps_msg.extend_from_slice(&8u16.to_le_bytes()); // octets_to_next_header
        rtps_msg.extend_from_slice(&[0x00; 8]); // payload

        // Parse message (should succeed)
        let parsed = RtpsMessage::parse(&rtps_msg).unwrap();
        assert_eq!(parsed.header.guid_prefix, [0x00; 12]);

        // Iterate submessages (DATA_FRAG should be parsed without error)
        let submessages = parsed.submessages();
        let collected: Vec<_> = submessages.collect();
        assert_eq!(collected.len(), 1, "Should parse DATA_FRAG submessage");
        assert_eq!(collected[0].0.submessage_id, SUBMESSAGE_DATA_FRAG);
    }

    #[test]
    fn test_rtps_multiple_submessages() {
        // WS-3.3: Message with INFO_TS + DATA + HEARTBEAT + DATA
        let mut rtps_msg = Vec::new();
        rtps_msg.extend_from_slice(RTPS_MAGIC);
        rtps_msg.extend_from_slice(&[0x02, 0x03]); // Protocol version
        rtps_msg.extend_from_slice(&[0x01, 0x0F]); // Vendor ID
        rtps_msg.extend_from_slice(&[0x00; 12]); // GUID prefix

        // INFO_TS submessage
        rtps_msg.push(SUBMESSAGE_INFO_TS);
        rtps_msg.push(0x01); // flags (little-endian)
        rtps_msg.extend_from_slice(&8u16.to_le_bytes()); // octets_to_next_header
        rtps_msg.extend_from_slice(&1234u32.to_le_bytes()); // seconds
        rtps_msg.extend_from_slice(&5678u32.to_le_bytes()); // fraction

        // DATA submessage
        rtps_msg.push(SUBMESSAGE_DATA);
        rtps_msg.push(0x01); // flags
        rtps_msg.extend_from_slice(&16u16.to_le_bytes());
        rtps_msg.extend_from_slice(&[0xAA; 16]); // payload

        // HEARTBEAT submessage (ID 0x07)
        rtps_msg.push(0x07);
        rtps_msg.push(0x01);
        rtps_msg.extend_from_slice(&12u16.to_le_bytes());
        rtps_msg.extend_from_slice(&[0xBB; 12]);

        // Another DATA submessage
        rtps_msg.push(SUBMESSAGE_DATA);
        rtps_msg.push(0x01);
        rtps_msg.extend_from_slice(&8u16.to_le_bytes());
        rtps_msg.extend_from_slice(&[0xCC; 8]);

        let parsed = RtpsMessage::parse(&rtps_msg).unwrap();
        let submessages = parsed.submessages();
        let collected: Vec<_> = submessages.collect();

        assert_eq!(collected.len(), 4, "Should parse all 4 submessages");
        assert_eq!(collected[0].0.submessage_id, SUBMESSAGE_INFO_TS);
        assert_eq!(collected[1].0.submessage_id, SUBMESSAGE_DATA);
        assert_eq!(collected[2].0.submessage_id, 0x07); // HEARTBEAT
        assert_eq!(collected[3].0.submessage_id, SUBMESSAGE_DATA);
    }

    #[test]
    fn test_rtps_big_endian_submessage() {
        // WS-3.3: Submessage with E-flag = 0 (big-endian)
        let mut rtps_msg = Vec::new();
        rtps_msg.extend_from_slice(RTPS_MAGIC);
        rtps_msg.extend_from_slice(&[0x02, 0x03]);
        rtps_msg.extend_from_slice(&[0x01, 0x0F]);
        rtps_msg.extend_from_slice(&[0x00; 12]);

        // DATA submessage with big-endian flag (0x00)
        rtps_msg.push(SUBMESSAGE_DATA);
        rtps_msg.push(0x00); // flags: big-endian (E-flag = 0)
        rtps_msg.extend_from_slice(&8u16.to_be_bytes()); // octets_to_next_header (big-endian)
        rtps_msg.extend_from_slice(&[0xDD; 8]);

        let parsed = RtpsMessage::parse(&rtps_msg).unwrap();
        let submessages = parsed.submessages();
        let collected: Vec<_> = submessages.collect();

        assert_eq!(collected.len(), 1, "Should parse big-endian submessage");
        let (header, payload) = &collected[0];
        assert!(!header.is_little_endian(), "Should detect big-endian");
        assert_eq!(header.octets_to_next_header, 8);
        assert_eq!(payload.len(), 8);
    }
}
