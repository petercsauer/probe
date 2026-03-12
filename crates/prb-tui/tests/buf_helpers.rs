//! Shared test utilities for TUI buffer inspection.

#![allow(dead_code)]

use ratatui::{buffer::Buffer, style::Color};

/// Extract a full row from the buffer as a plain string.
#[must_use]
pub fn row_text(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, y)].symbol().to_string())
        .collect()
}

/// Extract a rectangular region as a vec of row strings.
#[must_use]
pub fn region_text(buf: &Buffer, x: u16, y: u16, w: u16, h: u16) -> Vec<String> {
    (y..y + h)
        .map(|row| {
            (x..x + w)
                .map(|col| buf[(col, row)].symbol().to_string())
                .collect()
        })
        .collect()
}

/// Find the first (x, y) position where `text` appears on a single row.
#[must_use]
pub fn find_text(buf: &Buffer, text: &str) -> Option<(u16, u16)> {
    for y in 0..buf.area.height {
        let row = row_text(buf, y);
        if let Some(x) = row.find(text) {
            return Some((x as u16, y));
        }
    }
    None
}

/// Return the foreground color of a specific cell.
#[must_use]
pub fn cell_fg(buf: &Buffer, x: u16, y: u16) -> Color {
    buf[(x, y)].fg
}
