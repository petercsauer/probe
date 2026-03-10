use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, event::EnableMouseCapture, event::DisableMouseCapture};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::Terminal;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

use crate::event_store::EventStore;
use crate::panes::decode_tree::DecodeTreePane;
use crate::panes::event_list::EventListPane;
use crate::panes::hex_dump::HexDumpPane;
use crate::panes::timeline::TimelinePane;
use crate::panes::{Action, PaneComponent};
use crate::theme::Theme;

use prb_query::Filter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneId {
    EventList,
    DecodeTree,
    HexDump,
    Timeline,
}

impl PaneId {
    fn next(self) -> Self {
        match self {
            PaneId::EventList => PaneId::DecodeTree,
            PaneId::DecodeTree => PaneId::HexDump,
            PaneId::HexDump => PaneId::Timeline,
            PaneId::Timeline => PaneId::EventList,
        }
    }

    fn prev(self) -> Self {
        match self {
            PaneId::EventList => PaneId::Timeline,
            PaneId::DecodeTree => PaneId::EventList,
            PaneId::HexDump => PaneId::DecodeTree,
            PaneId::Timeline => PaneId::HexDump,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Filter,
    Help,
}

pub struct AppState {
    pub store: EventStore,
    pub filtered_indices: Vec<usize>,
    pub selected_event: Option<usize>,
    pub filter: Option<Filter>,
    pub filter_text: String,
}

pub struct App {
    state: AppState,
    focus: PaneId,
    input_mode: InputMode,
    filter_input: Input,
    filter_error: Option<String>,

    event_list: EventListPane,
    decode_tree: DecodeTreePane,
    hex_dump: HexDumpPane,
    timeline: TimelinePane,
}

impl App {
    pub fn new(store: EventStore, initial_filter: Option<String>) -> Self {
        let mut state = AppState {
            filtered_indices: store.all_indices(),
            selected_event: if store.is_empty() { None } else { Some(0) },
            filter: None,
            filter_text: String::new(),
            store,
        };

        if let Some(ref filter_str) = initial_filter {
            if let Ok(filter) = Filter::parse(filter_str) {
                state.filtered_indices = state.store.filter_indices(&filter);
                state.filter_text = filter_str.clone();
                state.filter = Some(filter);
                state.selected_event = if state.filtered_indices.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }

        App {
            state,
            focus: PaneId::EventList,
            input_mode: InputMode::Normal,
            filter_input: Input::default(),
            filter_error: None,
            event_list: EventListPane::new(),
            decode_tree: DecodeTreePane::new(),
            hex_dump: HexDumpPane::new(),
            timeline: TimelinePane::new(),
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        terminal::disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> anyhow::Result<()> {
        loop {
            self.draw(terminal)?;

            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key) {
                        return Ok(());
                    }
                }
            }
        }
    }

    fn draw(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> anyhow::Result<()> {
        terminal.draw(|frame| {
            let area = frame.area();
            let buf = frame.buffer_mut();
            self.render_all(area, buf);
        })?;
        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.input_mode {
            InputMode::Help => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                        self.input_mode = InputMode::Normal;
                    }
                    _ => {}
                }
                return false;
            }
            InputMode::Filter => {
                return self.handle_filter_key(key);
            }
            InputMode::Normal => {}
        }

        // Global keys
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return true;
            }
            KeyCode::Char('q') => return true,
            KeyCode::Char('?') => {
                self.input_mode = InputMode::Help;
                return false;
            }
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Filter;
                self.filter_input = Input::new(self.state.filter_text.clone());
                self.filter_error = None;
                return false;
            }
            KeyCode::Tab => {
                self.focus = self.focus.next();
                return false;
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev();
                return false;
            }
            KeyCode::Esc => {
                if self.state.filter.is_some() {
                    self.state.filter = None;
                    self.state.filter_text.clear();
                    self.state.filtered_indices = self.state.store.all_indices();
                    self.event_list.selected = 0;
                    self.event_list.scroll_offset = 0;
                    self.state.selected_event = if self.state.filtered_indices.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                }
                return false;
            }
            _ => {}
        }

        // Route to focused pane
        let action = match self.focus {
            PaneId::EventList => self.event_list.handle_key(key, &self.state),
            PaneId::DecodeTree => self.decode_tree.handle_key(key, &self.state),
            PaneId::HexDump => self.hex_dump.handle_key(key, &self.state),
            PaneId::Timeline => self.timeline.handle_key(key, &self.state),
        };

        self.process_action(action);
        false
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Enter => {
                let text = self.filter_input.value().to_string();
                if text.trim().is_empty() {
                    self.state.filter = None;
                    self.state.filter_text.clear();
                    self.state.filtered_indices = self.state.store.all_indices();
                } else {
                    match Filter::parse(&text) {
                        Ok(filter) => {
                            self.state.filtered_indices = self.state.store.filter_indices(&filter);
                            self.state.filter = Some(filter);
                            self.state.filter_text = text;
                            self.filter_error = None;
                        }
                        Err(e) => {
                            self.filter_error = Some(e.to_string());
                            return false;
                        }
                    }
                }
                self.event_list.selected = 0;
                self.event_list.scroll_offset = 0;
                self.state.selected_event = if self.state.filtered_indices.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.input_mode = InputMode::Normal;
                self.focus = PaneId::EventList;
                false
            }
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.filter_error = None;
                false
            }
            _ => {
                self.filter_input.handle_event(&Event::Key(key));
                self.filter_error = None;
                false
            }
        }
    }

    fn process_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::SelectEvent(idx) => {
                self.state.selected_event = Some(idx);
                self.hex_dump.scroll_offset = 0;
                self.hex_dump.clear_highlight();
                self.decode_tree.state = tui_tree_widget::TreeState::default();
            }
            Action::HighlightBytes { offset, len } => {
                self.hex_dump.set_highlight(offset, len);
            }
            Action::ClearHighlight => {
                self.hex_dump.clear_highlight();
            }
            Action::Quit => {}
        }
    }

    fn render_all(&mut self, area: Rect, buf: &mut Buffer) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // filter bar
                Constraint::Min(10),   // main content
                Constraint::Length(5), // timeline
                Constraint::Length(1),  // status bar
            ])
            .split(area);

        // Split main content: top = event list, bottom = decode tree + hex dump
        let vert_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(main_layout[1]);

        let horiz_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(vert_layout[1]);

        let focus = self.focus;
        self.event_list.render(vert_layout[0], buf, &self.state, focus == PaneId::EventList);
        self.decode_tree.render(horiz_layout[0], buf, &self.state, focus == PaneId::DecodeTree);
        self.hex_dump.render(horiz_layout[1], buf, &self.state, focus == PaneId::HexDump);
        self.timeline.render(main_layout[2], buf, &self.state, focus == PaneId::Timeline);

        Self::render_filter_bar_static(
            main_layout[0],
            buf,
            self.input_mode,
            &self.filter_input,
            &self.filter_error,
            &self.state,
        );
        Self::render_status_bar_static(main_layout[3], buf, &self.state);

        if self.input_mode == InputMode::Help {
            self.render_help_overlay(area, buf);
        }
    }

    fn render_filter_bar_static(
        area: Rect,
        buf: &mut Buffer,
        input_mode: InputMode,
        filter_input: &Input,
        filter_error: &Option<String>,
        state: &AppState,
    ) {
        let is_filtering = input_mode == InputMode::Filter;

        let filter_display = if is_filtering {
            filter_input.value().to_string()
        } else {
            state.filter_text.clone()
        };

        let match_count = state.filtered_indices.len();
        let total = state.store.len();

        let mut spans = vec![
            Span::styled(" / ", Theme::help_key()),
        ];

        if filter_display.is_empty() && !is_filtering {
            spans.push(Span::styled(
                "type / to filter",
                ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
            ));
        } else {
            spans.push(Span::styled(filter_display, Theme::filter_bar()));
        }

        if let Some(err) = filter_error {
            spans.push(Span::styled(format!("  ✗ {}", err), Theme::filter_error()));
        } else if state.filter.is_some() {
            spans.push(Span::styled(
                format!("  [{}/{}]", match_count, total),
                ratatui::style::Style::default().fg(ratatui::style::Color::Green),
            ));
        }

        if is_filtering {
            spans.push(Span::styled("▏", Theme::help_key()));
        }

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    fn render_status_bar_static(area: Rect, buf: &mut Buffer, state: &AppState) {
        let total = state.store.len();
        let filtered = state.filtered_indices.len();

        let mut spans = vec![Span::styled(
            format!(" {} events", total),
            Theme::status_bar(),
        )];

        if state.filter.is_some() {
            spans.push(Span::styled(
                format!(" ({} shown)", filtered),
                Theme::status_bar(),
            ));
        }

        spans.push(Span::styled(" │ ", Theme::status_bar()));

        let counts = state.store.protocol_counts(&state.filtered_indices);
        for (kind, count) in counts.iter().take(4) {
            let color = Theme::transport_color(*kind);
            spans.push(Span::styled(
                format!("{}: {} ", kind, count),
                Style::default().fg(color).bg(ratatui::style::Color::DarkGray),
            ));
        }

        // Right-aligned keybind hints
        let hint = " Tab:pane  /:filter  ?:help  q:quit ";
        let used: usize = spans.iter().map(|s| s.content.len()).sum();
        let padding = (area.width as usize).saturating_sub(used + hint.len());
        spans.push(Span::styled(
            " ".repeat(padding),
            Theme::status_bar(),
        ));
        spans.push(Span::styled(hint, Theme::status_bar()));

        let line = Line::from(spans);

        // Fill background
        for x in area.x..area.x + area.width {
            buf[(x, area.y)].set_style(Theme::status_bar());
        }
        buf.set_line(area.x, area.y, &line, area.width);
    }

    fn render_help_overlay(&self, area: Rect, buf: &mut Buffer) {
        let width = 50u16.min(area.width.saturating_sub(4));
        let height = 22u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Help (press ? or Esc to close) ");
        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        let help_lines = vec![
            ("Global", ""),
            ("  Tab / Shift+Tab", "Cycle panes"),
            ("  /", "Open filter bar"),
            ("  Esc", "Clear filter"),
            ("  ?", "Toggle help"),
            ("  q / Ctrl+C", "Quit"),
            ("", ""),
            ("Event List", ""),
            ("  j/k ↑/↓", "Navigate events"),
            ("  g / Home", "First event"),
            ("  G / End", "Last event"),
            ("  PgDn / PgUp", "Page up/down"),
            ("", ""),
            ("Decode Tree", ""),
            ("  j/k ↑/↓", "Navigate nodes"),
            ("  Enter / →", "Expand"),
            ("  Backspace / ←", "Collapse"),
            ("  Space", "Toggle"),
            ("", ""),
            ("Hex Dump", ""),
            ("  j/k ↑/↓", "Scroll"),
        ];

        for (i, (key, desc)) in help_lines.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            let line = if desc.is_empty() {
                Line::from(Span::styled(
                    key.to_string(),
                    Theme::help_key(),
                ))
            } else {
                Line::from(vec![
                    Span::styled(format!("{:<20}", key), Theme::help_key()),
                    Span::styled(desc.to_string(), Theme::help_desc()),
                ])
            };
            buf.set_line(inner.x, inner.y + i as u16, &line, inner.width);
        }
    }
}
