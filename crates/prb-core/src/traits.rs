//! Extension traits for the PRB universal message debugger.
//!
//! All traits are synchronous (Phase 1 is offline-only analysis).

use crate::{CoreError, DebugEvent, TransportKind};

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
pub trait ProtocolDecoder {
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

/// Decoding context for ProtocolDecoder.
#[derive(Debug, Clone)]
pub struct DecodeContext {
    /// Source address information (e.g., "192.168.1.1:8080").
    pub src_addr: Option<String>,
    /// Destination address information.
    pub dst_addr: Option<String>,
    /// Additional context metadata.
    pub metadata: std::collections::BTreeMap<String, String>,
}

impl DecodeContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self {
            src_addr: None,
            dst_addr: None,
            metadata: std::collections::BTreeMap::new(),
        }
    }

    /// Set source address.
    pub fn with_src_addr(mut self, addr: impl Into<String>) -> Self {
        self.src_addr = Some(addr.into());
        self
    }

    /// Set destination address.
    pub fn with_dst_addr(mut self, addr: impl Into<String>) -> Self {
        self.dst_addr = Some(addr.into());
        self
    }

    /// Add metadata entry.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl Default for DecodeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolved schema information.
#[derive(Debug, Clone)]
pub struct ResolvedSchema {
    /// Schema name.
    pub name: String,
    /// Schema content (e.g., protobuf FileDescriptorSet bytes, JSON schema).
    pub content: bytes::Bytes,
    /// Schema format identifier (e.g., "protobuf", "json-schema").
    pub format: String,
}

/// A correlation flow grouping related events.
#[derive(Debug, Clone)]
pub struct Flow<'a> {
    /// Unique flow identifier.
    pub id: String,
    /// Events in this flow (ordered by timestamp).
    pub events: Vec<&'a DebugEvent>,
    /// Flow metadata (e.g., connection info, topic name).
    pub metadata: std::collections::BTreeMap<String, String>,
}

impl<'a> Flow<'a> {
    /// Create a new flow with the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            events: Vec::new(),
            metadata: std::collections::BTreeMap::new(),
        }
    }

    /// Add an event to the flow.
    pub fn add_event(mut self, event: &'a DebugEvent) -> Self {
        self.events.push(event);
        self
    }

    /// Add metadata to the flow.
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}
