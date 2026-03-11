//! Which-key popup for showing available key continuations after a prefix.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget};
use ratatui::style::{Color, Style};
use crate::theme::Theme;

/// Which-key popup for contextual help after pressing a prefix key.
pub struct WhichKeyOverlay {
    pub prefix: char,
    pub options: Vec<(char, String)>,
}

impl WhichKeyOverlay {
    pub fn new(prefix: char, options: Vec<(char, String)>) -> Self {
        Self { prefix, options }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let height = (self.options.len() + 3) as u16;
        let width = 40u16.min(area.width.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height.min(area.height.saturating_sub(4)));

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(format!(" {} + ... ", self.prefix));

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        let mut lines = Vec::new();
        for (key, desc) in &self.options {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}", key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("  "),
                Span::styled(desc.clone(), Style::default().fg(Color::White)),
            ]));
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(inner, buf);
    }
}
