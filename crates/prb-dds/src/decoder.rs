//! DDS/RTPS protocol decoder implementing the ProtocolDecoder trait.

use crate::discovery::{
    DiscoveredEndpoint, Guid, RtpsDiscoveryTracker, well_known_entities,
};
use crate::error::DdsError;
use crate::rtps_parser::{
    DataSubmessage, InfoTsSubmessage, RtpsMessage, SUBMESSAGE_DATA, SUBMESSAGE_DATA_FRAG,
    SUBMESSAGE_INFO_TS,
};
use bytes::Bytes;
use prb_core::{
    CoreError, CorrelationKey, DebugEvent, DecodeContext, Direction, EventSource, NetworkAddr,
    Payload, ProtocolDecoder, Timestamp, TransportKind, METADATA_KEY_DDS_DOMAIN_ID,
    METADATA_KEY_DDS_TOPIC_NAME,
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
                            if let Err(e) =
                                self.process_discovery_data(&writer_guid, data_msg.serialized_payload)
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
    fn process_discovery_data(&mut self, _writer_guid: &Guid, payload: &[u8]) -> Result<(), DdsError> {
        // Parse CDR parameter list from SEDP payload
        // Format: encapsulation_header (4 bytes) + parameter_list

        if payload.len() < 4 {
            return Err(DdsError::DiscoveryParse("SEDP payload too short".to_string()));
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
                    topic_name = Self::parse_cdr_string(&payload[offset..offset + param_len], little_endian);
                }
                0x0007 => {
                    // PID_TYPE_NAME
                    type_name = Self::parse_cdr_string(&payload[offset..offset + param_len], little_endian);
                }
                0x005A => {
                    // PID_ENDPOINT_GUID (16 bytes)
                    if param_len >= 16 {
                        let guid_bytes = &payload[offset..offset + 16];
                        let prefix = [
                            guid_bytes[0], guid_bytes[1], guid_bytes[2], guid_bytes[3],
                            guid_bytes[4], guid_bytes[5], guid_bytes[6], guid_bytes[7],
                            guid_bytes[8], guid_bytes[9], guid_bytes[10], guid_bytes[11],
                        ];
                        let entity_id = [guid_bytes[12], guid_bytes[13], guid_bytes[14], guid_bytes[15]];
                        endpoint_guid = Some(Guid::new(prefix, entity_id));
                    }
                }
                0x0070 => {
                    // PID_KEY_HASH (16 bytes) - can also represent GUID
                    if param_len >= 16 && endpoint_guid.is_none() {
                        let guid_bytes = &payload[offset..offset + 16];
                        let prefix = [
                            guid_bytes[0], guid_bytes[1], guid_bytes[2], guid_bytes[3],
                            guid_bytes[4], guid_bytes[5], guid_bytes[6], guid_bytes[7],
                            guid_bytes[8], guid_bytes[9], guid_bytes[10], guid_bytes[11],
                        ];
                        let entity_id = [guid_bytes[12], guid_bytes[13], guid_bytes[14], guid_bytes[15]];
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
        let str_end = str_bytes.iter().position(|&b| b == 0).unwrap_or(str_bytes.len());
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

        let timestamp = self.last_timestamp
            .or(ctx.timestamp)
            .unwrap_or_else(Timestamp::now);

        let mut event_builder = DebugEvent::builder()
            .timestamp(timestamp)
            .source(EventSource {
                adapter: "pcap".to_string(),
                origin: ctx
                    .metadata
                    .get("origin")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                network: Some(NetworkAddr {
                    src: ctx.src_addr.clone().unwrap_or_else(|| "unknown".to_string()),
                    dst: ctx.dst_addr.clone().unwrap_or_else(|| "unknown".to_string()),
                }),
            })
            .transport(TransportKind::DdsRtps)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::copy_from_slice(payload),
            })
            .metadata("dds.writer_guid", writer_guid.to_hex_string())
            .metadata("dds.reader_entity", format!("{:02x}{:02x}{:02x}{:02x}",
                reader_entity[0], reader_entity[1], reader_entity[2], reader_entity[3]))
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
