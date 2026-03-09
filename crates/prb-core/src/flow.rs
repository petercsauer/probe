//! Correlation flow types.

use crate::DebugEvent;

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
