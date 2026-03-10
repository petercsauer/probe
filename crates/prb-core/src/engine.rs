//! Conversation reconstruction engine.
//!
//! Orchestrates protocol-specific correlation strategies to build conversations
//! from debug events.

use crate::{
    conversation::{Conversation, ConversationKind, ConversationState},
    CoreError, CorrelationStrategy, DebugEvent, EventId, TransportKind,
};
use std::collections::HashMap;

/// Orchestrates conversation reconstruction across protocols.
pub struct ConversationEngine {
    pub(crate) strategies: Vec<Box<dyn CorrelationStrategy>>,
}

impl ConversationEngine {
    /// Create a new conversation engine.
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    /// Register a protocol-specific correlation strategy.
    pub fn register(&mut self, strategy: Box<dyn CorrelationStrategy>) {
        self.strategies.push(strategy);
    }

    /// Build conversations from a slice of events.
    ///
    /// Each strategy handles events matching its transport. Events not claimed
    /// by any strategy are grouped into fallback TCP/UDP conversations by
    /// network address.
    pub fn build_conversations(
        &self,
        events: &[DebugEvent],
    ) -> Result<ConversationSet, CoreError> {
        let mut all_conversations = Vec::new();
        let mut claimed_events = std::collections::HashSet::new();

        // Partition events by protocol and dispatch to strategies
        for strategy in &self.strategies {
            let protocol = strategy.transport();
            let protocol_events: Vec<_> = events
                .iter()
                .filter(|e| e.transport == protocol)
                .cloned()
                .collect();

            if protocol_events.is_empty() {
                continue;
            }

            // Get flows from strategy
            let flows = strategy.correlate(&protocol_events)?;

            // Convert flows to conversations
            for flow in flows {
                let conversation = flow_to_conversation(flow, protocol)?;

                // Mark events as claimed
                for event_id in &conversation.event_ids {
                    claimed_events.insert(*event_id);
                }

                all_conversations.push(conversation);
            }
        }

        // Fallback: group unclaimed events by network address
        let unclaimed: Vec<_> = events
            .iter()
            .filter(|e| !claimed_events.contains(&e.id))
            .collect();

        if !unclaimed.is_empty() {
            let fallback_conversations = group_fallback_events(&unclaimed)?;
            all_conversations.extend(fallback_conversations);
        }

        // Build index
        let mut event_index = HashMap::new();
        for (idx, conv) in all_conversations.iter().enumerate() {
            for event_id in &conv.event_ids {
                event_index.insert(*event_id, idx);
            }
        }

        Ok(ConversationSet {
            conversations: all_conversations,
            event_index,
        })
    }

    /// Look up which conversation an event belongs to.
    pub fn conversation_for_event<'a>(
        &self,
        set: &'a ConversationSet,
        event_id: EventId,
    ) -> Option<&'a Conversation> {
        set.for_event(event_id)
    }
}

impl Default for ConversationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Holds all conversations plus an index for fast lookup.
pub struct ConversationSet {
    /// All reconstructed conversations.
    pub conversations: Vec<Conversation>,
    /// Maps event ID → conversation index for O(1) lookup.
    event_index: HashMap<EventId, usize>,
}

impl ConversationSet {
    /// Get conversation containing the given event.
    pub fn for_event(&self, event_id: EventId) -> Option<&Conversation> {
        self.event_index
            .get(&event_id)
            .map(|&idx| &self.conversations[idx])
    }

    /// Get all conversations, sorted by start time.
    pub fn sorted_by_time(&self) -> Vec<&Conversation> {
        let mut sorted: Vec<_> = self.conversations.iter().collect();
        sorted.sort_by_key(|c| c.metrics.start_time);
        sorted
    }

    /// Filter conversations by protocol.
    pub fn by_protocol(&self, protocol: TransportKind) -> Vec<&Conversation> {
        self.conversations
            .iter()
            .filter(|c| c.protocol == protocol)
            .collect()
    }

    /// Summary statistics.
    pub fn stats(&self) -> ConversationStats {
        let mut by_protocol = HashMap::new();
        let mut by_state = HashMap::new();
        let mut by_kind = HashMap::new();

        for conv in &self.conversations {
            *by_protocol.entry(conv.protocol).or_insert(0) += 1;
            *by_state.entry(conv.state).or_insert(0) += 1;
            *by_kind.entry(conv.kind).or_insert(0) += 1;
        }

        ConversationStats {
            total: self.conversations.len(),
            by_protocol,
            by_state,
            by_kind,
        }
    }
}

/// Summary statistics for a conversation set.
pub struct ConversationStats {
    /// Total conversation count.
    pub total: usize,
    /// Count by protocol.
    pub by_protocol: HashMap<TransportKind, usize>,
    /// Count by state.
    pub by_state: HashMap<ConversationState, usize>,
    /// Count by kind.
    pub by_kind: HashMap<ConversationKind, usize>,
}

/// Convert a Flow to a Conversation.
pub(crate) fn flow_to_conversation(
    flow: crate::Flow<'_>,
    protocol: TransportKind,
) -> Result<Conversation, CoreError> {
    use crate::conversation::ConversationId;

    let event_ids: Vec<_> = flow.events.iter().map(|e| e.id).collect();

    // Compute metrics from events
    let metrics = super::metrics::compute_metrics(&flow.events)?;

    // Determine conversation kind and state
    let (kind, state) = classify_conversation(&flow.events, protocol, &flow.metadata);

    // Generate summary
    let summary = generate_summary(&flow.metadata, kind, state, &metrics);

    let mut conversation = Conversation::new(
        ConversationId::new(flow.id),
        kind,
        protocol,
        state,
    );

    conversation.event_ids = event_ids;
    conversation.metrics = metrics;
    conversation.metadata = flow.metadata;
    conversation.summary = summary;

    Ok(conversation)
}

/// Classify conversation kind and state from events.
fn classify_conversation(
    events: &[&DebugEvent],
    protocol: TransportKind,
    metadata: &std::collections::BTreeMap<String, String>,
) -> (ConversationKind, ConversationState) {
    use crate::Direction;

    let outbound_count = events.iter().filter(|e| e.direction == Direction::Outbound).count();
    let inbound_count = events.iter().filter(|e| e.direction == Direction::Inbound).count();

    // Check for errors in metadata
    let has_error = metadata.get("grpc.status")
        .map(|s| s != "0")
        .unwrap_or(false);

    let state = if has_error {
        ConversationState::Error
    } else if outbound_count > 0 && inbound_count == 0 {
        ConversationState::Timeout
    } else if outbound_count == 0 && inbound_count > 0 {
        ConversationState::Incomplete
    } else if inbound_count > 0 {
        ConversationState::Complete
    } else {
        ConversationState::Active
    };

    let kind = match protocol {
        TransportKind::Grpc => classify_grpc_kind(outbound_count, inbound_count),
        TransportKind::Zmq => classify_zmq_kind(metadata),
        TransportKind::DdsRtps => ConversationKind::TopicExchange,
        TransportKind::RawTcp => ConversationKind::TcpStream,
        _ => ConversationKind::Unknown,
    };

    (kind, state)
}

/// Classify gRPC conversation kind.
fn classify_grpc_kind(outbound: usize, inbound: usize) -> ConversationKind {
    match (outbound, inbound) {
        (1, 1) => ConversationKind::UnaryRpc,
        (1, n) if n > 1 => ConversationKind::ServerStreaming,
        (n, 1) if n > 1 => ConversationKind::ClientStreaming,
        (n, m) if n > 1 && m > 1 => ConversationKind::BidirectionalStreaming,
        _ => ConversationKind::UnaryRpc,
    }
}

/// Classify ZMQ conversation kind from metadata.
fn classify_zmq_kind(metadata: &std::collections::BTreeMap<String, String>) -> ConversationKind {
    if let Some(socket_type) = metadata.get("zmq.socket_type") {
        match socket_type.as_str() {
            "PUB" | "SUB" => ConversationKind::PubSubChannel,
            "REQ" | "REP" | "DEALER" | "ROUTER" => ConversationKind::RequestReply,
            "PUSH" | "PULL" => ConversationKind::Pipeline,
            _ => ConversationKind::Unknown,
        }
    } else {
        ConversationKind::Unknown
    }
}

/// Generate a human-readable summary.
fn generate_summary(
    metadata: &std::collections::BTreeMap<String, String>,
    kind: ConversationKind,
    state: ConversationState,
    metrics: &crate::conversation::ConversationMetrics,
) -> String {
    // For gRPC: "POST /api.v1.Users/Get → OK (12ms)"
    if let Some(method) = metadata.get("grpc.method") {
        let status = metadata.get("grpc.status")
            .and_then(|s| grpc_status_name(s))
            .unwrap_or_else(|| state.to_string());
        let duration_ms = metrics.duration_ns / 1_000_000;
        return format!("{} → {} ({}ms)", method, status, duration_ms);
    }

    // For ZMQ PUB/SUB: "PUB topic=market.data — 142 messages (5.2s)"
    if let Some(topic) = metadata.get("zmq.topic") {
        let count = metrics.request_count + metrics.response_count;
        let duration_s = metrics.duration_ns as f64 / 1_000_000_000.0;
        return format!("PUB topic={} — {} messages ({:.1}s)", topic, count, duration_s);
    }

    // For DDS: "Topic=rt/chatter domain=0 — 256 samples"
    if let Some(topic) = metadata.get("dds.topic_name") {
        let domain = metadata.get("dds.domain_id").map(|d| d.as_str()).unwrap_or("?");
        let count = metrics.request_count + metrics.response_count;
        return format!("Topic={} domain={} — {} samples", topic, domain, count);
    }

    // Fallback
    format!("{} conversation — {} state", kind, state)
}

/// Map gRPC status code to name.
fn grpc_status_name(code: &str) -> Option<String> {
    match code {
        "0" => Some("OK".to_string()),
        "1" => Some("CANCELLED".to_string()),
        "2" => Some("UNKNOWN".to_string()),
        "3" => Some("INVALID_ARGUMENT".to_string()),
        "4" => Some("DEADLINE_EXCEEDED".to_string()),
        "5" => Some("NOT_FOUND".to_string()),
        "6" => Some("ALREADY_EXISTS".to_string()),
        "7" => Some("PERMISSION_DENIED".to_string()),
        "8" => Some("RESOURCE_EXHAUSTED".to_string()),
        "9" => Some("FAILED_PRECONDITION".to_string()),
        "10" => Some("ABORTED".to_string()),
        "11" => Some("OUT_OF_RANGE".to_string()),
        "12" => Some("UNIMPLEMENTED".to_string()),
        "13" => Some("INTERNAL".to_string()),
        "14" => Some("UNAVAILABLE".to_string()),
        "15" => Some("DATA_LOSS".to_string()),
        "16" => Some("UNAUTHENTICATED".to_string()),
        _ => None,
    }
}

/// Group unclaimed events into fallback TCP/UDP conversations.
fn group_fallback_events(events: &[&DebugEvent]) -> Result<Vec<Conversation>, CoreError> {
    use crate::conversation::ConversationId;
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, Vec<&DebugEvent>> = BTreeMap::new();

    for event in events {
        let key = if let Some(ref network) = event.source.network {
            format!("{}:{}->{}", event.transport, network.src, network.dst)
        } else {
            format!("{}:{}", event.transport, event.source.origin)
        };
        groups.entry(key).or_default().push(event);
    }

    let mut conversations = Vec::new();
    for (key, group) in groups {
        let protocol = group[0].transport;
        let event_ids: Vec<_> = group.iter().map(|e| e.id).collect();
        let metrics = super::metrics::compute_metrics(&group)?;

        let mut conversation = Conversation::new(
            ConversationId::new(key),
            ConversationKind::TcpStream,
            protocol,
            ConversationState::Incomplete,
        );

        conversation.event_ids = event_ids;
        conversation.metrics = metrics;
        conversation.summary = format!("Fallback {} conversation", protocol);

        conversations.push(conversation);
    }

    Ok(conversations)
}
