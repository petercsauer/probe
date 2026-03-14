//! Filter template overlay for quick access to common filter patterns.

use crate::filter_persistence::FilterTemplate;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget};

/// Filter template overlay for quick filter pattern selection.
pub struct FilterTemplateOverlay {
    pub input: String,
    pub templates: Vec<FilterTemplate>,
    pub selected: usize,
}

impl FilterTemplateOverlay {
    pub fn new(templates: Vec<FilterTemplate>) -> Self {
        Self {
            input: String::new(),
            templates,
            selected: 0,
        }
    }

    pub fn update_input(&mut self, input: String) {
        self.input = input;
        self.selected = 0; // Reset selection when input changes
    }

    pub fn move_selection(&mut self, delta: isize) {
        let filtered = self.filtered_templates();
        if filtered.is_empty() {
            return;
        }

        let new_idx = (self.selected as isize + delta).rem_euclid(filtered.len() as isize);
        self.selected = new_idx as usize;
    }

    pub fn filtered_templates(&self) -> Vec<&FilterTemplate> {
        if self.input.is_empty() {
            self.templates.iter().collect()
        } else {
            let needle = self.input.to_lowercase();
            self.templates
                .iter()
                .filter(|t| {
                    t.name.to_lowercase().contains(&needle)
                        || t.description.to_lowercase().contains(&needle)
                        || t.tags
                            .iter()
                            .any(|tag| tag.to_lowercase().contains(&needle))
                })
                .collect()
        }
    }

    pub fn selected_template(&self) -> Option<&FilterTemplate> {
        self.filtered_templates().get(self.selected).copied()
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let width = 70u16.min(area.width.saturating_sub(4));
        let height = 18u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Filter Templates ");

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        // Input line
        let input_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let input_line = Line::from(vec![
            Span::styled("Search: ", Style::default().fg(Color::Cyan)),
            Span::raw(&self.input),
            Span::styled("▏", Style::default().fg(Color::Cyan)),
        ]);
        buf.set_line(input_area.x, input_area.y, &input_line, input_area.width);

        // Template list
        let list_area = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(4),
        );

        let filtered = self.filtered_templates();
        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(i, template)| {
                let is_selected = i == self.selected;
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Format: name [category]
                let name_line = Line::from(vec![
                    Span::styled(
                        if is_selected { " ▸ " } else { "   " },
                        style.fg(Color::Cyan),
                    ),
                    Span::styled(&template.name, style),
                    Span::styled(
                        format!(" [{}]", template.category),
                        style.fg(Color::DarkGray),
                    ),
                ]);

                // Description on second line
                let desc_line = Line::from(vec![
                    Span::raw("   "),
                    Span::styled(&template.description, style.fg(Color::Gray)),
                ]);

                ListItem::new(vec![name_line, desc_line])
            })
            .collect();

        if !items.is_empty() {
            let list = List::new(items);
            list.render(list_area, buf);
        } else {
            let no_match =
                Paragraph::new("No matching templates").style(Style::default().fg(Color::DarkGray));
            no_match.render(list_area, buf);
        }

        // Help text at bottom
        let help_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(2),
            inner.width,
            1,
        );
        let help_line = Line::from(vec![
            Span::styled("F3: Close", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("Enter: Apply", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("↑↓: Navigate", Style::default().fg(Color::DarkGray)),
        ]);
        buf.set_line(help_area.x, help_area.y, &help_line, help_area.width);
    }
}
