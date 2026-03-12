//! Unit tests for TUI pane business logic (S12).
//! Focus on publicly accessible functionality.

use bytes::Bytes;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_tui::app::AppState;
use prb_tui::event_store::EventStore;
use prb_tui::panes::PaneComponent;
use prb_tui::panes::ai_panel::AiPanel;
use prb_tui::panes::conversation_list::ConversationListPane;
use prb_tui::panes::trace_correlation::TraceCorrelationPane;
use prb_tui::panes::waterfall::WaterfallPane;
use prb_tui::theme::ThemeConfig;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

fn make_test_event(id: u64, timestamp_ns: u64, src: &str, dst: &str) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_ns),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw {
            raw: Bytes::from(vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]),
        },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}

fn make_app_state(events: Vec<DebugEvent>) -> AppState {
    let store = EventStore::new(events);
    AppState {
        filtered_indices: store.all_indices(),
        selected_event: Some(0),
        filter: None,
        filter_text: String::new(),
        schema_registry: None,
        conversations: None,
        store,
        visible_columns: Vec::new(),
    }
}

// ========== Waterfall Pane Tests ==========

#[test]
fn test_waterfall_pane_default() {
    let pane = WaterfallPane::default();
    assert_eq!(pane.selected, 0);
    assert_eq!(pane.scroll_offset, 0);
    assert!(pane.sort_ascending);
}

#[test]
fn test_waterfall_pane_new() {
    let pane = WaterfallPane::new();
    assert_eq!(pane.selected, 0);
    assert_eq!(pane.scroll_offset, 0);
}

#[test]
fn test_waterfall_pane_handles_keys_without_conversations() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = WaterfallPane::new();

    // Should handle keys gracefully even without conversations
    pane.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        &state,
    );
}

#[test]
fn test_waterfall_pane_renders_without_conversations() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = WaterfallPane::new();

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    // Should render without panic
    pane.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

#[test]
fn test_waterfall_pane_renders_small_terminal() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = WaterfallPane::new();

    // Very small terminal
    let area = Rect::new(0, 0, 10, 5);
    let mut buffer = Buffer::empty(area);

    // Should handle gracefully
    pane.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

// ========== Conversation List Pane Tests ==========

#[test]
fn test_conversation_list_pane_default() {
    let pane = ConversationListPane::default();
    assert_eq!(pane.selected, 0);
    assert_eq!(pane.scroll_offset, 0);
    assert!(!pane.sort_reversed);
}

#[test]
fn test_conversation_list_pane_new() {
    let pane = ConversationListPane::new();
    assert_eq!(pane.selected, 0);
    assert_eq!(pane.scroll_offset, 0);
}

#[test]
fn test_conversation_list_pane_handles_keys() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = ConversationListPane::new();

    // Should handle keys without panic
    pane.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        &state,
    );
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        &state,
    );
    pane.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &state);
}

#[test]
fn test_conversation_list_pane_renders() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = ConversationListPane::new();

    let area = Rect::new(0, 0, 120, 24);
    let mut buffer = Buffer::empty(area);

    pane.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

#[test]
fn test_conversation_list_pane_renders_small_terminal() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = ConversationListPane::new();

    let area = Rect::new(0, 0, 40, 10);
    let mut buffer = Buffer::empty(area);

    pane.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

// ========== AI Panel Tests ==========

#[test]
fn test_ai_panel_default() {
    let panel = AiPanel::default();
    assert!(!panel.is_streaming());
}

#[test]
fn test_ai_panel_new() {
    let panel = AiPanel::new();
    assert!(!panel.is_streaming());
}

#[test]
fn test_ai_panel_clear() {
    let mut panel = AiPanel::new();
    panel.clear();
    assert!(!panel.is_streaming());
}

#[test]
fn test_ai_panel_show_anomalies_empty() {
    let mut panel = AiPanel::new();
    panel.show_anomalies(vec![]);

    // Should not panic
    assert!(!panel.is_streaming());
}

#[test]
fn test_ai_panel_show_anomalies_multiple() {
    use prb_tui::ai_smart::{Anomaly, AnomalySeverity};

    let mut panel = AiPanel::new();

    let anomalies = vec![
        Anomaly {
            title: "High Severity".into(),
            description: "Critical issue".into(),
            severity: AnomalySeverity::High,
            event_indices: vec![0, 1, 2],
        },
        Anomaly {
            title: "Medium Severity".into(),
            description: "Warning".into(),
            severity: AnomalySeverity::Medium,
            event_indices: vec![3],
        },
        Anomaly {
            title: "Low Severity".into(),
            description: "Minor issue".into(),
            severity: AnomalySeverity::Low,
            event_indices: vec![],
        },
    ];

    panel.show_anomalies(anomalies);
    assert!(!panel.is_streaming());
}

#[test]
fn test_ai_panel_show_protocol_hints_empty() {
    let mut panel = AiPanel::new();
    panel.show_protocol_hints(vec![]);

    assert!(!panel.is_streaming());
}

#[test]
fn test_ai_panel_show_protocol_hints_multiple() {
    use prb_tui::ai_smart::ProtocolHint;

    let mut panel = AiPanel::new();

    let hints = vec![
        ProtocolHint {
            protocol_name: "gRPC".into(),
            description: "High confidence".into(),
            confidence: 0.95,
        },
        ProtocolHint {
            protocol_name: "REST".into(),
            description: "Possible REST".into(),
            confidence: 0.6,
        },
    ];

    panel.show_protocol_hints(hints);
    assert!(!panel.is_streaming());
}

#[test]
fn test_ai_panel_handles_keys() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut panel = AiPanel::new();

    panel.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &state);
    panel.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    panel.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &state);
    panel.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &state);
    panel.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &state);
}

#[test]
fn test_ai_panel_renders() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut panel = AiPanel::new();

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    panel.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

#[test]
fn test_ai_panel_poll_stream_no_stream() {
    let mut panel = AiPanel::new();

    // Should not panic when polling with no stream
    panel.poll_stream(EventId::from_raw(1));
    assert!(!panel.is_streaming());
}

// ========== Trace Correlation Pane Tests ==========

#[test]
fn test_trace_correlation_pane_default() {
    let _pane = TraceCorrelationPane::default();
    // Public fields not directly accessible, but should construct without panic
}

#[test]
fn test_trace_correlation_pane_new() {
    let _pane = TraceCorrelationPane::new();
    // Should construct without panic
}

#[test]
fn test_trace_correlation_pane_handles_keys() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = TraceCorrelationPane::new();

    pane.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &state);
    pane.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &state);
    pane.handle_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE),
        &state,
    );
}

#[test]
fn test_trace_correlation_pane_renders_empty() {
    let events = vec![];
    let state = make_app_state(events);
    let mut pane = TraceCorrelationPane::new();

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    pane.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

#[test]
fn test_trace_correlation_pane_renders_with_events() {
    use prb_core::CorrelationKey;

    let mut event = make_test_event(1, 1_000_000_000, "10.0.0.1:1234", "10.0.0.2:5678");
    event.correlation_keys.push(CorrelationKey::TraceContext {
        trace_id: "trace1".into(),
        span_id: "span1".into(),
    });

    let state = make_app_state(vec![event]);
    let mut pane = TraceCorrelationPane::new();

    let area = Rect::new(0, 0, 80, 24);
    let mut buffer = Buffer::empty(area);

    pane.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

#[test]
fn test_trace_correlation_pane_renders_small_terminal() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);
    let mut pane = TraceCorrelationPane::new();

    let area = Rect::new(0, 0, 20, 5);
    let mut buffer = Buffer::empty(area);

    pane.render(area, &mut buffer, &state, &ThemeConfig::dark(), true);
}

// ========== Integration Tests ==========

#[test]
fn test_all_panes_handle_common_navigation_keys() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);

    let mut waterfall = WaterfallPane::new();
    let mut conv_list = ConversationListPane::new();
    let mut ai_panel = AiPanel::new();
    let mut trace = TraceCorrelationPane::new();

    // Common navigation keys should not panic on any pane
    let keys = vec![
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
    ];

    for key in keys {
        waterfall.handle_key(key, &state);
        conv_list.handle_key(key, &state);
        ai_panel.handle_key(key, &state);
        trace.handle_key(key, &state);
    }
}

#[test]
fn test_all_panes_render_in_various_sizes() {
    let events = vec![make_test_event(
        1,
        1_000_000_000,
        "10.0.0.1:1234",
        "10.0.0.2:5678",
    )];
    let state = make_app_state(events);

    let sizes = vec![
        Rect::new(0, 0, 120, 40), // Large
        Rect::new(0, 0, 80, 24),  // Standard
        Rect::new(0, 0, 40, 10),  // Small
        Rect::new(0, 0, 10, 3),   // Tiny
    ];

    for size in sizes {
        let mut buffer = Buffer::empty(size);

        let mut waterfall = WaterfallPane::new();
        let mut conv_list = ConversationListPane::new();
        let mut ai_panel = AiPanel::new();
        let mut trace = TraceCorrelationPane::new();

        waterfall.render(size, &mut buffer, &state, &ThemeConfig::dark(), true);
        conv_list.render(size, &mut buffer, &state, &ThemeConfig::dark(), true);
        ai_panel.render(size, &mut buffer, &state, &ThemeConfig::dark(), true);
        trace.render(size, &mut buffer, &state, &ThemeConfig::dark(), true);
    }
}
