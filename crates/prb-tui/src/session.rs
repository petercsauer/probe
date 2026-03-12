//! Session save/restore functionality for TUI state persistence.

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::app::PaneId;

/// Session file format version.
const SESSION_VERSION: &str = "1.0";

/// Serializable session state for save/restore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Format version for forward compatibility.
    pub version: String,

    /// Path to the input capture file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_file: Option<PathBuf>,

    /// Active filter expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,

    /// Scroll position in event list.
    #[serde(default)]
    pub scroll_offset: usize,

    /// Index of selected event.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_event: Option<usize>,

    /// Currently focused pane.
    pub pane_focus: PaneFocus,

    /// Path to TLS keylog file for decryption.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls_keylog: Option<PathBuf>,

    /// Vertical split percentage (event list height).
    #[serde(default = "default_vertical_split")]
    pub vertical_split: u16,

    /// Horizontal split percentage (decode tree width).
    #[serde(default = "default_horizontal_split")]
    pub horizontal_split: u16,

    /// Whether the AI panel is visible.
    #[serde(default)]
    pub ai_panel_visible: bool,

    /// Whether conversation view is enabled.
    #[serde(default)]
    pub showing_conversations: bool,

    /// Whether waterfall view is enabled.
    #[serde(default)]
    pub showing_waterfall: bool,
}

fn default_vertical_split() -> u16 {
    50
}

fn default_horizontal_split() -> u16 {
    50
}

/// Pane focus for serialization (mirrors PaneId).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneFocus {
    EventList,
    DecodeTree,
    HexDump,
    Timeline,
}

impl From<PaneId> for PaneFocus {
    fn from(pane_id: PaneId) -> Self {
        match pane_id {
            PaneId::EventList => PaneFocus::EventList,
            PaneId::DecodeTree => PaneFocus::DecodeTree,
            PaneId::HexDump => PaneFocus::HexDump,
            PaneId::Timeline => PaneFocus::Timeline,
        }
    }
}

impl From<PaneFocus> for PaneId {
    fn from(pane_focus: PaneFocus) -> Self {
        match pane_focus {
            PaneFocus::EventList => PaneId::EventList,
            PaneFocus::DecodeTree => PaneId::DecodeTree,
            PaneFocus::HexDump => PaneId::HexDump,
            PaneFocus::Timeline => PaneId::Timeline,
        }
    }
}

impl Session {
    /// Create a new empty session with defaults.
    pub fn new() -> Self {
        Self {
            version: SESSION_VERSION.to_string(),
            input_file: None,
            filter: None,
            scroll_offset: 0,
            selected_event: None,
            pane_focus: PaneFocus::EventList,
            tls_keylog: None,
            vertical_split: default_vertical_split(),
            horizontal_split: default_horizontal_split(),
            ai_panel_visible: false,
            showing_conversations: false,
            showing_waterfall: false,
        }
    }

    /// Save session to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize session")?;
        fs::write(path, json)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;
        Ok(())
    }

    /// Load session from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        let json = fs::read_to_string(path)
            .with_context(|| format!("Failed to read session from {}", path.display()))?;
        let session: Session = serde_json::from_str(&json)
            .context("Failed to deserialize session")?;

        // Version check (forward compatibility)
        if session.version != SESSION_VERSION {
            tracing::warn!(
                "Session file version {} differs from current version {}",
                session.version,
                SESSION_VERSION
            );
        }

        Ok(session)
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_session_save_load() {
        let mut session = Session::new();
        session.input_file = Some(PathBuf::from("/tmp/test.pcap"));
        session.filter = Some("grpc.status != 0".to_string());
        session.scroll_offset = 42;
        session.selected_event = Some(123);
        session.pane_focus = PaneFocus::DecodeTree;
        session.tls_keylog = Some(PathBuf::from("/tmp/keylog.txt"));

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        session.save(&path).unwrap();

        let loaded = Session::load(&path).unwrap();
        assert_eq!(loaded.version, SESSION_VERSION);
        assert_eq!(loaded.input_file, session.input_file);
        assert_eq!(loaded.filter, session.filter);
        assert_eq!(loaded.scroll_offset, 42);
        assert_eq!(loaded.selected_event, Some(123));
        assert_eq!(loaded.pane_focus, PaneFocus::DecodeTree);
        assert_eq!(loaded.tls_keylog, session.tls_keylog);
    }

    #[test]
    fn test_session_defaults() {
        let session = Session::new();
        assert_eq!(session.version, SESSION_VERSION);
        assert_eq!(session.input_file, None);
        assert_eq!(session.filter, None);
        assert_eq!(session.scroll_offset, 0);
        assert_eq!(session.selected_event, None);
        assert_eq!(session.pane_focus, PaneFocus::EventList);
        assert_eq!(session.vertical_split, 50);
        assert_eq!(session.horizontal_split, 50);
    }

    #[test]
    fn test_pane_focus_conversion() {
        assert_eq!(PaneId::from(PaneFocus::EventList), PaneId::EventList);
        assert_eq!(PaneId::from(PaneFocus::DecodeTree), PaneId::DecodeTree);
        assert_eq!(PaneId::from(PaneFocus::HexDump), PaneId::HexDump);
        assert_eq!(PaneId::from(PaneFocus::Timeline), PaneId::Timeline);

        assert_eq!(PaneFocus::from(PaneId::EventList), PaneFocus::EventList);
        assert_eq!(PaneFocus::from(PaneId::DecodeTree), PaneFocus::DecodeTree);
        assert_eq!(PaneFocus::from(PaneId::HexDump), PaneFocus::HexDump);
        assert_eq!(PaneFocus::from(PaneId::Timeline), PaneFocus::Timeline);
    }
}
