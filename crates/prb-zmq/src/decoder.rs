//! ZMQ protocol decoder implementing the ProtocolDecoder trait.

use crate::parser::{ZmtpEvent, ZmtpParser};
use bytes::Bytes;
use prb_core::{
    CorrelationKey, CoreError, DebugEvent, DecodeContext, Direction, EventSource, NetworkAddr,
    Payload, ProtocolDecoder, Timestamp, TransportKind, METADATA_KEY_ZMQ_TOPIC,
};
use std::collections::HashMap;

/// Well-known metadata key for ZMQ socket type.
const METADATA_KEY_ZMQ_SOCKET_TYPE: &str = "zmq.socket_type";

/// Well-known metadata key for ZMQ socket identity.
const METADATA_KEY_ZMQ_IDENTITY: &str = "zmq.identity";

/// Well-known metadata key for connection ID.
const METADATA_KEY_CONNECTION_ID: &str = "zmq.connection_id";

/// ZMQ protocol decoder.
///
/// Decodes ZMTP messages from reassembled TCP streams by:
/// 1. Parsing ZMTP greeting and handshake
/// 2. Extracting socket metadata (type, identity)
/// 3. Reassembling multipart messages
/// 4. Extracting PUB/SUB topics
/// 5. Supporting mid-stream graceful degradation
pub struct ZmqDecoder {
    /// ZMTP wire protocol parser.
    parser: ZmtpParser,
    /// Socket metadata from READY command.
    socket_metadata: HashMap<String, Vec<u8>>,
    /// Sequence counter for events.
    sequence: u64,
    /// Connection ID for correlation.
    connection_id: String,
}

impl ZmqDecoder {
    /// Create a new ZMQ decoder.
    pub fn new() -> Self {
        Self {
            parser: ZmtpParser::new(),
            socket_metadata: HashMap::new(),
            sequence: 0,
            connection_id: String::new(),
        }
    }

    /// Process ZMTP events and generate DebugEvents.
    fn process_zmtp_events(
        &mut self,
        events: Vec<ZmtpEvent>,
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        let mut debug_events = Vec::new();

        for event in events {
            match event {
                ZmtpEvent::Greeting(greeting) => {
                    // Store greeting metadata but don't emit event
                    tracing::debug!(
                        "ZMTP greeting: version={}.{}, mechanism={}",
                        greeting.major_version,
                        greeting.minor_version,
                        greeting.mechanism
                    );

                    // Generate connection ID from context
                    if self.connection_id.is_empty() {
                        self.connection_id = format!(
                            "{}->{}",
                            ctx.src_addr.as_ref().unwrap_or(&"unknown".to_string()),
                            ctx.dst_addr.as_ref().unwrap_or(&"unknown".to_string())
                        );
                    }
                }
                ZmtpEvent::Ready(ready) => {
                    // Store socket metadata
                    self.socket_metadata = ready.properties;
                    tracing::debug!("ZMTP READY: {:?}", self.socket_metadata.keys());
                }
                ZmtpEvent::Message(message) => {
                    // Create DebugEvent for message
                    if let Some(event) = self.create_message_event(message.frames, ctx)? {
                        debug_events.push(event);
                    }
                }
                ZmtpEvent::Command(cmd) => {
                    // Log commands but don't emit as DebugEvents
                    tracing::debug!("ZMTP command: {}", cmd.name);
                }
            }
        }

        Ok(debug_events)
    }

    /// Create a DebugEvent for a ZMQ message.
    fn create_message_event(
        &mut self,
        frames: Vec<Vec<u8>>,
        ctx: &DecodeContext,
    ) -> Result<Option<DebugEvent>, CoreError> {
        if frames.is_empty() {
            return Ok(None);
        }

        self.sequence += 1;

        // Extract socket type
        let socket_type = self
            .socket_metadata
            .get("Socket-Type")
            .and_then(|v| String::from_utf8(v.clone()).ok())
            .unwrap_or_else(|| "UNKNOWN".to_string());

        // Extract identity
        let identity = self
            .socket_metadata
            .get("Identity")
            .and_then(|v| String::from_utf8(v.clone()).ok())
            .unwrap_or_default();

        // For PUB/SUB, first frame is the topic
        let (topic, payload_frames) = if socket_type == "PUB" || socket_type == "SUB" {
            if frames.len() > 1 {
                let topic = String::from_utf8_lossy(&frames[0]).to_string();
                (Some(topic), &frames[1..])
            } else {
                (None, &frames[..])
            }
        } else {
            (None, &frames[..])
        };

        // Combine payload frames
        let mut payload_data = Vec::new();
        for frame in payload_frames {
            payload_data.extend_from_slice(frame);
        }

        // Build event
        let mut event_builder = DebugEvent::builder()
            .timestamp(ctx.timestamp.unwrap_or_else(Timestamp::now))
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
            .transport(TransportKind::Zmq)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from(payload_data),
            })
            .metadata(METADATA_KEY_ZMQ_SOCKET_TYPE, &socket_type)
            .metadata(METADATA_KEY_CONNECTION_ID, &self.connection_id)
            .sequence(self.sequence);

        if !identity.is_empty() {
            event_builder = event_builder.metadata(METADATA_KEY_ZMQ_IDENTITY, &identity);
        }

        if let Some(topic_str) = topic {
            event_builder = event_builder
                .metadata(METADATA_KEY_ZMQ_TOPIC, &topic_str)
                .correlation_key(CorrelationKey::Topic {
                    name: topic_str.clone(),
                });
        } else if !identity.is_empty() {
            event_builder = event_builder.correlation_key(CorrelationKey::Custom {
                key: "zmq-identity".to_string(),
                value: identity,
            });
        } else {
            event_builder = event_builder.correlation_key(CorrelationKey::Custom {
                key: "zmq-connection".to_string(),
                value: self.connection_id.clone(),
            });
        }

        // Add degradation warning if applicable
        if self.parser.is_degraded() {
            event_builder = event_builder.warning(
                "ZMTP degradation: mid-stream capture, message parsing may be incomplete or inaccurate",
            );
        }

        Ok(Some(event_builder.build()))
    }
}

impl Default for ZmqDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolDecoder for ZmqDecoder {
    fn protocol(&self) -> TransportKind {
        TransportKind::Zmq
    }

    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // Process data through ZMTP parser
        let zmtp_events = self
            .parser
            .feed(data)
            .map_err(|e| CoreError::PayloadDecode(e.to_string()))?;

        // Convert ZMTP events to DebugEvents
        self.process_zmtp_events(zmtp_events, ctx)
    }
}
