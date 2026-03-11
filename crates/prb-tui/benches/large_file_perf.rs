use bytes::Bytes;
use prb_core::{DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind};
use prb_query::Filter;
use prb_tui::{EventStore, panes::event_list::{EventListPane, SortColumn}};
use prb_tui::app::AppState;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

fn make_event(
    id: u64,
    ts_ns: u64,
    transport: TransportKind,
    direction: Direction,
    src: &str,
    dst: &str,
) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(ts_ns),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        },
        transport,
        direction,
        payload: Payload::Raw {
            raw: Bytes::new(),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

fn make_app_state(events: Vec<DebugEvent>) -> AppState {
    let mut store = EventStore::new(events);
    // Build index for realistic performance testing
    store.build_index();
    let filtered_indices = store.all_indices();
    AppState {
        filtered_indices,
        selected_event: if store.is_empty() { None } else { Some(0) },
        filter: None,
        filter_text: String::new(),
        store,
        schema_registry: None,
        conversations: None,
        visible_columns: Vec::new(),
    }
}

fn benchmark_filter_100k() {
    println!("Benchmark: Filter 100K events");

    // Create 100K events
    let events: Vec<_> = (0..100_000)
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
                if i % 2 == 0 {
                    Direction::Inbound
                } else {
                    Direction::Outbound
                },
                &format!("192.168.1.{}:{}", i % 256, 8080 + (i % 100)),
                &format!("10.0.0.{}:{}", i % 256, 9090 + (i % 100)),
            )
        })
        .collect();

    let state = make_app_state(events);

    // Benchmark: Filter by transport (should use index when available)
    let filter = Filter::parse(r#"transport == "gRPC""#).unwrap();
    let start = Instant::now();
    let filtered = state.store.filter_indices(&filter);
    let duration = start.elapsed();

    println!("  Filter time: {:?} (filtered to {} events)", duration, filtered.len());
    assert!(duration < Duration::from_millis(100), "Filter should complete in <100ms, took {:?}", duration);
}

fn benchmark_sort_100k() {
    println!("Benchmark: Sort 100K events");

    let events: Vec<_> = (0..100_000)
        .map(|i| {
            make_event(
                i,
                1000 * (100_000 - i), // Reverse order to test sorting
                TransportKind::Grpc,
                Direction::Inbound,
                &format!("192.168.1.{}:8080", i % 256),
                &format!("10.0.0.{}:9090", i % 256),
            )
        })
        .collect();

    let state = make_app_state(events);
    let mut pane = EventListPane::new();
    pane.sort_column = SortColumn::Time;

    // First call builds the sorted view
    let start = Instant::now();
    let sorted_len = pane.sorted_indices(&state).len();
    let duration = start.elapsed();

    println!("  Initial sort time: {:?} ({} events)", duration, sorted_len);
    assert!(duration < Duration::from_millis(500), "Sort should complete in <500ms, took {:?}", duration);

    // Second call should use cache
    let start = Instant::now();
    let sorted2_len = pane.sorted_indices(&state).len();
    let cached_duration = start.elapsed();

    println!("  Cached sort time: {:?}", cached_duration);
    assert!(cached_duration < Duration::from_millis(1), "Cached sort should complete in <1ms, took {:?}", cached_duration);
    assert_eq!(sorted_len, sorted2_len);
}

fn benchmark_render_100k() {
    println!("Benchmark: Virtual scroll render with 100K events");

    let events: Vec<_> = (0..100_000)
        .map(|i| {
            make_event(
                i,
                1000 * i,
                TransportKind::Grpc,
                Direction::Inbound,
                &format!("192.168.1.{}:8080", i % 256),
                &format!("10.0.0.{}:9090", i % 256),
            )
        })
        .collect();

    let state = make_app_state(events);
    let mut pane = EventListPane::new();

    // Simulate rendering at different scroll positions
    let positions = vec![0, 25_000, 50_000, 75_000, 99_000];

    for pos in positions {
        pane.selected = pos;
        pane.scroll_offset = pos;

        let start = Instant::now();
        // Getting sorted indices is what happens during render
        let sorted = pane.sorted_indices(&state);
        let duration = start.elapsed();

        println!("  Render at position {}: {:?}", pos, duration);
        assert!(duration < Duration::from_millis(16), "Frame render should complete in <16ms (60fps), took {:?}", duration);
        assert!(sorted.len() > 0);
    }
}

fn benchmark_protocol_counts() {
    println!("Benchmark: Protocol counts with 100K events");

    let events: Vec<_> = (0..100_000)
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
                Direction::Inbound,
                "192.168.1.1:8080",
                "10.0.0.1:9090",
            )
        })
        .collect();

    let state = make_app_state(events);
    let indices = state.store.all_indices();

    let start = Instant::now();
    let counts = state.store.protocol_counts(&indices);
    let duration = start.elapsed();

    println!("  Protocol counts time: {:?}", duration);
    println!("  Protocol distribution: {:?}", counts);
    assert!(duration < Duration::from_millis(50), "Protocol counts should complete in <50ms, took {:?}", duration);
    assert_eq!(counts.len(), 3); // 3 protocols
}

fn main() {
    println!("\n=== Large File Performance Benchmarks ===\n");

    benchmark_filter_100k();
    println!();

    benchmark_sort_100k();
    println!();

    benchmark_render_100k();
    println!();

    benchmark_protocol_counts();
    println!();

    println!("=== All benchmarks passed! ===\n");
}
