//! Async live capture tests for TUI integration

use bytes::Bytes;
use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
use prb_tui::App;
use prb_tui::event_store::EventStore;
use prb_tui::live::AppEvent;
use std::collections::BTreeMap;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

fn make_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3, 4]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

#[tokio::test]
async fn test_live_capture_event_stream() {
    // Create a channel to simulate live capture
    let (tx, mut rx) = mpsc::channel(100);

    // Create an app with initial empty store
    let store = EventStore::new(vec![]);
    let _app = App::new(store, None, None);

    // Send a test event
    let event1 = make_test_event(1, 1_000_000_000);
    tx.send(AppEvent::CapturedEvent(Box::new(event1.clone())))
        .await
        .unwrap();

    // Process the event from the channel
    if let Some(app_event) = rx.recv().await {
        match app_event {
            AppEvent::CapturedEvent(evt) => {
                // Verify the event was received
                assert_eq!(evt.id, EventId::from_raw(1));
                assert_eq!(evt.timestamp, Timestamp::from_nanos(1_000_000_000));
            }
            _ => panic!("Expected CapturedEvent"),
        }
    }

    drop(tx); // Close the channel
}

#[tokio::test]
async fn test_multiple_events_in_order() {
    let (tx, mut rx) = mpsc::channel(100);

    // Send multiple events
    for i in 1..=5 {
        let event = make_test_event(i, i * 1_000_000_000);
        tx.send(AppEvent::CapturedEvent(Box::new(event)))
            .await
            .unwrap();
    }

    drop(tx);

    // Verify events come through in order
    let mut received_ids = Vec::new();
    while let Some(app_event) = rx.recv().await {
        if let AppEvent::CapturedEvent(evt) = app_event {
            received_ids.push(evt.id.as_u64());
        }
    }

    assert_eq!(received_ids, vec![1, 2, 3, 4, 5]);
}

#[tokio::test]
async fn test_async_channel_capacity() {
    // Test that channel handles backpressure correctly
    let (tx, mut rx) = mpsc::channel(10); // Small capacity

    // Try to send 20 events
    let send_task = tokio::spawn(async move {
        for i in 1..=20 {
            let event = make_test_event(i, i * 1_000_000_000);
            if tx
                .send(AppEvent::CapturedEvent(Box::new(event)))
                .await
                .is_err()
            {
                break;
            }
            // Small delay to simulate real capture timing
            sleep(Duration::from_micros(10)).await;
        }
    });

    // Slowly consume events
    let mut count = 0;
    while let Some(_) = rx.recv().await {
        count += 1;
        sleep(Duration::from_micros(5)).await;
    }

    send_task.await.unwrap();

    // Should have received all 20 events despite small buffer
    assert_eq!(count, 20);
}

#[tokio::test]
async fn test_channel_close_handling() {
    let (tx, mut rx) = mpsc::channel(100);

    // Send some events
    for i in 1..=3 {
        let event = make_test_event(i, i * 1_000_000_000);
        tx.send(AppEvent::CapturedEvent(Box::new(event)))
            .await
            .unwrap();
    }

    // Close the sender
    drop(tx);

    // Receive all events
    let mut count = 0;
    while let Some(_) = rx.recv().await {
        count += 1;
    }

    assert_eq!(count, 3);

    // Channel should be closed now
    assert!(rx.recv().await.is_none());
}

#[tokio::test]
async fn test_capture_stopped_event() {
    let (tx, mut rx) = mpsc::channel(100);

    // Send some events followed by CaptureStopped
    let event1 = make_test_event(1, 1_000_000_000);
    tx.send(AppEvent::CapturedEvent(Box::new(event1)))
        .await
        .unwrap();

    tx.send(AppEvent::CaptureStopped).await.unwrap();

    drop(tx); // Close the sender

    // Verify we receive both events
    let mut received_capture_stopped = false;
    let mut event_count = 0;

    while let Some(app_event) = rx.recv().await {
        match app_event {
            AppEvent::CapturedEvent(_) => event_count += 1,
            AppEvent::CaptureStopped => received_capture_stopped = true,
            _ => {}
        }
    }

    assert_eq!(event_count, 1);
    assert!(received_capture_stopped);
}

#[tokio::test]
async fn test_mixed_event_types() {
    let (tx, mut rx) = mpsc::channel(100);

    // Send various event types
    tx.send(AppEvent::Tick).await.unwrap();
    let event = make_test_event(1, 1_000_000_000);
    tx.send(AppEvent::CapturedEvent(Box::new(event)))
        .await
        .unwrap();
    tx.send(AppEvent::Resize(80, 24)).await.unwrap();
    tx.send(AppEvent::CaptureStopped).await.unwrap();

    drop(tx);

    // Count each event type
    let mut tick_count = 0;
    let mut captured_count = 0;
    let mut resize_count = 0;
    let mut stopped_count = 0;

    while let Some(app_event) = rx.recv().await {
        match app_event {
            AppEvent::Tick => tick_count += 1,
            AppEvent::CapturedEvent(_) => captured_count += 1,
            AppEvent::Resize(_, _) => resize_count += 1,
            AppEvent::CaptureStopped => stopped_count += 1,
            _ => {}
        }
    }

    assert_eq!(tick_count, 1);
    assert_eq!(captured_count, 1);
    assert_eq!(resize_count, 1);
    assert_eq!(stopped_count, 1);
}

#[tokio::test]
async fn test_rapid_event_stream() {
    // Test handling of rapid event arrival (stress test)
    let (tx, mut rx) = mpsc::channel(1000);

    // Spawn task to send 100 events rapidly
    let send_task = tokio::spawn(async move {
        for i in 1..=100 {
            let event = make_test_event(i, i * 1_000_000);
            tx.send(AppEvent::CapturedEvent(Box::new(event)))
                .await
                .unwrap();
        }
    });

    // Receive all events
    let mut count = 0;
    let mut last_id = 0u64;

    // Use timeout to prevent hanging
    let receive_task = tokio::spawn(async move {
        while let Some(app_event) = rx.recv().await {
            if let AppEvent::CapturedEvent(evt) = app_event {
                count += 1;
                let current_id = evt.id.as_u64();
                // Verify ordering
                assert!(
                    current_id > last_id,
                    "Events out of order: {} after {}",
                    current_id,
                    last_id
                );
                last_id = current_id;
            }
        }
        count
    });

    send_task.await.unwrap();
    let received_count = tokio::time::timeout(Duration::from_secs(5), receive_task)
        .await
        .expect("Receive task timed out")
        .unwrap();

    assert_eq!(received_count, 100);
}
