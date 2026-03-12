//! JSON fixture file format definitions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Root fixture file structure.
#[derive(Debug, Deserialize, Serialize)]
pub struct FixtureFile {
    /// Fixture format version (must be 1).
    pub version: u64,
    /// Optional description of the fixture.
    #[serde(default)]
    pub description: Option<String>,
    /// List of events in the fixture.
    pub events: Vec<FixtureEvent>,
}

/// A single event in a fixture file.
#[derive(Debug, Deserialize, Serialize)]
pub struct FixtureEvent {
    /// Timestamp in nanoseconds since Unix epoch.
    pub timestamp_ns: u64,
    /// Transport protocol (e.g., "grpc", "zmq", "dds-rtps").
    pub transport: String,
    /// Message direction ("inbound", "outbound", "unknown").
    #[serde(default = "default_direction")]
    pub direction: String,
    /// Base64-encoded binary payload (mutually exclusive with `payload_utf8`).
    pub payload_base64: Option<String>,
    /// UTF-8 text payload (mutually exclusive with `payload_base64`).
    pub payload_utf8: Option<String>,
    /// Protocol-specific metadata.
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    /// Optional source information.
    #[serde(default)]
    pub source: Option<FixtureSource>,
}

/// Source information for a fixture event.
#[derive(Debug, Deserialize, Serialize)]
pub struct FixtureSource {
    /// Source network address (e.g., "192.168.1.1:8080").
    pub src: Option<String>,
    /// Destination network address.
    pub dst: Option<String>,
}

fn default_direction() -> String {
    "unknown".to_string()
}
