//! AI Explain Panel - streaming LLM explanations of debug events.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use std::collections::HashMap;
use tokio::sync::mpsc;

use prb_ai::{explain_event_stream, AiConfig, AiError};
use prb_core::{DebugEvent, EventId};

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;

/// AI panel state for streaming explanations.
pub struct AiPanel {
    /// Current explanation content (accumulated from stream).
    pub content: String,
    /// Whether we're currently streaming.
    pub streaming: bool,
    /// Scroll offset for long explanations.
    pub scroll_offset: usize,
    /// Cache of explanations per event ID.
    cached: HashMap<EventId, String>,
    /// Receiver for streaming chunks.
    stream_rx: Option<mpsc::UnboundedReceiver<Result<String, String>>>,
    /// Error message if explanation failed.
    error: Option<String>,
    /// AI configuration.
    config: AiConfig,
}

impl Default for AiPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AiPanel {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            streaming: false,
            scroll_offset: 0,
            cached: HashMap::new(),
            stream_rx: None,
            error: None,
            config: Self::load_config(),
        }
    }

    /// Load AI config from environment or use defaults.
    fn load_config() -> AiConfig {
        let mut config = AiConfig::default();

        // Check environment variables
        if let Ok(provider) = std::env::var("PRB_AI_PROVIDER") {
            if let Ok(p) = provider.parse() {
                config.provider = p;
            }
        }

        if let Ok(model) = std::env::var("PRB_AI_MODEL") {
            config.model = model;
        }

        if let Ok(endpoint) = std::env::var("PRB_AI_ENDPOINT") {
            config.base_url = endpoint;
        }

        config
    }

    /// Start explaining an event with streaming.
    pub fn start_explain(&mut self, event_idx: usize, state: &AppState) {
        // Clear any previous state
        self.content.clear();
        self.streaming = false;
        self.error = None;
        self.stream_rx = None;

        // Get the event
        let Some(store_idx) = state.filtered_indices.get(event_idx) else {
            self.error = Some("Invalid event index".to_string());
            return;
        };
        let Some(event) = state.store.get(*store_idx) else {
            self.error = Some("Event not found".to_string());
            return;
        };

        // Check cache first
        if let Some(cached) = self.cached.get(&event.id) {
            self.content = cached.clone();
            self.streaming = false;
            return;
        }

        // Prepare all events for context
        let all_events: Vec<DebugEvent> = (0..state.store.len())
            .filter_map(|i| state.store.get(i))
            .cloned()
            .collect();

        if all_events.is_empty() {
            self.error = Some("No events available".to_string());
            return;
        }

        // Start streaming
        let config = self.config.clone();
        let target_idx = *store_idx;
        let event_id = event.id;

        let (tx, rx) = mpsc::unbounded_channel();
        self.stream_rx = Some(rx);
        self.streaming = true;

        // Spawn async task to stream explanation
        tokio::spawn(async move {
            let result = explain_event_stream(&all_events, target_idx, &config, |chunk| {
                let _ = tx.send(Ok(chunk.to_string()));
            })
            .await;

            // Send final result or error
            match result {
                Ok(full_text) => {
                    // Signal completion with final text
                    let _ = tx.send(Ok(full_text));
                }
                Err(e) => {
                    let _ = tx.send(Err(format_ai_error(e)));
                }
            }
        });

        // Store event ID for caching
        self.cached.insert(event_id, String::new());
    }

    /// Poll the stream receiver and update content.
    pub fn poll_stream(&mut self) {
        if let Some(ref mut rx) = self.stream_rx {
            // Drain all available chunks
            while let Ok(result) = rx.try_recv() {
                match result {
                    Ok(chunk) => {
                        self.content.push_str(&chunk);
                    }
                    Err(err) => {
                        self.error = Some(err);
                        self.streaming = false;
                        self.stream_rx = None;
                        return;
                    }
                }
            }

            // Check if stream is done (would block)
            if rx.is_empty() && !self.streaming {
                self.stream_rx = None;
            }
        }

        // Cache the completed explanation
        if !self.streaming && !self.content.is_empty() {
            // Find the event ID we were explaining (stored in cached with empty string)
            for (_id, cached_content) in self.cached.iter_mut() {
                if cached_content.is_empty() {
                    *cached_content = self.content.clone();
                    break;
                }
            }
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    pub fn is_visible(&self) -> bool {
        !self.content.is_empty() || self.streaming || self.error.is_some()
    }

    pub fn config(&self) -> &AiConfig {
        &self.config
    }
}

impl PaneComponent for AiPanel {
    fn handle_key(&mut self, key: KeyEvent, _state: &AppState) -> Action {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.scroll_down(1);
                Action::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_up(1);
                Action::None
            }
            KeyCode::PageDown => {
                self.scroll_down(10);
                Action::None
            }
            KeyCode::PageUp => {
                self.scroll_up(10);
                Action::None
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                Action::None
            }
            KeyCode::End => {
                self.scroll_offset = usize::MAX; // Will be clamped in render
                Action::None
            }
            _ => Action::None,
        }
    }

    fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        _state: &AppState,
        theme: &ThemeConfig,
        focused: bool,
    ) {
        use ratatui::widgets::BorderType;

        // Poll stream updates
        self.poll_stream();

        // Build title
        let title = if self.streaming {
            " AI Explain [streaming...] "
        } else if self.error.is_some() {
            " AI Explain [error] "
        } else {
            " AI Explain "
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

        if inner.height < 1 || inner.width < 10 {
            return;
        }

        // Show error if present
        if let Some(ref error) = self.error {
            let error_text = ratatui::text::Text::styled(
                format!("  Error: {}", error),
                Style::default().fg(Color::Red),
            );
            let paragraph = Paragraph::new(error_text).wrap(Wrap { trim: false });
            Widget::render(paragraph, inner, buf);
            return;
        }

        // Show content or placeholder
        if self.content.is_empty() && !self.streaming {
            let msg = ratatui::text::Text::styled(
                "  Press 'a' on any event to get an AI explanation",
                Style::default().fg(Color::DarkGray),
            );
            Widget::render(msg, inner, buf);
            return;
        }

        // Render content with scroll
        let mut display_text = self.content.clone();
        if self.streaming {
            display_text.push_str("▌"); // Streaming cursor
        }

        let lines: Vec<&str> = display_text.lines().collect();
        let total_lines = lines.len();

        // Clamp scroll offset
        let max_scroll = total_lines.saturating_sub(inner.height as usize);
        let scroll = self.scroll_offset.min(max_scroll);

        // Build visible lines
        let visible_lines: Vec<Line> = lines
            .iter()
            .skip(scroll)
            .take(inner.height as usize)
            .map(|line| Line::from(Span::raw(*line)))
            .collect();

        let paragraph = Paragraph::new(visible_lines).wrap(Wrap { trim: false });
        Widget::render(paragraph, inner, buf);
    }
}

/// Format AiError for display.
fn format_ai_error(err: AiError) -> String {
    match err {
        AiError::NoEvents => "No events available".to_string(),
        AiError::EventNotFound(id) => format!("Event {} not found", id),
        AiError::MissingApiKey(provider) => {
            format!(
                "AI unavailable — configure {} API key in ~/.config/prb/config.toml or PRB_AI_API_KEY env var",
                provider
            )
        }
        AiError::ApiRequest(msg) => format!("API error: {}", msg),
        AiError::StreamInterrupted(msg) => format!("Stream interrupted: {}", msg),
        AiError::Serialization(msg) => format!("Serialization error: {}", msg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_store::EventStore;

    #[test]
    fn test_ai_panel_initial_state() {
        let panel = AiPanel::new();
        assert_eq!(panel.content, "");
        assert!(!panel.streaming);
        assert_eq!(panel.scroll_offset, 0);
        assert!(panel.cached.is_empty());
    }

    #[test]
    fn test_ai_panel_scroll() {
        let mut panel = AiPanel::new();
        panel.scroll_down(5);
        assert_eq!(panel.scroll_offset, 5);

        panel.scroll_up(3);
        assert_eq!(panel.scroll_offset, 2);

        panel.scroll_up(10); // Should saturate at 0
        assert_eq!(panel.scroll_offset, 0);
    }

    #[test]
    fn test_config_loading() {
        // Test default config
        let config = AiPanel::load_config();
        assert_eq!(config.provider.to_string(), "ollama");
        assert_eq!(config.model, "llama3.2");
    }

    #[test]
    fn test_visibility() {
        let mut panel = AiPanel::new();
        assert!(!panel.is_visible());

        panel.content = "Some content".to_string();
        assert!(panel.is_visible());

        panel.content.clear();
        panel.streaming = true;
        assert!(panel.is_visible());

        panel.streaming = false;
        panel.error = Some("Error".to_string());
        assert!(panel.is_visible());
    }
}
