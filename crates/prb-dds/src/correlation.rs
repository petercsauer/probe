//! DDS conversation correlation strategy.
//!
//! Groups DDS events by domain ID, topic name, and writer GUID.

use indexmap::IndexMap;
use prb_core::{
    CoreError, CorrelationStrategy, DebugEvent, Flow, TransportKind, METADATA_KEY_DDS_DOMAIN_ID,
    METADATA_KEY_DDS_TOPIC_NAME,
};
use std::collections::BTreeMap;

/// DDS correlation strategy.
///
/// Groups events by (domain_id, topic_name, writer_guid).
pub struct DdsCorrelationStrategy;

impl CorrelationStrategy for DdsCorrelationStrategy {
    fn transport(&self) -> TransportKind {
        TransportKind::DdsRtps
    }

    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError> {
        // Group events by (domain_id, topic_name, writer_guid)
        let mut groups: IndexMap<String, Vec<&'a DebugEvent>> = IndexMap::new();

        for event in events {
            let key = grouping_key(event);
            groups.entry(key).or_default().push(event);
        }

        // Convert groups to flows
        let mut flows = Vec::new();
        for (key, mut group) in groups {
            // Sort by sequence number if available, then timestamp
            group.sort_by(|a, b| {
                match (a.sequence, b.sequence) {
                    (Some(seq_a), Some(seq_b)) => seq_a.cmp(&seq_b),
                    _ => a.timestamp.cmp(&b.timestamp),
                }
            });

            // Extract metadata
            let mut metadata = BTreeMap::new();

            if let Some(domain_id) = group
                .iter()
                .find_map(|e| e.metadata.get(METADATA_KEY_DDS_DOMAIN_ID))
            {
                metadata.insert("dds.domain_id".to_string(), domain_id.clone());
            }

            if let Some(topic_name) = group
                .iter()
                .find_map(|e| e.metadata.get(METADATA_KEY_DDS_TOPIC_NAME))
            {
                metadata.insert("dds.topic_name".to_string(), topic_name.clone());
            }

            if let Some(writer_guid) = group.iter().find_map(|e| e.metadata.get("dds.writer_guid"))
            {
                metadata.insert("dds.writer_guid".to_string(), writer_guid.clone());
            }

            // Calculate sequence range
            let sequences: Vec<u64> = group.iter().filter_map(|e| e.sequence).collect();
            if !sequences.is_empty() {
                let first = sequences.iter().min().unwrap();
                let last = sequences.iter().max().unwrap();
                metadata.insert(
                    "sequence_range".to_string(),
                    format!("{}..{}", first, last),
                );

                // Calculate gap count
                let gap_count = calculate_gap_count(&sequences);
                if gap_count > 0 {
                    metadata.insert("gap_count".to_string(), gap_count.to_string());
                }
            }

            let mut flow = Flow::new(key);
            for event in group {
                flow = flow.add_event(event);
            }
            for (k, v) in metadata {
                flow = flow.add_metadata(k, v);
            }

            flows.push(flow);
        }

        Ok(flows)
    }
}

/// Generate grouping key for a DDS event.
fn grouping_key(event: &DebugEvent) -> String {
    let domain = event
        .metadata
        .get(METADATA_KEY_DDS_DOMAIN_ID)
        .map(|s| s.as_str())
        .unwrap_or("?");

    let topic = event
        .metadata
        .get(METADATA_KEY_DDS_TOPIC_NAME)
        .map(|s| s.as_str())
        .unwrap_or("(unknown)");

    let writer = event
        .metadata
        .get("dds.writer_guid")
        .map(|s| s.as_str())
        .unwrap_or("?");

    format!("dds:d{}/{}:w{}", domain, topic, writer)
}

/// Calculate the number of sequence gaps.
fn calculate_gap_count(sequences: &[u64]) -> usize {
    if sequences.len() < 2 {
        return 0;
    }

    let mut sorted = sequences.to_vec();
    sorted.sort_unstable();
    sorted.dedup();

    let first = *sorted.first().unwrap();
    let last = *sorted.last().unwrap();
    let expected_count = (last - first + 1) as usize;
    let actual_count = sorted.len();

    expected_count.saturating_sub(actual_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prb_core::{DebugEvent, Direction, EventSource, NetworkAddr, Payload, Timestamp};

    fn make_dds_event(
        domain_id: &str,
        topic_name: &str,
        writer_guid: &str,
        sequence: Option<u64>,
    ) -> DebugEvent {
        let mut builder = DebugEvent::builder()
            .source(EventSource {
                adapter: "test".to_string(),
                origin: "test.pcap".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:7400".to_string(),
                    dst: "239.255.0.1:7400".to_string(),
                }),
            })
            .transport(TransportKind::DdsRtps)
            .direction(Direction::Outbound)
            .payload(Payload::Raw {
                raw: bytes::Bytes::from("test"),
            })
            .timestamp(Timestamp::from_nanos(1000))
            .metadata("dds.domain_id", domain_id)
            .metadata("dds.topic_name", topic_name)
            .metadata("dds.writer_guid", writer_guid);

        if let Some(seq) = sequence {
            builder = builder.sequence(seq);
        }

        builder.build()
    }

    #[test]
    fn test_same_writer_same_topic() {
        let events = vec![
            make_dds_event("0", "rt/chatter", "writer1", Some(1)),
            make_dds_event("0", "rt/chatter", "writer1", Some(2)),
            make_dds_event("0", "rt/chatter", "writer1", Some(3)),
        ];

        let strategy = DdsCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].events.len(), 3);
        assert_eq!(
            flows[0].metadata.get("dds.topic_name"),
            Some(&"rt/chatter".to_string())
        );
        assert_eq!(
            flows[0].metadata.get("sequence_range"),
            Some(&"1..3".to_string())
        );
    }

    #[test]
    fn test_different_writers_same_topic() {
        let events = vec![
            make_dds_event("0", "rt/chatter", "writer1", Some(1)),
            make_dds_event("0", "rt/chatter", "writer2", Some(1)),
        ];

        let strategy = DdsCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Different writers → separate conversations
        assert_eq!(flows.len(), 2);
    }

    #[test]
    fn test_different_domains() {
        let events = vec![
            make_dds_event("0", "rt/chatter", "writer1", Some(1)),
            make_dds_event("1", "rt/chatter", "writer1", Some(1)),
        ];

        let strategy = DdsCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        // Different domains → separate conversations
        assert_eq!(flows.len(), 2);
    }

    #[test]
    fn test_sequence_gap_detection() {
        let events = vec![
            make_dds_event("0", "rt/chatter", "writer1", Some(1)),
            make_dds_event("0", "rt/chatter", "writer1", Some(2)),
            make_dds_event("0", "rt/chatter", "writer1", Some(4)), // Gap at 3
            make_dds_event("0", "rt/chatter", "writer1", Some(5)),
        ];

        let strategy = DdsCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].metadata.get("gap_count"), Some(&"1".to_string()));
    }

    #[test]
    fn test_duplicate_sequence_numbers() {
        let events = vec![
            make_dds_event("0", "rt/chatter", "writer1", Some(1)),
            make_dds_event("0", "rt/chatter", "writer1", Some(2)),
            make_dds_event("0", "rt/chatter", "writer1", Some(2)), // Duplicate
            make_dds_event("0", "rt/chatter", "writer1", Some(3)),
        ];

        let strategy = DdsCorrelationStrategy;
        let flows = strategy.correlate(&events).unwrap();

        assert_eq!(flows.len(), 1);
        // Gap count should be None (duplicates are removed during dedup, so no gaps)
        assert_eq!(flows[0].metadata.get("gap_count"), None);
    }
}
