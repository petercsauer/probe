//! Adapters to integrate native plugins with the PRB detection and decoding system.

use crate::loader::LoadedPlugin;
use bytes::Bytes;
use prb_core::{CoreError, DebugEvent, DecodeContext, ProtocolDecoder, TransportKind};
use prb_detect::{
    DecoderFactory, DetectionContext, DetectionMethod, DetectionResult, ProtocolDetector,
    ProtocolId, TransportLayer,
};
use prb_plugin_api::{DebugEventDto, DecodeCtx};
use std::sync::Arc;

/// Adapts a loaded native plugin to the DecoderFactory trait.
pub struct NativeDecoderFactory {
    plugin: Arc<LoadedPlugin>,
}

impl NativeDecoderFactory {
    /// Create a new factory from a loaded plugin.
    pub fn new(plugin: Arc<LoadedPlugin>) -> Self {
        Self { plugin }
    }
}

impl DecoderFactory for NativeDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(&self.plugin.metadata().protocol_id)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        Box::new(NativeDecoderInstance::new(Arc::clone(&self.plugin)))
    }
}

/// Wraps a native plugin decoder handle as a ProtocolDecoder.
struct NativeDecoderInstance {
    plugin: Arc<LoadedPlugin>,
    handle: *mut std::ffi::c_void,
}

impl NativeDecoderInstance {
    fn new(plugin: Arc<LoadedPlugin>) -> Self {
        let handle = plugin.create_decoder();
        Self { plugin, handle }
    }
}

impl ProtocolDecoder for NativeDecoderInstance {
    fn protocol(&self) -> TransportKind {
        // Map protocol ID to TransportKind
        // For now, we'll use a simple heuristic
        match self.plugin.metadata().protocol_id.as_str() {
            "grpc" | "http2" => TransportKind::Grpc,
            "zmtp" | "zeromq" => TransportKind::Zmq,
            "rtps" | "dds" => TransportKind::DdsRtps,
            "tcp" => TransportKind::RawTcp,
            "udp" => TransportKind::RawUdp,
            _ => TransportKind::JsonFixture, // Default fallback
        }
    }

    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        // Convert DecodeContext to DecodeCtx (plugin API type)
        let plugin_ctx = DecodeCtx {
            src_addr: ctx.src_addr.clone(),
            dst_addr: ctx.dst_addr.clone(),
            timestamp_nanos: ctx.timestamp.map(|t| t.as_nanos()),
            metadata: ctx.metadata.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        };

        // Serialize context to JSON
        let ctx_json = serde_json::to_vec(&plugin_ctx).map_err(|e| {
            CoreError::PayloadDecode(format!("Failed to serialize decode context: {}", e))
        })?;

        // Call plugin's decode function
        let result_buf = self.plugin.decode(self.handle, data, &ctx_json);

        // Convert OwnedBuffer to Vec<u8>
        let result_json = unsafe {
            if result_buf.ptr.is_null() {
                Vec::new()
            } else {
                result_buf.into_vec()
            }
        };

        if result_json.is_empty() {
            return Ok(Vec::new());
        }

        // Deserialize Vec<DebugEventDto>
        let event_dtos: Vec<DebugEventDto> =
            serde_json::from_slice(&result_json).map_err(|e| {
                CoreError::PayloadDecode(format!("Failed to deserialize plugin events: {}", e))
            })?;

        // Convert DTOs to DebugEvents
        let events = event_dtos
            .into_iter()
            .map(dto_to_debug_event)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(events)
    }
}

impl Drop for NativeDecoderInstance {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            self.plugin.destroy_decoder(self.handle);
        }
    }
}

unsafe impl Send for NativeDecoderInstance {}

/// Adapts a loaded native plugin to the ProtocolDetector trait.
pub struct NativeProtocolDetector {
    plugin: Arc<LoadedPlugin>,
    transport: TransportLayer,
}

impl NativeProtocolDetector {
    /// Create a new detector from a loaded plugin.
    ///
    /// The `transport` parameter specifies which transport layer this detector
    /// applies to.
    pub fn new(plugin: Arc<LoadedPlugin>, transport: TransportLayer) -> Self {
        Self { plugin, transport }
    }
}

impl ProtocolDetector for NativeProtocolDetector {
    fn name(&self) -> &str {
        &self.plugin.metadata().name
    }

    fn transport(&self) -> TransportLayer {
        self.transport
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        // Convert transport layer to u8
        let transport_u8 = match ctx.transport {
            TransportLayer::Tcp => 0,
            TransportLayer::Udp => 1,
        };

        // Call plugin's detect function
        let result = self
            .plugin
            .detect(ctx.initial_bytes, ctx.src_port, ctx.dst_port, transport_u8);

        if result.detected == 0 {
            return None;
        }

        Some(DetectionResult {
            protocol: ProtocolId::new(&self.plugin.metadata().protocol_id),
            confidence: result.confidence,
            method: DetectionMethod::Heuristic,
            version: None,
        })
    }
}

/// Convert a DebugEventDto to a DebugEvent.
fn dto_to_debug_event(dto: DebugEventDto) -> Result<DebugEvent, CoreError> {
    use prb_core::{CorrelationKey, Direction, EventSource, Payload, Timestamp};

    // Parse transport kind
    let transport = match dto.transport.as_str() {
        "grpc" => TransportKind::Grpc,
        "zmtp" | "zeromq" => TransportKind::Zmq,
        "rtps" | "dds" => TransportKind::DdsRtps,
        "tcp" => TransportKind::RawTcp,
        "udp" => TransportKind::RawUdp,
        _ => TransportKind::JsonFixture, // Default fallback
    };

    // Parse direction - map various directions to Inbound/Outbound/Unknown
    let direction = match dto.direction.as_str() {
        "request" | "subscribe" | "inbound" => Direction::Inbound,
        "response" | "publish" | "outbound" => Direction::Outbound,
        _ => Direction::Unknown,
    };

    // Convert payload
    let payload = if let Some(decoded_fields) = dto.payload_decoded {
        Payload::Decoded {
            raw: dto.payload_raw.map(Into::into).unwrap_or_default(),
            fields: decoded_fields,
            schema_name: dto.schema_name,
        }
    } else if let Some(raw) = dto.payload_raw {
        Payload::Raw { raw: raw.into() }
    } else {
        // If no payload, use empty raw bytes
        Payload::Raw { raw: Bytes::new() }
    };

    // Convert correlation keys - map from DTO to enum variants
    let correlation_keys: Vec<CorrelationKey> = dto
        .correlation_keys
        .into_iter()
        .filter_map(|k| match k.kind.as_str() {
            "stream_id" => k.value.parse::<u32>().ok().map(|id| CorrelationKey::StreamId { id }),
            "topic" => Some(CorrelationKey::Topic { name: k.value }),
            "connection_id" => Some(CorrelationKey::ConnectionId { id: k.value }),
            "trace_context" => {
                // Expect value format "trace_id:span_id"
                let parts: Vec<&str> = k.value.split(':').collect();
                if parts.len() == 2 {
                    Some(CorrelationKey::TraceContext {
                        trace_id: parts[0].to_string(),
                        span_id: parts[1].to_string(),
                    })
                } else {
                    None
                }
            }
            _ => Some(CorrelationKey::Custom {
                key: k.kind,
                value: k.value,
            }),
        })
        .collect();

    // Create event source
    let source = EventSource {
        adapter: "native-plugin".to_string(),
        origin: dto.src_addr.clone().unwrap_or_else(|| "unknown".to_string()),
        network: if dto.src_addr.is_some() && dto.dst_addr.is_some() {
            Some(prb_core::NetworkAddr {
                src: dto.src_addr.clone().unwrap(),
                dst: dto.dst_addr.clone().unwrap(),
            })
        } else {
            None
        },
    };

    // Build the event using the builder pattern
    let mut builder = DebugEvent::builder()
        .timestamp(Timestamp::from_nanos(dto.timestamp_nanos))
        .source(source)
        .transport(transport)
        .direction(direction)
        .payload(payload);

    // Add correlation keys
    for key in correlation_keys {
        builder = builder.correlation_key(key);
    }

    // Add metadata
    for (key, value) in dto.metadata {
        builder = builder.metadata(key, value);
    }

    // Add warnings
    for warning in dto.warnings {
        builder = builder.warning(warning);
    }

    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dto_to_debug_event_minimal() {
        let dto = DebugEventDto::minimal("grpc", "request");
        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.transport, TransportKind::Grpc);
        assert_eq!(event.direction, prb_core::Direction::Inbound);
    }

    #[test]
    fn test_dto_to_debug_event_with_payload() {
        let mut dto = DebugEventDto::minimal("zmtp", "publish");
        dto.payload_raw = Some(vec![1, 2, 3, 4]);
        dto.payload_decoded = Some(serde_json::json!({"key": "value"}));
        dto.schema_name = Some("test.Schema".to_string());

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.transport, TransportKind::Zmq);
        assert!(matches!(event.payload, prb_core::Payload::Decoded { .. }));
    }
}
