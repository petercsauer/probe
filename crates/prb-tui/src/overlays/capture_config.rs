//! Live capture configuration overlay.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Widget};
use ratatui::style::{Color, Modifier, Style};
use prb_capture::{InterfaceInfo, InterfaceEnumerator, PrivilegeCheck};
use tui_input::Input;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigField {
    Interface,
    BpfFilter,
    Snaplen,
    Promiscuous,
}

impl ConfigField {
    pub fn next(self) -> Self {
        match self {
            ConfigField::Interface => ConfigField::BpfFilter,
            ConfigField::BpfFilter => ConfigField::Snaplen,
            ConfigField::Snaplen => ConfigField::Promiscuous,
            ConfigField::Promiscuous => ConfigField::Interface,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            ConfigField::Interface => ConfigField::Promiscuous,
            ConfigField::BpfFilter => ConfigField::Interface,
            ConfigField::Snaplen => ConfigField::BpfFilter,
            ConfigField::Promiscuous => ConfigField::Snaplen,
        }
    }
}

/// Live capture configuration overlay.
pub struct CaptureConfigOverlay {
    pub interfaces: Vec<InterfaceInfo>,
    pub selected_interface: usize,
    pub bpf_filter: String,
    pub bpf_filter_input: Input,
    pub snaplen: u32,
    pub snaplen_input: Input,
    pub promiscuous: bool,
    pub focused_field: ConfigField,
    pub privilege_warning: Option<String>,
    pub bpf_validation_error: Option<String>,
}

impl CaptureConfigOverlay {
    pub fn new() -> Self {
        // Try to load interfaces; if it fails, start with empty list
        let interfaces = InterfaceEnumerator::list().unwrap_or_default();

        // Check privileges for the default interface
        let privilege_warning = if let Some(iface) = interfaces.first() {
            match PrivilegeCheck::check(&iface.name) {
                Ok(()) => None,
                Err(e) => Some(format!("⚠ {}", e)),
            }
        } else {
            Some("⚠ No network interfaces found".to_string())
        };

        Self {
            interfaces,
            selected_interface: 0,
            bpf_filter: String::new(),
            bpf_filter_input: Input::default(),
            snaplen: 65535,
            snaplen_input: Input::default().with_value("65535".to_string()),
            promiscuous: true,
            focused_field: ConfigField::Interface,
            privilege_warning,
            bpf_validation_error: None,
        }
    }

    /// Refresh interface list and privilege check.
    pub fn refresh(&mut self) {
        self.interfaces = InterfaceEnumerator::list().unwrap_or_default();

        // Re-check privileges for the selected interface
        if let Some(iface) = self.interfaces.get(self.selected_interface) {
            self.privilege_warning = match PrivilegeCheck::check(&iface.name) {
                Ok(()) => None,
                Err(e) => Some(format!("⚠ {}", e)),
            };
        } else {
            self.privilege_warning = Some("⚠ No network interfaces found".to_string());
            self.selected_interface = 0;
        }
    }

    /// Move interface selection up or down.
    pub fn move_selection(&mut self, delta: isize) {
        if self.interfaces.is_empty() {
            return;
        }

        let new_idx = (self.selected_interface as isize + delta)
            .rem_euclid(self.interfaces.len() as isize);
        self.selected_interface = new_idx as usize;

        // Update privilege check when interface changes
        if let Some(iface) = self.interfaces.get(self.selected_interface) {
            self.privilege_warning = match PrivilegeCheck::check(&iface.name) {
                Ok(()) => None,
                Err(e) => Some(format!("⚠ {}", e)),
            };
        }
    }

    /// Toggle promiscuous mode.
    pub fn toggle_promiscuous(&mut self) {
        self.promiscuous = !self.promiscuous;
    }

    /// Update BPF filter string.
    pub fn set_bpf_filter(&mut self, filter: String) {
        self.bpf_filter = filter.clone();
        self.bpf_filter_input = Input::default().with_value(filter);
    }

    /// Update snaplen value.
    pub fn set_snaplen(&mut self, snaplen: u32) {
        self.snaplen = snaplen;
        self.snaplen_input = Input::default().with_value(snaplen.to_string());
    }

    /// Get mutable reference to BPF filter input.
    pub fn bpf_filter_input_mut(&mut self) -> &mut Input {
        &mut self.bpf_filter_input
    }

    /// Get mutable reference to snaplen input.
    pub fn snaplen_input_mut(&mut self) -> &mut Input {
        &mut self.snaplen_input
    }

    /// Sync the input fields to their underlying values.
    pub fn sync_inputs(&mut self) {
        self.bpf_filter = self.bpf_filter_input.value().to_string();
        if let Ok(val) = self.snaplen_input.value().parse::<u32>()
            && val > 0
        {
            self.snaplen = val;
        }
    }

    /// Validate the BPF filter syntax.
    ///
    /// This attempts to compile the filter using libpcap to catch syntax errors
    /// before starting capture. Returns Ok(()) if valid, or an error message.
    pub fn validate_bpf_filter(&mut self) -> Result<(), String> {
        // Empty filter is valid (captures all traffic)
        if self.bpf_filter.is_empty() {
            self.bpf_validation_error = None;
            return Ok(());
        }

        // Get the selected interface (or use "any" as fallback for validation)
        let interface = self
            .get_selected_interface()
            .map(|i| i.name.as_str())
            .unwrap_or("any");

        // Try to open a dummy capture device and compile the filter
        let cap_result: Result<pcap::Capture<pcap::Active>, pcap::Error> =
            pcap::Capture::from_device(interface)
                .and_then(|c| c.open());

        match cap_result {
            Ok(mut active_cap) => {
                match active_cap.filter(&self.bpf_filter, true) {
                    Ok(_) => {
                        self.bpf_validation_error = None;
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!("Invalid BPF filter: {}", e);
                        self.bpf_validation_error = Some(error_msg.clone());
                        Err(error_msg)
                    }
                }
            }
            Err(e) => {
                // Can't open device for validation, accept the filter
                // (will be validated again when actually starting capture)
                tracing::warn!("Cannot validate BPF filter (device open failed): {}", e);
                self.bpf_validation_error = None;
                Ok(())
            }
        }
    }

    /// Get the currently selected interface.
    pub fn get_selected_interface(&self) -> Option<&InterfaceInfo> {
        self.interfaces.get(self.selected_interface)
    }

    /// Render the capture config overlay.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let width = 70u16.min(area.width.saturating_sub(4));
        let height = 25u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Live Capture Configuration ");

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        // Split inner area into sections
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Header
                Constraint::Min(8),     // Interface list
                Constraint::Length(3),  // BPF filter
                Constraint::Length(3),  // Settings
                Constraint::Length(3),  // Privilege warning
                Constraint::Length(2),  // Help text
            ])
            .split(inner);

        // Header
        let header = Line::from(vec![
            Span::styled(
                "Select network interface and configure capture settings",
                Style::default().fg(Color::Gray),
            ),
        ]);
        buf.set_line(layout[0].x, layout[0].y, &header, layout[0].width);

        // Interface list
        self.render_interface_list(layout[1], buf);

        // BPF filter
        self.render_bpf_filter(layout[2], buf);

        // Settings
        self.render_settings(layout[3], buf);

        // Privilege warning
        if let Some(ref warning) = self.privilege_warning {
            self.render_privilege_warning(layout[4], buf, warning);
        }

        // Help text
        let help = Line::from(vec![
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::raw(": navigate  "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(": next field  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": start  "),
            Span::styled("Esc/q", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel"),
        ]);
        buf.set_line(layout[5].x, layout[5].y, &help, layout[5].width);
    }

    fn render_interface_list(&self, area: Rect, buf: &mut Buffer) {
        let focused = self.focused_field == ConfigField::Interface;
        let border_style = if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Network Interfaces ");

        let inner = block.inner(area);
        block.render(area, buf);

        if self.interfaces.is_empty() {
            let line = Line::from(Span::styled(
                "No interfaces found",
                Style::default().fg(Color::Red),
            ));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        }

        // Render interface list
        let items: Vec<ListItem> = self
            .interfaces
            .iter()
            .enumerate()
            .map(|(idx, iface)| {
                let is_selected = idx == self.selected_interface;
                let status_color = if iface.is_up && iface.is_running {
                    Color::Green
                } else if iface.is_up {
                    Color::Yellow
                } else {
                    Color::DarkGray
                };

                let prefix = if is_selected { " > " } else { "   " };
                let name_style = if is_selected {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let status = iface.status();
                let addr_display = if iface.addresses.is_empty() {
                    String::new()
                } else {
                    format!("  {}", iface.addresses_display())
                };

                let suffix = if iface.is_loopback {
                    "  [loopback]"
                } else if iface.is_wireless {
                    "  [wireless]"
                } else {
                    ""
                };

                let line = Line::from(vec![
                    Span::raw(prefix),
                    Span::styled(&iface.name, name_style),
                    Span::raw("  "),
                    Span::styled(status, Style::default().fg(status_color)),
                    Span::styled(addr_display, Style::default().fg(Color::Gray)),
                    Span::styled(suffix, Style::default().fg(Color::DarkGray)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items);
        list.render(inner, buf);
    }

    fn render_bpf_filter(&self, area: Rect, buf: &mut Buffer) {
        let focused = self.focused_field == ConfigField::BpfFilter;

        // Border color: red if validation error, cyan if focused, gray otherwise
        let border_style = if self.bpf_validation_error.is_some() {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" BPF Filter ");

        let inner = block.inner(area);
        block.render(area, buf);

        if focused {
            // Show input widget when focused
            let input_value = self.bpf_filter_input.value();
            let display_text = if input_value.is_empty() {
                "(type filter here...)"
            } else {
                input_value
            };
            let cursor_pos = self.bpf_filter_input.cursor();

            let line = Line::from(Span::styled(display_text, Style::default().fg(Color::White)));
            buf.set_line(inner.x, inner.y, &line, inner.width);

            // Show cursor
            if cursor_pos < inner.width as usize {
                buf[(inner.x + cursor_pos as u16, inner.y)]
                    .set_style(Style::default().bg(Color::White).fg(Color::Black));
            }
        } else {
            // Show static text when not focused
            let display_text = if self.bpf_filter.is_empty() {
                Span::styled("(no filter - capture all traffic)", Style::default().fg(Color::DarkGray))
            } else {
                Span::styled(&self.bpf_filter, Style::default().fg(Color::White))
            };

            let line = Line::from(display_text);
            buf.set_line(inner.x, inner.y, &line, inner.width);
        }

        // Show validation error if present (on second line)
        if let Some(ref error) = self.bpf_validation_error {
            if inner.height > 1 {
                let error_line = Line::from(Span::styled(
                    format!("⚠ {}", error),
                    Style::default().fg(Color::Red),
                ));
                buf.set_line(inner.x, inner.y + 1, &error_line, inner.width);
            }
        }
    }

    fn render_settings(&self, area: Rect, buf: &mut Buffer) {
        let focused_snaplen = self.focused_field == ConfigField::Snaplen;
        let focused_promisc = self.focused_field == ConfigField::Promiscuous;

        let border_style = if focused_snaplen || focused_promisc {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Capture Settings ");

        let inner = block.inner(area);
        block.render(area, buf);

        // Snaplen line
        let snaplen_style = if focused_snaplen {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        if focused_snaplen {
            // Show input widget when focused
            let input_value = self.snaplen_input.value();
            let cursor_pos = self.snaplen_input.cursor();

            let snaplen_line = Line::from(vec![
                Span::styled("Snaplen: ", snaplen_style),
                Span::styled(format!("{} bytes", input_value), snaplen_style),
            ]);
            buf.set_line(inner.x, inner.y, &snaplen_line, inner.width);

            // Show cursor
            let cursor_x = inner.x + 9 + cursor_pos as u16; // "Snaplen: " = 9 chars
            if cursor_x < inner.x + inner.width {
                buf[(cursor_x, inner.y)]
                    .set_style(Style::default().bg(Color::White).fg(Color::Black));
            }
        } else {
            let snaplen_line = Line::from(vec![
                Span::styled("Snaplen: ", snaplen_style),
                Span::styled(
                    format!("{} bytes", self.snaplen),
                    snaplen_style,
                ),
            ]);
            buf.set_line(inner.x, inner.y, &snaplen_line, inner.width);
        }

        // Promiscuous mode line
        let promisc_style = if focused_promisc {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let promisc_value = if self.promiscuous { "ON" } else { "OFF" };
        let promisc_line = Line::from(vec![
            Span::styled("Promiscuous mode: ", promisc_style),
            Span::styled(
                promisc_value,
                Style::default().fg(if self.promiscuous { Color::Green } else { Color::Red }),
            ),
            Span::styled(" (Space to toggle)", Style::default().fg(Color::DarkGray)),
        ]);
        buf.set_line(inner.x, inner.y + 1, &promisc_line, inner.width);
    }

    fn render_privilege_warning(&self, area: Rect, buf: &mut Buffer, warning: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(area);
        block.render(area, buf);

        let lines: Vec<Line> = warning
            .lines()
            .map(|line| Line::from(Span::styled(line, Style::default().fg(Color::Yellow))))
            .collect();

        for (idx, line) in lines.iter().enumerate() {
            if idx < inner.height as usize {
                buf.set_line(inner.x, inner.y + idx as u16, line, inner.width);
            }
        }
    }
}

impl Default for CaptureConfigOverlay {
    fn default() -> Self {
        Self::new()
    }
}
