use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExportError {
    #[error("unsupported export format: {0}")]
    UnsupportedFormat(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("export error: {0}")]
    Other(String),
}
