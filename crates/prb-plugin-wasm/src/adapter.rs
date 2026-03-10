//! Adapters for WASM plugins to implement prb-detect and prb-core traits.

use crate::runtime::WasmLimits;
use bytes::Bytes;
use extism::{Manifest, Plugin, Wasm};
use prb_core::{CoreError, DebugEvent, DecodeContext, ProtocolDecoder, TransportKind};
use prb_detect::{
    DetectionContext, DetectionMethod, DetectionResult, DecoderFactory, ProtocolDetector,
    ProtocolId, TransportLayer,
};
use prb_plugin_api::{DecodeCtx, DetectContext, PluginMetadata};
use prb_plugin_api::types::WasmDecodeRequest;
use std::path::PathBuf;
use tracing::warn;

/// Factory for creating WASM decoder instances.
///
/// Each call to create_decoder() instantiates a new WASM plugin instance,
/// as WASM instances are not thread-safe or reentrant.
pub struct WasmDecoderFactory {
    plugin_path: PathBuf,
    info: PluginMetadata,
    limits: WasmLimits,
}

impl WasmDecoderFactory {
    pub fn new(plugin_path: PathBuf, info: PluginMetadata, limits: WasmLimits) -> Self {
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

        let instance = Plugin::new(&manifest, [], true)
            .expect("plugin already validated during load");

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
            .map_err(|e| CoreError::PayloadDecode(format!("serialize request: {}", e)))?;

        // Call WASM decode function
        let result_json = self
            .instance
            .call::<&str, String>("prb_plugin_decode", &request_json)
            .map_err(|e| CoreError::PayloadDecode(format!("WASM decode call: {}", e)))?;

        // Parse result
        let dtos: Vec<prb_plugin_api::DebugEventDto> = serde_json::from_str(&result_json)
            .map_err(|e| CoreError::PayloadDecode(format!("deserialize response: {}", e)))?;

        // Convert DTOs to DebugEvents
        dtos.into_iter()
            .map(convert_dto_to_event)
            .collect()
    }
}

/// Protocol detector that uses WASM plugin.
pub struct WasmProtocolDetector {
    plugin_path: PathBuf,
    info: PluginMetadata,
    limits: WasmLimits,
}

impl WasmProtocolDetector {
    pub fn new(plugin_path: PathBuf, info: PluginMetadata) -> Self {
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

/// Convert prb-core DecodeContext to plugin API DecodeCtx.
fn convert_decode_context(ctx: &DecodeContext) -> DecodeCtx {
    DecodeCtx {
        src_addr: ctx.src_addr.clone(),
        dst_addr: ctx.dst_addr.clone(),
        timestamp_nanos: ctx.timestamp.as_ref().map(|ts| ts.as_nanos()),
        metadata: ctx.metadata.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
    }
}

/// Convert DebugEventDto to DebugEvent.
fn convert_dto_to_event(
    dto: prb_plugin_api::DebugEventDto,
) -> Result<DebugEvent, CoreError> {
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
        origin: dto.src_addr.clone().unwrap_or_else(|| "unknown".to_string()),
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
