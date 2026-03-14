use crate::query_planner::{QueryPlan, QueryPlanner};
use prb_core::{DebugEvent, Timestamp, TransportKind};
use prb_query::Filter;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Index structure for fast lookups by protocol, source, and destination.
#[derive(Debug, Clone)]
pub struct EventIndex {
    pub by_protocol: HashMap<TransportKind, Vec<usize>>,
    pub by_source: HashMap<String, Vec<usize>>,
    pub by_dest: HashMap<String, Vec<usize>>,
    pub time_sorted: Vec<usize>,
}

/// Cache for incremental filtering.
/// Tracks which events have been checked against the current filter.
#[derive(Debug, Clone)]
struct FilterCache {
    /// The filter that was applied
    filter_hash: u64,
    /// Index of the last event that was checked
    last_checked: usize,
    /// Indices of events that match the filter
    matches: Vec<usize>,
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
    filter_cache: Option<FilterCache>,
    planner: QueryPlanner,
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
            filter_cache: None,
            planner: QueryPlanner::new(),
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
            filter_cache: None,
            planner: QueryPlanner::new(),
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

        // Note: filter_cache is not invalidated here - incremental filtering
        // will handle the new event on next filter_indices_incremental call
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

        // Note: filter_cache is not invalidated here - incremental filtering
        // will handle the new events on next filter_indices_incremental call
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

    /// Incrementally filter events, only checking new events since last call.
    /// This is much faster than re-filtering everything when events are streamed in.
    pub fn filter_indices_incremental(&mut self, filter: &Filter) -> Vec<usize> {
        // Compute hash of the filter
        let mut hasher = DefaultHasher::new();
        format!("{:?}", filter).hash(&mut hasher);
        let filter_hash = hasher.finish();

        // Check if we have a valid cache for this filter
        let mut cache = if let Some(ref cache) = self.filter_cache {
            if cache.filter_hash == filter_hash && cache.last_checked <= self.events.len() {
                // Valid cache - we can do incremental filtering
                cache.clone()
            } else {
                // Filter changed - start fresh
                FilterCache {
                    filter_hash,
                    last_checked: 0,
                    matches: Vec::new(),
                }
            }
        } else {
            // No cache - start fresh
            FilterCache {
                filter_hash,
                last_checked: 0,
                matches: Vec::new(),
            }
        };

        // Filter only new events since last check
        for idx in cache.last_checked..self.events.len() {
            if filter.matches(&self.events[idx]) {
                cache.matches.push(idx);
            }
        }

        // Update last_checked to current event count
        cache.last_checked = self.events.len();

        // Store the updated cache
        let result = cache.matches.clone();
        self.filter_cache = Some(cache);

        result
    }

    /// Clear the filter cache, forcing a full re-filter on next call.
    pub fn clear_filter_cache(&mut self) {
        self.filter_cache = None;
    }

    /// Apply a filter using the query planner for optimized execution.
    ///
    /// This method parses and caches the filter, generates an execution plan,
    /// and uses indices when possible for faster filtering.
    pub fn apply_filter_with_plan(&mut self, filter_str: &str) -> Result<Vec<usize>, String> {
        // Parse and cache the filter
        let filter = self
            .planner
            .parse_filter(filter_str)
            .map_err(|e| format!("Parse error: {}", e))?;

        // If filter is empty, return all indices
        if filter.source().trim().is_empty() {
            return Ok(self.all_indices());
        }

        // Get the expression from the filter
        let Some(expr) = filter.expr() else {
            // Empty expression - return all indices
            return Ok(self.all_indices());
        };

        // Generate query plan
        let plan = self.planner.plan(expr);

        // Execute the plan
        self.execute_plan(&plan, &filter)
    }

    /// Execute a query plan using the appropriate strategy.
    fn execute_plan(&self, plan: &QueryPlan, filter: &Filter) -> Result<Vec<usize>, String> {
        match plan {
            QueryPlan::IndexedByTransport { kind, remaining } => {
                // Get candidates from transport index if available
                let candidates = if let Some(ref index) = self.index {
                    index.by_protocol.get(kind).cloned().unwrap_or_default()
                } else {
                    // No index available, fall back to full scan
                    return Ok(self.filter_indices(filter));
                };

                // Apply remaining filter if present
                if let Some(remaining_expr) = remaining {
                    Ok(candidates
                        .into_iter()
                        .filter(|&idx| {
                            if let Some(event) = self.events.get(idx) {
                                prb_query::eval::eval(remaining_expr, event)
                            } else {
                                false
                            }
                        })
                        .collect())
                } else {
                    Ok(candidates)
                }
            }

            QueryPlan::IndexedBySrc { addr, remaining } => {
                // Get candidates from source index if available
                let candidates = if let Some(ref index) = self.index {
                    index.by_source.get(addr).cloned().unwrap_or_default()
                } else {
                    // No index available, fall back to full scan
                    return Ok(self.filter_indices(filter));
                };

                // Apply remaining filter if present
                if let Some(remaining_expr) = remaining {
                    Ok(candidates
                        .into_iter()
                        .filter(|&idx| {
                            if let Some(event) = self.events.get(idx) {
                                prb_query::eval::eval(remaining_expr, event)
                            } else {
                                false
                            }
                        })
                        .collect())
                } else {
                    Ok(candidates)
                }
            }

            QueryPlan::IndexedByDst { addr, remaining } => {
                // Get candidates from destination index if available
                let candidates = if let Some(ref index) = self.index {
                    index.by_dest.get(addr).cloned().unwrap_or_default()
                } else {
                    // No index available, fall back to full scan
                    return Ok(self.filter_indices(filter));
                };

                // Apply remaining filter if present
                if let Some(remaining_expr) = remaining {
                    Ok(candidates
                        .into_iter()
                        .filter(|&idx| {
                            if let Some(event) = self.events.get(idx) {
                                prb_query::eval::eval(remaining_expr, event)
                            } else {
                                false
                            }
                        })
                        .collect())
                } else {
                    Ok(candidates)
                }
            }

            QueryPlan::FullScan(_) => {
                // Fall back to full scan
                Ok(self.filter_indices(filter))
            }
        }
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
            payload: Payload::Raw { raw: Bytes::new() },
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

    #[test]
    fn incremental_filtering_basic() {
        let events = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 2000, TransportKind::Zmq),
            make_event(3, 3000, TransportKind::Grpc),
        ];
        let mut store = EventStore::new(events);

        let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
        let filtered = store.filter_indices_incremental(&filter);

        // Should match 2 gRPC events
        assert_eq!(filtered.len(), 2);
        assert_eq!(
            store.get(filtered[0]).unwrap().transport,
            TransportKind::Grpc
        );
        assert_eq!(
            store.get(filtered[1]).unwrap().transport,
            TransportKind::Grpc
        );
    }

    #[test]
    fn incremental_filtering_with_new_events() {
        let initial_events = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 2000, TransportKind::Zmq),
        ];
        let mut store = EventStore::new(initial_events);

        let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();

        // First filter - should find 1 match
        let filtered1 = store.filter_indices_incremental(&filter);
        assert_eq!(filtered1.len(), 1);

        // Add more events
        store.push(make_event(3, 3000, TransportKind::Grpc));
        store.push(make_event(4, 4000, TransportKind::Zmq));
        store.push(make_event(5, 5000, TransportKind::Grpc));

        // Second filter - should incrementally add new matches
        let filtered2 = store.filter_indices_incremental(&filter);
        assert_eq!(filtered2.len(), 3);

        // Verify all matches are gRPC
        for idx in &filtered2 {
            assert_eq!(store.get(*idx).unwrap().transport, TransportKind::Grpc);
        }
    }

    #[test]
    fn incremental_filtering_filter_change() {
        let events = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 2000, TransportKind::Zmq),
            make_event(3, 3000, TransportKind::Grpc),
        ];
        let mut store = EventStore::new(events);

        // Apply first filter
        let filter1 = Filter::parse(r#"transport == "gRPC""#).unwrap();
        let filtered1 = store.filter_indices_incremental(&filter1);
        assert_eq!(filtered1.len(), 2);

        // Change filter - should start fresh
        let filter2 = Filter::parse(r#"transport == "ZMQ""#).unwrap();
        let filtered2 = store.filter_indices_incremental(&filter2);
        assert_eq!(filtered2.len(), 1);
        assert_eq!(
            store.get(filtered2[0]).unwrap().transport,
            TransportKind::Zmq
        );
    }

    #[test]
    fn incremental_filtering_with_batches() {
        let mut store = EventStore::empty();

        let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();

        // Add first batch
        let batch1 = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 2000, TransportKind::Zmq),
        ];
        store.push_batch(batch1);
        let filtered1 = store.filter_indices_incremental(&filter);
        assert_eq!(filtered1.len(), 1);

        // Add second batch
        let batch2 = vec![
            make_event(3, 3000, TransportKind::Grpc),
            make_event(4, 4000, TransportKind::Grpc),
            make_event(5, 5000, TransportKind::Zmq),
        ];
        store.push_batch(batch2);
        let filtered2 = store.filter_indices_incremental(&filter);
        assert_eq!(filtered2.len(), 3);

        // All matches should be gRPC
        for idx in &filtered2 {
            assert_eq!(store.get(*idx).unwrap().transport, TransportKind::Grpc);
        }
    }

    #[test]
    fn test_index_building() {
        let events = vec![
            make_event(1, 1000, TransportKind::Grpc),
            make_event(2, 2000, TransportKind::Zmq),
            make_event(3, 3000, TransportKind::Grpc),
        ];
        let mut store = EventStore::new(events);

        // Index should not exist initially
        assert!(store.index().is_none());

        // Build index
        store.build_index();

        // Index should exist now
        assert!(store.index().is_some());

        let index = store.index().unwrap();

        // Check protocol index
        assert_eq!(
            index.by_protocol.get(&TransportKind::Grpc).unwrap().len(),
            2
        );
        assert_eq!(index.by_protocol.get(&TransportKind::Zmq).unwrap().len(), 1);

        // Check time_sorted
        assert_eq!(index.time_sorted.len(), 3);
    }

    #[test]
    fn test_large_dataset_performance() {
        // Create 10K events to test performance
        let events: Vec<_> = (0..10000)
            .map(|i| {
                make_event(
                    i,
                    1000 * i,
                    if i % 3 == 0 {
                        TransportKind::Grpc
                    } else if i % 3 == 1 {
                        TransportKind::Zmq
                    } else {
                        TransportKind::DdsRtps
                    },
                )
            })
            .collect();

        let mut store = EventStore::new(events);

        // Build index
        store.build_index();
        assert!(store.index().is_some());

        // Test incremental filtering on large dataset
        let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
        let start = std::time::Instant::now();
        let filtered = store.filter_indices_incremental(&filter);
        let duration = start.elapsed();

        // Should find ~3333 events (1/3 of 10K)
        assert!((filtered.len() as f64 - 3333.0).abs() < 10.0);

        // Filtering 10K events should be fast (< 50ms for incremental first pass)
        assert!(duration.as_millis() < 50, "Filtering took {:?}", duration);

        // Add more events and test incremental performance
        for i in 10000..10100 {
            store.push(make_event(
                i,
                1000 * i,
                if i % 3 == 0 {
                    TransportKind::Grpc
                } else {
                    TransportKind::Zmq
                },
            ));
        }

        let start = std::time::Instant::now();
        let filtered2 = store.filter_indices_incremental(&filter);
        let duration2 = start.elapsed();

        // Should have added ~33 more matches
        assert!(filtered2.len() > filtered.len());
        assert!(filtered2.len() - filtered.len() <= 34);

        // Incremental filtering should be very fast (< 5ms for 100 new events)
        assert!(
            duration2.as_millis() < 5,
            "Incremental filtering took {:?}",
            duration2
        );
    }
}
