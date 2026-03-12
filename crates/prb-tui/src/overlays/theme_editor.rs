//! Theme editor overlay for live color customization.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget};
use ratatui::style::{Color, Modifier, Style};
use crate::theme::ThemeConfig;

/// Color element that can be edited in the theme.
#[derive(Debug, Clone)]
pub struct ColorElement {
    pub name: String,
    pub description: String,
    pub color: Color,
    pub field: ColorField,
}

/// Field identifier for each editable color in ThemeConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorField {
    SelectedRowFg,
    SelectedRowBg,
    ZebraBg,
    WarningBg,
    FocusedBorder,
    UnfocusedBorder,
    FocusedTitleFg,
    UnfocusedTitleFg,
    HeaderFg,
    StatusBarFg,
    StatusBarBg,
    FilterBarFg,
    FilterBarBg,
    FilterErrorFg,
    WarningFg,
    TreeKeyFg,
    TreeValueFg,
    HexOffsetFg,
    HexByteFg,
    HexHighlightFg,
    HexHighlightBg,
    HexSearchMatchFg,
    HexSearchMatchBg,
    HexAsciiFg,
    HexNonprintFg,
    SparklineFg,
    HelpKeyFg,
    HelpDescFg,
}

impl ColorField {
    pub fn all() -> Vec<ColorField> {
        vec![
            ColorField::SelectedRowFg,
            ColorField::SelectedRowBg,
            ColorField::ZebraBg,
            ColorField::WarningBg,
            ColorField::FocusedBorder,
            ColorField::UnfocusedBorder,
            ColorField::FocusedTitleFg,
            ColorField::UnfocusedTitleFg,
            ColorField::HeaderFg,
            ColorField::StatusBarFg,
            ColorField::StatusBarBg,
            ColorField::FilterBarFg,
            ColorField::FilterBarBg,
            ColorField::FilterErrorFg,
            ColorField::WarningFg,
            ColorField::TreeKeyFg,
            ColorField::TreeValueFg,
            ColorField::HexOffsetFg,
            ColorField::HexByteFg,
            ColorField::HexHighlightFg,
            ColorField::HexHighlightBg,
            ColorField::HexSearchMatchFg,
            ColorField::HexSearchMatchBg,
            ColorField::HexAsciiFg,
            ColorField::HexNonprintFg,
            ColorField::SparklineFg,
            ColorField::HelpKeyFg,
            ColorField::HelpDescFg,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ColorField::SelectedRowFg => "Selected Row Foreground",
            ColorField::SelectedRowBg => "Selected Row Background",
            ColorField::ZebraBg => "Zebra Stripe Background",
            ColorField::WarningBg => "Warning Background",
            ColorField::FocusedBorder => "Focused Border",
            ColorField::UnfocusedBorder => "Unfocused Border",
            ColorField::FocusedTitleFg => "Focused Title",
            ColorField::UnfocusedTitleFg => "Unfocused Title",
            ColorField::HeaderFg => "Header",
            ColorField::StatusBarFg => "Status Bar Foreground",
            ColorField::StatusBarBg => "Status Bar Background",
            ColorField::FilterBarFg => "Filter Bar Foreground",
            ColorField::FilterBarBg => "Filter Bar Background",
            ColorField::FilterErrorFg => "Filter Error",
            ColorField::WarningFg => "Warning Text",
            ColorField::TreeKeyFg => "Tree Key",
            ColorField::TreeValueFg => "Tree Value",
            ColorField::HexOffsetFg => "Hex Offset",
            ColorField::HexByteFg => "Hex Byte",
            ColorField::HexHighlightFg => "Hex Highlight Foreground",
            ColorField::HexHighlightBg => "Hex Highlight Background",
            ColorField::HexSearchMatchFg => "Hex Search Match Foreground",
            ColorField::HexSearchMatchBg => "Hex Search Match Background",
            ColorField::HexAsciiFg => "Hex ASCII",
            ColorField::HexNonprintFg => "Hex Non-printable",
            ColorField::SparklineFg => "Sparkline",
            ColorField::HelpKeyFg => "Help Key",
            ColorField::HelpDescFg => "Help Description",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ColorField::SelectedRowFg => "Text color of selected row",
            ColorField::SelectedRowBg => "Background color of selected row",
            ColorField::ZebraBg => "Alternating row background",
            ColorField::WarningBg => "Background for warning rows",
            ColorField::FocusedBorder => "Border color when pane is focused",
            ColorField::UnfocusedBorder => "Border color when pane is not focused",
            ColorField::FocusedTitleFg => "Title color when pane is focused",
            ColorField::UnfocusedTitleFg => "Title color when pane is not focused",
            ColorField::HeaderFg => "Column header text color",
            ColorField::StatusBarFg => "Status bar text color",
            ColorField::StatusBarBg => "Status bar background color",
            ColorField::FilterBarFg => "Filter bar text color",
            ColorField::FilterBarBg => "Filter bar background color",
            ColorField::FilterErrorFg => "Filter error message color",
            ColorField::WarningFg => "Warning text color",
            ColorField::TreeKeyFg => "Tree view key color",
            ColorField::TreeValueFg => "Tree view value color",
            ColorField::HexOffsetFg => "Hex dump offset color",
            ColorField::HexByteFg => "Hex dump byte color",
            ColorField::HexHighlightFg => "Highlighted hex foreground",
            ColorField::HexHighlightBg => "Highlighted hex background",
            ColorField::HexSearchMatchFg => "Hex search match foreground",
            ColorField::HexSearchMatchBg => "Hex search match background",
            ColorField::HexAsciiFg => "Hex ASCII text color",
            ColorField::HexNonprintFg => "Hex non-printable character color",
            ColorField::SparklineFg => "Sparkline graph color",
            ColorField::HelpKeyFg => "Help screen key color",
            ColorField::HelpDescFg => "Help screen description color",
        }
    }
}

/// Theme editor overlay for live color customization.
pub struct ThemeEditorOverlay {
    pub current_theme: ThemeConfig,
    pub elements: Vec<ColorElement>,
    pub selected: usize,
    pub editing_color: bool,
    pub color_input: String,
    pub scroll_offset: usize,
}

impl ThemeEditorOverlay {
    pub fn new(theme: ThemeConfig) -> Self {
        let mut overlay = Self {
            current_theme: theme.clone(),
            elements: Vec::new(),
            selected: 0,
            editing_color: false,
            color_input: String::new(),
            scroll_offset: 0,
        };
        overlay.load_elements_from_theme(&theme);
        overlay
    }

    fn load_elements_from_theme(&mut self, theme: &ThemeConfig) {
        self.elements.clear();
        for field in ColorField::all() {
            let color = self.get_color_from_theme(theme, field);
            self.elements.push(ColorElement {
                name: field.name().to_string(),
                description: field.description().to_string(),
                color,
                field,
            });
        }
    }

    fn get_color_from_theme(&self, theme: &ThemeConfig, field: ColorField) -> Color {
        match field {
            ColorField::SelectedRowFg => theme.selected_row_fg,
            ColorField::SelectedRowBg => theme.selected_row_bg,
            ColorField::ZebraBg => theme.zebra_bg,
            ColorField::WarningBg => theme.warning_bg,
            ColorField::FocusedBorder => theme.focused_border,
            ColorField::UnfocusedBorder => theme.unfocused_border,
            ColorField::FocusedTitleFg => theme.focused_title_fg,
            ColorField::UnfocusedTitleFg => theme.unfocused_title_fg,
            ColorField::HeaderFg => theme.header_fg,
            ColorField::StatusBarFg => theme.status_bar_fg,
            ColorField::StatusBarBg => theme.status_bar_bg,
            ColorField::FilterBarFg => theme.filter_bar_fg,
            ColorField::FilterBarBg => theme.filter_bar_bg,
            ColorField::FilterErrorFg => theme.filter_error_fg,
            ColorField::WarningFg => theme.warning_fg,
            ColorField::TreeKeyFg => theme.tree_key_fg,
            ColorField::TreeValueFg => theme.tree_value_fg,
            ColorField::HexOffsetFg => theme.hex_offset_fg,
            ColorField::HexByteFg => theme.hex_byte_fg,
            ColorField::HexHighlightFg => theme.hex_highlight_fg,
            ColorField::HexHighlightBg => theme.hex_highlight_bg,
            ColorField::HexSearchMatchFg => theme.hex_search_match_fg,
            ColorField::HexSearchMatchBg => theme.hex_search_match_bg,
            ColorField::HexAsciiFg => theme.hex_ascii_fg,
            ColorField::HexNonprintFg => theme.hex_nonprint_fg,
            ColorField::SparklineFg => theme.sparkline_fg,
            ColorField::HelpKeyFg => theme.help_key_fg,
            ColorField::HelpDescFg => theme.help_desc_fg,
        }
    }

    pub fn apply_color_to_theme(&mut self, field: ColorField, color: Color) {
        match field {
            ColorField::SelectedRowFg => self.current_theme.selected_row_fg = color,
            ColorField::SelectedRowBg => self.current_theme.selected_row_bg = color,
            ColorField::ZebraBg => self.current_theme.zebra_bg = color,
            ColorField::WarningBg => self.current_theme.warning_bg = color,
            ColorField::FocusedBorder => self.current_theme.focused_border = color,
            ColorField::UnfocusedBorder => self.current_theme.unfocused_border = color,
            ColorField::FocusedTitleFg => self.current_theme.focused_title_fg = color,
            ColorField::UnfocusedTitleFg => self.current_theme.unfocused_title_fg = color,
            ColorField::HeaderFg => self.current_theme.header_fg = color,
            ColorField::StatusBarFg => self.current_theme.status_bar_fg = color,
            ColorField::StatusBarBg => self.current_theme.status_bar_bg = color,
            ColorField::FilterBarFg => self.current_theme.filter_bar_fg = color,
            ColorField::FilterBarBg => self.current_theme.filter_bar_bg = color,
            ColorField::FilterErrorFg => self.current_theme.filter_error_fg = color,
            ColorField::WarningFg => self.current_theme.warning_fg = color,
            ColorField::TreeKeyFg => self.current_theme.tree_key_fg = color,
            ColorField::TreeValueFg => self.current_theme.tree_value_fg = color,
            ColorField::HexOffsetFg => self.current_theme.hex_offset_fg = color,
            ColorField::HexByteFg => self.current_theme.hex_byte_fg = color,
            ColorField::HexHighlightFg => self.current_theme.hex_highlight_fg = color,
            ColorField::HexHighlightBg => self.current_theme.hex_highlight_bg = color,
            ColorField::HexSearchMatchFg => self.current_theme.hex_search_match_fg = color,
            ColorField::HexSearchMatchBg => self.current_theme.hex_search_match_bg = color,
            ColorField::HexAsciiFg => self.current_theme.hex_ascii_fg = color,
            ColorField::HexNonprintFg => self.current_theme.hex_nonprint_fg = color,
            ColorField::SparklineFg => self.current_theme.sparkline_fg = color,
            ColorField::HelpKeyFg => self.current_theme.help_key_fg = color,
            ColorField::HelpDescFg => self.current_theme.help_desc_fg = color,
        }
        // Update the element list
        if let Some(element) = self.elements.iter_mut().find(|e| e.field == field) {
            element.color = color;
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.elements.is_empty() {
            return;
        }
        let new_idx = (self.selected as isize + delta).rem_euclid(self.elements.len() as isize);
        self.selected = new_idx as usize;
    }

    pub fn start_editing(&mut self) {
        self.editing_color = true;
        let current_color = self.elements[self.selected].color;
        self.color_input = color_to_string(current_color);
    }

    pub fn cancel_editing(&mut self) {
        self.editing_color = false;
        self.color_input.clear();
    }

    pub fn confirm_edit(&mut self) -> Result<(), String> {
        let color = parse_color(&self.color_input)?;
        let field = self.elements[self.selected].field;
        self.apply_color_to_theme(field, color);
        self.editing_color = false;
        self.color_input.clear();
        Ok(())
    }

    pub fn reset_to_theme(&mut self, theme: &ThemeConfig) {
        self.current_theme = theme.clone();
        self.load_elements_from_theme(theme);
    }
}

impl Widget for &ThemeEditorOverlay {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the background
        Clear.render(area, buf);

        // Create centered area
        let popup_width = area.width.saturating_sub(10).min(80);
        let popup_height = area.height.saturating_sub(6).min(30);
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Main border block
        let title = if self.editing_color {
            format!(" Theme Editor - Editing: {} ", self.elements[self.selected].name)
        } else {
            format!(" Theme Editor - {} ", self.current_theme.name)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Split into list and help areas
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .split(inner);

        if self.editing_color {
            // Show color input field
            let input_text = format!("Color (hex or name): {}", self.color_input);
            let input_para = Paragraph::new(input_text)
                .style(Style::default().fg(Color::Yellow));
            input_para.render(chunks[0], buf);
        } else {
            // Show color element list
            let visible_height = chunks[0].height as usize;
            let start = self.scroll_offset;
            let end = (start + visible_height).min(self.elements.len());

            let items: Vec<ListItem> = self.elements[start..end]
                .iter()
                .enumerate()
                .map(|(i, element)| {
                    let global_idx = start + i;
                    let is_selected = global_idx == self.selected;

                    let color_preview = "  ".to_string();
                    let color_str = color_to_string(element.color);

                    let line = Line::from(vec![
                        Span::styled(
                            format!("{:30} ", element.name),
                            if is_selected {
                                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                        Span::styled(
                            color_preview,
                            Style::default().bg(element.color),
                        ),
                        Span::styled(
                            format!(" {}", color_str),
                            if is_selected {
                                Style::default().fg(Color::Yellow)
                            } else {
                                Style::default().fg(Color::Gray)
                            },
                        ),
                    ]);

                    ListItem::new(line)
                })
                .collect();

            let list = List::new(items);
            list.render(chunks[0], buf);
        }

        // Help text
        let help_text = if self.editing_color {
            "Enter: Apply | Esc: Cancel | Format: #RRGGBB or color name"
        } else {
            "↑/↓: Select | Enter: Edit | r: Reset | s: Save | Esc: Close"
        };

        let help = Paragraph::new(help_text)
            .style(Style::default().fg(Color::DarkGray));
        help.render(chunks[1], buf);
    }
}

/// Parse a color from a string (hex or color name).
pub fn parse_color(s: &str) -> Result<Color, String> {
    let s = s.trim();

    // Try hex format first
    if let Some(hex) = s.strip_prefix('#')
        && hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| format!("Invalid hex color: {}", s))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| format!("Invalid hex color: {}", s))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| format!("Invalid hex color: {}", s))?;
            return Ok(Color::Rgb(r, g, b));
        }

    // Try color names
    match s.to_lowercase().as_str() {
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        "yellow" => Ok(Color::Yellow),
        "blue" => Ok(Color::Blue),
        "magenta" => Ok(Color::Magenta),
        "cyan" => Ok(Color::Cyan),
        "gray" | "grey" => Ok(Color::Gray),
        "darkgray" | "darkgrey" => Ok(Color::DarkGray),
        "lightred" => Ok(Color::LightRed),
        "lightgreen" => Ok(Color::LightGreen),
        "lightyellow" => Ok(Color::LightYellow),
        "lightblue" => Ok(Color::LightBlue),
        "lightmagenta" => Ok(Color::LightMagenta),
        "lightcyan" => Ok(Color::LightCyan),
        "white" => Ok(Color::White),
        "reset" => Ok(Color::Reset),
        _ => Err(format!("Unknown color name: {}", s)),
    }
}

/// Convert a color to a string representation.
pub fn color_to_string(color: Color) -> String {
    match color {
        Color::Reset => "Reset".to_string(),
        Color::Black => "Black".to_string(),
        Color::Red => "Red".to_string(),
        Color::Green => "Green".to_string(),
        Color::Yellow => "Yellow".to_string(),
        Color::Blue => "Blue".to_string(),
        Color::Magenta => "Magenta".to_string(),
        Color::Cyan => "Cyan".to_string(),
        Color::Gray => "Gray".to_string(),
        Color::DarkGray => "DarkGray".to_string(),
        Color::LightRed => "LightRed".to_string(),
        Color::LightGreen => "LightGreen".to_string(),
        Color::LightYellow => "LightYellow".to_string(),
        Color::LightBlue => "LightBlue".to_string(),
        Color::LightMagenta => "LightMagenta".to_string(),
        Color::LightCyan => "LightCyan".to_string(),
        Color::White => "White".to_string(),
        Color::Rgb(r, g, b) => format!("#{:02x}{:02x}{:02x}", r, g, b),
        Color::Indexed(i) => format!("Indexed({})", i),
    }
}
