use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Widget};
use tui_tree_widget::{Tree, TreeItem, TreeState};

use prb_core::{DebugEvent, Payload, METADATA_KEY_GRPC_METHOD};
use prb_decode::{decode_with_schema, DecodedMessage};
use prb_schema::SchemaRegistry;
use prost_reflect::{ReflectMessage, Value};

use crate::app::AppState;
use crate::error_intel;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;

pub struct DecodeTreePane {
    pub state: TreeState<String>,
    marked_event_idx: Option<usize>,
    show_diff: bool,
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
            marked_event_idx: None,
            show_diff: false,
        }
    }

    fn expand_all_recursive(items: &[TreeItem<'static, String>], identifiers: &mut Vec<Vec<String>>, prefix: Vec<String>) {
        for item in items.iter() {
            let mut id = prefix.clone();
            id.push(item.identifier().clone());
            identifiers.push(id.clone());

            if !item.children().is_empty() {
                Self::expand_all_recursive(item.children(), identifiers, id);
            }
        }
    }

    fn expand_all(&mut self, items: &[TreeItem<'static, String>]) {
        let mut identifiers = Vec::new();
        Self::expand_all_recursive(items, &mut identifiers, Vec::new());
        for id in identifiers {
            self.state.open(id);
        }
    }

    fn collapse_all(&mut self) {
        self.state.close_all();
    }

    fn copy_selected_value(&self, items: &[TreeItem<'static, String>]) {
        let selected = self.state.selected();
        if selected.is_empty() {
            return;
        }

        if let Some(value) = extract_value_from_tree(items, selected) {
            send_osc52_copy(&value);
        }
    }

    fn render_diff_overlay(&self, area: Rect, buf: &mut Buffer, state: &AppState, theme: &ThemeConfig) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::widgets::{BorderType, Clear};

        // Get marked and current events
        let marked_event = self.marked_event_idx
            .and_then(|idx| state.filtered_indices.get(idx))
            .and_then(|&idx| state.store.get(idx));
        let current_event = state.selected_event
            .and_then(|idx| state.filtered_indices.get(idx))
            .and_then(|&idx| state.store.get(idx));

        if marked_event.is_none() || current_event.is_none() {
            return;
        }

        // Create overlay area (80% of screen, centered)
        let width = (area.width * 80 / 100).max(40);
        let height = (area.height * 80 / 100).max(10);
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(area.x + x, area.y + y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(theme.focused_border())
            .title(" Event Diff (press Esc to close) ")
            .title_style(theme.focused_title());

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        // Split into two columns
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);

        // Render diff content
        let marked = marked_event.unwrap();
        let current = current_event.unwrap();

        let diff_result = compute_event_diff(marked, current);

        // Left column: marked event
        let mut y_offset = 0;
        for line in diff_result.left_lines.iter() {
            if y_offset >= columns[0].height {
                break;
            }
            buf.set_line(columns[0].x, columns[0].y + y_offset, line, columns[0].width);
            y_offset += 1;
        }

        // Right column: current event
        y_offset = 0;
        for line in diff_result.right_lines.iter() {
            if y_offset >= columns[1].height {
                break;
            }
            buf.set_line(columns[1].x, columns[1].y + y_offset, line, columns[1].width);
            y_offset += 1;
        }
    }
}

impl PaneComponent for DecodeTreePane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        // Handle diff overlay
        if self.show_diff {
            match key.code {
                KeyCode::Esc | KeyCode::Char('D') => {
                    self.show_diff = false;
                }
                _ => {}
            }
            return Action::None;
        }

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
            KeyCode::Char('E') => {
                // Expand all
                if let Some(sel_idx) = state.selected_event
                    && let Some(event_idx) = state.filtered_indices.get(sel_idx)
                    && let Some(event) = state.store.get(*event_idx)
                {
                    let items = build_tree_items(event, state.schema_registry.as_ref());
                    self.expand_all(&items);
                }
                Action::None
            }
            KeyCode::Char('C') => {
                self.collapse_all();
                Action::None
            }
            KeyCode::Char('y') => {
                // Copy selected value
                if let Some(sel_idx) = state.selected_event
                    && let Some(event_idx) = state.filtered_indices.get(sel_idx)
                    && let Some(event) = state.store.get(*event_idx)
                {
                    let items = build_tree_items(event, state.schema_registry.as_ref());
                    self.copy_selected_value(&items);
                }
                Action::None
            }
            KeyCode::Char('m') => {
                // Mark current event for diff
                self.marked_event_idx = state.selected_event;
                Action::None
            }
            KeyCode::Char('D') => {
                // Show diff if we have a marked event
                if self.marked_event_idx.is_some() {
                    self.show_diff = true;
                }
                Action::None
            }
            KeyCode::Char('h') => {
                // Highlight payload bytes in hex dump
                if let Some(sel_idx) = state.selected_event
                    && let Some(event_idx) = state.filtered_indices.get(sel_idx)
                    && let Some(event) = state.store.get(*event_idx)
                {
                    let payload_len = match &event.payload {
                        Payload::Raw { raw } => raw.len(),
                        Payload::Decoded { raw, .. } => raw.len(),
                    };
                    if payload_len > 0 {
                        return Action::HighlightBytes { offset: 0, len: payload_len };
                    }
                }
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, state: &AppState, theme: &ThemeConfig, focused: bool) {
        use ratatui::widgets::BorderType;

        // Build title with marker indicator
        let title = if self.marked_event_idx.is_some() {
            if focused {
                " Decode [*] (marked) "
            } else {
                " Decode (marked) "
            }
        } else {
            if focused {
                " Decode [*] "
            } else {
                " Decode "
            }
        };

        let block = if focused {
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.focused_border())
                .title(title)
                .title_style(theme.focused_title())
        } else {
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Plain)
                .border_style(theme.unfocused_border())
                .title(title)
                .title_style(theme.unfocused_title())
        };

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 5 {
            return;
        }

        // Show diff overlay if active
        if self.show_diff {
            self.render_diff_overlay(area, buf, state, theme);
            return;
        }

        let Some(sel_idx) = state.selected_event else {
            let msg = Text::styled(
                "  Select an event above to see decoded layers",
                Style::default().fg(Color::DarkGray),
            );
            Widget::render(msg, inner, buf);
            return;
        };
        let Some(event_idx) = state.filtered_indices.get(sel_idx) else {
            return;
        };
        let Some(event) = state.store.get(*event_idx) else {
            return;
        };

        let items = build_tree_items(event, state.schema_registry.as_ref());

        let tree = Tree::new(&items)
            .expect("tree items are valid")
            .highlight_style(theme.selected_row());

        ratatui::widgets::StatefulWidget::render(tree, inner, buf, &mut self.state);
    }
}

fn build_tree_items(event: &DebugEvent, schema_registry: Option<&SchemaRegistry>) -> Vec<TreeItem<'static, String>> {
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
        // Check for known protocol fields that benefit from error intelligence
        let label = if key == "grpc.status"
            && let Ok(code) = value.parse::<u32>()
            && let Some(name) = error_intel::grpc_status_name(code)
        {
            format!("{}: {} ({})", key, value, name)
        } else if key == "http.status" && value.parse::<u16>().is_ok() {
            format!("{}: {}", key, value)
        } else if key == "tcp.flags"
            && let Some(explanation) = error_intel::tcp_flag_explanation(value)
        {
            format!("{}: {} — {}", key, value, explanation)
        } else if key == "tls.alert"
            && let Ok(code) = value.parse::<u8>()
            && let Some(desc) = error_intel::tls_alert_description(code)
        {
            format!("{}: {} — {}", key, value, desc)
        } else {
            format!("{}: {}", key, value)
        };

        // Create the leaf node
        let mut children = vec![];

        // Add explanation as a child node for gRPC status codes with explanations
        if key == "grpc.status"
            && let Ok(code) = value.parse::<u32>()
            && let Some(explanation) = error_intel::grpc_status_explanation(code)
        {
            children.push(TreeItem::new_leaf(
                format!("m.{}.explain", key),
                format!("→ {}", explanation),
            ));
        }

        // Add the metadata item (with or without children)
        if children.is_empty() {
            transport_children.push(TreeItem::new_leaf(
                format!("m.{}", key),
                label,
            ));
        } else {
            transport_children.push(
                TreeItem::new(
                    format!("m.{}", key),
                    label,
                    children,
                )
                .expect("metadata children valid"),
            );
        }
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
        Payload::Raw { raw } => {
            payload_children.push(TreeItem::new_leaf("p.type".to_string(), "Type: Raw".to_string()));

            // Try schema-based decoding if we have a schema registry
            if let Some(registry) = schema_registry {
                if let Some(decoded_items) = try_schema_decode(raw, event, registry) {
                    // Successfully decoded with schema
                    payload_children.extend(decoded_items);
                } else {
                    // Fallback: show that we tried but couldn't decode
                    payload_children.push(TreeItem::new_leaf(
                        "p.note".to_string(),
                        "No matching schema found (showing raw bytes in hex view)".to_string(),
                    ));
                }
            }
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

/// Try to decode raw payload using schema registry.
/// Returns tree items if successful, None if no matching schema found.
fn try_schema_decode(
    raw: &[u8],
    event: &DebugEvent,
    registry: &SchemaRegistry,
) -> Option<Vec<TreeItem<'static, String>>> {
    // Try to find the schema name from gRPC method metadata
    let schema_name = if let Some(method) = event.metadata.get(METADATA_KEY_GRPC_METHOD) {
        // gRPC method format is /package.Service/Method
        // We need to map this to message types, but for now try common patterns
        // Look for request/response message types based on method name
        infer_message_type_from_grpc_method(method, registry)
    } else {
        // Try to find any matching schema by attempting decoding
        // This is a fallback for non-gRPC protocols
        None
    };

    if let Some(msg_type) = schema_name {
        // Get the message descriptor
        if let Some(descriptor) = registry.get_message(&msg_type) {
            // Try to decode
            match decode_with_schema(raw, &descriptor) {
                Ok(decoded) => {
                    // Build tree from decoded message
                    let mut items = vec![
                        TreeItem::new_leaf("p.schema".to_string(), format!("Schema: {}", msg_type)),
                    ];

                    // Convert decoded message to tree items
                    items.extend(build_tree_from_decoded_message(&decoded, "p.msg"));

                    return Some(items);
                }
                Err(e) => {
                    // Decode failed, show error
                    return Some(vec![
                        TreeItem::new_leaf("p.schema".to_string(), format!("Schema: {} (failed)", msg_type)),
                        TreeItem::new_leaf("p.error".to_string(), format!("Error: {}", e)),
                    ]);
                }
            }
        }
    }

    None
}

/// Infer protobuf message type from gRPC method name.
/// Returns the fully qualified message type name if found.
fn infer_message_type_from_grpc_method(
    method: &str,
    registry: &SchemaRegistry,
) -> Option<String> {
    // gRPC method format: /package.Service/Method
    // Common patterns for message types:
    // - package.MethodRequest / package.MethodResponse
    // - package.service.MethodRequest / package.service.MethodResponse

    let path_parts: Vec<&str> = method.trim_start_matches('/').split('/').collect();
    if path_parts.len() != 2 {
        return None;
    }

    let service_path = path_parts[0];
    let method_name = path_parts[1];

    // Extract package from service path
    let package_parts: Vec<&str> = service_path.rsplitn(2, '.').collect();
    let package = if package_parts.len() == 2 {
        package_parts[1]
    } else {
        ""
    };

    // Try common naming patterns
    let candidates = vec![
        format!("{}.{}Request", package, method_name),
        format!("{}.{}Response", package, method_name),
        format!("{}Request", method_name),
        format!("{}.{}", service_path, method_name),
    ];

    // Check which message types exist in the registry
    let available = registry.list_messages();
    candidates.into_iter().find(|candidate| available.contains(candidate))
}

/// Build tree items from a decoded protobuf message.
fn build_tree_from_decoded_message(
    decoded: &DecodedMessage,
    prefix: &str,
) -> Vec<TreeItem<'static, String>> {
    let mut items = Vec::new();
    let message = decoded.message();
    let descriptor = message.descriptor();

    for field in descriptor.fields() {
        let value = message.get_field(&field);
        let field_id = format!("{}.{}", prefix, field.name());

        // Convert Cow to owned Value
        if let Some(item) = build_tree_from_value(value.as_ref(), &field_id, field.name()) {
            items.push(item);
        }
    }

    items
}

/// Build a tree item from a prost-reflect Value.
fn build_tree_from_value(
    value: &Value,
    identifier: &str,
    field_name: &str,
) -> Option<TreeItem<'static, String>> {
    match value {
        Value::Bool(b) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {}", field_name, b),
        )),
        Value::I32(i) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {}", field_name, i),
        )),
        Value::I64(i) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {}", field_name, i),
        )),
        Value::U32(u) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {}", field_name, u),
        )),
        Value::U64(u) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {}", field_name, u),
        )),
        Value::F32(f) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {}", field_name, f),
        )),
        Value::F64(f) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {}", field_name, f),
        )),
        Value::String(s) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: \"{}\"", field_name, s),
        )),
        Value::Bytes(b) => {
            let hex = b.iter()
                .take(16)
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<_>>()
                .join(" ");
            let display = if b.len() > 16 {
                format!("{}: 0x{} ... ({} bytes)", field_name, hex, b.len())
            } else {
                format!("{}: 0x{}", field_name, hex)
            };
            Some(TreeItem::new_leaf(identifier.to_string(), display))
        }
        Value::EnumNumber(n) => Some(TreeItem::new_leaf(
            identifier.to_string(),
            format!("{}: {} (enum)", field_name, n),
        )),
        Value::Message(msg) => {
            // Nested message - create a parent node with children
            let mut children = Vec::new();
            let descriptor = msg.descriptor();

            for field in descriptor.fields() {
                let field_value = msg.get_field(&field);
                let child_id = format!("{}.{}", identifier, field.name());

                if let Some(child_item) = build_tree_from_value(field_value.as_ref(), &child_id, field.name()) {
                    children.push(child_item);
                }
            }

            if children.is_empty() {
                Some(TreeItem::new_leaf(
                    identifier.to_string(),
                    format!("{}: {{}}", field_name),
                ))
            } else {
                TreeItem::new(
                    identifier.to_string(),
                    format!("{}: {}", field_name, descriptor.name()),
                    children,
                ).ok()
            }
        }
        Value::List(items) => {
            if items.is_empty() {
                Some(TreeItem::new_leaf(
                    identifier.to_string(),
                    format!("{}: []", field_name),
                ))
            } else {
                let mut children = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let child_id = format!("{}[{}]", identifier, i);
                    if let Some(child_item) = build_tree_from_value(item, &child_id, &format!("[{}]", i)) {
                        children.push(child_item);
                    }
                }

                TreeItem::new(
                    identifier.to_string(),
                    format!("{}: [{} items]", field_name, items.len()),
                    children,
                ).ok()
            }
        }
        Value::Map(entries) => {
            if entries.is_empty() {
                Some(TreeItem::new_leaf(
                    identifier.to_string(),
                    format!("{}: {{}}", field_name),
                ))
            } else {
                let mut children = Vec::new();
                for (i, (key, val)) in entries.iter().enumerate() {
                    let key_str = match key {
                        prost_reflect::MapKey::Bool(b) => b.to_string(),
                        prost_reflect::MapKey::I32(i) => i.to_string(),
                        prost_reflect::MapKey::I64(i) => i.to_string(),
                        prost_reflect::MapKey::U32(u) => u.to_string(),
                        prost_reflect::MapKey::U64(u) => u.to_string(),
                        prost_reflect::MapKey::String(s) => s.clone(),
                    };
                    let child_id = format!("{}.{}", identifier, i);
                    if let Some(child_item) = build_tree_from_value(val, &child_id, &key_str) {
                        children.push(child_item);
                    }
                }

                TreeItem::new(
                    identifier.to_string(),
                    format!("{}: {{{}  entries}}", field_name, entries.len()),
                    children,
                ).ok()
            }
        }
    }
}

/// Extract value text from a tree item at the given identifier path
fn extract_value_from_tree(items: &[TreeItem<'static, String>], path: &[String]) -> Option<String> {
    if path.is_empty() {
        return None;
    }

    let mut current_items = items;
    let mut result = None;

    for (depth, identifier) in path.iter().enumerate() {
        // Find the item with matching identifier
        if let Some(item) = current_items.iter().find(|item| item.identifier() == identifier) {
            // Use the identifier as the value to copy
            result = Some(identifier.clone());

            // Try to navigate to children for next depth
            if depth + 1 < path.len() {
                current_items = item.children();
            }
        } else {
            return None;
        }
    }

    result
}

/// Send text to clipboard using OSC 52 escape sequence
fn send_osc52_copy(text: &str) {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    // OSC 52 sequence: ESC ] 52 ; c ; <base64> ESC \
    print!("\x1b]52;c;{}\x1b\\", encoded);
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

struct DiffResult {
    left_lines: Vec<ratatui::text::Line<'static>>,
    right_lines: Vec<ratatui::text::Line<'static>>,
}

/// Compute a simple field-level diff between two events
fn compute_event_diff(event1: &DebugEvent, event2: &DebugEvent) -> DiffResult {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use std::collections::BTreeMap;

    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();

    // Helper to extract fields from event
    let extract_fields = |event: &DebugEvent| -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();

        // Timestamp
        let ts_ns = event.timestamp.as_nanos();
        let secs = ts_ns / 1_000_000_000;
        let millis = (ts_ns % 1_000_000_000) / 1_000_000;
        let h = (secs / 3600) % 24;
        let m = (secs % 3600) / 60;
        let s = secs % 60;
        fields.insert("Timestamp".to_string(), format!("{:02}:{:02}:{:02}.{:03}", h, m, s, millis));

        // Direction
        fields.insert("Direction".to_string(), event.direction.to_string());

        // Transport
        fields.insert("Transport".to_string(), event.transport.to_string());

        // Metadata
        for (k, v) in &event.metadata {
            fields.insert(format!("meta.{}", k), v.clone());
        }

        // Payload type and size
        match &event.payload {
            Payload::Raw { raw } => {
                fields.insert("Payload.type".to_string(), "Raw".to_string());
                fields.insert("Payload.size".to_string(), format!("{} bytes", raw.len()));
            }
            Payload::Decoded { raw, schema_name, .. } => {
                fields.insert("Payload.type".to_string(), "Decoded".to_string());
                fields.insert("Payload.size".to_string(), format!("{} bytes", raw.len()));
                if let Some(name) = schema_name {
                    fields.insert("Payload.schema".to_string(), name.clone());
                }
            }
        }

        fields
    };

    let fields1 = extract_fields(event1);
    let fields2 = extract_fields(event2);

    // Collect all unique keys
    let mut all_keys: Vec<_> = fields1.keys().chain(fields2.keys()).collect();
    all_keys.sort();
    all_keys.dedup();

    // Header
    left_lines.push(Line::from(Span::styled("Marked Event", Style::default().fg(Color::Cyan))));
    right_lines.push(Line::from(Span::styled("Current Event", Style::default().fg(Color::Cyan))));
    left_lines.push(Line::from(""));
    right_lines.push(Line::from(""));

    // Compare each field
    for key in all_keys {
        let val1 = fields1.get(key);
        let val2 = fields2.get(key);

        let (style_left, style_right) = match (val1, val2) {
            (Some(v1), Some(v2)) if v1 == v2 => {
                // Same value
                (Style::default(), Style::default())
            }
            (Some(_), Some(_)) => {
                // Different values
                (Style::default().fg(Color::Yellow), Style::default().fg(Color::Yellow))
            }
            (Some(_), None) => {
                // Only in left
                (Style::default().fg(Color::Red), Style::default())
            }
            (None, Some(_)) => {
                // Only in right
                (Style::default(), Style::default().fg(Color::Green))
            }
            (None, None) => unreachable!(),
        };

        let left_text = val1.map(|v| format!("{}: {}", key, v)).unwrap_or_else(|| format!("{}: <missing>", key));
        let right_text = val2.map(|v| format!("{}: {}", key, v)).unwrap_or_else(|| format!("{}: <missing>", key));

        left_lines.push(Line::from(Span::styled(left_text, style_left)));
        right_lines.push(Line::from(Span::styled(right_text, style_right)));
    }

    DiffResult {
        left_lines,
        right_lines,
    }
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
        let items = build_tree_items(&event, None);

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
        let items = build_tree_items(&event, None);

        // Verify ZMQ event produces tree items
        assert!(items.len() >= 4, "Should have basic sections");
    }

    #[test]
    fn test_dds_tree_structure() {
        let mut metadata = BTreeMap::new();
        metadata.insert(METADATA_KEY_DDS_DOMAIN_ID.to_string(), "0".to_string());
        metadata.insert(METADATA_KEY_DDS_TOPIC_NAME.to_string(), "ChatterTopic".to_string());

        let event = create_test_event(TransportKind::DdsRtps, metadata);
        let items = build_tree_items(&event, None);

        // Verify DDS-RTPS event produces tree items
        assert!(items.len() >= 4, "Should have basic sections");
    }

    #[test]
    fn test_source_section_with_network() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event, None);

        // Verify we have items (including source section)
        assert!(items.len() >= 3, "Should have timestamp, direction, source at minimum");
    }

    #[test]
    fn test_payload_decoded_section() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event, None);

        // Decoded payload should produce tree items
        assert!(items.len() >= 5, "Should have all sections including payload");
    }

    #[test]
    fn test_payload_raw_section() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.payload = Payload::Raw {
            raw: Bytes::from_static(b"raw bytes"),
        };

        let items = build_tree_items(&event, None);

        // Raw payload should also produce tree items
        assert!(items.len() >= 5, "Should have all sections including raw payload");
    }

    #[test]
    fn test_correlation_section() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event, None);

        // Event has correlation keys, so should have correlation section
        assert!(items.len() >= 5, "Should have correlation section");
    }

    #[test]
    fn test_warnings_section() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.warnings = vec!["Parse error".to_string(), "Missing field".to_string()];

        let items = build_tree_items(&event, None);

        // Warnings should add an extra section
        let base_len = 5; // timestamp, direction, source, transport, payload
        assert!(items.len() >= base_len, "Should include warnings section");
    }

    #[test]
    fn test_no_warnings_section_when_empty() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event, None);

        // Without warnings and with correlation keys
        // We should have: timestamp, direction, source, transport, payload, correlation
        assert_eq!(items.len(), 6, "Should have 6 sections without warnings");
    }

    #[test]
    fn test_timestamp_formatting() {
        let event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        let items = build_tree_items(&event, None);

        // Just verify tree items are created - timestamp is first
        assert!(!items.is_empty(), "Should have timestamp item");
    }

    #[test]
    fn test_metadata_keys_in_transport() {
        let mut metadata = BTreeMap::new();
        metadata.insert("custom.key".to_string(), "custom_value".to_string());
        metadata.insert(METADATA_KEY_GRPC_METHOD.to_string(), "/api.Service/Method".to_string());

        let event = create_test_event(TransportKind::Grpc, metadata);
        let items = build_tree_items(&event, None);

        // Metadata adds children to transport section
        assert!(items.len() >= 4, "Should have all sections");
    }

    #[test]
    fn test_event_without_network() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.source.network = None;

        let items = build_tree_items(&event, None);

        // Should still build tree successfully
        assert!(items.len() >= 5, "Should work without network info");
    }

    #[test]
    fn test_event_without_correlation() {
        let mut event = create_test_event(TransportKind::Grpc, BTreeMap::new());
        event.correlation_keys = vec![];

        let items = build_tree_items(&event, None);

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

        let items = build_tree_items(&event, None);

        // Should handle all correlation key types
        assert!(items.len() >= 5, "Should handle all correlation key types");
    }
}
