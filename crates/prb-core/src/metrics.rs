//! Conversation metrics computation.

use crate::{
    CoreError, DebugEvent, Direction, Payload, Timestamp,
    conversation::{ConversationError, ConversationMetrics},
};

/// Compute metrics for a flow.
///
/// This is protocol-agnostic and operates on event timestamps, directions,
/// and metadata.
pub fn compute_metrics(events: &[&DebugEvent]) -> Result<ConversationMetrics, CoreError> {
    if events.is_empty() {
        return Ok(ConversationMetrics::default());
    }

    let timestamps: Vec<Timestamp> = events.iter().map(|e| e.timestamp).collect();
    let start_time = timestamps.iter().min().copied();
    let end_time = timestamps.iter().max().copied();

    let duration_ns = match (start_time, end_time) {
        (Some(start), Some(end)) => end.as_nanos().saturating_sub(start.as_nanos()),
        _ => 0,
    };

    // Find first outbound and first inbound
    let first_outbound = events
        .iter()
        .find(|e| e.direction == Direction::Outbound)
        .map(|e| e.timestamp);

    let first_inbound = events
        .iter()
        .find(|e| e.direction == Direction::Inbound)
        .map(|e| e.timestamp);

    let time_to_first_response_ns = match (first_outbound, first_inbound) {
        (Some(out), Some(in_)) => Some(in_.as_nanos().saturating_sub(out.as_nanos())),
        _ => None,
    };

    let request_count = events
        .iter()
        .filter(|e| e.direction == Direction::Outbound)
        .count();

    let response_count = events
        .iter()
        .filter(|e| e.direction == Direction::Inbound)
        .count();

    let total_bytes = events.iter().map(|e| payload_size(&e.payload)).sum();

    // Extract error if present
    let error = extract_error(events);

    Ok(ConversationMetrics {
        start_time,
        end_time,
        duration_ns,
        time_to_first_response_ns,
        request_count,
        response_count,
        total_bytes,
        error,
    })
}

/// Get payload size in bytes.
const fn payload_size(payload: &Payload) -> u64 {
    match payload {
        Payload::Raw { raw } => raw.len() as u64,
        Payload::Decoded { raw, .. } => raw.len() as u64,
    }
}

/// Extract error from events.
fn extract_error(events: &[&DebugEvent]) -> Option<ConversationError> {
    // Check for gRPC error status
    for event in events {
        if let Some(status) = event.metadata.get("grpc.status")
            && status != "0"
        {
            let message = event
                .metadata
                .get("grpc.message")
                .cloned()
                .unwrap_or_else(|| format!("gRPC error status {status}"));

            return Some(ConversationError::new("grpc-status", message).with_code(status.clone()));
        }

        // Check for RST_STREAM
        if event.metadata.get("h2.frame_type") == Some(&"RST_STREAM".to_string()) {
            return Some(ConversationError::new("rst-stream", "HTTP/2 stream reset"));
        }
    }

    // Check for DDS sequence gaps
    if let Some(gap_count) = check_dds_sequence_gaps(events)
        && gap_count > 0
    {
        return Some(ConversationError::new(
            "sequence-gap",
            format!("{gap_count} missing sequence numbers"),
        ));
    }

    // Check for timeout (outbound but no inbound)
    let has_outbound = events.iter().any(|e| e.direction == Direction::Outbound);
    let has_inbound = events.iter().any(|e| e.direction == Direction::Inbound);

    if has_outbound && !has_inbound {
        return Some(ConversationError::new("timeout", "No response received"));
    }

    None
}

/// Check for DDS sequence gaps.
fn check_dds_sequence_gaps(events: &[&DebugEvent]) -> Option<usize> {
    let mut sequences: Vec<u64> = events.iter().filter_map(|e| e.sequence).collect();

    if sequences.is_empty() {
        return None;
    }

    sequences.sort_unstable();
    sequences.dedup();

    let first = *sequences.first()?;
    let last = *sequences.last()?;
    let expected_count = (last - first + 1) as usize;
    let actual_count = sequences.len();

    Some(expected_count.saturating_sub(actual_count))
}

/// Compute aggregate metrics for multiple conversations.
#[must_use] 
pub fn compute_aggregate_metrics(
    conversations: &[&crate::conversation::Conversation],
) -> AggregateMetrics {
    if conversations.is_empty() {
        return AggregateMetrics::default();
    }

    let total_conversations = conversations.len();

    let error_count = conversations
        .iter()
        .filter(|c| c.state == crate::conversation::ConversationState::Error)
        .count();

    let error_rate = error_count as f64 / total_conversations as f64;

    // Collect all durations for percentile calculation
    let mut durations: Vec<u64> = conversations
        .iter()
        .map(|c| c.metrics.duration_ns)
        .collect();

    durations.sort_unstable();

    let latency_p50_ns = percentile(&durations, 0.50);
    let latency_p95_ns = percentile(&durations, 0.95);
    let latency_p99_ns = percentile(&durations, 0.99);

    let total_bytes = conversations.iter().map(|c| c.metrics.total_bytes).sum();

    // Compute conversations per second
    let time_span_ns = if let (Some(first), Some(last)) = (
        conversations
            .iter()
            .filter_map(|c| c.metrics.start_time)
            .min(),
        conversations
            .iter()
            .filter_map(|c| c.metrics.end_time)
            .max(),
    ) {
        last.as_nanos().saturating_sub(first.as_nanos())
    } else {
        0
    };

    let conversations_per_second = if time_span_ns > 0 {
        (total_conversations as f64) / (time_span_ns as f64 / 1_000_000_000.0)
    } else {
        0.0
    };

    AggregateMetrics {
        total_conversations,
        error_rate,
        latency_p50_ns,
        latency_p95_ns,
        latency_p99_ns,
        total_bytes,
        conversations_per_second,
    }
}

/// Aggregate metrics across conversations.
#[derive(Debug, Clone, Default)]
pub struct AggregateMetrics {
    /// Total number of conversations.
    pub total_conversations: usize,
    /// Error rate (0.0 to 1.0).
    pub error_rate: f64,
    /// 50th percentile latency in nanoseconds.
    pub latency_p50_ns: u64,
    /// 95th percentile latency in nanoseconds.
    pub latency_p95_ns: u64,
    /// 99th percentile latency in nanoseconds.
    pub latency_p99_ns: u64,
    /// Total bytes across all conversations.
    pub total_bytes: u64,
    /// Average conversations per second.
    pub conversations_per_second: f64,
}

/// Calculate percentile from sorted values.
pub(crate) fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }

    let idx = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}
