//! Common types for plugin API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin metadata returned by plugin info functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name.
    pub name: String,
    /// Plugin version (semver).
    pub version: String,
    /// Plugin description.
    pub description: String,
    /// Protocol identifier this plugin handles.
    pub protocol_id: String,
    /// API version this plugin was compiled against.
    #[serde(default = "default_api_version")]
    pub api_version: String,
}

fn default_api_version() -> String {
    crate::API_VERSION.to_string()
}

/// Transport layer (TCP or UDP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportLayer {
    /// TCP transport.
    Tcp,
    /// UDP transport.
    Udp,
}

/// Detection context passed to plugin detect functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectContext {
    /// First bytes of the stream (base64-encoded for WASM).
    #[cfg(feature = "wasm-pdk")]
    pub initial_bytes_b64: String,
    /// First bytes of the stream (raw for native).
    #[cfg(not(feature = "wasm-pdk"))]
    #[serde(skip)]
    pub initial_bytes: Vec<u8>,
    /// Source port.
    pub src_port: u16,
    /// Destination port.
    pub dst_port: u16,
    /// Transport layer.
    pub transport: TransportLayer,
}

#[cfg(feature = "wasm-pdk")]
impl DetectContext {
    /// Decode initial bytes from base64 (WASM plugins).
    pub fn initial_bytes(&self) -> Vec<u8> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(&self.initial_bytes_b64)
            .unwrap_or_default()
    }
}

/// Decode context passed to plugin decode functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodeCtx {
    /// Source address (e.g., "192.168.1.1:8080").
    pub src_addr: Option<String>,
    /// Destination address.
    pub dst_addr: Option<String>,
    /// Timestamp in nanoseconds since UNIX epoch.
    pub timestamp_nanos: Option<u64>,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
}

/// Request sent to WASM decode function (combines data + context).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmDecodeRequest {
    /// Raw stream data, base64-encoded.
    pub data_b64: String,
    /// Decode context.
    pub ctx: DecodeCtx,
}
