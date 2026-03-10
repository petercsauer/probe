//! WASM runtime configuration and resource limits.

use std::time::Duration;

/// Resource limits for WASM plugin execution.
#[derive(Debug, Clone)]
pub struct WasmLimits {
    /// Maximum memory in WASM pages (64KB each). Default: 256 (16MB).
    pub memory_max_pages: u32,
    /// Execution timeout. Default: 30 seconds.
    pub timeout: Duration,
}

impl Default for WasmLimits {
    fn default() -> Self {
        Self {
            memory_max_pages: 256,
            timeout: Duration::from_secs(30),
        }
    }
}

impl WasmLimits {
    /// Create limits suitable for protocol detection (fast, minimal memory).
    pub fn for_detection() -> Self {
        Self {
            memory_max_pages: 16, // 1MB
            timeout: Duration::from_millis(100),
        }
    }

    /// Create limits suitable for decoding (more memory, longer timeout).
    pub fn for_decoding() -> Self {
        Self::default()
    }
}
