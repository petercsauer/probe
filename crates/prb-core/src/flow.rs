//! Correlation flow types.

use crate::DebugEvent;

/// A correlation flow grouping related events.
///
/// Flows are intermediate structures used by correlation strategies to
/// group related events before they are converted into conversations.
///
/// # Examples
///
/// ```
/// use prb_core::{Flow, DebugEvent, EventSource, TransportKind, Direction, Payload};
/// use bytes::Bytes;
///
/// let event = DebugEvent::builder()
///     .source(EventSource {
///         adapter: "test".to_string(),
///         origin: "test".to_string(),
///         network: None,
///     })
///     .transport(TransportKind::Grpc)
///     .direction(Direction::Outbound)
///     .payload(Payload::Raw { raw: Bytes::new() })
///     .build();
///
/// let flow = Flow::new("stream-1")
///     .add_event(&event)
///     .add_metadata("method", "/api.Service/Method");
///
/// assert_eq!(flow.id, "stream-1");
/// assert_eq!(flow.events.len(), 1);
/// ```
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
    #[must_use]
    pub fn add_event(mut self, event: &'a DebugEvent) -> Self {
        self.events.push(event);
        self
    }

    /// Add metadata to the flow.
    #[must_use]
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}
