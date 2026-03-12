//! Adapters for WASM plugins to implement prb-detect and prb-core traits.

use crate::runtime::WasmLimits;
use bytes::Bytes;
use extism::{Manifest, Plugin, Wasm};
use prb_core::{CoreError, DebugEvent, DecodeContext, ProtocolDecoder, TransportKind};
use prb_detect::{
    DecoderFactory, DetectionContext, DetectionMethod, DetectionResult, ProtocolDetector,
    ProtocolId, TransportLayer,
};
use prb_plugin_api::types::WasmDecodeRequest;
use prb_plugin_api::{DecodeCtx, DetectContext, PluginMetadata};
use std::path::PathBuf;
use tracing::warn;

/// Factory for creating WASM decoder instances.
///
/// Each call to `create_decoder()` instantiates a new WASM plugin instance,
/// as WASM instances are not thread-safe or reentrant.
pub struct WasmDecoderFactory {
    plugin_path: PathBuf,
    info: PluginMetadata,
    limits: WasmLimits,
}

impl WasmDecoderFactory {
    #[must_use]
    pub const fn new(plugin_path: PathBuf, info: PluginMetadata, limits: WasmLimits) -> Self {
        Self {
            plugin_path,
            info,
            limits,
        }
    }
}

impl DecoderFactory for WasmDecoderFactory {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::new(&self.info.protocol_id)
    }

    fn create_decoder(&self) -> Box<dyn ProtocolDecoder> {
        // Each decoder instance gets its own WASM plugin instance
        let manifest = Manifest::new([Wasm::file(&self.plugin_path)])
            .with_memory_max(self.limits.memory_max_pages)
            .with_timeout(self.limits.timeout);

        let instance =
            Plugin::new(&manifest, [], true).expect("plugin already validated during load");

        Box::new(WasmDecoderInstance { instance })
    }
}

/// WASM decoder instance wrapping an Extism plugin.
struct WasmDecoderInstance {
    instance: Plugin,
}

impl ProtocolDecoder for WasmDecoderInstance {
    fn protocol(&self) -> TransportKind {
        // TODO: Wire through protocol from metadata
        TransportKind::RawTcp
    }

    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> {
        use base64::Engine;

        // Prepare decode request
        let request = WasmDecodeRequest {
            data_b64: base64::engine::general_purpose::STANDARD.encode(data),
            ctx: convert_decode_context(ctx),
        };

        let request_json = serde_json::to_string(&request)
            .map_err(|e| CoreError::PayloadDecode(format!("serialize request: {e}")))?;

        // Call WASM decode function
        let result_json = self
            .instance
            .call::<&str, String>("prb_plugin_decode", &request_json)
            .map_err(|e| CoreError::PayloadDecode(format!("WASM decode call: {e}")))?;

        // Parse result
        let dtos: Vec<prb_plugin_api::DebugEventDto> = serde_json::from_str(&result_json)
            .map_err(|e| CoreError::PayloadDecode(format!("deserialize response: {e}")))?;

        // Convert DTOs to DebugEvents
        dtos.into_iter().map(convert_dto_to_event).collect()
    }
}

/// Protocol detector that uses WASM plugin.
pub struct WasmProtocolDetector {
    plugin_path: PathBuf,
    info: PluginMetadata,
    limits: WasmLimits,
}

impl WasmProtocolDetector {
    #[must_use]
    pub const fn new(plugin_path: PathBuf, info: PluginMetadata) -> Self {
        Self {
            plugin_path,
            info,
            limits: WasmLimits::for_detection(),
        }
    }
}

impl ProtocolDetector for WasmProtocolDetector {
    fn name(&self) -> &str {
        &self.info.name
    }

    fn transport(&self) -> TransportLayer {
        // TODO: Wire through from plugin metadata
        TransportLayer::Tcp
    }

    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        // Create a temporary plugin instance for detection
        let manifest = Manifest::new([Wasm::file(&self.plugin_path)])
            .with_memory_max(self.limits.memory_max_pages)
            .with_timeout(self.limits.timeout);

        let mut instance = match Plugin::new(&manifest, [], true) {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "Failed to create plugin instance for detection");
                return None;
            }
        };

        // Prepare detect context
        let detect_ctx = DetectContext {
            initial_bytes: ctx.initial_bytes.to_vec(),
            src_port: ctx.src_port,
            dst_port: ctx.dst_port,
            transport: match ctx.transport {
                TransportLayer::Tcp => prb_plugin_api::TransportLayer::Tcp,
                TransportLayer::Udp => prb_plugin_api::TransportLayer::Udp,
            },
        };

        let ctx_json = match serde_json::to_string(&detect_ctx) {
            Ok(j) => j,
            Err(e) => {
                warn!(error = %e, "Failed to serialize detect context");
                return None;
            }
        };

        // Call detect function
        let result_json = match instance.call::<&str, String>("prb_plugin_detect", &ctx_json) {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "WASM detect call failed");
                return None;
            }
        };

        // Parse confidence result
        let confidence: Option<f32> = match serde_json::from_str(&result_json) {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "Failed to parse detect result");
                return None;
            }
        };

        confidence.map(|c| DetectionResult {
            protocol: ProtocolId::new(&self.info.protocol_id),
            confidence: c,
            method: DetectionMethod::Heuristic,
            version: None,
        })
    }
}

/// Convert prb-core `DecodeContext` to plugin API `DecodeCtx`.
fn convert_decode_context(ctx: &DecodeContext) -> DecodeCtx {
    DecodeCtx {
        src_addr: ctx.src_addr.clone(),
        dst_addr: ctx.dst_addr.clone(),
        timestamp_nanos: ctx.timestamp.as_ref().map(prb_core::Timestamp::as_nanos),
        metadata: ctx
            .metadata
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    }
}

/// Convert `DebugEventDto` to `DebugEvent`.
fn convert_dto_to_event(dto: prb_plugin_api::DebugEventDto) -> Result<DebugEvent, CoreError> {
    use prb_core::{Direction, EventSource, NetworkAddr, Payload, Timestamp};

    let direction = match dto.direction.as_str() {
        "inbound" | "request" | "subscribe" => Direction::Inbound,
        "outbound" | "response" | "publish" => Direction::Outbound,
        _ => Direction::Unknown,
    };

    let transport = match dto.transport.as_str() {
        "grpc" | "http2" => TransportKind::Grpc,
        "zmtp" | "zmq" => TransportKind::Zmq,
        "rtps" | "dds-rtps" | "ddsrtps" => TransportKind::DdsRtps,
        "tcp" | "raw-tcp" => TransportKind::RawTcp,
        "udp" | "raw-udp" => TransportKind::RawUdp,
        _ => TransportKind::RawTcp,
    };

    // Create event source
    let source = EventSource {
        adapter: "wasm-plugin".to_string(),
        origin: dto
            .src_addr
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        network: match (dto.src_addr.clone(), dto.dst_addr.clone()) {
            (Some(src), Some(dst)) => Some(NetworkAddr { src, dst }),
            _ => None,
        },
    };

    let mut builder = DebugEvent::builder()
        .timestamp(Timestamp::from_nanos(dto.timestamp_nanos))
        .transport(transport)
        .direction(direction)
        .source(source);

    // Set payload
    if let Some(raw) = dto.payload_raw {
        builder = builder.payload(Payload::Raw {
            raw: Bytes::from(raw),
        });
    } else if let Some(decoded) = dto.payload_decoded {
        builder = builder.payload(Payload::Decoded {
            raw: Bytes::new(),
            fields: decoded,
            schema_name: dto.schema_name,
        });
    } else {
        // Default to empty raw payload if neither is present
        builder = builder.payload(Payload::Raw { raw: Bytes::new() });
    }

    // Add metadata
    for (k, v) in dto.metadata {
        builder = builder.metadata(k, v);
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
    use prb_core::{DecodeContext, Timestamp};
    use prb_plugin_api::DebugEventDto;
    use std::collections::{BTreeMap, HashMap};

    #[test]
    fn test_convert_decode_context_full() {
        let mut metadata = BTreeMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());

        let ctx = DecodeContext {
            src_addr: Some("192.168.1.1:8080".to_string()),
            dst_addr: Some("192.168.1.2:9090".to_string()),
            timestamp: Some(Timestamp::from_nanos(1234567890)),
            metadata,
        };

        let dto = convert_decode_context(&ctx);

        assert_eq!(dto.src_addr, Some("192.168.1.1:8080".to_string()));
        assert_eq!(dto.dst_addr, Some("192.168.1.2:9090".to_string()));
        assert_eq!(dto.timestamp_nanos, Some(1234567890));
        assert_eq!(dto.metadata.len(), 2);
        assert_eq!(dto.metadata.get("key1").unwrap(), "value1");
        assert_eq!(dto.metadata.get("key2").unwrap(), "value2");
    }

    #[test]
    fn test_convert_decode_context_minimal() {
        let ctx = DecodeContext {
            src_addr: None,
            dst_addr: None,
            timestamp: None,
            metadata: BTreeMap::new(),
        };

        let dto = convert_decode_context(&ctx);

        assert!(dto.src_addr.is_none());
        assert!(dto.dst_addr.is_none());
        assert!(dto.timestamp_nanos.is_none());
        assert!(dto.metadata.is_empty());
    }

    #[test]
    fn test_convert_decode_context_partial_addresses() {
        let ctx = DecodeContext {
            src_addr: Some("127.0.0.1:8080".to_string()),
            dst_addr: None,
            timestamp: None,
            metadata: BTreeMap::new(),
        };

        let dto = convert_decode_context(&ctx);

        assert_eq!(dto.src_addr, Some("127.0.0.1:8080".to_string()));
        assert!(dto.dst_addr.is_none());
    }

    #[test]
    fn test_convert_dto_to_event_minimal() {
        let dto = DebugEventDto::minimal("grpc", "request");
        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.transport, TransportKind::Grpc);
        assert_eq!(event.direction, prb_core::Direction::Inbound);
        assert_eq!(event.source.adapter, "wasm-plugin");
    }

    #[test]
    fn test_convert_dto_to_event_all_transports() {
        let transports = vec![
            ("grpc", TransportKind::Grpc),
            ("http2", TransportKind::Grpc),
            ("zmtp", TransportKind::Zmq),
            ("zmq", TransportKind::Zmq),
            ("rtps", TransportKind::DdsRtps),
            ("dds-rtps", TransportKind::DdsRtps),
            ("ddsrtps", TransportKind::DdsRtps),
            ("tcp", TransportKind::RawTcp),
            ("raw-tcp", TransportKind::RawTcp),
            ("udp", TransportKind::RawUdp),
            ("raw-udp", TransportKind::RawUdp),
            ("unknown", TransportKind::RawTcp), // Default fallback
        ];

        for (transport_str, expected_kind) in transports {
            let dto = DebugEventDto::minimal(transport_str, "request");
            let event = convert_dto_to_event(dto).expect("conversion should succeed");
            assert_eq!(
                event.transport, expected_kind,
                "failed for transport: {transport_str}"
            );
        }
    }

    #[test]
    fn test_convert_dto_to_event_all_directions() {
        let directions = vec![
            ("inbound", prb_core::Direction::Inbound),
            ("request", prb_core::Direction::Inbound),
            ("subscribe", prb_core::Direction::Inbound),
            ("outbound", prb_core::Direction::Outbound),
            ("response", prb_core::Direction::Outbound),
            ("publish", prb_core::Direction::Outbound),
            ("unknown", prb_core::Direction::Unknown),
            ("invalid", prb_core::Direction::Unknown),
        ];

        for (direction_str, expected_dir) in directions {
            let dto = DebugEventDto::minimal("tcp", direction_str);
            let event = convert_dto_to_event(dto).expect("conversion should succeed");
            assert_eq!(
                event.direction, expected_dir,
                "failed for direction: {direction_str}"
            );
        }
    }

    #[test]
    fn test_convert_dto_to_event_with_raw_payload() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.payload_raw = Some(vec![1, 2, 3, 4, 5]);

        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        match event.payload {
            prb_core::Payload::Raw { raw } => {
                assert_eq!(raw.as_ref(), &[1, 2, 3, 4, 5]);
            }
            _ => panic!("expected raw payload"),
        }
    }

    #[test]
    fn test_convert_dto_to_event_with_decoded_payload() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.payload_decoded = Some(serde_json::json!({
            "message": "test",
            "count": 42
        }));
        dto.schema_name = Some("TestMessage".to_string());

        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        match event.payload {
            prb_core::Payload::Decoded {
                raw,
                fields,
                schema_name,
            } => {
                assert!(raw.is_empty());
                assert_eq!(fields["message"], "test");
                assert_eq!(fields["count"], 42);
                assert_eq!(schema_name, Some("TestMessage".to_string()));
            }
            _ => panic!("expected decoded payload"),
        }
    }

    #[test]
    fn test_convert_dto_to_event_with_metadata() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        let mut metadata = HashMap::new();
        metadata.insert("key1".to_string(), "value1".to_string());
        metadata.insert("key2".to_string(), "value2".to_string());
        dto.metadata = metadata;

        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.metadata.len(), 2);
        assert_eq!(event.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(event.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_convert_dto_to_event_with_warnings() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.warnings = vec![
            "warning 1".to_string(),
            "warning 2".to_string(),
            "warning 3".to_string(),
        ];

        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.warnings.len(), 3);
        assert!(event.warnings.contains(&"warning 1".to_string()));
        assert!(event.warnings.contains(&"warning 3".to_string()));
    }

    #[test]
    fn test_convert_dto_to_event_with_full_addresses() {
        let mut dto = DebugEventDto::minimal("tcp", "request");
        dto.src_addr = Some("192.168.1.1:8080".to_string());
        dto.dst_addr = Some("192.168.1.2:9090".to_string());

        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.origin, "192.168.1.1:8080");
        assert!(event.source.network.is_some());

        let network = event.source.network.unwrap();
        assert_eq!(network.src, "192.168.1.1:8080");
        assert_eq!(network.dst, "192.168.1.2:9090");
    }

    #[test]
    fn test_convert_dto_to_event_with_partial_addresses() {
        let mut dto = DebugEventDto::minimal("tcp", "request");
        dto.src_addr = Some("192.168.1.1:8080".to_string());

        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.origin, "192.168.1.1:8080");
        assert!(event.source.network.is_none());
    }

    #[test]
    fn test_convert_dto_to_event_no_addresses() {
        let dto = DebugEventDto::minimal("tcp", "request");
        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.origin, "unknown");
        assert!(event.source.network.is_none());
    }

    #[test]
    fn test_convert_dto_to_event_with_timestamp() {
        let mut dto = DebugEventDto::minimal("grpc", "request");
        dto.timestamp_nanos = 9876543210;

        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.timestamp.as_nanos(), 9876543210);
    }

    #[test]
    fn test_convert_dto_to_event_source_adapter_name() {
        let dto = DebugEventDto::minimal("grpc", "request");
        let event = convert_dto_to_event(dto).expect("conversion should succeed");

        assert_eq!(event.source.adapter, "wasm-plugin");
    }
}
