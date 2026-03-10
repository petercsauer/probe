use prb_core::{DebugEvent, METADATA_KEY_OTEL_TRACE_ID, METADATA_KEY_OTEL_SPAN_ID};
use std::collections::BTreeMap;

/// Merged event with optional OTel span enrichment.
#[derive(Debug, Clone)]
pub struct MergedEvent {
    pub event: DebugEvent,
    pub otel_span: Option<SpanSummary>,
}

/// Summary of OTel span information for enrichment.
#[derive(Debug, Clone)]
pub struct SpanSummary {
    pub service_name: String,
    pub operation_name: String,
    pub duration_us: u64,
    pub status: String,
}

/// Merge OTLP trace events with packet-level events.
///
/// Events are matched by trace_id + span_id. Packet events get enriched with
/// span metadata. Trace events without matching packets are included as-is.
/// The result is sorted by timestamp.
pub fn merge_traces_with_packets(
    packet_events: &[DebugEvent],
    trace_events: &[DebugEvent],
) -> Vec<MergedEvent> {
    // Build index of trace events by (trace_id, span_id)
    let mut trace_index: BTreeMap<(String, String), &DebugEvent> = BTreeMap::new();
    for event in trace_events {
        if let (Some(trace_id), Some(span_id)) = (
            event.metadata.get(METADATA_KEY_OTEL_TRACE_ID),
            event.metadata.get(METADATA_KEY_OTEL_SPAN_ID),
        ) {
            trace_index.insert((trace_id.clone(), span_id.clone()), event);
        }
    }

    let mut merged = Vec::new();

    // Process packet events and enrich with trace data
    for event in packet_events {
        let otel_span = if let (Some(trace_id), Some(span_id)) = (
            event.metadata.get(METADATA_KEY_OTEL_TRACE_ID),
            event.metadata.get(METADATA_KEY_OTEL_SPAN_ID),
        ) {
            trace_index
                .get(&(trace_id.clone(), span_id.clone()))
                .map(|trace_event| extract_span_summary(trace_event))
        } else {
            None
        };

        merged.push(MergedEvent {
            event: event.clone(),
            otel_span,
        });
    }

    // Add trace events that don't have matching packet events
    for event in trace_events {
        if let (Some(trace_id), Some(span_id)) = (
            event.metadata.get(METADATA_KEY_OTEL_TRACE_ID),
            event.metadata.get(METADATA_KEY_OTEL_SPAN_ID),
        ) {
            // Check if this span is already represented in packet events
            let has_packet = packet_events.iter().any(|pe| {
                pe.metadata.get(METADATA_KEY_OTEL_TRACE_ID) == Some(trace_id)
                    && pe.metadata.get(METADATA_KEY_OTEL_SPAN_ID) == Some(span_id)
            });

            if !has_packet {
                merged.push(MergedEvent {
                    event: event.clone(),
                    otel_span: Some(extract_span_summary(event)),
                });
            }
        }
    }

    // Sort by timestamp
    merged.sort_by_key(|m| m.event.timestamp.as_nanos());

    merged
}

fn extract_span_summary(event: &DebugEvent) -> SpanSummary {
    let service_name = event.source.origin.clone();
    let operation_name = event
        .metadata
        .get("grpc.method")
        .or_else(|| event.metadata.get("name"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    // Duration is not stored in DebugEvent (events are point-in-time), so we default to 0
    let duration_us = 0;

    let status = event
        .metadata
        .get("grpc.status")
        .map(|s| {
            if s == "0" {
                "OK".to_string()
            } else {
                format!("ERROR ({})", s)
            }
        })
        .unwrap_or_else(|| "UNKNOWN".to_string());

    SpanSummary {
        service_name,
        operation_name,
        duration_us,
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::*;

    fn sample_packet_event(trace_id: &str, span_id: &str) -> DebugEvent {
        DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:50051".into(),
                    dst: "10.0.0.2:8080".into(),
                }),
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"packet"),
            })
            .metadata(METADATA_KEY_OTEL_TRACE_ID, trace_id)
            .metadata(METADATA_KEY_OTEL_SPAN_ID, span_id)
            .metadata("grpc.method", "/api.v1.Users/Get")
            .build()
    }

    fn sample_trace_event(trace_id: &str, span_id: &str) -> DebugEvent {
        DebugEvent::builder()
            .id(EventId::from_raw(2))
            .timestamp(Timestamp::from_nanos(1_710_000_000_000_000_000))
            .source(EventSource {
                adapter: "otlp-import".into(),
                origin: "my-service".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"trace"),
            })
            .metadata(METADATA_KEY_OTEL_TRACE_ID, trace_id)
            .metadata(METADATA_KEY_OTEL_SPAN_ID, span_id)
            .metadata("grpc.method", "/api.v1.Users/Get")
            .metadata("grpc.status", "0")
            .build()
    }

    #[test]
    fn test_merge_matching_trace_ids() {
        let packet = sample_packet_event("aaaa", "bbbb");
        let trace = sample_trace_event("aaaa", "bbbb");

        let merged = merge_traces_with_packets(&[packet], &[trace]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].event.source.adapter, "pcap");
        assert!(merged[0].otel_span.is_some());
        assert_eq!(merged[0].otel_span.as_ref().unwrap().service_name, "my-service");
    }

    #[test]
    fn test_merge_no_overlap() {
        let packet = sample_packet_event("aaaa", "bbbb");
        let trace = sample_trace_event("cccc", "dddd");

        let merged = merge_traces_with_packets(&[packet], &[trace]);

        // Both events should be included
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_sort_order() {
        let packet1 = DebugEvent::builder()
            .id(EventId::from_raw(1))
            .timestamp(Timestamp::from_nanos(1_710_000_002_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"packet"),
            })
            .metadata(METADATA_KEY_OTEL_TRACE_ID, "aaaa")
            .metadata(METADATA_KEY_OTEL_SPAN_ID, "bbbb")
            .build();

        let packet2 = DebugEvent::builder()
            .id(EventId::from_raw(2))
            .timestamp(Timestamp::from_nanos(1_710_000_001_000_000_000))
            .source(EventSource {
                adapter: "pcap".into(),
                origin: "test.pcap".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: Bytes::from_static(b"packet"),
            })
            .metadata(METADATA_KEY_OTEL_TRACE_ID, "cccc")
            .metadata(METADATA_KEY_OTEL_SPAN_ID, "dddd")
            .build();

        let merged = merge_traces_with_packets(&[packet1, packet2], &[]);

        assert_eq!(merged.len(), 2);
        // Should be sorted by timestamp
        assert_eq!(merged[0].event.id.as_u64(), 2);
        assert_eq!(merged[1].event.id.as_u64(), 1);
    }
}
