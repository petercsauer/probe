// Stub file - AI panel not yet fully implemented

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;

pub struct AiPanel;

impl Default for AiPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl AiPanel {
    pub fn new() -> Self {
        AiPanel
    }

    pub fn clear(&mut self) {
        // Stub - not yet implemented
    }

    pub fn start_explain(&mut self, _event: &prb_core::DebugEvent, _all_events: &[prb_core::DebugEvent], _config: &prb_ai::AiConfig) {
        // Stub - not yet implemented
    }
}

impl PaneComponent for AiPanel {
    fn handle_key(&mut self, _key: KeyEvent, _state: &AppState) -> Action {
        Action::None
    }

    fn render(&mut self, _area: Rect, _buf: &mut Buffer, _state: &AppState, _theme: &ThemeConfig, _focused: bool) {
        // Stub - not yet implemented
    }
}
