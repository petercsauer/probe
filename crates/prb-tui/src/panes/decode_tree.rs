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
