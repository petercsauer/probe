// WIP modules commented out to fix build
pub mod ai_panel;
pub mod conversation_list;
pub mod decode_tree;
pub mod event_list;
pub mod hex_dump;
pub mod timeline;
pub mod trace_correlation;
pub mod waterfall;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::app::AppState;
use crate::theme::ThemeConfig;

pub enum Action {
    None,
    SelectEvent(usize),
    // WIP: SelectConversation(usize),
    HighlightBytes { offset: usize, len: usize },
    ClearHighlight,
    Quit,
}

pub trait PaneComponent {
    fn handle_key(&mut self, key: crossterm::event::KeyEvent, state: &AppState) -> Action;
    fn render(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        state: &AppState,
        theme: &ThemeConfig,
        focused: bool,
    );
}
