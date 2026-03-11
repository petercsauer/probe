// Stub file - AI panel not yet fully implemented

use crossterm::event::KeyEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::AppState;
use crate::panes::{Action, PaneComponent};
use crate::theme::ThemeConfig;

pub struct AiPanel;

impl AiPanel {
    pub fn new() -> Self {
        AiPanel
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
