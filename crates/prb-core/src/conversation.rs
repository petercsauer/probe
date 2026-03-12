//! Conversation reconstruction types.
//!
//! Groups related events into logical conversations with timing metrics,
//! error classification, and state tracking.

use crate::{EventId, Timestamp, TransportKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unique conversation identifier.
///
/// Format: "{`protocol}:{grouping_key`}" e.g. "grpc:10.0.0.1:50051->10.0.0.2:8080/s3"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConversationId(pub String);

impl ConversationId {
    /// Create a new conversation ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the string value.
    #[must_use] 
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The kind of conversation, protocol-dependent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversationKind {
    /// Single request, single response (gRPC unary).
    UnaryRpc,
    /// Single request, streaming responses (gRPC server-streaming).
    ServerStreaming,
    /// Streaming requests, single response (gRPC client-streaming).
    ClientStreaming,
    /// Bidirectional streaming (gRPC bidi).
    BidirectionalStreaming,
    /// ZMQ REQ/REP paired exchange.
    RequestReply,
    /// ZMQ PUB/SUB topic channel.
    PubSubChannel,
    /// ZMQ PUSH/PULL one-directional pipeline.
    Pipeline,
    /// DDS writer→reader(s) topic exchange.
    TopicExchange,
    /// Raw TCP connection (when protocol isn't decoded).
    TcpStream,
    /// Fallback for unknown patterns.
    Unknown,
}

impl std::fmt::Display for ConversationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnaryRpc => write!(f, "unary-rpc"),
            Self::ServerStreaming => write!(f, "server-streaming"),
            Self::ClientStreaming => write!(f, "client-streaming"),
            Self::BidirectionalStreaming => write!(f, "bidirectional-streaming"),
            Self::RequestReply => write!(f, "request-reply"),
            Self::PubSubChannel => write!(f, "pub-sub"),
            Self::Pipeline => write!(f, "pipeline"),
            Self::TopicExchange => write!(f, "topic-exchange"),
            Self::TcpStream => write!(f, "tcp-stream"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Lifecycle state of a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConversationState {
    /// Conversation is ongoing (e.g., streaming).
    Active,
    /// Completed successfully (response received, status OK).
    Complete,
    /// Completed with error (gRPC error status, `RST_STREAM`, etc.).
    Error,
    /// No response within expected time / RST without response.
    Timeout,
    /// Incomplete capture (e.g., mid-stream join).
    Incomplete,
}

impl std::fmt::Display for ConversationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Complete => write!(f, "complete"),
            Self::Error => write!(f, "error"),
            Self::Timeout => write!(f, "timeout"),
            Self::Incomplete => write!(f, "incomplete"),
        }
    }
}

/// Error classification for a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationError {
    /// Error kind (e.g., "grpc-status", "rst-stream", "timeout").
    pub kind: String,
    /// Error code (e.g., gRPC status code "14").
    pub code: Option<String>,
    /// Human-readable error message.
    pub message: String,
}

impl ConversationError {
    /// Create a new conversation error.
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            code: None,
            message: message.into(),
        }
    }

    /// Set the error code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
}

/// Timing and size metrics for a conversation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConversationMetrics {
    /// Timestamp of the first event.
    pub start_time: Option<Timestamp>,
    /// Timestamp of the last event.
    pub end_time: Option<Timestamp>,
    /// Wall-clock duration (end - start).
    pub duration_ns: u64,
    /// Time from first outbound event to first inbound event.
    pub time_to_first_response_ns: Option<u64>,
    /// Number of outbound (request) messages.
    pub request_count: usize,
    /// Number of inbound (response) messages.
    pub response_count: usize,
    /// Total payload bytes across all events.
    pub total_bytes: u64,
    /// Error detail, if conversation ended in error.
    pub error: Option<ConversationError>,
}

/// A reconstructed conversation grouping related events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// Unique conversation identifier.
    pub id: ConversationId,
    /// The kind of conversation.
    pub kind: ConversationKind,
    /// Protocol of the conversation.
    pub protocol: TransportKind,
    /// Current lifecycle state.
    pub state: ConversationState,
    /// Ordered event IDs belonging to this conversation.
    pub event_ids: Vec<EventId>,
    /// Computed timing and size metrics.
    pub metrics: ConversationMetrics,
    /// Conversation-level metadata (method, topic, etc.).
    pub metadata: BTreeMap<String, String>,
    /// Human-readable summary line.
    pub summary: String,
}

impl Conversation {
    /// Create a new conversation.
    #[must_use] 
    pub fn new(
        id: ConversationId,
        kind: ConversationKind,
        protocol: TransportKind,
        state: ConversationState,
    ) -> Self {
        Self {
            id,
            kind,
            protocol,
            state,
            event_ids: Vec::new(),
            metrics: ConversationMetrics::default(),
            metadata: BTreeMap::new(),
            summary: String::new(),
        }
    }

    /// Add an event to the conversation.
    pub fn add_event(&mut self, event_id: EventId) {
        self.event_ids.push(event_id);
    }

    /// Add metadata to the conversation.
    pub fn add_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    /// Set the summary.
    pub fn set_summary(&mut self, summary: impl Into<String>) {
        self.summary = summary.into();
    }

    /// Set the metrics.
    pub fn set_metrics(&mut self, metrics: ConversationMetrics) {
        self.metrics = metrics;
    }
}
