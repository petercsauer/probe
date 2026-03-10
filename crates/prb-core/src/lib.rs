//! Core types and traits for the PRB universal message debugger.
//!
//! This crate provides the foundational data model, error types, and traits
//! used throughout the PRB ecosystem.

pub mod conversation;
pub mod decode;
pub mod engine;
pub mod error;
pub mod event;
pub mod flow;
pub mod metrics;
pub mod schema;
pub mod trace;
pub mod traits;

#[cfg(test)]
mod event_tests;

pub use conversation::{
    Conversation, ConversationError, ConversationId, ConversationKind, ConversationMetrics,
    ConversationState,
};
pub use engine::{ConversationEngine, ConversationSet, ConversationStats};
pub use error::CoreError;
pub use event::{
    CorrelationKey, DebugEvent, DebugEventBuilder, Direction, EventId, EventSource, NetworkAddr,
    Payload, Timestamp, TransportKind,
};
pub use metrics::{compute_aggregate_metrics, compute_metrics, AggregateMetrics};
pub use trace::{
    extract_trace_context, parse_b3_multi, parse_b3_single, parse_uber_trace_id,
    parse_w3c_traceparent, TraceContext,
};
pub use traits::{
    CaptureAdapter, CorrelationStrategy, DecodeContext, EventNormalizer, Flow, ProtocolDecoder,
    ResolvedSchema, SchemaResolver,
};

// Re-export metadata key constants
pub use event::{
    METADATA_KEY_DDS_DOMAIN_ID, METADATA_KEY_DDS_TOPIC_NAME, METADATA_KEY_GRPC_METHOD,
    METADATA_KEY_H2_STREAM_ID, METADATA_KEY_OTEL_PARENT_SPAN_ID, METADATA_KEY_OTEL_SPAN_ID,
    METADATA_KEY_OTEL_TRACE_FLAGS, METADATA_KEY_OTEL_TRACE_ID, METADATA_KEY_OTEL_TRACE_SAMPLED,
    METADATA_KEY_ZMQ_TOPIC,
};
