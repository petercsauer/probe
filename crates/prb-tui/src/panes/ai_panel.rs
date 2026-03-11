//! AI Panel pane (stub implementation)

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use crossterm::event::KeyEvent;

use crate::app::AppState;
use crate::panes::Action;
use crate::theme::ThemeConfig;

pub struct AiPanel {}

impl AiPanel {
    pub fn new() -> Self {
        Self {}
    }

    pub fn start_explain(&mut self, _event_idx: usize, _state: &AppState) {
        // TODO: Implement
    }

    pub fn render(&mut self, _area: Rect, _buf: &mut Buffer, _state: &AppState, _theme: &ThemeConfig, _focused: bool) {
        // TODO: Implement
    }

    pub fn handle_key(&mut self, _key: KeyEvent, _state: &AppState) -> Action {
        Action::None
    }
}

impl Default for AiPanel {
    fn default() -> Self {
        Self::new()
    }
}
