//! Command palette for fuzzy-searchable command list.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget};
use ratatui::style::{Color, Modifier, Style};
use crate::theme::Theme;

/// Command palette entry mapping display name to action.
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub key_hint: String,
}

/// Command palette overlay for fuzzy command search.
pub struct CommandPaletteOverlay {
    pub input: String,
    pub commands: Vec<Command>,
    pub selected: usize,
}

impl CommandPaletteOverlay {
    pub fn new() -> Self {
        let commands = vec![
            Command {
                name: "Filter events".to_string(),
                description: "Open filter bar".to_string(),
                key_hint: "/".to_string(),
            },
            Command {
                name: "Clear filter".to_string(),
                description: "Remove active filter".to_string(),
                key_hint: "Esc".to_string(),
            },
            Command {
                name: "Help".to_string(),
                description: "Show keybinding help".to_string(),
                key_hint: "?".to_string(),
            },
            Command {
                name: "Next pane".to_string(),
                description: "Cycle to next pane".to_string(),
                key_hint: "Tab".to_string(),
            },
            Command {
                name: "Previous pane".to_string(),
                description: "Cycle to previous pane".to_string(),
                key_hint: "Shift+Tab".to_string(),
            },
            Command {
                name: "First event".to_string(),
                description: "Jump to first event".to_string(),
                key_hint: "g".to_string(),
            },
            Command {
                name: "Last event".to_string(),
                description: "Jump to last event".to_string(),
                key_hint: "G".to_string(),
            },
            Command {
                name: "Quit".to_string(),
                description: "Exit the application".to_string(),
                key_hint: "q".to_string(),
            },
        ];

        Self {
            input: String::new(),
            commands,
            selected: 0,
        }
    }

    pub fn update_input(&mut self, input: String) {
        self.input = input;
        self.selected = 0; // Reset selection when input changes
    }

    pub fn move_selection(&mut self, delta: isize) {
        let filtered = self.filtered_commands();
        if filtered.is_empty() {
            return;
        }

        let new_idx = (self.selected as isize + delta).rem_euclid(filtered.len() as isize);
        self.selected = new_idx as usize;
    }

    pub fn filtered_commands(&self) -> Vec<&Command> {
        if self.input.is_empty() {
            self.commands.iter().collect()
        } else {
            let needle = self.input.to_lowercase();
            self.commands
                .iter()
                .filter(|c| {
                    c.name.to_lowercase().contains(&needle)
                        || c.description.to_lowercase().contains(&needle)
                })
                .collect()
        }
    }

    pub fn selected_command(&self) -> Option<&Command> {
        self.filtered_commands().get(self.selected).copied()
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let width = 50u16.min(area.width.saturating_sub(4));
        let height = 15u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Command Palette ");

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        // Input line
        let input_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let input_line = Line::from(vec![
            Span::styled(": ", Style::default().fg(Color::Cyan)),
            Span::raw(&self.input),
            Span::styled("▏", Style::default().fg(Color::Cyan)),
        ]);
        buf.set_line(input_area.x, input_area.y, &input_line, input_area.width);

        // Command list
        let list_area = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(2),
        );

        let filtered = self.filtered_commands();
        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let style = if i == self.selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!(" {:<20}", cmd.name),
                        style,
                    ),
                    Span::styled(
                        format!(" {:<10}", cmd.key_hint),
                        style.fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        if !items.is_empty() {
            let list = List::new(items);
            list.render(list_area, buf);
        } else {
            let no_match = Paragraph::new("No matching commands")
                .style(Style::default().fg(Color::DarkGray));
            no_match.render(list_area, buf);
        }
    }
}

impl Default for CommandPaletteOverlay {
    fn default() -> Self {
        Self::new()
    }
}
