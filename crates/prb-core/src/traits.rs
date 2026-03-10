//! Extension traits for the PRB universal message debugger.
//!
//! All traits are synchronous (Phase 1 is offline-only analysis).

use crate::{CoreError, DebugEvent, TransportKind};

pub use crate::decode::DecodeContext;
pub use crate::flow::Flow;
pub use crate::schema::ResolvedSchema;

/// Reads from a capture source and produces DebugEvents.
///
/// Implemented by:
/// - JsonFixtureAdapter (Subsection 1)
/// - PcapAdapter (Subsection 3)
pub trait CaptureAdapter {
    /// Returns the adapter name (e.g., "json-fixture", "pcap").
    fn name(&self) -> &str;

    /// Produces an iterator of DebugEvents from the capture source.
    ///
    /// The iterator yields events in order and may produce errors during iteration.
    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_>;
}

/// Decodes protocol-specific byte sequences into structured events.
///
/// Implemented by:
/// - GrpcDecoder (Subsection 4)
/// - ZmqDecoder (Subsection 4)
/// - DdsDecoder (Subsection 4)
pub trait ProtocolDecoder: Send {
    /// Returns the transport protocol this decoder handles.
    fn protocol(&self) -> TransportKind;

    /// Decodes a byte stream into zero or more DebugEvents.
    ///
    /// # Arguments
    /// * `data` - The raw byte sequence to decode
    /// * `ctx` - Decoding context with metadata
    ///
    /// # Returns
    /// A vector of decoded events. May be empty if the data is incomplete or not yet decodable.
    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError>;
}

/// Resolves message schemas for payload decoding.
///
/// Implemented by:
/// - ProtobufSchemaResolver (Subsection 2)
pub trait SchemaResolver {
    /// Resolves a schema by name.
    ///
    /// Returns `Ok(Some(schema))` if found, `Ok(None)` if not found, or an error if resolution fails.
    fn resolve(&self, schema_name: &str) -> Result<Option<ResolvedSchema>, CoreError>;

    /// Lists all available schema names.
    fn list_schemas(&self) -> Vec<String>;
}

/// Normalizes events from adapter-specific format to canonical DebugEvent.
///
/// Implemented by per-adapter normalizers as needed.
pub trait EventNormalizer {
    /// Normalizes a batch of events.
    ///
    /// This can perform transformations like deduplication, timestamp adjustment,
    /// or metadata enrichment.
    fn normalize(&self, events: Vec<DebugEvent>) -> Result<Vec<DebugEvent>, CoreError>;
}

/// Groups related events into correlation flows.
///
/// Implemented by per-protocol strategies (Subsection 5).
pub trait CorrelationStrategy {
    /// Returns the transport protocol this strategy handles.
    fn transport(&self) -> TransportKind;

    /// Correlates a slice of events into flows.
    ///
    /// Returns a vector of flows, each containing references to related events.
    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError>;
}
