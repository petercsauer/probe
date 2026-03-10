use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget,
};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::Theme;

pub struct EventListPane {
    pub selected: usize,
    pub scroll_offset: usize,
}

impl EventListPane {
    pub fn new() -> Self {
        EventListPane {
            selected: 0,
            scroll_offset: 0,
        }
    }

    fn visible_height(area: Rect) -> usize {
        area.height.saturating_sub(3) as usize // borders + header
    }

    fn ensure_visible(&mut self, area_height: u16) {
        let vis = area_height.saturating_sub(3) as usize;
        if vis == 0 {
            return;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + vis {
            self.scroll_offset = self.selected.saturating_sub(vis - 1);
        }
    }

    fn total_items(state: &AppState) -> usize {
        state.filtered_indices.len()
    }
}

impl PaneComponent for EventListPane {
    fn handle_key(&mut self, key: KeyEvent, state: &AppState) -> Action {
        let total = Self::total_items(state);
        if total == 0 {
            return Action::None;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < total {
                    self.selected += 1;
                }
                Action::SelectEvent(self.selected)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Action::SelectEvent(self.selected)
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.selected = 0;
                self.scroll_offset = 0;
                Action::SelectEvent(self.selected)
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.selected = total.saturating_sub(1);
                Action::SelectEvent(self.selected)
            }
            KeyCode::PageDown => {
                self.selected = (self.selected + 20).min(total.saturating_sub(1));
                Action::SelectEvent(self.selected)
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(20);
                Action::SelectEvent(self.selected)
            }
            KeyCode::Enter => Action::SelectEvent(self.selected),
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
            .title(" Events ");

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 2 || inner.width < 10 {
            return;
        }

        // Header row
        let header = format_header(inner.width);
        buf.set_line(inner.x, inner.y, &header, inner.width);

        let vis_height = inner.height.saturating_sub(1) as usize;
        let total = state.filtered_indices.len();

        // Clone to allow scrolling adjustment
        let scroll_offset = {
            let mut off = self.scroll_offset;
            if self.selected < off {
                off = self.selected;
            } else if self.selected >= off + vis_height {
                off = self.selected.saturating_sub(vis_height - 1);
            }
            off
        };

        for i in 0..vis_height {
            let idx = scroll_offset + i;
            if idx >= total {
                break;
            }
            let event_idx = state.filtered_indices[idx];
            if let Some(event) = state.store.get(event_idx) {
                let y = inner.y + 1 + i as u16;
                let is_selected = idx == self.selected;

                let row_style = if is_selected {
                    Theme::selected_row()
                } else {
                    Style::default()
                };

                let transport_color = Theme::transport_color(event.transport);
                let dir_sym = Theme::direction_symbol(event.direction);

                let ts_ns = event.timestamp.as_nanos();
                let secs = ts_ns / 1_000_000_000;
                let millis = (ts_ns % 1_000_000_000) / 1_000_000;
                let h = (secs / 3600) % 24;
                let m = (secs % 3600) / 60;
                let s = secs % 60;

                let src = event
                    .source
                    .network
                    .as_ref()
                    .map(|n| n.src.as_str())
                    .unwrap_or("-");
                let dst = event
                    .source
                    .network
                    .as_ref()
                    .map(|n| n.dst.as_str())
                    .unwrap_or("-");

                let summary = event
                    .metadata
                    .values()
                    .next()
                    .cloned()
                    .unwrap_or_default();

                let w = inner.width as usize;
                let fixed_cols = 6 + 1 + 13 + 1 + 18 + 1 + 18 + 1 + 10 + 1 + 3 + 1;
                let summary_w = w.saturating_sub(fixed_cols);

                let truncated_src = truncate_str(src, 18);
                let truncated_dst = truncate_str(dst, 18);
                let truncated_summary = truncate_str(&summary, summary_w);

                let line = Line::from(vec![
                    Span::styled(format!("{:>5} ", event.id.as_u64()), row_style),
                    Span::styled(
                        format!("{:02}:{:02}:{:02}.{:03} ", h, m, s, millis),
                        row_style,
                    ),
                    Span::styled(format!("{:<18} ", truncated_src), row_style),
                    Span::styled(format!("{:<18} ", truncated_dst), row_style),
                    Span::styled(
                        format!("{:<10} ", event.transport),
                        row_style.fg(transport_color),
                    ),
                    Span::styled(format!("{:<3} ", dir_sym), row_style),
                    Span::styled(truncated_summary, row_style),
                ]);

                buf.set_line(inner.x, y, &line, inner.width);
            }
        }

        // Scrollbar
        if total > vis_height {
            let mut scrollbar_state = ScrollbarState::new(total)
                .position(scroll_offset)
                .viewport_content_length(vis_height);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
            scrollbar.render(inner, buf, &mut scrollbar_state);
        }
    }
}

fn format_header(width: u16) -> Line<'static> {
    let style = Theme::header();
    let w = width as usize;
    let summary_w = w.saturating_sub(6 + 1 + 13 + 1 + 18 + 1 + 18 + 1 + 10 + 1 + 3 + 1);

    Line::from(vec![
        Span::styled(format!("{:>5} ", "#"), style),
        Span::styled(format!("{:<13}", "Time"), style),
        Span::styled(format!("{:<18} ", "Source"), style),
        Span::styled(format!("{:<18} ", "Destination"), style),
        Span::styled(format!("{:<10} ", "Protocol"), style),
        Span::styled(format!("{:<3} ", "Dir"), style),
        Span::styled(
            format!("{:<width$}", "Summary", width = summary_w),
            style.add_modifier(Modifier::BOLD),
        ),
    ])
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 3 {
        format!("{}...", &s[..max - 3])
    } else {
        s[..max].to_string()
    }
}
