//! Extension traits for the PRB universal message debugger.
//!
//! All traits are synchronous (Phase 1 is offline-only analysis).

use crate::{CoreError, DebugEvent, TransportKind};

pub use crate::decode::DecodeContext;
pub use crate::flow::Flow;
pub use crate::schema::ResolvedSchema;

/// Reads from a capture source and produces `DebugEvents`.
///
/// Implemented by:
/// - `JsonFixtureAdapter` (Subsection 1)
/// - `PcapAdapter` (Subsection 3)
///
/// # Examples
///
/// Implementing a simple adapter:
///
/// ```
/// use prb_core::{CaptureAdapter, DebugEvent, CoreError, EventSource, TransportKind, Direction, Payload};
/// use bytes::Bytes;
///
/// struct TestAdapter {
///     events: Vec<DebugEvent>,
/// }
///
/// impl CaptureAdapter for TestAdapter {
///     fn name(&self) -> &str {
///         "test-adapter"
///     }
///
///     fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_> {
///         Box::new(self.events.drain(..).map(Ok))
///     }
/// }
///
/// let mut adapter = TestAdapter {
///     events: vec![
///         DebugEvent::builder()
///             .source(EventSource {
///                 adapter: "test".to_string(),
///                 origin: "test".to_string(),
///                 network: None,
///             })
///             .transport(TransportKind::Grpc)
///             .direction(Direction::Outbound)
///             .payload(Payload::Raw { raw: Bytes::new() })
///             .build(),
///     ],
/// };
///
/// assert_eq!(adapter.name(), "test-adapter");
/// let events: Vec<_> = adapter.ingest().collect();
/// assert_eq!(events.len(), 1);
/// ```
pub trait CaptureAdapter {
    /// Returns the adapter name (e.g., "json-fixture", "pcap").
    fn name(&self) -> &str;

    /// Produces an iterator of `DebugEvents` from the capture source.
    ///
    /// The iterator yields events in order and may produce errors during iteration.
    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_>;
}

/// Decodes protocol-specific byte sequences into structured events.
///
/// Implemented by:
/// - `GrpcDecoder` (Subsection 4)
/// - `ZmqDecoder` (Subsection 4)
/// - `DdsDecoder` (Subsection 4)
///
/// # Examples
///
/// Implementing a simple decoder:
///
/// ```
/// use prb_core::{ProtocolDecoder, DebugEvent, CoreError, TransportKind, DecodeContext};
/// use prb_core::{EventSource, Direction, Payload};
/// use bytes::Bytes;
///
/// struct TestDecoder;
///
/// impl ProtocolDecoder for TestDecoder {
///     fn protocol(&self) -> TransportKind {
///         TransportKind::Grpc
///     }
///
///     fn decode_stream(
///         &mut self,
///         data: &[u8],
///         ctx: &DecodeContext,
///     ) -> Result<Vec<DebugEvent>, CoreError> {
///         if data.is_empty() {
///             return Ok(Vec::new());
///         }
///
///         let event = DebugEvent::builder()
///             .source(EventSource {
///                 adapter: "decoder".to_string(),
///                 origin: ctx.src_addr.clone().unwrap_or_default(),
///                 network: None,
///             })
///             .transport(self.protocol())
///             .direction(Direction::Inbound)
///             .payload(Payload::Raw {
///                 raw: Bytes::copy_from_slice(data),
///             })
///             .build();
///
///         Ok(vec![event])
///     }
/// }
///
/// let mut decoder = TestDecoder;
/// let ctx = DecodeContext::new();
/// let events = decoder.decode_stream(b"test", &ctx).unwrap();
/// assert_eq!(events.len(), 1);
/// ```
pub trait ProtocolDecoder: Send {
    /// Returns the transport protocol this decoder handles.
    fn protocol(&self) -> TransportKind;

    /// Decodes a byte stream into zero or more `DebugEvents`.
    ///
    /// # Arguments
    /// * `data` - The raw byte sequence to decode
    /// * `ctx` - Decoding context with metadata
    ///
    /// # Returns
    /// A vector of decoded events. May be empty if the data is incomplete or not yet decodable.
    ///
    /// # Errors
    /// Returns an error if the data is malformed or cannot be decoded.
    fn decode_stream(
        &mut self,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError>;
}

/// Resolves message schemas for payload decoding.
///
/// Implemented by:
/// - `ProtobufSchemaResolver` (Subsection 2)
///
/// # Examples
///
/// Implementing a simple schema resolver:
///
/// ```
/// use prb_core::{SchemaResolver, ResolvedSchema, CoreError};
/// use bytes::Bytes;
/// use std::collections::HashMap;
///
/// struct TestResolver {
///     schemas: HashMap<String, Bytes>,
/// }
///
/// impl SchemaResolver for TestResolver {
///     fn resolve(&self, schema_name: &str) -> Result<Option<ResolvedSchema>, CoreError> {
///         Ok(self.schemas.get(schema_name).map(|content| ResolvedSchema {
///             name: schema_name.to_string(),
///             content: content.clone(),
///             format: "test".to_string(),
///         }))
///     }
///
///     fn list_schemas(&self) -> Vec<String> {
///         self.schemas.keys().cloned().collect()
///     }
/// }
///
/// let mut schemas = HashMap::new();
/// schemas.insert("test.Message".to_string(), Bytes::from("schema data"));
/// let resolver = TestResolver { schemas };
///
/// let schema = resolver.resolve("test.Message").unwrap();
/// assert!(schema.is_some());
/// assert_eq!(resolver.list_schemas().len(), 1);
/// ```
pub trait SchemaResolver {
    /// Resolves a schema by name.
    ///
    /// Returns `Ok(Some(schema))` if found, `Ok(None)` if not found, or an error if resolution fails.
    ///
    /// # Errors
    /// Returns an error if schema resolution fails due to I/O or parsing errors.
    fn resolve(&self, schema_name: &str) -> Result<Option<ResolvedSchema>, CoreError>;

    /// Lists all available schema names.
    fn list_schemas(&self) -> Vec<String>;
}

/// Normalizes events from adapter-specific format to canonical `DebugEvent`.
///
/// Implemented by per-adapter normalizers as needed.
pub trait EventNormalizer {
    /// Normalizes a batch of events.
    ///
    /// This can perform transformations like deduplication, timestamp adjustment,
    /// or metadata enrichment.
    ///
    /// # Errors
    /// Returns an error if normalization fails due to invalid event data.
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
    ///
    /// # Errors
    /// Returns an error if correlation logic fails.
    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError>;
}
