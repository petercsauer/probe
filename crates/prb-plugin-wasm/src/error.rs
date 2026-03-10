//! Error types for WASM plugin system.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("Failed to load WASM plugin: {0}")]
    Load(String),

    #[error("WASM plugin execution error: {0}")]
    Execution(String),

    #[error("Plugin API version mismatch: {0}")]
    ApiVersion(String),

    #[error("Missing required export: {0}")]
    MissingExport(String),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("Extism error: {0}")]
    Extism(#[from] extism::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
