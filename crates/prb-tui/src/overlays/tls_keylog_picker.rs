//! TLS keylog file picker overlay.

use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget};
use std::fs;
use std::path::{Path, PathBuf};
use tui_input::Input;

/// TLS keylog file picker overlay for selecting keylog files.
pub struct TlsKeylogPickerOverlay {
    /// Current directory path input.
    pub path_input: Input,
    /// Files in the current directory.
    pub files: Vec<PathBuf>,
    /// Selected file index.
    pub selected: usize,
    /// Whether the path input is being edited.
    pub editing_path: bool,
    /// Error message if directory cannot be read.
    pub error: Option<String>,
}

impl TlsKeylogPickerOverlay {
    /// Create a new TLS keylog picker starting at the given directory.
    pub fn new(start_dir: Option<&Path>) -> Self {
        let start_path = start_dir
            .and_then(|p| p.to_str())
            .unwrap_or(".")
            .to_string();

        let mut picker = Self {
            path_input: Input::default().with_value(start_path.clone()),
            files: Vec::new(),
            selected: 0,
            editing_path: false,
            error: None,
        };

        picker.refresh_files();
        picker
    }

    /// Refresh the file list for the current directory.
    pub fn refresh_files(&mut self) {
        let path_str = self.path_input.value();
        let path = Path::new(path_str);

        self.files.clear();
        self.error = None;

        // Try to read the directory
        match fs::read_dir(path) {
            Ok(entries) => {
                // Add parent directory entry if not at root
                if path.parent().is_some() {
                    self.files.push(PathBuf::from(".."));
                }

                // Collect files and directories
                let mut items: Vec<PathBuf> =
                    entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();

                // Sort: directories first, then files
                items.sort_by(|a, b| {
                    let a_is_dir = a.is_dir();
                    let b_is_dir = b.is_dir();
                    match (a_is_dir, b_is_dir) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.file_name().cmp(&b.file_name()),
                    }
                });

                self.files.extend(items);
                self.selected = 0;
            }
            Err(e) => {
                self.error = Some(format!("Cannot read directory: {}", e));
            }
        }
    }

    /// Move selection up or down in the file list.
    pub fn move_selection(&mut self, delta: isize) {
        if self.files.is_empty() {
            return;
        }

        let new_idx = (self.selected as isize + delta).rem_euclid(self.files.len() as isize);
        self.selected = new_idx as usize;
    }

    /// Get the currently selected file path.
    pub fn selected_path(&self) -> Option<PathBuf> {
        self.files.get(self.selected).cloned()
    }

    /// Navigate into a directory or return the selected file path.
    pub fn select_current(&mut self) -> Option<PathBuf> {
        if let Some(path) = self.selected_path() {
            if path == Path::new("..") {
                // Navigate to parent directory
                let current = Path::new(self.path_input.value());
                if let Some(parent) = current.parent() {
                    self.path_input =
                        Input::default().with_value(parent.to_string_lossy().to_string());
                    self.refresh_files();
                }
                None
            } else if path.is_dir() {
                // Navigate into directory
                self.path_input = Input::default().with_value(path.to_string_lossy().to_string());
                self.refresh_files();
                None
            } else {
                // Return the selected file
                Some(path)
            }
        } else {
            None
        }
    }

    /// Render the overlay.
    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        let width = 60u16.min(area.width.saturating_sub(4));
        let height = 20u16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let overlay_area = Rect::new(x, y, width, height);

        Clear.render(overlay_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Theme::focused_border())
            .title(" Select TLS Keylog File ");

        let inner = block.inner(overlay_area);
        block.render(overlay_area, buf);

        // Path input line
        let input_area = Rect::new(inner.x, inner.y, inner.width, 1);
        let input_style = if self.editing_path {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let input_line = Line::from(vec![
            Span::styled("Path: ", input_style),
            Span::raw(self.path_input.value()),
            if self.editing_path {
                Span::styled("▏", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ]);
        buf.set_line(input_area.x, input_area.y, &input_line, input_area.width);

        // File list area
        let list_area = Rect::new(
            inner.x,
            inner.y + 2,
            inner.width,
            inner.height.saturating_sub(4),
        );

        if let Some(ref error) = self.error {
            let error_para = Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red));
            error_para.render(list_area, buf);
        } else if self.files.is_empty() {
            let empty_para =
                Paragraph::new("(empty directory)").style(Style::default().fg(Color::DarkGray));
            empty_para.render(list_area, buf);
        } else {
            let items: Vec<ListItem> = self
                .files
                .iter()
                .enumerate()
                .map(|(i, path)| {
                    let is_dir = path.is_dir() || path == Path::new("..");
                    let display_name = if path == Path::new("..") {
                        "..".to_string()
                    } else {
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("?")
                            .to_string()
                    };

                    let prefix = if is_dir { "📁 " } else { "📄 " };
                    let style = if i == self.selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else if is_dir {
                        Style::default().fg(Color::Blue)
                    } else {
                        Style::default()
                    };

                    let line = Line::from(vec![Span::styled(
                        format!("{}{}", prefix, display_name),
                        style,
                    )]);
                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items);
            list.render(list_area, buf);
        }

        // Help text
        let help_area = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        );
        let help_text = if self.editing_path {
            "Enter: confirm path | Esc: cancel edit"
        } else {
            "↑/↓: navigate | Enter: select/open | e: edit path | Esc: cancel"
        };
        let help_line = Line::from(Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        ));
        buf.set_line(help_area.x, help_area.y, &help_line, help_area.width);
    }
}

impl Default for TlsKeylogPickerOverlay {
    fn default() -> Self {
        Self::new(None)
    }
}
