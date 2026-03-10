//! DDS/RTPS protocol decoder implementing the ProtocolDecoder trait.

use crate::discovery::{
    DiscoveredEndpoint, Guid, RtpsDiscoveryTracker, well_known_entities,
};
use crate::error::DdsError;
use bytes::Bytes;
use prb_core::{
    CoreError, CorrelationKey, DebugEvent, DecodeContext, Direction, EventSource, NetworkAddr,
    Payload, ProtocolDecoder, Timestamp, TransportKind, METADATA_KEY_DDS_DOMAIN_ID,
    METADATA_KEY_DDS_TOPIC_NAME,
};
use rtps_parser::rtps::{
    messages::overall_structure::{RtpsMessageRead, RtpsSubmessageReadKind},
    types::{EntityId as RtpsEntityId, SequenceNumber},
};
use std::sync::Arc;

/// Convert rtps-parser EntityId to our [u8; 4] representation.
fn convert_entity_id(entity_id: RtpsEntityId) -> [u8; 4] {
    let key = entity_id.entity_key();
    let kind = entity_id.entity_kind();
    [key[0], key[1], key[2], kind]
}

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
        // Check magic
        if !Self::has_rtps_magic(data) {
            let magic = if data.len() >= 4 {
                [data[0], data[1], data[2], data[3]]
            } else {
                [0, 0, 0, 0]
            };
            return Err(CoreError::UnsupportedTransport(format!(
                "Not an RTPS message: {:?}",
                DdsError::InvalidMagic(magic)
            )));
        }

        // Parse RTPS message
        let arc_data = Arc::from(data);
        let rtps_msg = RtpsMessageRead::new(arc_data);

        let guid_prefix = rtps_msg.header().guid_prefix();

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
        for submessage in rtps_msg.submessages() {
            match submessage {
                RtpsSubmessageReadKind::InfoTimestamp(info_ts) => {
                    // Store timestamp for subsequent DATA submessages
                    let ts = info_ts.timestamp();
                    let nanos = (ts.seconds() as u64) * 1_000_000_000
                        + (ts.fraction() as u64 * 1_000_000_000 / (1u64 << 32));
                    self.last_timestamp = Some(Timestamp::from_nanos(nanos));
                }
                RtpsSubmessageReadKind::Data(data_msg) => {
                    // Extract writer and reader entity IDs
                    let writer_entity = data_msg.writer_id();
                    let reader_entity = data_msg.reader_id();
                    let writer_guid = Guid::new(guid_prefix, convert_entity_id(writer_entity));

                    // Check if this is a SEDP discovery message
                    let writer_entity_bytes = convert_entity_id(writer_entity);
                    if well_known_entities::is_sedp_entity(&writer_entity_bytes) {
                        // Process discovery data
                        let payload = data_msg.serialized_payload();
                        if let Err(e) = self.process_discovery_data(&writer_guid, payload.as_ref()) {
                            tracing::debug!("Failed to parse discovery data: {}", e);
                        }
                    } else {
                        // User data message
                        let payload = data_msg.serialized_payload();
                        if let Some(event) = self.create_data_event(
                            &writer_guid,
                            convert_entity_id(reader_entity),
                            data_msg.writer_sn(),
                            payload.as_ref(),
                            domain_id,
                            ctx,
                        )? {
                            events.push(event);
                        }
                    }
                }
                RtpsSubmessageReadKind::DataFrag(_) => {
                    // DATA_FRAG: fragmented data (Phase 1 logs warning)
                    tracing::debug!(
                        "DATA_FRAG submessage detected (fragmented data not yet supported)"
                    );
                }
                RtpsSubmessageReadKind::Heartbeat(_) | RtpsSubmessageReadKind::AckNack(_) => {
                    // Reliability protocol messages - no events
                }
                _ => {
                    // Other submessage types - ignore
                }
            }
        }

        Ok(events)
    }

    /// Process SEDP discovery data to extract topic name.
    fn process_discovery_data(&mut self, writer_guid: &Guid, payload: &[u8]) -> Result<(), DdsError> {
        // SEDP discovery data uses CDR encoding
        // Phase 1: simple string extraction for topic_name field
        // Full CDR decode deferred to later phase

        // Look for topic_name string in payload (simplified heuristic)
        // CDR strings: 4-byte length + UTF-8 data
        let topic_name = Self::extract_cdr_string(payload, b"topicName")
            .or_else(|| Self::extract_cdr_string(payload, b"topic_name"))
            .unwrap_or_else(|| "unknown".to_string());

        let type_name = Self::extract_cdr_string(payload, b"typeName")
            .or_else(|| Self::extract_cdr_string(payload, b"type_name"))
            .unwrap_or_else(|| "unknown".to_string());

        self.discovery.register_endpoint(
            *writer_guid,
            DiscoveredEndpoint {
                topic_name,
                type_name,
            },
        );

        Ok(())
    }

    /// Extract a CDR-encoded string from payload (heuristic for Phase 1).
    fn extract_cdr_string(payload: &[u8], field_marker: &[u8]) -> Option<String> {
        // Search for field marker followed by string length + data
        let mut pos = 0;
        while pos < payload.len().saturating_sub(field_marker.len() + 4) {
            if &payload[pos..pos + field_marker.len()] == field_marker {
                // Found marker, try to read string after it
                let mut str_pos = pos + field_marker.len();

                // Skip null terminators and padding
                while str_pos < payload.len() && payload[str_pos] == 0 {
                    str_pos += 1;
                }

                if str_pos + 4 <= payload.len() {
                    // Read length (little-endian)
                    let len = u32::from_le_bytes([
                        payload[str_pos],
                        payload[str_pos + 1],
                        payload[str_pos + 2],
                        payload[str_pos + 3],
                    ]) as usize;

                    if len > 0 && len < 256 && str_pos + 4 + len <= payload.len() {
                        // Extract string (null-terminated)
                        let str_bytes = &payload[str_pos + 4..str_pos + 4 + len];
                        if let Some(null_pos) = str_bytes.iter().position(|&b| b == 0) {
                            if let Ok(s) = String::from_utf8(str_bytes[..null_pos].to_vec()) {
                                return Some(s);
                            }
                        }
                    }
                }
            }
            pos += 1;
        }
        None
    }

    /// Create a DebugEvent for a user DATA submessage.
    fn create_data_event(
        &mut self,
        writer_guid: &Guid,
        reader_entity: [u8; 4],
        sequence_number: SequenceNumber,
        payload: &[u8],
        domain_id: Option<u32>,
        ctx: &DecodeContext,
    ) -> Result<Option<DebugEvent>, CoreError> {
        self.sequence += 1;

        // Look up topic name from discovery
        let topic_name = self.discovery.lookup_topic_name(writer_guid);

        let timestamp = self.last_timestamp.unwrap_or_else(Timestamp::now);

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
            .metadata("dds.sequence_number", i64::from(sequence_number).to_string())
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
