//! Session metadata types.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Metadata for an MCAP session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Source file that was ingested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,

    /// Tool used for capture (e.g., "tcpdump", "wireshark", "prb").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_tool: Option<String>,

    /// Timestamp when ingest occurred.
    pub ingest_timestamp: String,

    /// Tool version (prb version).
    pub tool_version: String,

    /// Command-line arguments used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_args: Option<String>,

    /// Additional custom metadata.
    #[serde(flatten)]
    pub custom: BTreeMap<String, String>,
}

impl SessionMetadata {
    /// Create a new `SessionMetadata` with minimal required fields.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            source_file: None,
            capture_tool: None,
            ingest_timestamp: chrono::Utc::now().to_rfc3339(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            command_args: None,
            custom: BTreeMap::new(),
        }
    }

    /// Set the source file.
    pub fn with_source_file(mut self, source_file: impl Into<String>) -> Self {
        self.source_file = Some(source_file.into());
        self
    }

    /// Set the capture tool.
    pub fn with_capture_tool(mut self, capture_tool: impl Into<String>) -> Self {
        self.capture_tool = Some(capture_tool.into());
        self
    }

    /// Set the command arguments.
    pub fn with_command_args(mut self, command_args: impl Into<String>) -> Self {
        self.command_args = Some(command_args.into());
        self
    }

    /// Add custom metadata.
    pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }
}

impl Default for SessionMetadata {
    fn default() -> Self {
        Self::new()
    }
}
