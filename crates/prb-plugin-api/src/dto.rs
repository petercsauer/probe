//! Data Transfer Objects for crossing the FFI/WASM boundary.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serializable correlation key DTO.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationKeyDto {
    /// The kind of correlation key (e.g., "stream-id", "topic").
    pub kind: String,
    /// The correlation value.
    pub value: String,
}

/// Serializable event DTO (mirrors prb-core `DebugEvent`).
///
/// Used for transferring events across FFI/WASM boundaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugEventDto {
    /// Timestamp in nanoseconds since UNIX epoch.
    pub timestamp_nanos: u64,
    /// Transport protocol (e.g., "grpc", "zmtp", "rtps").
    pub transport: String,
    /// Direction ("request", "response", "publish", "subscribe").
    pub direction: String,
    /// Raw payload bytes (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_raw: Option<Vec<u8>>,
    /// Decoded payload as JSON (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload_decoded: Option<serde_json::Value>,
    /// Schema name used for decoding (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_name: Option<String>,
    /// Event metadata.
    pub metadata: HashMap<String, String>,
    /// Correlation keys.
    pub correlation_keys: Vec<CorrelationKeyDto>,
    /// Warnings encountered during decoding.
    pub warnings: Vec<String>,
    /// Source address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_addr: Option<String>,
    /// Destination address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dst_addr: Option<String>,
}

impl DebugEventDto {
    /// Create a minimal DTO for testing.
    #[must_use]
    pub fn minimal(transport: &str, direction: &str) -> Self {
        Self {
            timestamp_nanos: 0,
            transport: transport.to_string(),
            direction: direction.to_string(),
            payload_raw: None,
            payload_decoded: None,
            schema_name: None,
            metadata: HashMap::new(),
            correlation_keys: Vec::new(),
            warnings: Vec::new(),
            src_addr: None,
            dst_addr: None,
        }
    }
}
