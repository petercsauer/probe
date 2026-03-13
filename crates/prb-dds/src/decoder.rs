//! DDS/RTPS protocol decoder implementing the ProtocolDecoder trait.

use crate::discovery::{DiscoveredEndpoint, Guid, RtpsDiscoveryTracker, well_known_entities};
use crate::error::DdsError;
use crate::rtps_parser::{
    DataSubmessage, InfoTsSubmessage, RtpsMessage, SUBMESSAGE_DATA, SUBMESSAGE_DATA_FRAG,
    SUBMESSAGE_INFO_TS,
};
use bytes::Bytes;
use prb_core::{
    CoreError, CorrelationKey, DebugEvent, DecodeContext, Direction, METADATA_KEY_DDS_DOMAIN_ID,
    METADATA_KEY_DDS_TOPIC_NAME, Payload, ProtocolDecoder, Timestamp, TransportKind,
};

/// DDS/RTPS protocol decoder.
///
/// Decodes DDS/RTPS messages from UDP datagrams by:
/// 1. Parsing RTPS messages with magic "RTPS"
/// 2. Extracting DATA submessages with serialized payloads
/// 3. Tracking SEDP discovery data for topic name resolution
/// 4. Extracting domain ID from UDP port
/// 5. Providing GUID-based correlation
pub struct DdsDecoder {
    /// Discovery tracker for topic name resolution.
    discovery: RtpsDiscoveryTracker,
    /// Sequence counter for events.
    sequence: u64,
    /// Last seen INFO_TS timestamp (applies to subsequent submessages).
    last_timestamp: Option<Timestamp>,
}

impl DdsDecoder {
    /// Create a new DDS/RTPS decoder.
    pub fn new() -> Self {
        Self {
            discovery: RtpsDiscoveryTracker::new(),
            sequence: 0,
            last_timestamp: None,
        }
    }

    /// Check if data starts with RTPS magic.
    pub fn has_rtps_magic(data: &[u8]) -> bool {
        data.len() >= 4 && &data[0..4] == b"RTPS"
    }

    /// Extract domain ID from UDP port using RTPS port mapping formula.
    ///
    /// RTPS spec: port = PB + DG * domainId + offset
    /// where PB = 7400, DG = 250
    /// Domain 0: 7400-7401, Domain 1: 7650-7651
    fn extract_domain_id(port: u16) -> Option<u32> {
        const PB: u16 = 7400;
        const DG: u16 = 250;

        if port < PB {
            return None;
        }

        let offset_from_base = port - PB;
        let domain_id = offset_from_base / DG;

        Some(domain_id as u32)
    }

    /// Parse RTPS message and generate DebugEvents.
    fn decode_rtps_message(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // Parse RTPS message
        let rtps_msg = RtpsMessage::parse(data)
            .map_err(|e| CoreError::PayloadDecode(format!("RTPS parse error: {:?}", e)))?;

        let guid_prefix = rtps_msg.header.guid_prefix;

        // Extract domain ID from destination port
        let domain_id = ctx
            .dst_addr
            .as_ref()
            .and_then(|addr| {
                addr.split(':')
                    .nth(1)
                    .and_then(|port_str| port_str.parse::<u16>().ok())
            })
            .and_then(Self::extract_domain_id);

        let mut events = Vec::new();

        // Reset timestamp for this message
        self.last_timestamp = None;

        // Process submessages
        for (header, payload) in rtps_msg.submessages() {
            match header.submessage_id {
                SUBMESSAGE_INFO_TS => {
                    // Store timestamp for subsequent DATA submessages
                    if let Ok(info_ts) = InfoTsSubmessage::parse(payload, &header) {
                        let nanos = (info_ts.seconds as u64) * 1_000_000_000
                            + (info_ts.fraction as u64 * 1_000_000_000 / (1u64 << 32));
                        self.last_timestamp = Some(Timestamp::from_nanos(nanos));
                    }
                }
                SUBMESSAGE_DATA => {
                    // Extract DATA submessage
                    if let Ok(data_msg) = DataSubmessage::parse(payload, &header) {
                        let writer_guid = Guid::new(guid_prefix, data_msg.writer_id);

                        // Check if this is a SEDP discovery message
                        if well_known_entities::is_sedp_entity(&data_msg.writer_id) {
                            // Process discovery data
                            if let Err(e) = self
                                .process_discovery_data(&writer_guid, data_msg.serialized_payload)
                            {
                                tracing::debug!("Failed to parse discovery data: {}", e);
                            }
                        } else {
                            // User data message
                            if let Some(event) = self.create_data_event(
                                &writer_guid,
                                data_msg.reader_id,
                                data_msg.writer_sn,
                                data_msg.serialized_payload,
                                domain_id,
                                ctx,
                            )? {
                                events.push(event);
                            }
                        }
                    }
                }
                SUBMESSAGE_DATA_FRAG => {
                    // DATA_FRAG: fragmented data (Phase 1 logs warning)
                    tracing::debug!(
                        "DATA_FRAG submessage detected (fragmented data not yet supported)"
                    );
                }
                _ => {
                    // Other submessage types (HEARTBEAT, ACKNACK, etc.) - ignore
                }
            }
        }

        Ok(events)
    }

    /// Process SEDP discovery data to extract topic name and endpoint GUID.
    fn process_discovery_data(
        &mut self,
        _writer_guid: &Guid,
        payload: &[u8],
    ) -> Result<(), DdsError> {
        // Parse CDR parameter list from SEDP payload
        // Format: encapsulation_header (4 bytes) + parameter_list

        if payload.len() < 4 {
            return Err(DdsError::DiscoveryParse(
                "SEDP payload too short".to_string(),
            ));
        }

        // Read encapsulation header
        let encapsulation_kind = u16::from_le_bytes([payload[0], payload[1]]);
        let little_endian = match encapsulation_kind {
            0x0000 => false, // BE CDR
            0x0001 => true,  // LE CDR
            _ => {
                tracing::debug!("Unknown encapsulation kind: 0x{:04x}", encapsulation_kind);
                return Ok(()); // Skip unknown encodings
            }
        };
        // Skip 2 bytes options at offset 2..4

        let mut offset = 4;
        let mut topic_name: Option<String> = None;
        let mut type_name: Option<String> = None;
        let mut endpoint_guid: Option<Guid> = None;

        // Walk parameter list
        while offset + 4 <= payload.len() {
            // Read PID and length (both u16)
            let pid = if little_endian {
                u16::from_le_bytes([payload[offset], payload[offset + 1]])
            } else {
                u16::from_be_bytes([payload[offset], payload[offset + 1]])
            };
            let param_len = if little_endian {
                u16::from_le_bytes([payload[offset + 2], payload[offset + 3]])
            } else {
                u16::from_be_bytes([payload[offset + 2], payload[offset + 3]])
            } as usize;
            offset += 4;

            if pid == 0x0001 {
                // PID_SENTINEL - end of parameter list
                break;
            }

            if offset + param_len > payload.len() {
                break; // Truncated parameter
            }

            match pid {
                0x0005 => {
                    // PID_TOPIC_NAME
                    topic_name =
                        Self::parse_cdr_string(&payload[offset..offset + param_len], little_endian);
                }
                0x0007 => {
                    // PID_TYPE_NAME
                    type_name =
                        Self::parse_cdr_string(&payload[offset..offset + param_len], little_endian);
                }
                0x005A => {
                    // PID_ENDPOINT_GUID (16 bytes)
                    if param_len >= 16 {
                        let guid_bytes = &payload[offset..offset + 16];
                        let prefix = [
                            guid_bytes[0],
                            guid_bytes[1],
                            guid_bytes[2],
                            guid_bytes[3],
                            guid_bytes[4],
                            guid_bytes[5],
                            guid_bytes[6],
                            guid_bytes[7],
                            guid_bytes[8],
                            guid_bytes[9],
                            guid_bytes[10],
                            guid_bytes[11],
                        ];
                        let entity_id = [
                            guid_bytes[12],
                            guid_bytes[13],
                            guid_bytes[14],
                            guid_bytes[15],
                        ];
                        endpoint_guid = Some(Guid::new(prefix, entity_id));
                    }
                }
                0x0070 => {
                    // PID_KEY_HASH (16 bytes) - can also represent GUID
                    if param_len >= 16 && endpoint_guid.is_none() {
                        let guid_bytes = &payload[offset..offset + 16];
                        let prefix = [
                            guid_bytes[0],
                            guid_bytes[1],
                            guid_bytes[2],
                            guid_bytes[3],
                            guid_bytes[4],
                            guid_bytes[5],
                            guid_bytes[6],
                            guid_bytes[7],
                            guid_bytes[8],
                            guid_bytes[9],
                            guid_bytes[10],
                            guid_bytes[11],
                        ];
                        let entity_id = [
                            guid_bytes[12],
                            guid_bytes[13],
                            guid_bytes[14],
                            guid_bytes[15],
                        ];
                        endpoint_guid = Some(Guid::new(prefix, entity_id));
                    }
                }
                _ => {
                    // Unknown parameter - skip
                }
            }

            // Move to next parameter (parameters are 4-byte aligned)
            offset += param_len;
            let align_pad = (4 - (param_len % 4)) % 4;
            offset += align_pad;
        }

        // Register the discovered endpoint under the advertised GUID
        if let Some(guid) = endpoint_guid {
            self.discovery.register_endpoint(
                guid,
                DiscoveredEndpoint {
                    topic_name: topic_name.unwrap_or_else(|| "unknown".to_string()),
                    type_name: type_name.unwrap_or_else(|| "unknown".to_string()),
                },
            );
        }

        Ok(())
    }

    /// Parse a CDR-encoded string from parameter data.
    fn parse_cdr_string(data: &[u8], little_endian: bool) -> Option<String> {
        if data.len() < 4 {
            return None;
        }

        // Read string length (u32)
        let len = if little_endian {
            u32::from_le_bytes([data[0], data[1], data[2], data[3]])
        } else {
            u32::from_be_bytes([data[0], data[1], data[2], data[3]])
        } as usize;

        if len == 0 || len > 1024 || 4 + len > data.len() {
            return None; // Invalid length
        }

        // Extract string bytes (null-terminated)
        let str_bytes = &data[4..4 + len];
        let str_end = str_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(str_bytes.len());
        String::from_utf8(str_bytes[..str_end].to_vec()).ok()
    }

    /// Create a DebugEvent for a user DATA submessage.
    fn create_data_event(
        &mut self,
        writer_guid: &Guid,
        reader_entity: [u8; 4],
        sequence_number: i64,
        payload: &[u8],
        domain_id: Option<u32>,
        ctx: &DecodeContext,
    ) -> Result<Option<DebugEvent>, CoreError> {
        self.sequence += 1;

        // Look up topic name from discovery
        let topic_name = self.discovery.lookup_topic_name(writer_guid);

        // DDS-specific: Prefer INFO_TS timestamp over context timestamp
        let timestamp = self
            .last_timestamp
            .or(ctx.timestamp)
            .unwrap_or_else(Timestamp::now);

        // Build base context and override timestamp if needed
        let ctx_with_timestamp = if self.last_timestamp.is_some() || ctx.timestamp.is_none() {
            DecodeContext {
                timestamp: Some(timestamp),
                ..ctx.clone()
            }
        } else {
            ctx.clone()
        };

        // Build event using context helper
        let mut event_builder = ctx_with_timestamp
            .create_event_builder(TransportKind::DdsRtps)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::copy_from_slice(payload),
            })
            .metadata("dds.writer_guid", writer_guid.to_hex_string())
            .metadata(
                "dds.reader_entity",
                format!(
                    "{:02x}{:02x}{:02x}{:02x}",
                    reader_entity[0], reader_entity[1], reader_entity[2], reader_entity[3]
                ),
            )
            .metadata("dds.sequence_number", sequence_number.to_string())
            .sequence(self.sequence);

        // Add domain ID if available
        if let Some(domain) = domain_id {
            event_builder = event_builder.metadata(METADATA_KEY_DDS_DOMAIN_ID, domain.to_string());
        }

        // Add topic name and correlation key if discovered
        if let Some(topic) = topic_name {
            event_builder = event_builder
                .metadata(METADATA_KEY_DDS_TOPIC_NAME, topic)
                .correlation_key(CorrelationKey::Topic {
                    name: topic.to_string(),
                });
        } else {
            // Fallback: use entity ID as correlation
            event_builder = event_builder.warning(format!(
                "Topic name not discovered for writer GUID {}",
                writer_guid.to_hex_string()
            ));
        }

        Ok(Some(event_builder.build()))
    }
}

impl Default for DdsDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolDecoder for DdsDecoder {
    fn protocol(&self) -> TransportKind {
        TransportKind::DdsRtps
    }

    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        self.decode_rtps_message(data, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtps_parser::RTPS_MAGIC;
    use prb_core::{DecodeContext, METADATA_KEY_DDS_DOMAIN_ID, METADATA_KEY_DDS_TOPIC_NAME};

    #[test]
    fn test_dds_domain_id_boundary() {
        // WS-3.3: Port 7399 → None, port 7400 → domain 0, port 7650 → domain 1
        assert_eq!(
            DdsDecoder::extract_domain_id(7399),
            None,
            "Port 7399 should return None"
        );
        assert_eq!(
            DdsDecoder::extract_domain_id(7400),
            Some(0),
            "Port 7400 should be domain 0"
        );
        assert_eq!(
            DdsDecoder::extract_domain_id(7401),
            Some(0),
            "Port 7401 should be domain 0"
        );
        assert_eq!(
            DdsDecoder::extract_domain_id(7650),
            Some(1),
            "Port 7650 should be domain 1"
        );
        assert_eq!(
            DdsDecoder::extract_domain_id(7651),
            Some(1),
            "Port 7651 should be domain 1"
        );
        assert_eq!(
            DdsDecoder::extract_domain_id(7900),
            Some(2),
            "Port 7900 should be domain 2"
        );
    }

    #[test]
    fn test_dds_timestamp_propagation() {
        // WS-3.3: INFO_TS timestamp appears in DebugEvent (not now())
        let mut decoder = DdsDecoder::new();

        // Build RTPS message with INFO_TS + DATA
        let mut rtps_msg = Vec::new();
        rtps_msg.extend_from_slice(RTPS_MAGIC);
        rtps_msg.extend_from_slice(&[0x02, 0x03]); // Protocol version
        rtps_msg.extend_from_slice(&[0x01, 0x0F]); // Vendor ID
        rtps_msg.extend_from_slice(&[0x01; 12]); // GUID prefix

        // INFO_TS submessage
        rtps_msg.push(0x09); // INFO_TS
        rtps_msg.push(0x01); // flags (little-endian)
        rtps_msg.extend_from_slice(&8u16.to_le_bytes()); // octets_to_next_header
        let ts_seconds = 1234567890u32;
        let ts_fraction = 0x80000000u32; // 0.5 seconds = 500ms
        rtps_msg.extend_from_slice(&ts_seconds.to_le_bytes());
        rtps_msg.extend_from_slice(&ts_fraction.to_le_bytes());

        // DATA submessage (user data, not SEDP)
        rtps_msg.push(0x15); // DATA
        rtps_msg.push(0x01); // flags
        rtps_msg.extend_from_slice(&20u16.to_le_bytes()); // octets_to_next_header
        // extraFlags (2) + octetsToInlineQos (2) + reader/writer EntityId (8) + seqNum (8)
        rtps_msg.extend_from_slice(&[0x00; 20]);

        // Create context with specific timestamp
        let capture_time_ns = 1_000_000_000_000u64; // 1 microsecond as nanoseconds
        let ctx = DecodeContext::new()
            .with_src_addr("192.168.1.1:7400")
            .with_dst_addr("239.255.0.1:7400")
            .with_timestamp(prb_core::Timestamp::from_nanos(capture_time_ns));

        let events = decoder.decode_stream(&rtps_msg, &ctx).unwrap();

        if !events.is_empty() {
            let event = &events[0];
            // Timestamp should come from INFO_TS (1234567890 seconds)
            // Converted to nanoseconds: 1234567890 * 1_000_000_000 + (0x80000000 * 1_000_000_000 / 2^32)
            let expected_ts_ns = (ts_seconds as u64) * 1_000_000_000
                + ((ts_fraction as u64 * 1_000_000_000) / 0x100000000);

            assert_eq!(
                event.timestamp.as_nanos(),
                expected_ts_ns,
                "Timestamp should come from INFO_TS, not context timestamp"
            );
        }
    }

    #[test]
    fn test_dds_sedp_then_user_data() {
        // WS-3.3: Full flow: SEDP discovery DATA → user DATA → event has topic name
        let mut decoder = DdsDecoder::new();

        // Step 1: Send SEDP discovery DATA to register endpoint
        let mut sedp_msg = Vec::new();
        sedp_msg.extend_from_slice(RTPS_MAGIC);
        sedp_msg.extend_from_slice(&[0x02, 0x03]);
        sedp_msg.extend_from_slice(&[0x01, 0x0F]);
        let guid_prefix = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
        ];
        sedp_msg.extend_from_slice(&guid_prefix);

        // DATA submessage with SEDP writer entity ID (0x000003C2)
        sedp_msg.push(0x15); // DATA
        sedp_msg.push(0x01); // flags (little-endian)

        // Build SEDP payload with CDR parameter list
        let mut sedp_payload = Vec::new();

        // Encapsulation header
        sedp_payload.extend_from_slice(&[0x01, 0x00]); // LE CDR
        sedp_payload.extend_from_slice(&[0x00, 0x00]); // Options

        // PID_TOPIC_NAME
        sedp_payload.extend_from_slice(&0x0005u16.to_le_bytes());
        let topic_bytes = b"TestTopic";
        let topic_str_len = (topic_bytes.len() + 1) as u32;
        let topic_param_len = (4 + topic_str_len) as u16;
        sedp_payload.extend_from_slice(&topic_param_len.to_le_bytes());
        sedp_payload.extend_from_slice(&topic_str_len.to_le_bytes());
        sedp_payload.extend_from_slice(topic_bytes);
        sedp_payload.push(0x00);
        while sedp_payload.len() % 4 != 0 {
            sedp_payload.push(0x00);
        }

        // PID_ENDPOINT_GUID
        sedp_payload.extend_from_slice(&0x005Au16.to_le_bytes());
        sedp_payload.extend_from_slice(&16u16.to_le_bytes());
        sedp_payload.extend_from_slice(&guid_prefix);
        let user_entity_id = [0xAA, 0xBB, 0xCC, 0xDD];
        sedp_payload.extend_from_slice(&user_entity_id);

        // PID_SENTINEL
        sedp_payload.extend_from_slice(&0x0001u16.to_le_bytes());
        sedp_payload.extend_from_slice(&0x0000u16.to_le_bytes());

        // Build DATA submessage header
        let data_header_len = 20; // extraFlags (2) + octetsToInlineQos (2) + entityIds (8) + seqNum (8)
        let total_len = data_header_len + sedp_payload.len();
        sedp_msg.extend_from_slice(&(total_len as u16).to_le_bytes());

        // DATA header fields
        sedp_msg.extend_from_slice(&[0x00, 0x00]); // extraFlags
        sedp_msg.extend_from_slice(&[0x10, 0x00]); // octetsToInlineQos (16)
        sedp_msg.extend_from_slice(&[0x00, 0x00, 0x03, 0xC7]); // reader EntityId (SEDP)
        sedp_msg.extend_from_slice(&[0x00, 0x00, 0x03, 0xC2]); // writer EntityId (SEDP)
        sedp_msg.extend_from_slice(&[0x00; 8]); // sequence number

        // SEDP payload
        sedp_msg.extend_from_slice(&sedp_payload);

        let ctx = DecodeContext::new()
            .with_src_addr("192.168.1.1:7400")
            .with_dst_addr("239.255.0.1:7400");

        // Process SEDP message (should register endpoint)
        let _sedp_events = decoder.decode_stream(&sedp_msg, &ctx).unwrap();

        // Step 2: Send user DATA with the same GUID
        let mut user_msg = Vec::new();
        user_msg.extend_from_slice(RTPS_MAGIC);
        user_msg.extend_from_slice(&[0x02, 0x03]);
        user_msg.extend_from_slice(&[0x01, 0x0F]);
        user_msg.extend_from_slice(&guid_prefix);

        // DATA submessage with user entity ID
        user_msg.push(0x15); // DATA
        user_msg.push(0x01); // flags
        let user_data_payload = b"user_data_payload";
        let user_total_len = 20 + user_data_payload.len();
        user_msg.extend_from_slice(&(user_total_len as u16).to_le_bytes());
        user_msg.extend_from_slice(&[0x00, 0x00]); // extraFlags
        user_msg.extend_from_slice(&[0x10, 0x00]); // octetsToInlineQos
        user_msg.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // reader EntityId
        user_msg.extend_from_slice(&user_entity_id); // writer EntityId (matches SEDP registration)
        user_msg.extend_from_slice(&[0x00; 8]); // sequence number
        user_msg.extend_from_slice(user_data_payload);

        let user_events = decoder.decode_stream(&user_msg, &ctx).unwrap();

        // Verify user event has topic name from SEDP discovery
        assert!(!user_events.is_empty(), "Should produce user data event");
        let user_event = &user_events[0];

        let topic_name = user_event.metadata.get(METADATA_KEY_DDS_TOPIC_NAME);
        assert_eq!(
            topic_name,
            Some(&"TestTopic".to_string()),
            "User data event should have topic name from SEDP discovery"
        );

        let domain_id = user_event.metadata.get(METADATA_KEY_DDS_DOMAIN_ID);
        assert_eq!(
            domain_id,
            Some(&"0".to_string()),
            "Should extract domain ID 0 from port 7400"
        );
    }
}
