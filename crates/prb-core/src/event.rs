//! Core event types for the universal message debugger.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// Well-known metadata key for gRPC method name.
pub const METADATA_KEY_GRPC_METHOD: &str = "grpc.method";

/// Well-known metadata key for HTTP/2 stream ID.
pub const METADATA_KEY_H2_STREAM_ID: &str = "h2.stream_id";

/// Well-known metadata key for ZeroMQ topic.
pub const METADATA_KEY_ZMQ_TOPIC: &str = "zmq.topic";

/// Well-known metadata key for DDS domain ID.
pub const METADATA_KEY_DDS_DOMAIN_ID: &str = "dds.domain_id";

/// Well-known metadata key for DDS topic name.
pub const METADATA_KEY_DDS_TOPIC_NAME: &str = "dds.topic_name";

/// Well-known metadata key for OpenTelemetry trace ID.
pub const METADATA_KEY_OTEL_TRACE_ID: &str = "otel.trace_id";

/// Well-known metadata key for OpenTelemetry span ID.
pub const METADATA_KEY_OTEL_SPAN_ID: &str = "otel.span_id";

/// Well-known metadata key for OpenTelemetry trace flags.
pub const METADATA_KEY_OTEL_TRACE_FLAGS: &str = "otel.trace_flags";

/// Well-known metadata key for OpenTelemetry parent span ID.
pub const METADATA_KEY_OTEL_PARENT_SPAN_ID: &str = "otel.parent_span_id";

/// Well-known metadata key for OpenTelemetry trace sampled flag.
pub const METADATA_KEY_OTEL_TRACE_SAMPLED: &str = "otel.trace_sampled";

/// Monotonic event identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(u64);

static NEXT_EVENT_ID: AtomicU64 = AtomicU64::new(1);

impl EventId {
    /// Generate the next monotonic event ID.
    pub fn next() -> Self {
        Self(NEXT_EVENT_ID.fetch_add(1, Ordering::SeqCst))
    }

    /// Create an event ID from a raw u64 (for testing/deserialization).
    pub fn from_raw(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw u64 value.
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Nanosecond-precision timestamp since Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Create a timestamp from nanoseconds since Unix epoch.
    pub fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// Get the timestamp as nanoseconds since Unix epoch.
    pub fn as_nanos(&self) -> u64 {
        self.0
    }

    /// Get the current system time as a Timestamp.
    pub fn now() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before Unix epoch");
        Self(now.as_nanos() as u64)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ns", self.0)
    }
}

/// Network address information.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NetworkAddr {
    /// Source IP address and port.
    pub src: String,
    /// Destination IP address and port.
    pub dst: String,
}

/// Event source information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSource {
    /// Adapter type (e.g., "pcap", "json-fixture").
    pub adapter: String,
    /// Origin identifier (e.g., file path, device name).
    pub origin: String,
    /// Optional network address information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkAddr>,
}

impl fmt::Display for EventSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref network) = self.network {
            write!(f, "{}", network.src)
        } else {
            write!(f, "{}", self.origin)
        }
    }
}

/// Transport protocol kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TransportKind {
    /// gRPC over HTTP/2.
    Grpc,
    /// ZeroMQ (ZMTP protocol).
    Zmq,
    /// DDS RTPS protocol.
    DdsRtps,
    /// Raw TCP stream.
    RawTcp,
    /// Raw UDP datagram.
    RawUdp,
    /// JSON fixture input.
    JsonFixture,
}

impl fmt::Display for TransportKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Grpc => write!(f, "gRPC"),
            Self::Zmq => write!(f, "ZMQ"),
            Self::DdsRtps => write!(f, "DDS-RTPS"),
            Self::RawTcp => write!(f, "TCP"),
            Self::RawUdp => write!(f, "UDP"),
            Self::JsonFixture => write!(f, "JSON-Fixture"),
        }
    }
}

impl std::str::FromStr for TransportKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "grpc" => Ok(Self::Grpc),
            "zmq" => Ok(Self::Zmq),
            "dds-rtps" | "ddsrtps" => Ok(Self::DdsRtps),
            "raw-tcp" | "rawtcp" | "tcp" => Ok(Self::RawTcp),
            "raw-udp" | "rawudp" | "udp" => Ok(Self::RawUdp),
            "json-fixture" | "jsonfixture" => Ok(Self::JsonFixture),
            _ => Err(format!("Unknown transport kind: {}", s)),
        }
    }
}

/// Message direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Inbound message (received).
    Inbound,
    /// Outbound message (sent).
    Outbound,
    /// Direction unknown or not applicable.
    Unknown,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inbound => write!(f, "inbound"),
            Self::Outbound => write!(f, "outbound"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Message payload, either raw bytes or decoded with schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Payload {
    /// Raw bytes, not decoded.
    Raw {
        /// The raw payload bytes.
        #[serde(with = "serde_bytes_base64")]
        raw: Bytes,
    },
    /// Decoded payload with structured fields.
    Decoded {
        /// Original raw bytes.
        #[serde(with = "serde_bytes_base64")]
        raw: Bytes,
        /// Decoded fields as JSON.
        fields: serde_json::Value,
        /// Optional schema name.
        #[serde(skip_serializing_if = "Option::is_none")]
        schema_name: Option<String>,
    },
}

/// Serde module for base64-encoding Bytes.
mod serde_bytes_base64 {
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        serializer.serialize_str(&encoded)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        use base64::Engine;
        let s = String::deserialize(deserializer)?;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&s)
            .map_err(serde::de::Error::custom)?;
        Ok(Bytes::from(decoded))
    }
}

/// Correlation key for linking related events.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CorrelationKey {
    /// Stream ID (e.g., HTTP/2 stream).
    StreamId {
        /// The stream identifier.
        id: u32
    },
    /// Topic name (e.g., ZMQ, DDS).
    Topic {
        /// The topic name.
        name: String
    },
    /// Connection identifier.
    ConnectionId {
        /// The connection identifier string.
        id: String
    },
    /// OpenTelemetry trace context.
    TraceContext {
        /// The trace ID (32 hex characters).
        trace_id: String,
        /// The span ID (16 hex characters).
        span_id: String
    },
    /// Custom key-value pair.
    Custom {
        /// The custom key.
        key: String,
        /// The custom value.
        value: String
    },
}

/// Main debug event structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugEvent {
    /// Unique event identifier.
    pub id: EventId,
    /// Event timestamp.
    pub timestamp: Timestamp,
    /// Event source information.
    pub source: EventSource,
    /// Transport protocol.
    pub transport: TransportKind,
    /// Message direction.
    pub direction: Direction,
    /// Message payload.
    pub payload: Payload,
    /// Protocol-specific metadata.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    /// Correlation keys for linking events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub correlation_keys: Vec<CorrelationKey>,
    /// Optional sequence number within a stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    /// Parse warnings (per no-ignore-failure rule).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl DebugEvent {
    /// Create a new debug event builder.
    pub fn builder() -> DebugEventBuilder {
        DebugEventBuilder::default()
    }
}

/// Builder for DebugEvent.
#[derive(Default)]
pub struct DebugEventBuilder {
    id: Option<EventId>,
    timestamp: Option<Timestamp>,
    source: Option<EventSource>,
    transport: Option<TransportKind>,
    direction: Option<Direction>,
    payload: Option<Payload>,
    metadata: BTreeMap<String, String>,
    correlation_keys: Vec<CorrelationKey>,
    sequence: Option<u64>,
    warnings: Vec<String>,
}

impl DebugEventBuilder {
    /// Set the event ID (defaults to auto-generated).
    pub fn id(mut self, id: EventId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the timestamp (defaults to current time).
    pub fn timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Set the event source.
    pub fn source(mut self, source: EventSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Set the transport kind.
    pub fn transport(mut self, transport: TransportKind) -> Self {
        self.transport = Some(transport);
        self
    }

    /// Set the direction.
    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = Some(direction);
        self
    }

    /// Set the payload.
    pub fn payload(mut self, payload: Payload) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Add a metadata entry.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Add a correlation key.
    pub fn correlation_key(mut self, key: CorrelationKey) -> Self {
        self.correlation_keys.push(key);
        self
    }

    /// Set the sequence number.
    pub fn sequence(mut self, seq: u64) -> Self {
        self.sequence = Some(seq);
        self
    }

    /// Add a warning.
    pub fn warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Build the DebugEvent.
    pub fn build(self) -> DebugEvent {
        DebugEvent {
            id: self.id.unwrap_or_else(EventId::next),
            timestamp: self.timestamp.unwrap_or_else(Timestamp::now),
            source: self.source.expect("source is required"),
            transport: self.transport.expect("transport is required"),
            direction: self.direction.expect("direction is required"),
            payload: self.payload.expect("payload is required"),
            metadata: self.metadata,
            correlation_keys: self.correlation_keys,
            sequence: self.sequence,
            warnings: self.warnings,
        }
    }
}
