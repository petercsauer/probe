//! Core types and traits for the PRB universal message debugger.
//!
//! This crate provides the foundational data model, error types, and traits
//! used throughout the PRB ecosystem.

pub mod error;
pub mod event;
pub mod traits;

#[cfg(test)]
mod event_tests;

pub use error::CoreError;
pub use event::{
    CorrelationKey, DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload,
    Timestamp, TransportKind,
};
pub use traits::{
    CaptureAdapter, CorrelationStrategy, DecodeContext, EventNormalizer, Flow, ProtocolDecoder,
    ResolvedSchema, SchemaResolver,
};

// Re-export metadata key constants
pub use event::{
    METADATA_KEY_DDS_DOMAIN_ID, METADATA_KEY_DDS_TOPIC_NAME, METADATA_KEY_GRPC_METHOD,
    METADATA_KEY_H2_STREAM_ID, METADATA_KEY_ZMQ_TOPIC,
};
