//! Unit tests for live.rs

use prb_tui::live::{AppEvent, CaptureState};

#[test]
fn test_capture_state_variants() {
    // Test all CaptureState variants exist
    let capturing = CaptureState::Capturing;
    let paused = CaptureState::Paused;
    let stopped = CaptureState::Stopped;

    assert_eq!(capturing, CaptureState::Capturing);
    assert_eq!(paused, CaptureState::Paused);
    assert_eq!(stopped, CaptureState::Stopped);
}

#[test]
fn test_capture_state_equality() {
    assert_eq!(CaptureState::Capturing, CaptureState::Capturing);
    assert_eq!(CaptureState::Paused, CaptureState::Paused);
    assert_eq!(CaptureState::Stopped, CaptureState::Stopped);

    assert_ne!(CaptureState::Capturing, CaptureState::Paused);
    assert_ne!(CaptureState::Paused, CaptureState::Stopped);
    assert_ne!(CaptureState::Stopped, CaptureState::Capturing);
}

#[test]
fn test_capture_state_clone() {
    let state = CaptureState::Capturing;
    let cloned = state;
    assert_eq!(state, cloned);
}

#[test]
fn test_app_event_key() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let key_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let app_event = AppEvent::Key(key_event);

    match app_event {
        AppEvent::Key(k) => {
            assert_eq!(k.code, KeyCode::Char('q'));
        }
        _ => panic!("Expected Key variant"),
    }
}

#[test]
fn test_app_event_tick() {
    let app_event = AppEvent::Tick;
    match app_event {
        AppEvent::Tick => {}
        _ => panic!("Expected Tick variant"),
    }
}

#[test]
fn test_app_event_resize() {
    let app_event = AppEvent::Resize(80, 24);
    match app_event {
        AppEvent::Resize(w, h) => {
            assert_eq!(w, 80);
            assert_eq!(h, 24);
        }
        _ => panic!("Expected Resize variant"),
    }
}

#[test]
fn test_app_event_capture_stopped() {
    let app_event = AppEvent::CaptureStopped;
    match app_event {
        AppEvent::CaptureStopped => {}
        _ => panic!("Expected CaptureStopped variant"),
    }
}

#[test]
fn test_app_event_captured_event() {
    use bytes::Bytes;
    use prb_core::{DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind};
    use std::collections::BTreeMap;

    let debug_event = DebugEvent {
        id: EventId::from_raw(1),
        timestamp: Timestamp::from_nanos(1_000_000_000),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: None,
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![1, 2, 3]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    };

    let app_event = AppEvent::CapturedEvent(Box::new(debug_event));

    match app_event {
        AppEvent::CapturedEvent(event) => {
            assert_eq!(event.id.as_u64(), 1);
        }
        _ => panic!("Expected CapturedEvent variant"),
    }
}

#[test]
fn test_app_event_capture_stats() {
    use prb_capture::CaptureStats;
    use std::time::Duration;

    let stats = CaptureStats {
        packets_received: 100,
        packets_dropped_kernel: 5,
        packets_dropped_channel: 2,
        bytes_received: 10000,
        capture_duration: Duration::from_secs(10),
        packets_per_second: 10.0,
        bytes_per_second: 1000.0,
    };

    let app_event = AppEvent::CaptureStats(stats);

    match app_event {
        AppEvent::CaptureStats(s) => {
            assert_eq!(s.packets_received, 100);
            assert_eq!(s.packets_dropped_kernel, 5);
            assert_eq!(s.packets_dropped_channel, 2);
            assert_eq!(s.bytes_received, 10000);
            assert_eq!(s.packets_per_second, 10.0);
            assert_eq!(s.bytes_per_second, 1000.0);
        }
        _ => panic!("Expected CaptureStats variant"),
    }
}
