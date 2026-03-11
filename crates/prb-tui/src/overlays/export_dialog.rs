//! Export dialog for saving events in multiple formats.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget};
use ratatui::style::{Color, Modifier, Style};
use tui_input::Input;
use crate::theme::Theme;

/// Export format option with display information.
#[derive(Debug, Clone)]
pub struct ExportFormat {
    pub format: String,
    pub description: String,
    pub extension: String,
}

/// Export dialog overlay for format selection and export.
pub struct ExportDialogOverlay {
    pub formats: Vec<ExportFormat>,
    pub selected: usize,
    pub output_path_input: Input,
    pub editing_path: bool,
    pub filtered_count: usize,
}

impl ExportDialogOverlay {
    pub fn new(filtered_count: usize) -> Self {
        let formats = vec![
            ExportFormat {
                format: "json".to_string(),
                description: "JSON (single event)".to_string(),
                extension: "json".to_string(),
            },
            ExportFormat {
                format: "json-all".to_string(),
                description: format!("JSON (all filtered: {})", filtered_count),
                extension: "json".to_string(),
            },
            ExportFormat {
                format: "csv".to_string(),
                description: format!("CSV (all filtered: {})", filtered_count),
                extension: "csv".to_string(),
            },
            ExportFormat {
                format: "har".to_string(),
                description: "HAR (gRPC conversations)".to_string(),
                extension: "har".to_string(),
            },
            ExportFormat {
                format: "html".to_string(),
                description: "HTML (report)".to_string(),
                extension: "html".to_string(),
            },
        ];

        let default_path = format!("./export.{}", formats[0].extension);
        let mut output_path_input = Input::default();
        output_path_input = output_path_input.with_value(default_path);

        Self {
            formats,
            selected: 0,
            output_path_input,
            editing_path: false,
            filtered_count,
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.formats.is_empty() {
            return;
        }

        let new_idx = (self.selected as isize + delta).rem_euclid(self.formats.len() as isize);
        self.selected = new_idx as usize;

        // Update output path extension when format changes
        if !self.editing_path {
            let selected_format = &self.formats[self.selected];
            let path = format!("./export.{}", selected_format.extension);
            self.output_path_input = Input::default().with_value(path);
        }
    }

    pub fn selected_format(&self) -> Option<&ExportFormat> {
        self.formats.get(self.selected)
    }

    pub fn toggle_path_editing(&mut self) {
        self.editing_path = !self.editing_path;
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
            .title(" Export ");

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        // Format list
        let list_height = self.formats.len().min(inner.height.saturating_sub(4) as usize);
        let list_area = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            list_height as u16,
        );

        let items: Vec<ListItem> = self.formats
            .iter()
            .enumerate()
            .map(|(i, fmt)| {
                let style = if i == self.selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                let prefix = if i == self.selected { "> " } else { "  " };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{}{}", prefix, fmt.description),
                        style,
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items);
        list.render(list_area, buf);

        // Output path
        let output_y = inner.y + list_height as u16 + 1;
        let output_area = Rect::new(inner.x, output_y, inner.width, 1);

        let output_style = if self.editing_path {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let output_line = Line::from(vec![
            Span::raw("Output: "),
            Span::styled(self.output_path_input.value(), output_style),
            if self.editing_path {
                Span::styled("▏", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("")
            },
        ]);
        buf.set_line(output_area.x, output_area.y, &output_line, output_area.width);

        // Help text
        let help_y = output_y + 2;
        let help_area = Rect::new(inner.x, help_y, inner.width, 1);
        let help_text = if self.editing_path {
            "Enter: confirm  Esc: cancel edit"
        } else {
            "Enter: export  Tab: edit path  Esc: cancel"
        };
        let help_line = Line::from(Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        ));
        buf.set_line(help_area.x, help_area.y, &help_line, help_area.width);
    }
}

impl Default for ExportDialogOverlay {
    fn default() -> Self {
        Self::new(0)
    }
}
