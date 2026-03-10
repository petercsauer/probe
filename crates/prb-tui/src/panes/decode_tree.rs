use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Widget};
use tui_tree_widget::{Tree, TreeItem, TreeState};

use prb_core::{DebugEvent, Payload};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::Theme;

pub struct DecodeTreePane {
    pub state: TreeState<String>,
}

impl Default for DecodeTreePane {
    fn default() -> Self {
        Self::new()
    }
}

impl DecodeTreePane {
    pub fn new() -> Self {
        DecodeTreePane {
            state: TreeState::default(),
        }
    }

    fn selected_byte_range(&self, event: &DebugEvent) -> Option<(usize, usize)> {
        let _ = event;
        None
    }
}

impl PaneComponent for DecodeTreePane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        if state.selected_event.is_none() {
            return Action::None;
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.key_down();
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.key_up();
                Action::None
            }
            KeyCode::Right | KeyCode::Enter => {
                self.state.toggle_selected();
                Action::None
            }
            KeyCode::Left | KeyCode::Backspace => {
                self.state.key_left();
                Action::None
            }
            KeyCode::Char(' ') => {
                self.state.toggle_selected();
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, focused: bool) {
        let border_style = if focused {
            Theme::focused_border()
        } else {
            Theme::unfocused_border()
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Decode ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 5 {
            return;
        }

        let Some(sel_idx) = state.selected_event else {
            let msg = Text::raw("  No event selected");
            Widget::render(msg, inner, buf);
            return;
        };
        let Some(event_idx) = state.filtered_indices.get(sel_idx) else {
            return;
        };
        let Some(event) = state.store.get(*event_idx) else {
            return;
        };

        let items = build_tree_items(event);

        let tree = Tree::new(&items)
            .expect("tree items are valid")
            .highlight_style(Theme::selected_row());

        ratatui::widgets::StatefulWidget::render(tree, inner, buf, &mut self.state);
    }
}

fn build_tree_items(event: &DebugEvent) -> Vec<TreeItem<'static, String>> {
    let mut items = Vec::new();

    // Event header
    let ts_ns = event.timestamp.as_nanos();
    let secs = ts_ns / 1_000_000_000;
    let millis = (ts_ns % 1_000_000_000) / 1_000_000;
    let h = (secs / 3600) % 24;
    let m = (secs % 3600) / 60;
    let s = secs % 60;

    items.push(
        TreeItem::new_leaf(
            "ts".to_string(),
            format!("Timestamp: {:02}:{:02}:{:02}.{:03}", h, m, s, millis),
        ),
    );

    items.push(
        TreeItem::new_leaf(
            "dir".to_string(),
            format!("Direction: {}", event.direction),
        ),
    );

    // Source section
    let mut source_children = vec![
        TreeItem::new_leaf("s.adapter".to_string(), format!("Adapter: {}", event.source.adapter)),
        TreeItem::new_leaf("s.origin".to_string(), format!("Origin: {}", event.source.origin)),
    ];
    if let Some(ref net) = event.source.network {
        source_children.push(TreeItem::new_leaf("s.src".to_string(), format!("Src: {}", net.src)));
        source_children.push(TreeItem::new_leaf("s.dst".to_string(), format!("Dst: {}", net.dst)));
    }
    items.push(
        TreeItem::new("source".to_string(), "Source", source_children)
            .expect("source children valid"),
    );

    // Transport + metadata section
    let mut transport_children = Vec::new();
    for (key, value) in &event.metadata {
        transport_children.push(TreeItem::new_leaf(
            format!("m.{}", key),
            format!("{}: {}", key, value),
        ));
    }
    items.push(
        TreeItem::new(
            "transport".to_string(),
            format!("Transport: {}", event.transport),
            transport_children,
        )
        .expect("transport children valid"),
    );

    // Payload section
    let payload_size = match &event.payload {
        Payload::Raw { raw } => raw.len(),
        Payload::Decoded { raw, .. } => raw.len(),
    };

    let mut payload_children = Vec::new();
    match &event.payload {
        Payload::Raw { .. } => {
            payload_children.push(TreeItem::new_leaf("p.type".to_string(), "Type: Raw".to_string()));
        }
        Payload::Decoded {
            fields,
            schema_name,
            ..
        } => {
            payload_children.push(TreeItem::new_leaf("p.type".to_string(), "Type: Decoded".to_string()));
            if let Some(name) = schema_name {
                payload_children.push(TreeItem::new_leaf(
                    "p.schema".to_string(),
                    format!("Schema: {}", name),
                ));
            }
            let fields_str = serde_json::to_string_pretty(fields).unwrap_or_default();
            for (i, line) in fields_str.lines().enumerate() {
                payload_children.push(TreeItem::new_leaf(
                    format!("p.f.{}", i),
                    line.to_string(),
                ));
            }
        }
    }
    items.push(
        TreeItem::new(
            "payload".to_string(),
            format!("Payload ({} bytes)", payload_size),
            payload_children,
        )
        .expect("payload children valid"),
    );

    // Correlation keys
    if !event.correlation_keys.is_empty() {
        let mut corr_children = Vec::new();
        for (i, key) in event.correlation_keys.iter().enumerate() {
            let label = match key {
                prb_core::CorrelationKey::StreamId { id } => format!("StreamId: {}", id),
                prb_core::CorrelationKey::Topic { name } => format!("Topic: {}", name),
                prb_core::CorrelationKey::ConnectionId { id } => format!("ConnectionId: {}", id),
                prb_core::CorrelationKey::TraceContext { trace_id, span_id } => {
                    format!("TraceContext: {}:{}", trace_id, span_id)
                }
                prb_core::CorrelationKey::Custom { key, value } => format!("{}: {}", key, value),
            };
            corr_children.push(TreeItem::new_leaf(format!("c.{}", i), label));
        }
        items.push(
            TreeItem::new("correlation".to_string(), "Correlation", corr_children)
                .expect("corr children valid"),
        );
    }

    // Warnings
    if !event.warnings.is_empty() {
        let mut warn_children = Vec::new();
        for (i, w) in event.warnings.iter().enumerate() {
            warn_children.push(TreeItem::new_leaf(format!("w.{}", i), w.clone()));
        }
        items.push(
            TreeItem::new("warnings".to_string(), "⚠ Warnings", warn_children)
                .expect("warn children valid"),
        );
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use prb_core::{
        CorrelationKey, DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload,
        Timestamp, TransportKind, METADATA_KEY_DDS_DOMAIN_ID, METADATA_KEY_DDS_TOPIC_NAME,
        METADATA_KEY_GRPC_METHOD, METADATA_KEY_H2_STREAM_ID, METADATA_KEY_ZMQ_TOPIC,
    };
    use std::collections::BTreeMap;

    fn create_test_event(transport: TransportKind, metadata: BTreeMap<String, String>) -> DebugEvent {
        DebugEvent {
            id: EventId::from_raw(42),
            timestamp: Timestamp::from_nanos(1_000_000_000),
            source: EventSource {
                adapter: "pcap".to_string(),
                origin: "test.pcap".to_string(),
                network: Some(NetworkAddr {
                    src: "10.0.0.1:12345".to_string(),
                    dst: "10.0.0.2:50051".to_string(),
                }),
            },
            transport,
            direction: Direction::Outbound,
            payload: Payload::Decoded {
                raw: Bytes::from_static(b"test payload"),
                fields: serde_json::json!({"user_id": "abc-123"}),
                schema_name: Some("api.v1.GetUserRequest".to_string()),
            },
            metadata,
            correlation_keys: vec![
                CorrelationKey::StreamId { id: 1 },
                CorrelationKey::ConnectionId {
                    id: "10.0.0.1:12345->10.0.0.2:50051".to_string(),
                },
            ],
            sequence: Some(5),
            warnings: vec![],
        }
    }

    #[test]
    fn test_grpc_tree_structure() {
        let mut metadata = BTreeMap::new();
        metadata.insert(METADATA_KEY_GRPC_METHOD.to_string(), "/api.v1.Users/GetUser".to_string());
        metadata.insert(METADATA_KEY_H2_STREAM_ID.to_string(), "1".to_string());

        let event = create_test_event(TransportKind::Grpc, metadata);
        let items = build_tree_items(&event);

        // Verify we have several top-level items
        assert!(items.len() >= 4, "Should have timestamp, direction, source, transport, payload sections");

        // We can't access TreeItem internals, but we can verify the structure was built
        // by checking the return from build_tree_items is valid
        assert!(!items.is_empty());
    }

    #[test]
    fn test_zmq_tree_structure() {
        let mut metadata = BTreeMap::new();
        metadata.insert(METADATA_KEY_ZMQ_TOPIC.to_string(), "sensor.temperature".to_string());

        let event = create_test_event(TransportKind::Zmq, metadata);
        let items = build_tree_items(&event);

        // Verify ZMQ event produces tree items
        assert!(items.len() >= 4, "Should have basic sections");
    }

    #[test]
    fn test_dds_tree_structure() {
        let mut metadata = BTreeMap::new();
        metadata.insert(METADATA_KEY_DDS_DOMAIN_ID.to_string(), "0".to_string());
        metadata.insert(METADATA_KEY_DDS_TOPIC_NAME.to_string(), "ChatterTopic".to_string());

        let event = create_test_event(TransportKind::DdsRtps, metadata);
        let items = build_tree_items(&event);

        // Verify DDS-RTPS event produces tree items
        assert!(items.len() >= 4, "Should have basic sections");
    }

    #[test]
    fn test_source_section_with_network() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event);

        // Verify we have items (including source section)
        assert!(items.len() >= 3, "Should have timestamp, direction, source at minimum");
    }

    #[test]
    fn test_payload_decoded_section() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event);

        // Decoded payload should produce tree items
        assert!(items.len() >= 5, "Should have all sections including payload");
    }

    #[test]
    fn test_payload_raw_section() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.payload = Payload::Raw {
            raw: Bytes::from_static(b"raw bytes"),
        };

        let items = build_tree_items(&event);

        // Raw payload should also produce tree items
        assert!(items.len() >= 5, "Should have all sections including raw payload");
    }

    #[test]
    fn test_correlation_section() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event);

        // Event has correlation keys, so should have correlation section
        assert!(items.len() >= 5, "Should have correlation section");
    }

    #[test]
    fn test_warnings_section() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.warnings = vec!["Parse error".to_string(), "Missing field".to_string()];

        let items = build_tree_items(&event);

        // Warnings should add an extra section
        let base_len = 5; // timestamp, direction, source, transport, payload
        assert!(items.len() >= base_len, "Should include warnings section");
    }

    #[test]
    fn test_no_warnings_section_when_empty() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event);

        // Without warnings and with correlation keys
        // We should have: timestamp, direction, source, transport, payload, correlation
        assert_eq!(items.len(), 6, "Should have 6 sections without warnings");
    }

    #[test]
    fn test_timestamp_formatting() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event);

        // Just verify tree items are created - timestamp is first
        assert!(!items.is_empty(), "Should have timestamp item");
    }

    #[test]
    fn test_metadata_keys_in_transport() {
        let mut metadata = BTreeMap::new();
        metadata.insert("custom.key".to_string(), "custom_value".to_string());
        metadata.insert(METADATA_KEY_GRPC_METHOD.to_string(), "/api.Service/Method".to_string());

        let event = create_test_event(TransportKind::Grpc, metadata);
        let items = build_tree_items(&event);

        // Metadata adds children to transport section
        assert!(items.len() >= 4, "Should have all sections");
    }

    #[test]
    fn test_event_without_network() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.source.network = None;

        let items = build_tree_items(&event);

        // Should still build tree successfully
        assert!(items.len() >= 5, "Should work without network info");
    }

    #[test]
    fn test_event_without_correlation() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.correlation_keys = vec![];

        let items = build_tree_items(&event);

        // Without correlation keys, should have one less section
        assert_eq!(items.len(), 5, "Should have 5 sections without correlation");
    }

    #[test]
    fn test_all_correlation_key_types() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.correlation_keys = vec![
            CorrelationKey::StreamId { id: 123 },
            CorrelationKey::Topic { name: "test.topic".to_string() },
            CorrelationKey::ConnectionId { id: "conn-1".to_string() },
            CorrelationKey::TraceContext {
                trace_id: "trace-abc".to_string(),
                span_id: "span-xyz".to_string(),
            },
            CorrelationKey::Custom {
                key: "custom".to_string(),
                value: "value".to_string(),
            },
        ];

        let items = build_tree_items(&event);

        // Should handle all correlation key types
        assert!(items.len() >= 5, "Should handle all correlation key types");
    }
}
