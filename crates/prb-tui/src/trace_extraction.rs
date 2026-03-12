use prb_core::{CorrelationKey, DebugEvent};
use std::collections::HashMap;

/// A trace span representing a unit of work in a distributed trace.
#[derive(Debug, Clone)]
pub struct TraceSpan {
    /// Trace ID this span belongs to.
    pub trace_id: String,
    /// Unique span ID.
    pub span_id: String,
    /// Parent span ID, if any.
    pub parent_span_id: Option<String>,
    /// Event index in the event store that contains this span.
    pub event_idx: usize,
    /// Timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Span name or operation.
    pub name: String,
}

/// A complete trace tree containing a root trace ID and all spans.
#[derive(Debug, Clone)]
pub struct TraceTree {
    /// Root trace ID.
    pub trace_id: String,
    /// All spans in this trace, indexed by span_id.
    pub spans: HashMap<String, TraceSpan>,
    /// Root span IDs (spans without parents).
    pub root_span_ids: Vec<String>,
}

impl TraceTree {
    /// Build a tree of child span IDs for a given parent span ID.
    pub fn children_of(&self, span_id: &str) -> Vec<&TraceSpan> {
        self.spans
            .values()
            .filter(|span| {
                span.parent_span_id
                    .as_ref()
                    .map(|p| p == span_id)
                    .unwrap_or(false)
            })
            .collect()
    }
}

/// Extract trace ID from event metadata or correlation keys.
pub fn extract_trace_context(event: &DebugEvent) -> Option<(String, String, Option<String>)> {
    // First, check correlation keys for TraceContext
    for key in &event.correlation_keys {
        if let CorrelationKey::TraceContext { trace_id, span_id } = key {
            // Try to find parent span ID in metadata
            let parent_span_id = event
                .metadata
                .get(prb_core::METADATA_KEY_OTEL_PARENT_SPAN_ID)
                .cloned();
            return Some((trace_id.clone(), span_id.clone(), parent_span_id));
        }
    }

    // Fall back to metadata-based extraction
    let trace_id = event.metadata.get(prb_core::METADATA_KEY_OTEL_TRACE_ID)?;
    let span_id = event.metadata.get(prb_core::METADATA_KEY_OTEL_SPAN_ID)?;
    let parent_span_id = event
        .metadata
        .get(prb_core::METADATA_KEY_OTEL_PARENT_SPAN_ID)
        .cloned();

    Some((trace_id.clone(), span_id.clone(), parent_span_id))
}

/// Build trace trees from a collection of events.
pub fn build_trace_trees(events: &[&DebugEvent], event_indices: &[usize]) -> Vec<TraceTree> {
    let mut traces: HashMap<String, TraceTree> = HashMap::new();

    // First pass: collect all spans
    for &event_idx in event_indices.iter() {
        if let Some(event) = events.get(event_idx)
            && let Some((trace_id, span_id, parent_span_id)) = extract_trace_context(event)
        {
            let span = TraceSpan {
                trace_id: trace_id.clone(),
                span_id: span_id.clone(),
                parent_span_id,
                event_idx,
                timestamp_ns: event.timestamp.as_nanos(),
                name: format!("Span {}", &span_id[..8.min(span_id.len())]),
            };

            traces
                .entry(trace_id.clone())
                .or_insert_with(|| TraceTree {
                    trace_id: trace_id.clone(),
                    spans: HashMap::new(),
                    root_span_ids: Vec::new(),
                })
                .spans
                .insert(span_id, span);
        }
    }

    // Second pass: identify root spans (those without parents or with missing parents)
    for tree in traces.values_mut() {
        let mut roots = Vec::new();
        for (span_id, span) in &tree.spans {
            let is_root = span.parent_span_id.is_none()
                || span
                    .parent_span_id
                    .as_ref()
                    .map(|p| !tree.spans.contains_key(p))
                    .unwrap_or(false);

            if is_root {
                roots.push(span_id.clone());
            }
        }
        tree.root_span_ids = roots;
    }

    traces.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prb_core::{DebugEvent, Direction, EventSource, Payload, Timestamp, TransportKind};

    #[test]
    fn test_extract_trace_context_from_correlation_keys() {
        use bytes::Bytes;

        let mut event = DebugEvent::builder()
            .id(prb_core::EventId::next())
            .timestamp(Timestamp::from_nanos(1000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from(vec![]),
            })
            .build();

        event.correlation_keys.push(CorrelationKey::TraceContext {
            trace_id: "trace123".into(),
            span_id: "span456".into(),
        });

        let result = extract_trace_context(&event);
        assert!(result.is_some());
        let (trace_id, span_id, parent) = result.unwrap();
        assert_eq!(trace_id, "trace123");
        assert_eq!(span_id, "span456");
        assert!(parent.is_none());
    }

    #[test]
    fn test_extract_trace_context_from_metadata() {
        use bytes::Bytes;

        let event = DebugEvent::builder()
            .id(prb_core::EventId::next())
            .timestamp(Timestamp::from_nanos(2000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from(vec![]),
            })
            .metadata(prb_core::METADATA_KEY_OTEL_TRACE_ID, "trace789")
            .metadata(prb_core::METADATA_KEY_OTEL_SPAN_ID, "span012")
            .metadata(prb_core::METADATA_KEY_OTEL_PARENT_SPAN_ID, "parent345")
            .build();

        let result = extract_trace_context(&event);
        assert!(result.is_some());
        let (trace_id, span_id, parent) = result.unwrap();
        assert_eq!(trace_id, "trace789");
        assert_eq!(span_id, "span012");
        assert_eq!(parent, Some("parent345".into()));
    }

    #[test]
    fn test_build_trace_trees() {
        use bytes::Bytes;

        let mut events = Vec::new();

        // Create a simple trace: root -> child1
        let mut root = DebugEvent::builder()
            .id(prb_core::EventId::next())
            .timestamp(Timestamp::from_nanos(1000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw {
                raw: Bytes::from(vec![]),
            })
            .build();
        root.correlation_keys.push(CorrelationKey::TraceContext {
            trace_id: "trace1".into(),
            span_id: "root".into(),
        });
        events.push(root);

        let mut child1 = DebugEvent::builder()
            .id(prb_core::EventId::next())
            .timestamp(Timestamp::from_nanos(2000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from(vec![]),
            })
            .metadata(prb_core::METADATA_KEY_OTEL_PARENT_SPAN_ID, "root")
            .build();
        child1.correlation_keys.push(CorrelationKey::TraceContext {
            trace_id: "trace1".into(),
            span_id: "child1".into(),
        });
        events.push(child1);

        let event_refs: Vec<&DebugEvent> = events.iter().collect();
        let indices: Vec<usize> = (0..events.len()).collect();
        let trees = build_trace_trees(&event_refs, &indices);

        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].trace_id, "trace1");
        assert_eq!(trees[0].spans.len(), 2);
        assert_eq!(trees[0].root_span_ids.len(), 1);
        assert_eq!(trees[0].root_span_ids[0], "root");

        let children = trees[0].children_of("root");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].span_id, "child1");
    }
}
