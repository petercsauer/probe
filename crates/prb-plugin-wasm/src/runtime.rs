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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_limits_default() {
        let limits = WasmLimits::default();
        assert_eq!(limits.memory_max_pages, 256);
        assert_eq!(limits.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_wasm_limits_for_detection() {
        let limits = WasmLimits::for_detection();
        assert_eq!(limits.memory_max_pages, 16);
        assert_eq!(limits.timeout, Duration::from_millis(100));
    }

    #[test]
    fn test_wasm_limits_for_decoding() {
        let limits = WasmLimits::for_decoding();
        // Should be same as default
        assert_eq!(limits.memory_max_pages, 256);
        assert_eq!(limits.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_wasm_limits_clone() {
        let limits1 = WasmLimits::for_detection();
        let limits2 = limits1.clone();
        assert_eq!(limits2.memory_max_pages, limits1.memory_max_pages);
        assert_eq!(limits2.timeout, limits1.timeout);
    }

    #[test]
    fn test_wasm_limits_custom() {
        let limits = WasmLimits {
            memory_max_pages: 128,
            timeout: Duration::from_secs(60),
        };
        assert_eq!(limits.memory_max_pages, 128);
        assert_eq!(limits.timeout, Duration::from_secs(60));
    }
}
