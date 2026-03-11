//! Plugin management overlay for the TUI.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::style::{Color, Modifier, Style};
use crate::theme::Theme;
use prb_plugin_api::PluginMetadata;

/// Plugin type (native or WASM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    Native,
    Wasm,
}

impl PluginType {
    pub fn as_str(&self) -> &str {
        match self {
            PluginType::Native => "native",
            PluginType::Wasm => "wasm",
        }
    }
}

/// Represents a loaded plugin entry in the manager.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    pub metadata: PluginMetadata,
    pub plugin_type: PluginType,
    pub enabled: bool,
}

/// Plugin manager overlay state.
pub struct PluginManagerOverlay {
    pub plugins: Vec<PluginEntry>,
    pub selected: usize,
    pub show_info: bool,
}

impl PluginManagerOverlay {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            selected: 0,
            show_info: false,
        }
    }

    /// Add a plugin to the manager.
    pub fn add_plugin(&mut self, metadata: PluginMetadata, plugin_type: PluginType, enabled: bool) {
        self.plugins.push(PluginEntry {
            metadata,
            plugin_type,
            enabled,
        });
    }

    /// Toggle the selected plugin's enabled state.
    pub fn toggle_enabled(&mut self) {
        if let Some(entry) = self.plugins.get_mut(self.selected) {
            entry.enabled = !entry.enabled;
        }
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        if !self.plugins.is_empty() && self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        if !self.plugins.is_empty() && self.selected < self.plugins.len() - 1 {
            self.selected += 1;
        }
    }

    /// Render the plugin manager overlay.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let width = 60u16.min(area.width.saturating_sub(4));
        let height = 20u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        // Clear the overlay area
        Clear.render(overlay_area, buf);

        if self.show_info {
            self.render_info_view(overlay_area, buf);
        } else {
            self.render_list_view(overlay_area, buf);
        }
    }

    /// Render the plugin list view.
    fn render_list_view(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Plugin Manager (Ctrl+P to close) ");
        let inner = block.inner(area);
        block.render(area, buf);

        // Render plugin list
        let mut y = inner.y;

        if self.plugins.is_empty() {
            let line = Line::from(Span::styled(
                "  No plugins found in ~/.prb/plugins/",
                Style::default().fg(Color::DarkGray),
            ));
            buf.set_line(inner.x, y, &line, inner.width);
        } else {
            for (i, entry) in self.plugins.iter().enumerate() {
                if y >= inner.y + inner.height - 2 {
                    break;
                }

                let is_selected = i == self.selected;
                let checkbox = if entry.enabled { "[x]" } else { "[ ]" };
                let type_str = entry.plugin_type.as_str();

                let line = if is_selected {
                    Line::from(vec![
                        Span::styled(" > ", Theme::focused_border()),
                        Span::styled(
                            format!("{} {} ({}) ", checkbox, entry.metadata.name, type_str),
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                        ),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(
                            format!("{} {} ({}) ", checkbox, entry.metadata.name, type_str),
                            Style::default().fg(Color::White),
                        ),
                    ])
                };

                buf.set_line(inner.x, y, &line, inner.width);
                y += 1;
            }
        }

        // Render help bar at the bottom
        let help_y = area.y + area.height - 2;
        buf.set_line(
            area.x + 1,
            help_y,
            &Line::from("─".repeat((area.width - 2) as usize)),
            area.width - 2,
        );

        let help_line = Line::from(vec![
            Span::styled(" j/k:", Theme::help_key()),
            Span::raw(" nav  "),
            Span::styled("Space:", Theme::help_key()),
            Span::raw(" toggle  "),
            Span::styled("i:", Theme::help_key()),
            Span::raw(" info  "),
            Span::styled("q/Esc:", Theme::help_key()),
            Span::raw(" close "),
        ]);
        buf.set_line(area.x + 2, help_y + 1, &help_line, area.width - 4);
    }

    /// Render the plugin info view.
    fn render_info_view(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Plugin Info (press i or Esc to go back) ");
        let inner = block.inner(area);
        block.render(area, buf);

        if let Some(entry) = self.plugins.get(self.selected) {
            let lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Theme::help_key()),
                    Span::raw(&entry.metadata.name),
                ]),
                Line::from(vec![
                    Span::styled("Version: ", Theme::help_key()),
                    Span::raw(&entry.metadata.version),
                ]),
                Line::from(vec![
                    Span::styled("Type: ", Theme::help_key()),
                    Span::raw(entry.plugin_type.as_str()),
                ]),
                Line::from(vec![
                    Span::styled("Protocol: ", Theme::help_key()),
                    Span::raw(&entry.metadata.protocol_id),
                ]),
                Line::from(vec![
                    Span::styled("API Version: ", Theme::help_key()),
                    Span::raw(&entry.metadata.api_version),
                ]),
                Line::from(vec![
                    Span::styled("Enabled: ", Theme::help_key()),
                    Span::raw(if entry.enabled { "Yes" } else { "No" }),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Description:", Theme::help_key()),
                ]),
                Line::from(Span::raw(&entry.metadata.description)),
            ];

            for (i, line) in lines.iter().enumerate() {
                if i as u16 >= inner.height {
                    break;
                }
                buf.set_line(inner.x + 1, inner.y + i as u16, line, inner.width - 2);
            }
        }
    }
}

impl Default for PluginManagerOverlay {
    fn default() -> Self {
        Self::new()
    }
}
