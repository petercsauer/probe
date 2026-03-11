use prb_core::{DebugEvent, Timestamp, TransportKind};
use prb_query::Filter;
use std::collections::HashMap;

/// Index structure for fast lookups by protocol, source, and destination.
#[derive(Debug, Clone)]
pub struct EventIndex {
    pub by_protocol: HashMap<TransportKind, Vec<usize>>,
    pub by_source: HashMap<String, Vec<usize>>,
    pub by_dest: HashMap<String, Vec<usize>>,
    pub time_sorted: Vec<usize>,
}

impl EventIndex {
    /// Build an index from a list of events.
    pub fn build(events: &[DebugEvent]) -> Self {
        let mut by_protocol: HashMap<TransportKind, Vec<usize>> = HashMap::new();
        let mut by_source: HashMap<String, Vec<usize>> = HashMap::new();
        let mut by_dest: HashMap<String, Vec<usize>> = HashMap::new();
        let mut time_sorted: Vec<usize> = (0..events.len()).collect();

        for (idx, event) in events.iter().enumerate() {
            by_protocol.entry(event.transport).or_default().push(idx);

            if let Some(ref network) = event.source.network {
                by_source.entry(network.src.clone()).or_default().push(idx);
                by_dest.entry(network.dst.clone()).or_default().push(idx);
            }
        }

        // Sort time_sorted by timestamp
        time_sorted.sort_by_key(|&idx| events[idx].timestamp);

        EventIndex {
            by_protocol,
            by_source,
            by_dest,
            time_sorted,
        }
    }
}

pub struct EventStore {
    events: Vec<DebugEvent>,
    time_range: Option<(Timestamp, Timestamp)>,
    index: Option<EventIndex>,
}

impl EventStore {
    pub fn new(mut events: Vec<DebugEvent>) -> Self {
        events.sort_by_key(|e| e.timestamp);

        let time_range = if events.is_empty() {
            None
        } else {
            Some((events[0].timestamp, events[events.len() - 1].timestamp))
        };

        EventStore {
            events,
            time_range,
            index: None,
        }
    }

    /// Build the index for fast lookups. Should be called in a background task
    /// after initial loading to avoid blocking the UI.
    pub fn build_index(&mut self) {
        self.index = Some(EventIndex::build(&self.events));
    }

    /// Get the index if it has been built.
    pub fn index(&self) -> Option<&EventIndex> {
        self.index.as_ref()
    }

    /// Create an empty event store for live capture mode.
    pub fn empty() -> Self {
        EventStore {
            events: Vec::new(),
            time_range: None,
            index: None,
        }
    }

    /// Append a new event to the store (for live capture).
    ///
    /// Events should be appended in timestamp order for optimal performance,
    /// but out-of-order events are supported (they will be sorted on next filter).
    pub fn push(&mut self, event: DebugEvent) {
        let event_ts = event.timestamp;
        self.events.push(event);

        // Update time range
        self.time_range = match self.time_range {
            None => Some((event_ts, event_ts)),
            Some((start, end)) => Some((start.min(event_ts), end.max(event_ts))),
        };

        // Invalidate index when new events are added
        self.index = None;
    }

    /// Append a batch of events to the store (for streaming file load).
    ///
    /// More efficient than calling push() repeatedly.
    pub fn push_batch(&mut self, mut batch: Vec<DebugEvent>) {
        if batch.is_empty() {
            return;
        }

        // Update time range
        for event in &batch {
            let event_ts = event.timestamp;
            self.time_range = match self.time_range {
                None => Some((event_ts, event_ts)),
                Some((start, end)) => Some((start.min(event_ts), end.max(event_ts))),
            };
        }

        self.events.append(&mut batch);

        // Invalidate index when new events are added
        self.index = None;
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&DebugEvent> {
        self.events.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut DebugEvent> {
        self.events.get_mut(index)
    }

    pub fn events(&self) -> &[DebugEvent] {
        &self.events
    }

    pub fn time_range(&self) -> Option<(Timestamp, Timestamp)> {
        self.time_range
    }

    pub fn filter_indices(&self, filter: &Filter) -> Vec<usize> {
        self.events
            .iter()
            .enumerate()
            .filter(|(_, e)| filter.matches(e))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn all_indices(&self) -> Vec<usize> {
        (0..self.events.len()).collect()
    }

    pub fn protocol_counts(&self, indices: &[usize]) -> Vec<(TransportKind, usize)> {
        let mut counts = std::collections::HashMap::new();
        for &idx in indices {
            if let Some(event) = self.events.get(idx) {
                *counts.entry(event.transport).or_insert(0usize) += 1;
            }
        }
        let mut result: Vec<_> = counts.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        result
    }

    pub fn time_buckets(&self, indices: &[usize], bucket_count: usize) -> Vec<u64> {
        if bucket_count == 0 {
            return vec![];
        }
        let Some((start, end)) = self.time_range else {
            return vec![0; bucket_count];
        };
        let range = end.as_nanos().saturating_sub(start.as_nanos());
        if range == 0 {
            let mut buckets = vec![0u64; bucket_count];
            if !indices.is_empty() {
                buckets[0] = indices.len() as u64;
            }
            return buckets;
        }

        let bucket_width = range / bucket_count as u64;
        let mut buckets = vec![0u64; bucket_count];

        for &idx in indices {
            if let Some(event) = self.events.get(idx) {
                let offset = event.timestamp.as_nanos().saturating_sub(start.as_nanos());
                let bucket = if bucket_width > 0 {
                    (offset / bucket_width).min(bucket_count as u64 - 1) as usize
                } else {
                    0
                };
                buckets[bucket] += 1;
            }
        }

        buckets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::*;
    use std::collections::BTreeMap;

    fn make_event(id: u64, ts_ns: u64, transport: TransportKind) -> DebugEvent {
        DebugEvent {
            id: EventId::from_raw(id),
            timestamp: Timestamp::from_nanos(ts_ns),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            },
            transport,
            direction: Direction::Inbound,
            payload: Payload::Raw {
                raw: Bytes::new(),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        }
    }

    #[test]
    fn store_sorts_by_timestamp() {
        let events = vec![
            make_event(2, 2000, TransportKind::Grpc),
            make_event(1, 1000, TransportKind::Zmq),
            make_event(3, 3000, TransportKind::Grpc),
        ];
        let store = EventStore::new(events);
        assert_eq!(store.get(0).unwrap().id.as_u64(), 1);
        assert_eq!(store.get(2).unwrap().id.as_u64(), 3);
    }

    #[test]
    fn store_time_range() {
        let events = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 5000, TransportKind::Grpc),
        ];
        let store = EventStore::new(events);
        let (start, end) = store.time_range().unwrap();
        assert_eq!(start.as_nanos(), 1000);
        assert_eq!(end.as_nanos(), 5000);
    }

    #[test]
    fn store_protocol_counts() {
        let events = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 2000, TransportKind::Zmq),
            make_event(3, 3000, TransportKind::Grpc),
        ];
        let store = EventStore::new(events);
        let indices = store.all_indices();
        let counts = store.protocol_counts(&indices);
        assert_eq!(counts[0], (TransportKind::Grpc, 2));
        assert_eq!(counts[1], (TransportKind::Zmq, 1));
    }

    #[test]
    fn store_time_buckets() {
        let events = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 3000, TransportKind::Grpc),
            make_event(3, 5000, TransportKind::Grpc),
        ];
        let store = EventStore::new(events);
        let indices = store.all_indices();
        let buckets = store.time_buckets(&indices, 4);
        assert_eq!(buckets.len(), 4);
        let total: u64 = buckets.iter().sum();
        assert_eq!(total, 3);
    }

    #[test]
    fn store_empty() {
        let store = EventStore::new(vec![]);
        assert!(store.is_empty());
        assert!(store.time_range().is_none());
    }
}
