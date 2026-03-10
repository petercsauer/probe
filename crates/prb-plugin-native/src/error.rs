//! Error types for plugin operations.

use thiserror::Error;

/// Errors that can occur during plugin operations.
#[derive(Debug, Error)]
pub enum PluginError {
    /// Failed to load the shared library.
    #[error("Failed to load plugin library: {0}")]
    LibraryLoad(#[from] libloading::Error),

    /// Plugin is missing a required symbol.
    #[error("Plugin missing required symbol: {0}")]
    MissingSymbol(String),

    /// Plugin API version is incompatible.
    #[error("Plugin API version incompatible: {0}")]
    IncompatibleVersion(String),

    /// Invalid plugin metadata.
    #[error("Invalid plugin metadata: {0}")]
    InvalidMetadata(String),

    /// Plugin panicked or returned invalid data.
    #[error("Plugin runtime error: {0}")]
    Runtime(String),

    /// JSON serialization/deserialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid UTF-8 in plugin string.
    #[error("Invalid UTF-8 in plugin string: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    /// Null pointer from plugin.
    #[error("Plugin returned null pointer for: {0}")]
    NullPointer(String),
}
