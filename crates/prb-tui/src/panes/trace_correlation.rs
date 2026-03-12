use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;
use crate::trace_extraction::{build_trace_trees, TraceSpan, TraceTree};

/// A tree node for rendering the trace hierarchy.
#[derive(Debug, Clone)]
struct TreeNode {
    /// Span ID.
    span_id: String,
    /// Display name.
    name: String,
    /// Event index.
    event_idx: usize,
    /// Timestamp in nanoseconds.
    timestamp_ns: u64,
    /// Indentation level (0 = root).
    level: usize,
    /// Children span IDs.
    children: Vec<String>,
}

pub struct TraceCorrelationPane {
    /// Cached trace trees from current event store.
    trees: Vec<TraceTree>,
    /// Flattened tree nodes for rendering.
    flat_nodes: Vec<TreeNode>,
    /// Selected node index in flat_nodes.
    selected: usize,
    /// Scroll offset.
    scroll_offset: usize,
    /// Cache generation (tracks when we need to rebuild).
    cache_gen: usize,
}

impl Default for TraceCorrelationPane {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceCorrelationPane {
    pub fn new() -> Self {
        Self {
            trees: Vec::new(),
            flat_nodes: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            cache_gen: 0,
        }
    }

    /// Rebuild the trace trees from the current event store.
    fn rebuild_trees(&mut self, state: &AppState) {
        let events: Vec<&prb_core::DebugEvent> = state.store.events().iter().collect();
        self.trees = build_trace_trees(&events, &state.filtered_indices);
        self.flatten_trees();
    }

    /// Flatten the trace trees into a list of nodes for rendering.
    fn flatten_trees(&mut self) {
        self.flat_nodes.clear();

        // Clone trees to avoid borrow conflict
        let trees = self.trees.clone();

        for tree in &trees {
            // Add a separator for each trace
            if !self.flat_nodes.is_empty() {
                self.flat_nodes.push(TreeNode {
                    span_id: String::new(),
                    name: "---".into(),
                    event_idx: 0,
                    timestamp_ns: 0,
                    level: 0,
                    children: Vec::new(),
                });
            }

            // Render each root span and its children recursively
            for root_id in &tree.root_span_ids {
                if let Some(root_span) = tree.spans.get(root_id) {
                    self.add_span_recursive(tree, root_span, 0);
                }
            }
        }

        // Clamp selection
        if !self.flat_nodes.is_empty() {
            self.selected = self.selected.min(self.flat_nodes.len() - 1);
        } else {
            self.selected = 0;
        }
    }

    /// Recursively add a span and its children to the flat node list.
    fn add_span_recursive(&mut self, tree: &TraceTree, span: &TraceSpan, level: usize) {
        let children = tree.children_of(&span.span_id);
        let child_ids: Vec<String> = children.iter().map(|c| c.span_id.clone()).collect();

        self.flat_nodes.push(TreeNode {
            span_id: span.span_id.clone(),
            name: span.name.clone(),
            event_idx: span.event_idx,
            timestamp_ns: span.timestamp_ns,
            level,
            children: child_ids.clone(),
        });

        // Recursively add children
        for child_span in children {
            self.add_span_recursive(tree, child_span, level + 1);
        }
    }

    /// Ensure the selected item is visible within the scroll window.
    fn ensure_visible(&mut self, visible_height: usize) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected.saturating_sub(visible_height - 1);
        }
    }
}

impl PaneComponent for TraceCorrelationPane {
    fn handle_key(&mut self, key: KeyEvent, _state: &AppState) -> Action {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.flat_nodes.is_empty() {
                    self.selected = (self.selected + 1).min(self.flat_nodes.len() - 1);
                }
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            KeyCode::PageDown => {
                if !self.flat_nodes.is_empty() {
                    self.selected = (self.selected + 10).min(self.flat_nodes.len() - 1);
                }
                Action::None
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(10);
                Action::None
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
                Action::None
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !self.flat_nodes.is_empty() {
                    self.selected = self.flat_nodes.len() - 1;
                }
                Action::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                // Jump to the packet/event associated with this span
                if let Some(node) = self.flat_nodes.get(self.selected)
                    && !node.span_id.is_empty()
                {
                    return Action::SelectEvent(node.event_idx);
                }
                Action::None
            }
            KeyCode::Char('r') => {
                // Force rebuild of trace trees
                self.cache_gen = self.cache_gen.wrapping_add(1);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        state: &AppState,
        theme: &ThemeConfig,
        focused: bool,
    ) {
        let border_style = if focused {
            theme.focused_border()
        } else {
            theme.unfocused_border()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Trace Correlation ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 {
            return;
        }

        // Rebuild trees if needed (simple cache invalidation based on event count)
        let current_gen = state.store.len();
        if current_gen != self.cache_gen {
            self.rebuild_trees(state);
            self.cache_gen = current_gen;
        }

        // Show help message if no traces found
        if self.flat_nodes.is_empty() {
            let msg = if state.store.is_empty() {
                "  Load a capture with trace instrumentation"
            } else {
                "  No traces found in current events"
            };
            let line = Line::styled(msg, Style::default().fg(Color::DarkGray));
            Widget::render(line, inner, buf);
            return;
        }

        // Adjust scroll to keep selection visible
        let visible_height = inner.height as usize;
        self.ensure_visible(visible_height);

        // Render visible nodes
        let start = self.scroll_offset;
        let end = (start + visible_height).min(self.flat_nodes.len());

        for (row_idx, node_idx) in (start..end).enumerate() {
            if let Some(node) = self.flat_nodes.get(node_idx) {
                let y = inner.y + row_idx as u16;
                if y >= inner.y + inner.height {
                    break;
                }

                // Separator row
                if node.span_id.is_empty() {
                    let line = Line::from(vec![Span::styled(
                        "─".repeat(inner.width as usize),
                        Style::default().fg(Color::DarkGray),
                    )]);
                    buf.set_line(inner.x, y, &line, inner.width);
                    continue;
                }

                // Build the tree structure prefix
                let indent = "  ".repeat(node.level);
                let prefix = if node.children.is_empty() {
                    format!("{}├─ ", indent)
                } else {
                    format!("{}├┬ ", indent)
                };

                // Format timestamp
                let ts_str = format_timestamp_short(node.timestamp_ns);

                // Format the span line
                let name_display = if node.name.len() > 30 {
                    format!("{}...", &node.name[..27])
                } else {
                    node.name.clone()
                };

                let mut spans = vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        name_display,
                        Style::default().fg(theme.transport_color(prb_core::TransportKind::Grpc)),
                    ),
                    Span::styled(" ", Style::default()),
                    Span::styled(
                        format!("({})", ts_str),
                        Style::default().fg(Color::DarkGray),
                    ),
                ];

                // Highlight selected row
                if node_idx == self.selected {
                    for span in &mut spans {
                        span.style = span.style.add_modifier(Modifier::REVERSED);
                    }
                }

                let line = Line::from(spans);
                buf.set_line(inner.x, y, &line, inner.width);
            }
        }

        // Show scroll indicator
        if self.flat_nodes.len() > visible_height {
            let status_y = inner.y + inner.height.saturating_sub(1);
            let status_line = Line::from(vec![Span::styled(
                format!(
                    " {}-{} of {} traces ",
                    self.scroll_offset + 1,
                    end,
                    self.flat_nodes.len()
                ),
                Style::default().fg(Color::DarkGray),
            )]);
            buf.set_line(inner.x, status_y, &status_line, inner.width);
        }
    }
}

fn format_timestamp_short(ns: u64) -> String {
    let secs = ns / 1_000_000_000;
    let millis = (ns % 1_000_000_000) / 1_000_000;
    let h = (secs / 3600) % 24;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prb_core::{DebugEvent, Timestamp, EventSource, TransportKind, Direction, Payload, CorrelationKey};

    #[test]
    fn test_trace_correlation_pane_empty() {
        let mut pane = TraceCorrelationPane::new();
        let state = AppState {
            store: crate::event_store::EventStore::new(vec![]),
            filtered_indices: Vec::new(),
            selected_event: None,
            filter: None,
            filter_text: String::new(),
            conversations: None,
            schema_registry: None,
            visible_columns: vec!["timestamp".into(), "transport".into()],
        };

        pane.rebuild_trees(&state);
        assert!(pane.flat_nodes.is_empty());
    }

    #[test]
    fn test_trace_correlation_pane_with_trace() {
        use bytes::Bytes;

        let mut pane = TraceCorrelationPane::new();

        let mut event = DebugEvent::builder()
            .id(prb_core::EventId::next())
            .timestamp(Timestamp::from_nanos(1000))
            .source(EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            })
            .transport(TransportKind::Grpc)
            .direction(Direction::Inbound)
            .payload(Payload::Raw { raw: Bytes::from(vec![]) })
            .build();
        event.correlation_keys.push(CorrelationKey::TraceContext {
            trace_id: "trace1".into(),
            span_id: "span1".into(),
        });

        let store = crate::event_store::EventStore::new(vec![event]);

        let state = AppState {
            store,
            filtered_indices: vec![0],
            selected_event: None,
            filter: None,
            filter_text: String::new(),
            conversations: None,
            schema_registry: None,
            visible_columns: vec!["timestamp".into(), "transport".into()],
        };

        pane.rebuild_trees(&state);
        assert!(!pane.flat_nodes.is_empty());
        assert_eq!(pane.trees.len(), 1);
        assert_eq!(pane.trees[0].spans.len(), 1);
    }
}
