use std::collections::HashMap;
use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use tokio::sync::mpsc;

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;
use prb_core::{DebugEvent, EventId};

pub struct AiPanel {
    content: String,
    streaming: bool,
    scroll_offset: usize,
    cached: HashMap<EventId, String>,
    stream_rx: Option<mpsc::UnboundedReceiver<String>>,
    error: Option<String>,
}

impl Default for AiPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AiPanel {
    pub fn new() -> Self {
        AiPanel {
            content: String::new(),
            streaming: false,
            scroll_offset: 0,
            cached: HashMap::new(),
            stream_rx: None,
            error: None,
        }
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.streaming = false;
        self.scroll_offset = 0;
        self.stream_rx = None;
        self.error = None;
    }

    pub fn start_explain(&mut self, event: &DebugEvent, all_events: &[DebugEvent], config: &prb_ai::AiConfig) {
        // Check cache first
        if let Some(cached) = self.cached.get(&event.id) {
            self.content = cached.clone();
            self.streaming = false;
            self.error = None;
            return;
        }

        // Clear previous content and error
        self.content.clear();
        self.error = None;
        self.streaming = true;

        // Find the event index in all_events
        let target_idx = all_events.iter().position(|e| e.id == event.id).unwrap_or(0);

        // Create channel for streaming tokens
        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_rx = Some(rx);

        // Clone data for the async task
        let events_clone = all_events.to_vec();
        let config_clone = config.clone();

        // Spawn async task to stream explanation
        tokio::spawn(async move {
            let result = prb_ai::explain_event_stream(
                &events_clone,
                target_idx,
                &config_clone,
                |chunk: &str| {
                    let _ = tx.send(chunk.to_string());
                },
            )
            .await;

            // Send completion marker
            match result {
                Ok(_) => {
                    let _ = tx.send("\n[END]".to_string());
                }
                Err(e) => {
                    let _ = tx.send(format!("\n[ERROR: {}]", e));
                }
            }
        });
    }

    /// Poll the stream receiver and update content. Should be called each frame.
    pub fn poll_stream(&mut self, event_id: EventId) {
        if let Some(ref mut rx) = self.stream_rx {
            // Drain all available chunks
            while let Ok(chunk) = rx.try_recv() {
                if chunk == "\n[END]" {
                    self.streaming = false;
                    self.stream_rx = None;
                    // Cache the completed explanation
                    self.cached.insert(event_id, self.content.clone());
                    break;
                } else if chunk.starts_with("\n[ERROR:") {
                    self.streaming = false;
                    self.stream_rx = None;
                    self.error = Some(chunk[9..chunk.len()-1].to_string());
                    break;
                } else {
                    self.content.push_str(&chunk);
                }
            }
        }
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming
    }
}

impl PaneComponent for AiPanel {
    fn handle_key(&mut self, key: KeyEvent, _state: &AppState) -> Action {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                Action::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
                Action::None
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
                Action::None
            }
            KeyCode::PageDown => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
                Action::None
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _state: &AppState, theme: &ThemeConfig, focused: bool) {
        let border_style = if focused {
            theme.focused_border()
        } else {
            theme.border()
        };

        let title = if self.streaming {
            " AI Explain (streaming...) "
        } else if self.error.is_some() {
            " AI Explain (error) "
        } else {
            " AI Explain "
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title);

        let inner = block.inner(area);
        block.render(area, buf);

        let display_text = if let Some(ref error) = self.error {
            format!("Error: {}\n\nPress 'a' or 'Esc' to close.", error)
        } else if self.content.is_empty() && !self.streaming {
            "No explanation available. Press 'a' or 'Esc' to close.".to_string()
        } else {
            let mut text = self.content.clone();
            if self.streaming {
                text.push_str("▌");
            }
            text
        };

        let paragraph = Paragraph::new(display_text)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset as u16, 0));

        paragraph.render(inner, buf);
    }
}
