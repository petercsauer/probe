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

/// Adapts a loaded native plugin to the `DecoderFactory` trait.
pub struct NativeDecoderFactory {
    plugin: Arc<LoadedPlugin>,
}

impl NativeDecoderFactory {
    /// Create a new factory from a loaded plugin.
    #[must_use] 
    pub const fn new(plugin: Arc<LoadedPlugin>) -> Self {
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

/// Wraps a native plugin decoder handle as a `ProtocolDecoder`.
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
            metadata: ctx
                .metadata
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        };

        // Serialize context to JSON
        let ctx_json = serde_json::to_vec(&plugin_ctx).map_err(|e| {
            CoreError::PayloadDecode(format!("Failed to serialize decode context: {e}"))
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
        let event_dtos: Vec<DebugEventDto> = serde_json::from_slice(&result_json).map_err(|e| {
            CoreError::PayloadDecode(format!("Failed to deserialize plugin events: {e}"))
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

/// Adapts a loaded native plugin to the `ProtocolDetector` trait.
pub struct NativeProtocolDetector {
    plugin: Arc<LoadedPlugin>,
    transport: TransportLayer,
}

impl NativeProtocolDetector {
    /// Create a new detector from a loaded plugin.
    ///
    /// The `transport` parameter specifies which transport layer this detector
    /// applies to.
    #[must_use] 
    pub const fn new(plugin: Arc<LoadedPlugin>, transport: TransportLayer) -> Self {
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
        let result =
            self.plugin
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

/// Convert a `DebugEventDto` to a `DebugEvent`.
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
            "stream_id" => k
                .value
                .parse::<u32>()
                .ok()
                .map(|id| CorrelationKey::StreamId { id }),
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
        origin: dto
            .src_addr
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
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

    #[test]
    fn test_dto_to_debug_event_all_transports() {
        let transports = vec![
            ("grpc", TransportKind::Grpc),
            ("zmtp", TransportKind::Zmq),
            ("zeromq", TransportKind::Zmq),
            ("rtps", TransportKind::DdsRtps),
            ("dds", TransportKind::DdsRtps),
            ("tcp", TransportKind::RawTcp),
            ("udp", TransportKind::RawUdp),
            ("unknown", TransportKind::JsonFixture),
            ("http2", TransportKind::JsonFixture),
        ];

        for (transport_str, expected_kind) in transports {
            let dto = DebugEventDto::minimal(transport_str, "request");
            let event = dto_to_debug_event(dto).expect("conversion should succeed");
            assert_eq!(
                event.transport, expected_kind,
                "failed for transport: {transport_str}"
            );
        }
    }

    #[test]
    fn test_dto_to_debug_event_all_directions() {
        let directions = vec![
            ("request", prb_core::Direction::Inbound),
            ("subscribe", prb_core::Direction::Inbound),
            ("inbound", prb_core::Direction::Inbound),
            ("response", prb_core::Direction::Outbound),
            ("publish", prb_core::Direction::Outbound),
            ("outbound", prb_core::Direction::Outbound),
            ("unknown", prb_core::Direction::Unknown),
            ("invalid", prb_core::Direction::Unknown),
        ];

        for (direction_str, expected_dir) in directions {
            let dto = DebugEventDto::minimal("tcp", direction_str);
            let event = dto_to_debug_event(dto).expect("conversion should succeed");
            assert_eq!(
                event.direction, expected_dir,
                "failed for direction: {direction_str}"
            );
        }
    }

    #[test]
    fn test_dto_to_debug_event_with_raw_payload_only() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.payload_raw = Some(vec![1, 2, 3, 4]);

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        match event.payload {
            prb_core::Payload::Raw { raw } => {
                assert_eq!(raw.as_ref(), &[1, 2, 3, 4]);
            }
            _ => panic!("expected raw payload"),
        }
    }

    #[test]
    fn test_dto_to_debug_event_with_decoded_payload_only() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.payload_decoded = Some(serde_json::json!({"key": "value", "number": 42}));
        dto.schema_name = Some("MySchema".to_string());

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        match event.payload {
            prb_core::Payload::Decoded {
                raw: _,
                fields,
                schema_name,
            } => {
                assert_eq!(fields["key"], "value");
                assert_eq!(fields["number"], 42);
                assert_eq!(schema_name, Some("MySchema".to_string()));
            }
            _ => panic!("expected decoded payload"),
        }
    }

    #[test]
    fn test_dto_to_debug_event_correlation_key_stream_id() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.correlation_keys = vec![prb_plugin_api::CorrelationKeyDto {
            kind: "stream_id".to_string(),
            value: "42".to_string(),
        }];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.correlation_keys.len(), 1);
        match &event.correlation_keys[0] {
            prb_core::CorrelationKey::StreamId { id } => assert_eq!(*id, 42),
            _ => panic!("expected StreamId"),
        }
    }

    #[test]
    fn test_dto_to_debug_event_correlation_key_topic() {
        let mut dto = DebugEventDto::minimal("zmtp", "publish");
        dto.correlation_keys = vec![prb_plugin_api::CorrelationKeyDto {
            kind: "topic".to_string(),
            value: "events.topic".to_string(),
        }];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.correlation_keys.len(), 1);
        match &event.correlation_keys[0] {
            prb_core::CorrelationKey::Topic { name } => assert_eq!(name, "events.topic"),
            _ => panic!("expected Topic"),
        }
    }

    #[test]
    fn test_dto_to_debug_event_correlation_key_connection_id() {
        let mut dto = DebugEventDto::minimal("tcp", "request");
        dto.correlation_keys = vec![prb_plugin_api::CorrelationKeyDto {
            kind: "connection_id".to_string(),
            value: "conn-abc-123".to_string(),
        }];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.correlation_keys.len(), 1);
        match &event.correlation_keys[0] {
            prb_core::CorrelationKey::ConnectionId { id } => assert_eq!(id, "conn-abc-123"),
            _ => panic!("expected ConnectionId"),
        }
    }

    #[test]
    fn test_dto_to_debug_event_correlation_key_trace_context() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.correlation_keys = vec![prb_plugin_api::CorrelationKeyDto {
            kind: "trace_context".to_string(),
            value: "trace-abc:span-xyz".to_string(),
        }];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.correlation_keys.len(), 1);
        match &event.correlation_keys[0] {
            prb_core::CorrelationKey::TraceContext { trace_id, span_id } => {
                assert_eq!(trace_id, "trace-abc");
                assert_eq!(span_id, "span-xyz");
            }
            _ => panic!("expected TraceContext"),
        }
    }

    #[test]
    fn test_dto_to_debug_event_correlation_key_trace_invalid() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.correlation_keys = vec![prb_plugin_api::CorrelationKeyDto {
            kind: "trace_context".to_string(),
            value: "invalid-no-colon".to_string(),
        }];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");
        assert_eq!(event.correlation_keys.len(), 0);
    }

    #[test]
    fn test_dto_to_debug_event_correlation_key_custom() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.correlation_keys = vec![prb_plugin_api::CorrelationKeyDto {
            kind: "custom_key".to_string(),
            value: "custom_value".to_string(),
        }];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.correlation_keys.len(), 1);
        match &event.correlation_keys[0] {
            prb_core::CorrelationKey::Custom { key, value } => {
                assert_eq!(key, "custom_key");
                assert_eq!(value, "custom_value");
            }
            _ => panic!("expected Custom"),
        }
    }

    #[test]
    fn test_dto_to_debug_event_correlation_key_invalid_stream_id() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.correlation_keys = vec![prb_plugin_api::CorrelationKeyDto {
            kind: "stream_id".to_string(),
            value: "not-a-number".to_string(),
        }];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");
        assert_eq!(event.correlation_keys.len(), 0);
    }

    #[test]
    fn test_dto_to_debug_event_multiple_correlation_keys() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.correlation_keys = vec![
            prb_plugin_api::CorrelationKeyDto {
                kind: "stream_id".to_string(),
                value: "123".to_string(),
            },
            prb_plugin_api::CorrelationKeyDto {
                kind: "topic".to_string(),
                value: "test.topic".to_string(),
            },
            prb_plugin_api::CorrelationKeyDto {
                kind: "custom".to_string(),
                value: "value".to_string(),
            },
        ];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");
        assert_eq!(event.correlation_keys.len(), 3);
    }

    #[test]
    fn test_dto_to_debug_event_with_metadata() {
        use std::collections::HashMap;
        let mut dto = DebugEventDto::minimal("grpc", "request");
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());
        dto.metadata = metadata;

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.metadata.len(), 2);
        assert_eq!(event.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(event.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_dto_to_debug_event_with_warnings() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.warnings = vec!["warning 1".to_string(), "warning 2".to_string()];

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.warnings.len(), 2);
        assert!(event.warnings.contains(&"warning 1".to_string()));
        assert!(event.warnings.contains(&"warning 2".to_string()));
    }

    #[test]
    fn test_dto_to_debug_event_with_addresses() {
        let mut dto = DebugEventDto::minimal("tcp", "request");
        dto.src_addr = Some("192.168.1.1:8080".to_string());
        dto.dst_addr = Some("192.168.1.2:9090".to_string());

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.origin, "192.168.1.1:8080");
        assert!(event.source.network.is_some());

        let network = event.source.network.unwrap();
        assert_eq!(network.src, "192.168.1.1:8080");
        assert_eq!(network.dst, "192.168.1.2:9090");
    }

    #[test]
    fn test_dto_to_debug_event_no_addresses() {
        let dto = DebugEventDto::minimal("tcp", "request");
        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.origin, "unknown");
        assert!(event.source.network.is_none());
    }

    #[test]
    fn test_dto_to_debug_event_partial_address() {
        let mut dto = DebugEventDto::minimal("tcp", "request");
        dto.src_addr = Some("192.168.1.1:8080".to_string());

        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.origin, "192.168.1.1:8080");
        assert!(event.source.network.is_none());
    }

    #[test]
    fn test_dto_to_debug_event_with_timestamp() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.timestamp_nanos = 1234567890123456789;

        let event = dto_to_debug_event(dto).expect("conversion should succeed");
        assert_eq!(event.timestamp.as_nanos(), 1234567890123456789);
    }

    #[test]
    fn test_dto_to_debug_event_source_adapter_name() {
        let dto = DebugEventDto::minimal("grpc", "request");
        let event = dto_to_debug_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.adapter, "native-plugin");
    }
}
