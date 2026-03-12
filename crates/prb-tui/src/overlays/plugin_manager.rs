//! Plugin management overlay for the TUI.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::style::{Color, Modifier, Style};
use crate::theme::Theme;
use prb_plugin_api::PluginMetadata;
use tui_input::Input;

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

/// View mode for the plugin manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginManagerView {
    List,
    Info,
    Install,
    Configure,
}

/// Plugin manager overlay state.
pub struct PluginManagerOverlay {
    pub plugins: Vec<PluginEntry>,
    pub selected: usize,
    pub view: PluginManagerView,
    pub install_input: Input,
    pub status_message: Option<String>,
}

impl PluginManagerOverlay {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            selected: 0,
            view: PluginManagerView::List,
            install_input: Input::default(),
            status_message: None,
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

    /// Start install mode.
    pub fn start_install(&mut self) {
        self.view = PluginManagerView::Install;
        self.install_input = Input::default();
        self.status_message = None;
    }

    /// Start configure mode.
    pub fn start_configure(&mut self) {
        if self.plugins.get(self.selected).is_some() {
            self.view = PluginManagerView::Configure;
            self.status_message = None;
        }
    }

    /// Get the install path from input.
    pub fn get_install_path(&self) -> String {
        self.install_input.value().to_string()
    }

    /// Set a status message.
    pub fn set_status(&mut self, message: String) {
        self.status_message = Some(message);
    }

    /// Clear status message.
    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    /// Remove a plugin by index.
    pub fn remove_plugin(&mut self, index: usize) {
        if index < self.plugins.len() {
            self.plugins.remove(index);
            if self.selected >= self.plugins.len() && self.selected > 0 {
                self.selected -= 1;
            }
        }
    }

    /// Return to list view.
    pub fn show_list(&mut self) {
        self.view = PluginManagerView::List;
    }

    /// Toggle info view.
    pub fn toggle_info(&mut self) {
        if self.view == PluginManagerView::Info {
            self.view = PluginManagerView::List;
        } else {
            self.view = PluginManagerView::Info;
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

        match self.view {
            PluginManagerView::List => self.render_list_view(overlay_area, buf),
            PluginManagerView::Info => self.render_info_view(overlay_area, buf),
            PluginManagerView::Install => self.render_install_view(overlay_area, buf),
            PluginManagerView::Configure => self.render_configure_view(overlay_area, buf),
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
            Span::raw(" info/install  "),
            Span::styled("r:", Theme::help_key()),
            Span::raw(" reload  "),
            Span::styled("c:", Theme::help_key()),
            Span::raw(" config  "),
        ]);
        buf.set_line(area.x + 2, help_y + 1, &help_line, area.width - 4);

        // Show status message if present
        if let Some(ref msg) = self.status_message {
            let status_line = Line::from(Span::styled(
                format!(" {} ", msg),
                Style::default().fg(Color::Yellow),
            ));
            let status_y = area.y + area.height - 3;
            buf.set_line(area.x + 2, status_y, &status_line, area.width - 4);
        }
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

    /// Render the install plugin view.
    fn render_install_view(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Install Plugin ");
        let inner = block.inner(area);
        block.render(area, buf);

        let mut y = inner.y;

        // Instructions
        let instruction_lines = vec![
            Line::from(Span::raw("Enter path to plugin file:")),
            Line::from(Span::raw("")),
            Line::from(Span::styled("  Native: ", Theme::help_key())),
            Line::from(Span::raw("    /path/to/plugin.dylib")),
            Line::from(Span::raw("")),
            Line::from(Span::styled("  WASM: ", Theme::help_key())),
            Line::from(Span::raw("    /path/to/plugin.wasm")),
            Line::from(Span::raw("")),
        ];

        for line in instruction_lines {
            if y >= inner.y + inner.height - 4 {
                break;
            }
            buf.set_line(inner.x + 1, y, &line, inner.width - 2);
            y += 1;
        }

        // Input field
        let input_label = Line::from(Span::styled("Path: ", Theme::help_key()));
        buf.set_line(inner.x + 1, y, &input_label, inner.width - 2);
        y += 1;

        let input_line = Line::from(Span::raw(self.install_input.value()));
        buf.set_line(inner.x + 3, y, &input_line, inner.width - 4);

        // Help text at bottom
        let help_y = area.y + area.height - 2;
        buf.set_line(
            area.x + 1,
            help_y,
            &Line::from("─".repeat((area.width - 2) as usize)),
            area.width - 2,
        );

        let help_line = Line::from(vec![
            Span::styled(" Enter:", Theme::help_key()),
            Span::raw(" install  "),
            Span::styled("Esc:", Theme::help_key()),
            Span::raw(" cancel "),
        ]);
        buf.set_line(area.x + 2, help_y + 1, &help_line, area.width - 4);
    }

    /// Render the configure plugin view.
    fn render_configure_view(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Plugin Configuration ");
        let inner = block.inner(area);
        block.render(area, buf);

        if let Some(entry) = self.plugins.get(self.selected) {
            let lines = [
                Line::from(vec![
                    Span::styled("Plugin: ", Theme::help_key()),
                    Span::raw(&entry.metadata.name),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Plugin configuration is not yet implemented.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::raw("This feature will allow you to:")),
                Line::from(Span::raw("  • Set plugin-specific options")),
                Line::from(Span::raw("  • Configure protocol detection")),
                Line::from(Span::raw("  • Adjust decoding parameters")),
            ];

            for (i, line) in lines.iter().enumerate() {
                if i as u16 >= inner.height - 2 {
                    break;
                }
                buf.set_line(inner.x + 1, inner.y + i as u16, line, inner.width - 2);
            }
        }

        // Help text at bottom
        let help_y = area.y + area.height - 2;
        buf.set_line(
            area.x + 1,
            help_y,
            &Line::from("─".repeat((area.width - 2) as usize)),
            area.width - 2,
        );

        let help_line = Line::from(vec![
            Span::styled(" Esc:", Theme::help_key()),
            Span::raw(" back "),
        ]);
        buf.set_line(area.x + 2, help_y + 1, &help_line, area.width - 4);
    }
}

impl Default for PluginManagerOverlay {
    fn default() -> Self {
        Self::new()
    }
}
